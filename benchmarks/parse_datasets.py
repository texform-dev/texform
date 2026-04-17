"""Benchmark: parse TeXForm against local Parquet corpora of LaTeX formulas.

Sources are declared in ``datasets.yaml`` and point to one or more local
Parquet files. Each source is loaded, normalized, and globally deduplicated
(SHA-256 of the normalized text) before parsing. Every unique formula is
parsed twice -- in ``strict=True`` and ``strict=False`` modes -- so the
report captures both failure profiles.

Outputs go under ``benchmarks/results/parse_datasets/commit-<hash8>/``:

    manifest.json     Run configuration and preprocessing stats.
    results.parquet   One row per unique formula with per-source counts and
                      per-mode timing / success flags.
    errors.jsonl      Strict-mode failures with full diagnostics.
    summary.json      Aggregated metrics (success rate, p50/p95/p99 timings).

After a successful run, ``summary.json`` is also copied to
``benchmarks/results/parse_datasets/summary.json`` -- a rolling, git-tracked
snapshot used to review performance changes across commits.

Each invocation wipes the output directory before starting. Benchmarks on
the full corpus finish in a minute or two, so there is no resume machinery.

Use ``benchmarks/extract_formulas.py`` to turn an upstream dataset (typically
a Hub snapshot with heavy image payloads) into a compact text-only Parquet
under ``benchmarks/cache/`` that this script can consume.

Usage:
    uv run python benchmarks/parse_datasets.py
    uv run python benchmarks/parse_datasets.py --limit 10000
    uv run python benchmarks/parse_datasets.py --workers 8
    uv run python benchmarks/parse_datasets.py --output-dir benchmarks/results/custom
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

import polars as pl
import yaml
from tqdm import tqdm

# ---------------------------------------------------------------------------
# Paths and constants
# ---------------------------------------------------------------------------

_BENCH_DIR = Path(__file__).resolve().parent  # benchmarks/
_DEFAULT_CONFIG = _BENCH_DIR / "datasets.yaml"
_RESULTS_ROOT = _BENCH_DIR / "results" / "parse_datasets"

PREPROCESSING_VERSION = "v2"
PARSE_MODE = "strict=true+nonstrict=true"
MIN_NO_WS_LEN = 5

# ---------------------------------------------------------------------------
# Source configuration
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SourceConfig:
    name: str
    column: str
    files: tuple[str, ...]
    max_rows: int | None = None

    @property
    def column_key(self) -> str:
        """Sanitized identifier used to build Parquet column names."""
        return _sanitize_identifier(self.name)


def load_source_configs(
    config_path: Path, selected: set[str] | None = None
) -> list[SourceConfig]:
    with config_path.open("r", encoding="utf-8") as f:
        cfg = yaml.safe_load(f) or {}
    raw_sources = cfg.get("sources", [])
    if not raw_sources:
        raise SystemExit(f"No `sources` defined in {config_path}")

    sources: list[SourceConfig] = []
    for entry in raw_sources:
        name = entry["name"]
        if selected is not None and name not in selected:
            continue
        files = entry.get("files")
        if not files:
            raise SystemExit(
                f"Source {name!r} is missing `files` (list of Parquet paths / globs)."
            )
        sources.append(
            SourceConfig(
                name=name,
                column=entry["column"],
                files=tuple(files),
                max_rows=entry.get("max_rows"),
            )
        )

    if not sources:
        names = [entry["name"] for entry in raw_sources]
        raise SystemExit(
            f"--sources filter matched nothing. Available in {config_path}: {names}"
        )
    return sources


_IDENT_RE = re.compile(r"[^0-9a-zA-Z_]+")


def _sanitize_identifier(name: str) -> str:
    s = _IDENT_RE.sub("_", name.strip().lower())
    s = s.strip("_")
    if not s:
        raise ValueError(f"Cannot sanitize source name {name!r}")
    if s[0].isdigit():
        s = f"_{s}"
    return s


# ---------------------------------------------------------------------------
# Source iteration
# ---------------------------------------------------------------------------


def iter_source(
    source: SourceConfig,
    *,
    silent: bool = False,
) -> Iterator[tuple[str, str]]:
    """Yield ``(sample_id, formula)`` rows for a single source.

    Honors ``source.max_rows``. Column pruning keeps memory linear in the
    text column alone, not the full row schema.
    """
    limit = source.max_rows
    files = [_resolve_local_path(p) for p in source.files]
    lf = pl.scan_parquet(files).select(pl.col(source.column).alias("__formula__"))
    df = lf.collect(engine="streaming")
    col = df["__formula__"]
    total = len(col) if limit is None else min(len(col), limit)
    bar = tqdm(total=total, desc=f"load {source.name}", disable=silent, unit="rows")
    for idx in range(total):
        formula = col[idx]
        if not isinstance(formula, str):
            continue
        yield (f"{source.name}:{idx}", formula)
        bar.update(1)
    bar.close()


def _resolve_local_path(p: str) -> str:
    path = Path(p)
    if not path.is_absolute():
        path = (_BENCH_DIR / path).resolve()
    return str(path)


# ---------------------------------------------------------------------------
# Normalization and filtering
# ---------------------------------------------------------------------------


_RE_WHITESPACE = re.compile(r"\s+")
_DELIMITER_PAIRS = [("$$", "$$"), ("$", "$"), (r"\[", r"\]"), (r"\(", r"\)")]


def normalize(raw: str) -> str:
    """Lightweight text-level normalization.

    Strips a single outer math delimiter pair (``$...$``, ``$$...$$``,
    ``\\[...\\]``, ``\\(...\\)``) and collapses runs of whitespace. Skips the
    strip when the content still contains the same opener inside -- that is
    the signature of concatenated formulas like ``\\[A\\] \\[B\\]``, which
    are split into segments by ``split_display_segments`` at preprocess time.
    """
    s = raw.replace("\r\n", "\n").replace("\r", "\n").strip()
    for open_, close in _DELIMITER_PAIRS:
        if (
            s.startswith(open_)
            and s.endswith(close)
            and len(s) > len(open_) + len(close)
        ):
            inner = s[len(open_) : -len(close)]
            if open_ not in inner:
                s = inner.strip()
                break
    return _RE_WHITESPACE.sub(" ", s)


_RE_DISPLAY_SEGMENTS = re.compile(r"\\\[(.*?)\\\]", re.DOTALL)


def split_display_segments(s: str) -> list[str]:
    """Split multi-segment display math into individual formula strings.

    A normalized formula that still starts with ``\\[`` is a concatenation of
    two or more display-math blocks (``\\[A\\] \\[B\\]``). Extract each block's
    inner content and return them as separate strings. Single-segment or
    non-display formulas are returned unchanged as a one-element list.
    """
    if not s.startswith(r"\["):
        return [s]
    segments = [seg.strip() for seg in _RE_DISPLAY_SEGMENTS.findall(s) if seg.strip()]
    return segments if len(segments) > 1 else [s]


def _no_ws_len(s: str) -> int:
    return len(re.sub(r"\s", "", s))


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


# ---------------------------------------------------------------------------
# Preprocessing
# ---------------------------------------------------------------------------


@dataclass
class UniqueFormula:
    formula_key: str
    normalized: str
    formula_no_ws_len: int
    per_source_counts: dict[str, int]
    per_source_sample_ids: dict[str, str | None]


def preprocess_all_sources(
    sources: list[SourceConfig],
    *,
    global_limit: int | None,
    raw_out: dict[str, int],
    filtered_out: dict[str, int],
    silent: bool = False,
) -> dict[str, UniqueFormula]:
    """Load, normalize, filter, and globally deduplicate formulas.

    Mutates ``raw_out`` / ``filtered_out`` with per-source counts.
    """
    per_source_counts: dict[str, dict[str, int]] = {s.column_key: {} for s in sources}
    per_source_sample_ids: dict[str, dict[str, str]] = {s.column_key: {} for s in sources}
    normalized_map: dict[str, str] = {}

    for source in sources:
        raw = 0
        kept = 0
        counts = per_source_counts[source.column_key]
        samples = per_source_sample_ids[source.column_key]
        for rid, formula in iter_source(source, silent=silent):
            raw += 1
            if global_limit is not None and raw > global_limit:
                break
            norm = normalize(formula)
            segments = split_display_segments(norm)
            for seg_idx, seg in enumerate(segments):
                if _no_ws_len(seg) < MIN_NO_WS_LEN:
                    continue
                kept += 1
                seg_id = f"{rid}:s{seg_idx}" if len(segments) > 1 else rid
                key = _sha256(seg)
                counts[key] = counts.get(key, 0) + 1
                if key not in samples:
                    samples[key] = seg_id
                if key not in normalized_map:
                    normalized_map[key] = seg
        raw_out[source.name] = raw
        filtered_out[source.name] = kept
        if not silent:
            print(
                f"  {source.name}: {raw:,} raw -> {kept:,} kept "
                f"({raw - kept:,} filtered)"
            )

    result: dict[str, UniqueFormula] = {}
    empty_counts = {s.column_key: 0 for s in sources}
    empty_samples: dict[str, str | None] = {s.column_key: None for s in sources}
    for key, norm in normalized_map.items():
        counts = {
            s.column_key: per_source_counts[s.column_key].get(key, 0) for s in sources
        }
        samples = {
            s.column_key: per_source_sample_ids[s.column_key].get(key)
            for s in sources
        }
        result[key] = UniqueFormula(
            formula_key=key,
            normalized=norm,
            formula_no_ws_len=_no_ws_len(norm),
            per_source_counts={**empty_counts, **counts},
            per_source_sample_ids={**empty_samples, **samples},
        )
    if not silent:
        print(f"  unique formulas (after global dedupe): {len(result):,}")
    return result


# ---------------------------------------------------------------------------
# Worker (subprocess entry -- must be top-level for pickling)
# ---------------------------------------------------------------------------

_pytexform = None


def _init_worker() -> None:
    global _pytexform
    import pytexform  # noqa: PLC0415 -- intentional lazy import per-process

    _pytexform = pytexform


def _do_parse(formula: str, strict: bool) -> dict:
    t0 = time.perf_counter()
    try:
        _pytexform.parse(formula, strict=strict)
        elapsed_ms = (time.perf_counter() - t0) * 1000
        return {
            "timing_ms": elapsed_ms,
            "parse_ok": True,
            "diagnostic_count": 0,
            "has_partial_result": False,
            "diagnostics": None,
        }
    except _pytexform.ParseError as e:
        elapsed_ms = (time.perf_counter() - t0) * 1000
        diagnostics = list(getattr(e, "diagnostics", None) or [])
        partial = getattr(e, "partial_result", None)
        return {
            "timing_ms": elapsed_ms,
            "parse_ok": False,
            "diagnostic_count": len(diagnostics),
            "has_partial_result": partial is not None,
            "diagnostics": diagnostics,
        }


def _parse_one(args: tuple[str, str]) -> dict:
    formula_key, formula = args
    strict_r = _do_parse(formula, strict=True)
    nonstrict_r = _do_parse(formula, strict=False)
    return {
        "formula_key": formula_key,
        "strict_timing_ms": strict_r["timing_ms"],
        "strict_parse_ok": strict_r["parse_ok"],
        "strict_diagnostic_count": strict_r["diagnostic_count"],
        "strict_has_partial_result": strict_r["has_partial_result"],
        "strict_diagnostics": strict_r["diagnostics"],
        "nonstrict_timing_ms": nonstrict_r["timing_ms"],
        "nonstrict_parse_ok": nonstrict_r["parse_ok"],
        "nonstrict_diagnostic_count": nonstrict_r["diagnostic_count"],
        "nonstrict_has_partial_result": nonstrict_r["has_partial_result"],
    }


# ---------------------------------------------------------------------------
# Parquet schema (dynamic per-source columns)
# ---------------------------------------------------------------------------

_RESULT_COLUMNS: dict[str, pl.PolarsDataType] = {
    "strict_timing_ms": pl.Float64,
    "strict_parse_ok": pl.Boolean,
    "strict_diagnostic_count": pl.Int32,
    "strict_has_partial_result": pl.Boolean,
    "nonstrict_timing_ms": pl.Float64,
    "nonstrict_parse_ok": pl.Boolean,
    "nonstrict_diagnostic_count": pl.Int32,
    "nonstrict_has_partial_result": pl.Boolean,
}


def build_parquet_schema(sources: list[SourceConfig]) -> dict[str, pl.PolarsDataType]:
    schema: dict[str, pl.PolarsDataType] = {
        "formula_key": pl.String,
        "formula": pl.String,
        "formula_no_ws_len": pl.Int32,
    }
    for s in sources:
        schema[f"{s.column_key}_count"] = pl.Int32
        schema[f"{s.column_key}_sample_id"] = pl.String
    schema.update(_RESULT_COLUMNS)
    return schema


def _build_row(
    parse_result: dict, meta: UniqueFormula, sources: list[SourceConfig]
) -> dict:
    row: dict = {
        "formula_key": meta.formula_key,
        "formula": meta.normalized,
        "formula_no_ws_len": meta.formula_no_ws_len,
    }
    for s in sources:
        row[f"{s.column_key}_count"] = meta.per_source_counts.get(s.column_key, 0)
        row[f"{s.column_key}_sample_id"] = meta.per_source_sample_ids.get(s.column_key)
    for k in _RESULT_COLUMNS:
        row[k] = parse_result[k]
    return row


# ---------------------------------------------------------------------------
# Manifest
# ---------------------------------------------------------------------------


def _serialize_sources(sources: list[SourceConfig]) -> list[dict]:
    return [
        {
            "name": s.name,
            "column": s.column,
            "files": list(s.files),
            "max_rows": s.max_rows,
        }
        for s in sources
    ]


def build_manifest(
    output_dir: Path,
    args: argparse.Namespace,
    sources: list[SourceConfig],
    all_formulas: dict[str, UniqueFormula],
    planned_count: int,
    raw_counts: dict[str, int],
    filtered_counts: dict[str, int],
    commit_full: str,
) -> dict:
    return {
        "output_dir": str(output_dir),
        "commit_hash": commit_full[:8],
        "commit_full": commit_full,
        "sources": _serialize_sources(sources),
        "preprocessing_version": PREPROCESSING_VERSION,
        "parse_mode": PARSE_MODE,
        "workers": args.workers,
        "limit": args.limit,
        "preprocessing_stats": {
            "raw_counts": raw_counts,
            "filtered_counts": filtered_counts,
            "unique_formula_count": len(all_formulas),
            "planned_task_count": planned_count,
        },
    }


# ---------------------------------------------------------------------------
# Parse + write results
# ---------------------------------------------------------------------------


def parse_and_write_results(
    all_formulas: dict[str, UniqueFormula],
    output_dir: Path,
    workers: int | None,
    sources: list[SourceConfig],
    *,
    silent: bool = False,
) -> Path | None:
    """Parse every unique formula, write results.parquet + errors.jsonl.

    Returns the path to ``results.parquet`` on success, or ``None`` if there
    was nothing to parse.
    """
    if not all_formulas:
        if not silent:
            print("No formulas to parse.")
        return None

    schema = build_parquet_schema(sources)
    errors_path = output_dir / "errors.jsonl"
    results_path = output_dir / "results.parquet"

    items = [(k, m.normalized) for k, m in all_formulas.items()]

    # Preallocate column buffers once; appending to lists is faster than
    # building a per-row dict and transposing at the end.
    columns: dict[str, list] = {col: [] for col in schema}

    with open(errors_path, "w", encoding="utf-8") as errors_file:
        with ProcessPoolExecutor(
            max_workers=workers, initializer=_init_worker
        ) as executor:
            for parse_result in tqdm(
                executor.map(_parse_one, items, chunksize=500),
                total=len(items),
                desc="parse",
                disable=silent,
            ):
                meta = all_formulas[parse_result["formula_key"]]
                row = _build_row(parse_result, meta, sources)
                for col in schema:
                    columns[col].append(row[col])

                if not parse_result["strict_parse_ok"]:
                    record = dict(row)
                    record["strict_diagnostics"] = (
                        parse_result["strict_diagnostics"] or []
                    )
                    errors_file.write(
                        json.dumps(record, ensure_ascii=False) + "\n"
                    )

    pl.DataFrame(columns, schema=schema).write_parquet(results_path)
    if not silent:
        print(f"Wrote {len(items):,} rows -> {results_path}")
    return results_path


# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------


def _timing_stats(series: pl.Series) -> dict:
    return {
        "mean": round(float(series.mean() or 0), 3),
        "p50": round(float(series.quantile(0.5) or 0), 3),
        "p95": round(float(series.quantile(0.95) or 0), 3),
        "p99": round(float(series.quantile(0.99) or 0), 3),
        "max": round(float(series.max() or 0), 3),
    }


def write_summary(
    output_dir: Path,
    wall_clock_secs: float,
    planned_count: int,
    *,
    silent: bool = False,
) -> Path | None:
    results_path = output_dir / "results.parquet"
    if not results_path.exists():
        if not silent:
            print("results.parquet missing -- cannot write summary")
        return None

    manifest = json.loads((output_dir / "manifest.json").read_text())

    df = pl.read_parquet(results_path)
    total = len(df)

    strict_ok = int(df["strict_parse_ok"].sum())
    strict_failed = total - strict_ok
    nonstrict_ok = int(df["nonstrict_parse_ok"].sum())
    nonstrict_failed = total - nonstrict_ok

    summary = {
        "commit_hash": manifest.get("commit_hash"),
        "commit_full": manifest.get("commit_full"),
        "sources": manifest["sources"],
        "preprocessing_stats": manifest["preprocessing_stats"],
        "total_tasks": planned_count,
        "completed": total,
        "strict": {
            "ok": strict_ok,
            "failed": strict_failed,
            "failure_rate_pct": (
                round(strict_failed / total * 100, 2) if total else 0
            ),
            "timing_ms": _timing_stats(df["strict_timing_ms"]),
        },
        "nonstrict": {
            "ok": nonstrict_ok,
            "failed": nonstrict_failed,
            "failure_rate_pct": (
                round(nonstrict_failed / total * 100, 2) if total else 0
            ),
            "timing_ms": _timing_stats(df["nonstrict_timing_ms"]),
        },
    }

    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, ensure_ascii=False))

    if not silent:
        s, ns = summary["strict"], summary["nonstrict"]
        formulas_per_sec = (
            round(total / wall_clock_secs, 1) if wall_clock_secs > 0.1 else float("nan")
        )
        throughput = (
            f" | throughput: {formulas_per_sec:.1f} formulas/s"
            if formulas_per_sec == formulas_per_sec
            else ""
        )
        print(
            f"\n{'=' * 60}\n"
            f"Done: {total:,} formulas\n"
            f"Strict    -- ok {s['ok']:,} / failed {s['failed']:,}"
            f" ({s['failure_rate_pct']:.2f}%)"
            f"  p50={s['timing_ms']['p50']}ms"
            f" p95={s['timing_ms']['p95']}ms"
            f" p99={s['timing_ms']['p99']}ms\n"
            f"Nonstrict -- ok {ns['ok']:,} / failed {ns['failed']:,}"
            f" ({ns['failure_rate_pct']:.2f}%)"
            f"  p50={ns['timing_ms']['p50']}ms"
            f" p95={ns['timing_ms']['p95']}ms"
            f" p99={ns['timing_ms']['p99']}ms\n"
            f"Wall-clock: {wall_clock_secs:.1f}s"
            + throughput
            + f"\nSummary written: {summary_path}"
        )

    return summary_path


def copy_summary_to_rolling(summary_path: Path, *, silent: bool = False) -> None:
    """Copy summary.json up to results/parse_datasets/summary.json.

    Only fires when the run directory sits inside ``_RESULTS_ROOT``; custom
    ``--output-dir`` paths elsewhere leave the rolling snapshot untouched.
    """
    try:
        summary_path.resolve().relative_to(_RESULTS_ROOT.resolve())
    except ValueError:
        if not silent:
            print(
                f"Summary at {summary_path} is outside {_RESULTS_ROOT}; "
                "skipping rolling copy."
            )
        return
    rolling_path = _RESULTS_ROOT / "summary.json"
    _RESULTS_ROOT.mkdir(parents=True, exist_ok=True)
    shutil.copy2(summary_path, rolling_path)
    if not silent:
        print(f"Rolling summary updated: {rolling_path}")


# ---------------------------------------------------------------------------
# Git helpers
# ---------------------------------------------------------------------------


def current_commit_hash(full: bool = False) -> str:
    """Return the current git HEAD for the repository that owns this file."""
    try:
        toplevel = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=_BENCH_DIR,
            text=True,
        ).strip()
        head = subprocess.check_output(
            ["git", "rev-parse", "HEAD"],
            cwd=toplevel,
            text=True,
        ).strip()
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        raise SystemExit(
            f"Failed to resolve git HEAD for {_BENCH_DIR}: {exc}.\n"
            "Use --output-dir to bypass the commit-aware default path."
        ) from exc
    return head if full else head[:8]


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def prepare_output_dir(output_dir: Path, *, silent: bool = False) -> None:
    """Wipe and recreate the output directory so every run starts fresh."""
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    if not silent:
        print(f"Output dir prepared: {output_dir}")


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Benchmark TeXForm by parsing real-world formula datasets. "
            "Writes results under benchmarks/results/parse_datasets/commit-<hash8>/ "
            "by default. Each run wipes its output directory and starts fresh."
        )
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=_DEFAULT_CONFIG,
        help=f"Path to the sources YAML (default: {_DEFAULT_CONFIG.name})",
    )
    parser.add_argument(
        "--sources",
        type=str,
        default=None,
        help="Comma-separated source names to include (default: all entries in --config)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help=(
            "Output directory. Defaults to "
            "benchmarks/results/parse_datasets/commit-<hash8>/ where <hash8> is "
            "the first 8 chars of `git rev-parse HEAD`."
        ),
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Per-source row limit applied while loading (useful for smoke tests)",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=None,
        help="Process pool size (default: CPU count)",
    )
    parser.add_argument(
        "--silent",
        action="store_true",
        help="Suppress progress bar and informational output",
    )
    return parser.parse_args()


def _resolve_output_dir(args: argparse.Namespace, commit_hash: str) -> Path:
    if args.output_dir is not None:
        return args.output_dir
    return _RESULTS_ROOT / f"commit-{commit_hash}"


def main() -> None:
    args = _parse_args()
    silent = args.silent

    config_path = args.config
    if not config_path.exists():
        raise SystemExit(f"Config file not found: {config_path}")

    selected = (
        {s.strip() for s in args.sources.split(",") if s.strip()}
        if args.sources
        else None
    )
    sources = load_source_configs(config_path, selected=selected)

    commit_full = current_commit_hash(full=True)
    commit_hash = commit_full[:8]
    output_dir = _resolve_output_dir(args, commit_hash)

    if not silent:
        print(f"\n{'=' * 60}")
        print("TeXForm Parse Benchmark -- parse_datasets")
        print(f"Commit: {commit_hash}")
        print(f"Output: {output_dir}")
        print(f"Sources: {', '.join(s.name for s in sources)}")
        if args.limit:
            print(f"Limit (per source): {args.limit:,}")
        print(f"{'=' * 60}\n")

    prepare_output_dir(output_dir, silent=silent)

    if not silent:
        print("Loading and preprocessing formulas...")
    raw_counts: dict[str, int] = {}
    filtered_counts: dict[str, int] = {}
    all_formulas = preprocess_all_sources(
        sources,
        global_limit=args.limit,
        raw_out=raw_counts,
        filtered_out=filtered_counts,
        silent=silent,
    )

    planned_count = len(all_formulas)
    if not silent:
        print(f"\nPlanned tasks: {planned_count:,}\n")

    manifest = build_manifest(
        output_dir,
        args,
        sources,
        all_formulas,
        planned_count,
        raw_counts,
        filtered_counts,
        commit_full,
    )
    (output_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False)
    )

    wall_start = time.perf_counter()
    parse_and_write_results(
        all_formulas, output_dir, args.workers, sources, silent=silent
    )
    wall_secs = time.perf_counter() - wall_start

    summary_path = write_summary(output_dir, wall_secs, planned_count, silent=silent)
    if summary_path is not None:
        copy_summary_to_rolling(summary_path, silent=silent)

    if not silent:
        print(f"\n{'=' * 60}\nDone! Results: {output_dir}\n{'=' * 60}")


if __name__ == "__main__":
    sys.exit(main())
