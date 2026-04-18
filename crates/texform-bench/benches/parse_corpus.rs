use clap::Parser;
use std::time::Instant;
use texform_bench::{config, data, output, runner};

#[derive(Parser)]
#[command(
    name = "texform-bench",
    about = "Benchmark texform parser on real-world corpora"
)]
struct Args {
    #[arg(long = "dataset")]
    datasets: Vec<String>,

    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, hide = true)]
    bench: bool,
}

fn main() {
    let args = Args::parse();
    let bench_root = config::resolve_bench_root();
    let results_root = bench_root.join("results");

    let config = match config::DatasetsConfig::load(&bench_root) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Failed to load datasets.yaml: {error}");
            return;
        }
    };

    let selected = config.filter_by_slugs(&args.datasets);
    if selected.is_empty() {
        eprintln!(
            "No datasets selected. Available: {:?}",
            config
                .datasets
                .iter()
                .map(|dataset| &dataset.slug)
                .collect::<Vec<_>>()
        );
        return;
    }

    let (commit_hash, commit_full) = output::git_hash();
    let mut summaries = Vec::new();
    let mut total_tasks = 0_usize;
    let mut total_strict_failed = 0_usize;
    let mut total_nonstrict_failed = 0_usize;
    let mut ran_datasets = 0_usize;

    for entry in &selected {
        let data_path = bench_root.join(&entry.file);
        match data::check_data_file(&data_path) {
            data::DataFileStatus::Missing => {
                eprintln!(
                    "[{}] data file missing, skipping (run `git lfs pull` to fetch)",
                    entry.slug
                );
                continue;
            }
            data::DataFileStatus::LfsPointer => {
                eprintln!(
                    "[{}] LFS pointer not resolved, skipping (run `git lfs pull` to fetch)",
                    entry.slug
                );
                continue;
            }
            data::DataFileStatus::Ready => {}
        }

        eprintln!(
            "[{}] Loading formulas from {}...",
            entry.slug,
            data_path.display()
        );
        let records = match data::read_formula_records(&data_path, args.limit) {
            Ok(records) => records,
            Err(error) => {
                eprintln!("[{}] Failed to read parquet: {error}", entry.slug);
                continue;
            }
        };
        eprintln!(
            "[{}] Loaded {} formulas, benchmarking...",
            entry.slug,
            records.len()
        );

        let start = Instant::now();
        let results = runner::run_bench(&records);
        let elapsed = start.elapsed();
        let summary = output::build_summary(&entry.slug, &records, &results);

        total_tasks += summary.total_tasks;
        total_strict_failed += summary.strict.failed;
        total_nonstrict_failed += summary.nonstrict.failed;
        ran_datasets += 1;
        summaries.push(summary);

        let summary = summaries
            .last()
            .expect("summary was just pushed and must exist");

        if let Err(error) = output::write_summary(&results_root, &entry.slug, &summary) {
            eprintln!("[{}] Failed to write summary: {error}", entry.slug);
        }
        if let Err(error) = output::write_commit_results(
            &results_root,
            &entry.slug,
            &summary,
            &records,
            &results,
            &commit_hash,
            &commit_full,
        ) {
            eprintln!("[{}] Failed to write commit results: {error}", entry.slug);
        }

        eprintln!(
            "[{}] {} formulas in {:.1}s | strict: {:.2}% fail | nonstrict: {:.2}% fail",
            entry.slug,
            records.len(),
            elapsed.as_secs_f64(),
            summary.strict.failure_rate_pct,
            summary.nonstrict.failure_rate_pct,
        );
    }

    if !summaries.is_empty()
        && let Err(error) = output::write_overall(&results_root, &summaries)
    {
        eprintln!("Failed to write overall summary: {error}");
    }

    if ran_datasets > 1 && total_tasks > 0 {
        let strict_fail_pct = total_strict_failed as f64 / total_tasks as f64 * 100.0;
        let nonstrict_fail_pct = total_nonstrict_failed as f64 / total_tasks as f64 * 100.0;
        eprintln!(
            "\nTotal: {} tasks | strict: {:.2}% fail ({}/{}) | nonstrict: {:.2}% fail ({}/{})",
            total_tasks,
            strict_fail_pct,
            total_strict_failed,
            total_tasks,
            nonstrict_fail_pct,
            total_nonstrict_failed,
            total_tasks,
        );
    }
}
