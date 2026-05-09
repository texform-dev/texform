//! Dump rule target counters per formula across bench datasets to a single
//! parquet file.

use clap::Parser;
use rayon::prelude::*;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;
use texform_bench::dump::{FormulaCounter, RowBuffer, count_node};
use texform_bench::{config, data};
use texform_core::api;

#[derive(Parser)]
#[command(
    name = "texform-counter-dump",
    about = "Dump per-formula rule target counter rows to parquet"
)]
struct Args {
    /// Output parquet path. Defaults to <bench>/results/counter_map.parquet.
    #[arg(long)]
    out: Option<PathBuf>,

    /// Dataset slug; repeat to select multiple. Empty selects all datasets.
    #[arg(long = "dataset")]
    datasets: Vec<String>,

    /// Per-dataset row cap, useful for debugging.
    #[arg(long)]
    limit: Option<usize>,
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
    let bench_root = config::resolve_bench_root();
    let results_root = bench_root.join("results");
    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| results_root.join("counter_map.parquet"));

    let cfg = config::DatasetsConfig::load(&bench_root)
        .map_err(|error| format!("Failed to load datasets.yaml: {error}"))?;
    let selected = cfg.filter_by_slugs(&args.datasets);
    if selected.is_empty() {
        return Err(format!(
            "No datasets selected. Available: {:?}",
            cfg.datasets.iter().map(|d| &d.slug).collect::<Vec<_>>()
        ));
    }

    let _ = api::parse_latex("", false);

    let mut total_buf = RowBuffer::new();
    let mut total_formulas = 0_usize;
    let mut total_emitted = 0_usize;
    let mut total_skipped_parse = 0_usize;
    let mut total_skipped_empty = 0_usize;

    let started = Instant::now();
    for entry in &selected {
        let data_path = bench_root.join(&entry.file);
        match data::check_data_file(&data_path) {
            data::DataFileStatus::Missing => {
                return Err(format!(
                    "[{}] data file missing (run `git lfs pull` to fetch)",
                    entry.slug
                ));
            }
            data::DataFileStatus::LfsPointer => {
                return Err(format!(
                    "[{}] LFS pointer not resolved (run `git lfs pull` to fetch)",
                    entry.slug
                ));
            }
            data::DataFileStatus::Ready => {}
        }

        eprintln!("[{}] loading {}...", entry.slug, data_path.display());
        let records = data::read_formula_records(&data_path, args.limit)
            .map_err(|error| format!("[{}] read parquet: {error}", entry.slug))?;
        eprintln!("[{}] {} formulas, dumping...", entry.slug, records.len());

        let dataset_slug = entry.slug.clone();
        let dataset_buf = records
            .par_iter()
            .fold(
                || (RowBuffer::new(), 0_usize, 0_usize, 0_usize),
                |(mut buf, emitted, skipped_parse, skipped_empty), record| {
                    let output = api::parse_latex(&record.formula, false);
                    if !output.diagnostics.is_empty() {
                        return (buf, emitted, skipped_parse + 1, skipped_empty);
                    }
                    let Some(parsed) = output.result else {
                        return (buf, emitted, skipped_parse + 1, skipped_empty);
                    };
                    let mut counter = FormulaCounter::default();
                    count_node(&parsed.node, &mut counter);
                    if counter.is_empty() {
                        return (buf, emitted, skipped_parse, skipped_empty + 1);
                    }
                    buf.extend_from_counter(&dataset_slug, &record.formula_id, &counter);
                    (buf, emitted + 1, skipped_parse, skipped_empty)
                },
            )
            .reduce(
                || (RowBuffer::new(), 0_usize, 0_usize, 0_usize),
                |(mut a, ea, pa, ema), (b, eb, pb, emb)| {
                    a.merge(b);
                    (a, ea + eb, pa + pb, ema + emb)
                },
            );

        total_formulas += records.len();
        total_emitted += dataset_buf.1;
        total_skipped_parse += dataset_buf.2;
        total_skipped_empty += dataset_buf.3;
        eprintln!(
            "[{}] emitted={} skipped(parse-fail)={} skipped(zero-target)={}",
            entry.slug, dataset_buf.1, dataset_buf.2, dataset_buf.3,
        );
        total_buf.merge(dataset_buf.0);
    }

    eprintln!(
        "writing {} rows to {}...",
        total_buf.len(),
        out_path.display()
    );
    total_buf
        .write_parquet(&out_path)
        .map_err(|error| format!("write {}: {error}", out_path.display()))?;
    eprintln!(
        "done in {:.1}s. formulas={} emitted={} skipped(parse-fail)={} skipped(zero-target)={}",
        started.elapsed().as_secs_f64(),
        total_formulas,
        total_emitted,
        total_skipped_parse,
        total_skipped_empty,
    );
    Ok(())
}
