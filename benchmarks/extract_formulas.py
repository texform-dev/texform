"""Extract a text column from a directory of Parquet shards.

Most LaTeX formula datasets published on the Hub (including
``OleehyO/latex-formulas-80M``) bundle heavy binary payloads -- rendered
images, bounding boxes, etc. -- alongside the formula text. The parse
benchmark only needs the text column, so this helper pulls a single column
out of every Parquet shard under a directory tree and writes a compact
single-file Parquet into ``benchmarks/cache/``.

The output file can then be referenced from ``datasets.yaml`` as a
``type: parquet`` source, keeping the benchmark loop independent of HF.

Usage:
    uv run python benchmarks/extract_formulas.py --input /path/to/dataset
    uv run python benchmarks/extract_formulas.py \\
        --input /path/to/dataset \\
        --column latex_formula \\
        --name latex-formulas-80m

    # Only include the four benchmark_* subsets (immediate subdirectories):
    uv run python benchmarks/extract_formulas.py \\
        --input /path/to/dataset \\
        --subset benchmark_complex benchmark_matrix benchmark_ordinary benchmark_sample
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import polars as pl

_BENCH_DIR = Path(__file__).resolve().parent
_DEFAULT_OUTPUT_DIR = _BENCH_DIR / "cache"

# Canonical set of benchmark subsets shipped with OleehyO/latex-formulas-80M.
BENCHMARK_SUBSETS = [
    "benchmark_complex",
    "benchmark_matrix",
    "benchmark_ordinary",
    "benchmark_sample",
]


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Extract a single text column from every Parquet shard under "
            "--input into a compact Parquet file inside benchmarks/cache/."
        )
    )
    parser.add_argument(
        "--input",
        type=Path,
        required=True,
        help="Directory containing Parquet shards (scanned recursively).",
    )
    parser.add_argument(
        "--column",
        type=str,
        default="latex_formula",
        help="Column to keep (default: latex_formula).",
    )
    parser.add_argument(
        "--name",
        type=str,
        default=None,
        help=(
            "Output filename stem (default: the input directory's basename). "
            "Writes to <--output-dir>/<name>.parquet."
        ),
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=_DEFAULT_OUTPUT_DIR,
        help=f"Output directory (default: {_DEFAULT_OUTPUT_DIR.relative_to(_BENCH_DIR)})",
    )
    parser.add_argument(
        "--subset",
        metavar="NAME",
        nargs="+",
        default=None,
        help=(
            "Restrict extraction to these immediate subdirectories (subsets) of --input. "
            "May be specified multiple times or as a space-separated list. "
            "Shards not under a matching subdirectory are skipped. "
            f"Benchmark subsets: {', '.join(BENCHMARK_SUBSETS)}"
        ),
    )
    parser.add_argument(
        "--dedupe",
        action="store_true",
        help="Drop exact duplicates on the text column before writing.",
    )
    parser.add_argument(
        "--compression",
        type=str,
        default="zstd",
        help="Parquet compression codec (default: zstd).",
    )
    return parser.parse_args()


def _filter_by_subsets(shards: list[Path], input_root: Path, subsets: list[str]) -> list[Path]:
    """Keep only shards whose first path component (relative to input_root) is in *subsets*."""
    subset_set = set(subsets)
    filtered = [p for p in shards if p.relative_to(input_root).parts[0] in subset_set]
    missing = subset_set - {p.relative_to(input_root).parts[0] for p in shards}
    if missing:
        print(f"Warning: subset(s) not found under {input_root}: {', '.join(sorted(missing))}")
    return filtered


def main() -> int:
    args = _parse_args()

    input_root: Path = args.input.resolve()
    if not input_root.is_dir():
        raise SystemExit(f"--input is not a directory: {input_root}")

    shards = sorted(input_root.rglob("*.parquet"))
    if not shards:
        raise SystemExit(f"No .parquet files found under {input_root}")

    if args.subset:
        shards = _filter_by_subsets(shards, input_root, args.subset)
        if not shards:
            raise SystemExit(
                f"No .parquet files remain after subset filter: {args.subset}"
            )

    name = args.name or input_root.name
    output_dir: Path = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / f"{name}.parquet"

    print(f"Input:       {input_root}")
    if args.subset:
        print(f"Subsets:     {', '.join(args.subset)}")
    print(f"Shards:      {len(shards):,}")
    print(f"Column:      {args.column}")
    print(f"Output:      {output_path}")
    print(f"Dedupe:      {args.dedupe}")
    print(f"Compression: {args.compression}")

    # scan_parquet prunes to the requested column, so image / bbox payloads
    # are never materialized.
    lf = pl.scan_parquet([str(p) for p in shards]).select(
        pl.col(args.column).cast(pl.String)
    )

    if args.dedupe:
        # Dedupe forces a full materialization (hash set over all rows).
        # Fine for modest corpora; for 10M+ rows prefer running without it
        # and letting parse_datasets.py dedupe by SHA-256 at bench time.
        lf = lf.unique(subset=[args.column], keep="first", maintain_order=False)
        df = lf.collect(engine="streaming")
        rows = df.height
        if rows == 0:
            raise SystemExit(
                f"No rows extracted. Does column {args.column!r} exist in the shards?"
            )
        df.write_parquet(output_path, compression=args.compression)
    else:
        # Streaming sink keeps memory bounded regardless of corpus size.
        lf.sink_parquet(output_path, compression=args.compression)
        rows = pl.scan_parquet(output_path).select(pl.len()).collect().item()
        if rows == 0:
            raise SystemExit(
                f"No rows extracted. Does column {args.column!r} exist in the shards?"
            )

    size_mb = output_path.stat().st_size / (1024 * 1024)
    print(f"Wrote {rows:,} rows to {output_path} ({size_mb:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
