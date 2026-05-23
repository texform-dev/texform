//! Dump rule target counters per formula into one parquet (or a directory of parquet
//! parts) per dataset.
//!
//! The parent process inspects each dataset's parquet row count, splits the work into
//! fixed-size chunks, and spawns a hidden `--direct` worker for each chunk. Datasets
//! whose total row count fits in a single chunk land in `counter_map/<slug>.parquet`,
//! matching the conventional small-dataset layout. Larger datasets are written as
//! `counter_map/<slug>/part-<offset>-<limit>.parquet`, which downstream consumers read
//! as a multi-file parquet dataset (Polars, PyArrow, and DuckDB all do this natively),
//! so no merge step is needed.
//!
//! Process exit serves as the memory release boundary: each chunk runs in its own
//! short-lived process, so allocator retention from a long-running parse + Arrow
//! workload cannot accumulate.

use clap::Parser;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;
use texform_bench::dump::{FormulaCounter, ParquetRowWriter, RowBuffer, count_node};
use texform_bench::{config, data};
use texform_core::parse::{ParseConfig, ParseContext};

const FLUSH_ROW_THRESHOLD: usize = 250_000;
const DEFAULT_CHUNK_SIZE: usize = 1_000_000;

#[derive(Parser)]
#[command(
    name = "counter_dump",
    about = "Dump per-formula rule target counter rows to parquet"
)]
struct Args {
    /// Dataset configuration YAML. Defaults to the texform repo bench/datasets.yaml.
    #[arg(long)]
    datasets_yaml: Option<PathBuf>,

    /// Result output directory. Defaults to a results/ directory next to datasets-yaml.
    #[arg(long)]
    results_root: Option<PathBuf>,

    /// Counter map output directory. Defaults to <results-root>/counter_map.
    /// Each dataset writes to a subdirectory `<counter-map-root>/<slug>/` containing
    /// one or more `part-<offset>-<limit>.parquet` files.
    #[arg(long)]
    counter_map_root: Option<PathBuf>,

    /// Dataset slug; repeat to select multiple. Empty selects all datasets.
    #[arg(long = "dataset")]
    datasets: Vec<String>,

    /// Per-dataset row cap, useful for debugging.
    #[arg(long)]
    limit: Option<usize>,

    /// Per-dataset formula chunk size. Defaults to 1,000,000.
    #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
    chunk_size: usize,

    /// Reuse existing readable part files that are already in the current plan.
    #[arg(long)]
    reuse_chunks: bool,

    /// Internal: run a single chunk and exit. Hidden from public help.
    #[arg(long, hide = true)]
    direct: bool,

    /// Internal worker formula offset.
    #[arg(long, default_value_t = 0, hide = true)]
    offset: usize,

    /// Internal worker output parquet path.
    #[arg(long, hide = true)]
    out_path: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), String> {
    let datasets_yaml = args
        .datasets_yaml
        .clone()
        .unwrap_or_else(config::default_datasets_yaml);
    let results_root = args
        .results_root
        .clone()
        .unwrap_or_else(|| config::default_results_root(&datasets_yaml));
    let counter_map_root = args
        .counter_map_root
        .clone()
        .unwrap_or_else(|| results_root.join("counter_map"));

    let cfg = config::DatasetsConfig::load_from_yaml(&datasets_yaml)
        .map_err(|error| format!("Failed to load datasets.yaml: {error}"))?;
    let selected = cfg.filter_by_slugs(&args.datasets);
    if selected.is_empty() {
        return Err(format!(
            "No datasets selected. Available: {:?}",
            cfg.datasets.iter().map(|d| &d.slug).collect::<Vec<_>>()
        ));
    }

    if args.direct {
        return run_direct(&args, &selected, &datasets_yaml);
    }
    if args.chunk_size == 0 {
        return Err("--chunk-size must be greater than zero".to_string());
    }
    if args.offset != 0 {
        return Err("--offset is only supported in internal direct mode".to_string());
    }
    if args.out_path.is_some() {
        return Err("--out-path is only supported in internal direct mode".to_string());
    }

    let started = Instant::now();
    for entry in &selected {
        let data_path = config::resolve_dataset_file(&datasets_yaml, entry);
        ensure_data_ready(&data_path, &entry.slug)?;

        let total_rows = parquet_row_count(&data_path)
            .map_err(|error| format!("[{}] inspect parquet: {error}", entry.slug))?;
        let planned_rows = args
            .limit
            .map(|limit| limit.min(total_rows))
            .unwrap_or(total_rows);

        let plan = chunk_plan(planned_rows, args.chunk_size);
        let chunk_paths = plan_output_paths(&counter_map_root, &entry.slug, &plan);
        std::fs::create_dir_all(&counter_map_root)
            .map_err(|error| format!("create {}: {error}", counter_map_root.display()))?;

        prune_conflicting_outputs(
            &counter_map_root,
            &entry.slug,
            &chunk_paths,
            args.reuse_chunks,
        )
        .map_err(|error| format!("[{}] prune stale outputs: {error}", entry.slug))?;

        let layout = if plan.len() <= 1 {
            "single-file"
        } else {
            "multi-part"
        };
        eprintln!(
            "[{}] layout={} formulas={} planned={} chunk-size={} parts={}",
            entry.slug,
            layout,
            total_rows,
            planned_rows,
            args.chunk_size,
            plan.len(),
        );

        for (chunk, path) in plan.iter().zip(chunk_paths.iter()) {
            if args.reuse_chunks && readable_parquet(path) {
                eprintln!(
                    "[{}] reusing offset={} limit={} ({})",
                    entry.slug,
                    chunk.offset,
                    chunk.limit,
                    path.display()
                );
                continue;
            }
            run_chunk_process(&datasets_yaml, path, &entry.slug, chunk)?;
        }
    }

    eprintln!(
        "chunked dump done in {:.1}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[derive(Clone, Copy)]
struct ChunkSpec {
    offset: usize,
    limit: usize,
}

fn chunk_plan(planned_rows: usize, chunk_size: usize) -> Vec<ChunkSpec> {
    let mut plan = Vec::new();
    let mut offset = 0_usize;
    while offset < planned_rows {
        let limit = chunk_size.min(planned_rows - offset);
        plan.push(ChunkSpec { offset, limit });
        offset += limit;
    }
    plan
}

fn part_filename(chunk: &ChunkSpec) -> String {
    format!("part-{}-{}.parquet", chunk.offset, chunk.limit)
}

fn single_file_path(counter_map_root: &Path, slug: &str) -> PathBuf {
    counter_map_root.join(format!("{slug}.parquet"))
}

fn dataset_dir_path(counter_map_root: &Path, slug: &str) -> PathBuf {
    counter_map_root.join(slug)
}

/// Decide where each chunk in `plan` writes its parquet. A plan with at most one
/// chunk produces a single `counter_map/<slug>.parquet` (the conventional parquet
/// layout for small datasets); a multi-chunk plan produces
/// `counter_map/<slug>/part-<offset>-<limit>.parquet`, which downstream consumers
/// read as a multi-file parquet dataset.
fn plan_output_paths(counter_map_root: &Path, slug: &str, plan: &[ChunkSpec]) -> Vec<PathBuf> {
    if plan.len() <= 1 {
        return plan
            .iter()
            .map(|_| single_file_path(counter_map_root, slug))
            .collect();
    }
    let dir = dataset_dir_path(counter_map_root, slug);
    plan.iter()
        .map(|chunk| dir.join(part_filename(chunk)))
        .collect()
}

/// Remove leftover outputs that would shadow the current plan:
///   * if we are about to write a single file but a dataset directory exists, drop it;
///   * if we are about to write a multi-part directory but a single-file shard exists,
///     drop that file;
///   * inside a multi-part directory, drop `part-*.parquet` files that are not in the
///     current plan (a previous `--chunk-size` would otherwise leave duplicate rows).
///
/// When `reuse_chunks` is true, files that match the current plan are kept so the
/// worker can skip them.
fn prune_conflicting_outputs(
    counter_map_root: &Path,
    slug: &str,
    chunk_paths: &[PathBuf],
    reuse_chunks: bool,
) -> Result<(), std::io::Error> {
    let single_file = single_file_path(counter_map_root, slug);
    let dir = dataset_dir_path(counter_map_root, slug);
    let target_single_file = chunk_paths.len() == 1 && chunk_paths[0] == single_file;

    if target_single_file {
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        if !reuse_chunks && single_file.exists() {
            std::fs::remove_file(&single_file)?;
        }
        return Ok(());
    }

    if single_file.exists() {
        std::fs::remove_file(&single_file)?;
    }
    std::fs::create_dir_all(&dir)?;
    let planned: HashSet<PathBuf> = chunk_paths.iter().cloned().collect();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("part-") || !name.ends_with(".parquet") {
            continue;
        }
        let keep = reuse_chunks && planned.contains(&path);
        if !keep {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn run_direct(
    args: &Args,
    selected: &[config::DatasetEntry],
    datasets_yaml: &Path,
) -> Result<(), String> {
    let out_path = args
        .out_path
        .as_ref()
        .ok_or_else(|| "--direct requires --out-path".to_string())?;
    let entry = selected
        .first()
        .ok_or_else(|| "--direct requires exactly one --dataset".to_string())?;
    if selected.len() > 1 {
        return Err("--direct supports a single dataset per invocation".to_string());
    }

    let data_path = config::resolve_dataset_file(datasets_yaml, entry);
    ensure_data_ready(&data_path, &entry.slug)?;

    // Warm up the parse context's lazy globals (avoids first-call cost showing
    // up in the per-chunk timing breakdown).
    let config = ParseConfig::NONSTRICT_NO_RECOVER;
    let parse_ctx = ParseContext::shared();
    let _ = parse_ctx.parse("", &config);

    let dataset_slug = entry.slug.clone();
    eprintln!(
        "[{}] writing counter rows to {}...",
        dataset_slug,
        out_path.display()
    );
    let mut writer = ParquetRowWriter::try_new(out_path)
        .map_err(|error| format!("[{}] create {}: {error}", dataset_slug, out_path.display()))?;
    let mut dataset_buf = RowBuffer::new();
    let mut formulas_seen = 0_usize;
    let mut emitted = 0_usize;
    let mut skipped_parse = 0_usize;
    let mut skipped_empty = 0_usize;

    let started = Instant::now();
    data::read_formula_record_batches(&data_path, args.offset, args.limit, |records| {
        formulas_seen += records.len();
        let batch_buf = records
            .par_iter()
            .fold(
                || (RowBuffer::new(), 0_usize, 0_usize, 0_usize),
                |(mut buf, e, sp, se), record| {
                    let output = parse_ctx.parse(&record.formula, &config);
                    if !output.diagnostics.is_empty() {
                        return (buf, e, sp + 1, se);
                    }
                    let Some(parsed) = output.result else {
                        return (buf, e, sp + 1, se);
                    };
                    let mut counter = FormulaCounter::default();
                    count_node(&parsed.node, &mut counter);
                    if counter.is_empty() {
                        return (buf, e, sp, se + 1);
                    }
                    buf.extend_from_counter(&dataset_slug, &record.formula_id, &counter);
                    (buf, e + 1, sp, se)
                },
            )
            .reduce(
                || (RowBuffer::new(), 0_usize, 0_usize, 0_usize),
                |(mut a, ea, pa, ema), (b, eb, pb, emb)| {
                    a.merge(b);
                    (a, ea + eb, pa + pb, ema + emb)
                },
            );
        emitted += batch_buf.1;
        skipped_parse += batch_buf.2;
        skipped_empty += batch_buf.3;
        dataset_buf.merge(batch_buf.0);

        if dataset_buf.len() >= FLUSH_ROW_THRESHOLD {
            let buffer = std::mem::take(&mut dataset_buf);
            writer.write_buffer(buffer)?;
        }
        Ok(())
    })
    .map_err(|error| format!("[{}] process parquet: {error}", dataset_slug))?;

    if !dataset_buf.is_empty() {
        writer
            .write_buffer(std::mem::take(&mut dataset_buf))
            .map_err(|error| format!("[{}] write parquet: {error}", dataset_slug))?;
    }
    let counter_rows = writer
        .finish()
        .map_err(|error| format!("[{}] finish parquet: {error}", dataset_slug))?;

    eprintln!(
        "[{}] chunk done in {:.1}s. formulas={} rows={} emitted={} skipped(parse-fail)={} skipped(zero-target)={}",
        dataset_slug,
        started.elapsed().as_secs_f64(),
        formulas_seen,
        counter_rows,
        emitted,
        skipped_parse,
        skipped_empty,
    );
    Ok(())
}

fn ensure_data_ready(path: &Path, slug: &str) -> Result<(), String> {
    match data::check_data_file(path) {
        data::DataFileStatus::Ready => Ok(()),
        data::DataFileStatus::Missing => Err(format!(
            "[{slug}] data file missing (run `git lfs pull` to fetch)"
        )),
        data::DataFileStatus::LfsPointer => Err(format!(
            "[{slug}] LFS pointer not resolved (run `git lfs pull` to fetch)"
        )),
    }
}

fn parquet_row_count(path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = SerializedFileReader::new(file)?;
    let rows = reader.metadata().file_metadata().num_rows();
    Ok(rows.try_into()?)
}

fn readable_parquet(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    File::open(path)
        .ok()
        .and_then(|file| ParquetRecordBatchReaderBuilder::try_new(file).ok())
        .is_some()
}

fn run_chunk_process(
    datasets_yaml: &Path,
    out_path: &Path,
    slug: &str,
    chunk: &ChunkSpec,
) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("resolve current exe: {error}"))?;
    let status = Command::new(&exe)
        .arg("--direct")
        .arg("--datasets-yaml")
        .arg(datasets_yaml)
        .arg("--dataset")
        .arg(slug)
        .arg("--offset")
        .arg(chunk.offset.to_string())
        .arg("--limit")
        .arg(chunk.limit.to_string())
        .arg("--out-path")
        .arg(out_path)
        .status()
        .map_err(|error| format!("run chunk process: {error}"))?;
    if !status.success() {
        return Err(format!(
            "[{}] chunk offset={} limit={} failed with status {}",
            slug, chunk.offset, chunk.limit, status
        ));
    }
    Ok(())
}
