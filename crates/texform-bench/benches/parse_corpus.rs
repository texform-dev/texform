use clap::Parser;
use std::time::Instant;
use texform_bench::stats::ModeStats;
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

    #[arg(long, help = "Run without writing any result files")]
    dry_run: bool,

    #[arg(
        long,
        help = "Compare results against saved summaries; exit non-zero on mismatch"
    )]
    check: bool,

    #[arg(long, hide = true)]
    bench: bool,
}

fn main() {
    let args = Args::parse();
    let bench_root = config::resolve_bench_root();
    let results_root = bench_root.join("results");
    let history_root = bench_root.join("history");
    let write = !args.dry_run && !args.check;
    let latest_baseline = if write {
        match output::latest_commit_baseline(&history_root) {
            Ok(baseline) => baseline,
            Err(error) => {
                eprintln!("Failed to load latest benchmark baseline: {error}");
                None
            }
        }
    } else {
        None
    };

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

    let commit_info = if write {
        Some(output::git_commit_info())
    } else {
        None
    };
    let mut summaries = Vec::new();
    let mut check_failures = Vec::new();
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

        if args.check {
            match output::check_summary(&results_root, &entry.slug, summary) {
                Ok(Some(diff)) => check_failures.push(diff),
                Ok(None) => {}
                Err(error) => eprintln!("[{}] Failed to check summary: {error}", entry.slug),
            }
        }

        if write {
            if let Err(error) = output::write_summary(&results_root, &entry.slug, summary) {
                eprintln!("[{}] Failed to write summary: {error}", entry.slug);
            }
            if let Some((ref commit_hash, ref commit_full, ref commit_date)) = commit_info {
                if let Err(error) = output::write_commit_results(
                    &history_root,
                    &entry.slug,
                    summary,
                    &records,
                    &results,
                    commit_hash,
                    commit_full,
                    commit_date,
                ) {
                    eprintln!("[{}] Failed to write commit results: {error}", entry.slug);
                }
            }
        }

        println!(
            "[{}] {} formulas in {:.1}s\n  {}\n  {}",
            entry.slug,
            records.len(),
            elapsed.as_secs_f64(),
            format_mode_stats("strict", &summary.strict),
            format_mode_stats("nonstrict", &summary.nonstrict),
        );
    }

    if !summaries.is_empty() {
        let overall = output::build_overall(&summaries);
        if write {
            if let Err(error) = output::write_overall(&results_root, &overall) {
                eprintln!("Failed to write overall summary: {error}");
            }
        }

        if total_tasks > 0 {
            let strict_fail_pct = total_strict_failed as f64 / total_tasks as f64 * 100.0;
            let nonstrict_fail_pct = total_nonstrict_failed as f64 / total_tasks as f64 * 100.0;
            println!(
                "\nTotal: {} tasks across {} dataset(s)\n  {} ({}/{})\n  {} ({}/{})",
                total_tasks,
                ran_datasets,
                format_mode_stats_with_failures("strict", &overall.strict, strict_fail_pct),
                total_strict_failed,
                total_tasks,
                format_mode_stats_with_failures(
                    "nonstrict",
                    &overall.nonstrict,
                    nonstrict_fail_pct,
                ),
                total_nonstrict_failed,
                total_tasks,
            );

            if let Some(baseline) = &latest_baseline {
                for warning in output::detect_mean_regressions(&summaries, baseline) {
                    println!(
                        "WARNING: {} mean latency regressed vs latest snapshot {}: current {:.2}ms, baseline {:.2}ms ({:.1}% of baseline)",
                        warning.mode,
                        warning.baseline_commit_hash,
                        warning.current_mean_ms,
                        warning.baseline_mean_ms,
                        warning.ratio_pct(),
                    );
                }
            }
        }
    }

    if args.check && !check_failures.is_empty() {
        eprintln!("\nBench results have changed:");
        for failure in &check_failures {
            eprintln!("  {failure}");
        }
        eprintln!("\nRun the following command and commit the updated results:");
        eprintln!("  cargo bench -p texform-bench --bench parse_corpus");
        std::process::exit(1);
    }
}

fn format_mode_stats(label: &str, stats: &ModeStats) -> String {
    format!(
        "{label}: {:.2}% fail | mean {:.2}ms | p50 {:.2} | p95 {:.2} | p99 {:.2} | max {:.2}",
        stats.failure_rate_pct,
        stats.timing_ms.mean,
        stats.timing_ms.p50,
        stats.timing_ms.p95,
        stats.timing_ms.p99,
        stats.timing_ms.max,
    )
}

fn format_mode_stats_with_failures(
    label: &str,
    stats: &ModeStats,
    failure_rate_pct: f64,
) -> String {
    format!(
        "{label}: {:.2}% fail | mean {:.2}ms | p50 {:.2} | p95 {:.2} | p99 {:.2} | max {:.2}",
        failure_rate_pct,
        stats.timing_ms.mean,
        stats.timing_ms.p50,
        stats.timing_ms.p95,
        stats.timing_ms.p99,
        stats.timing_ms.max,
    )
}
