use clap::Parser;
use std::process::ExitCode;
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
        help = "Pre-commit probe for selected datasets; refresh all bench results if any summary changed"
    )]
    check: bool,

    #[arg(long, hide = true)]
    bench: bool,
}

struct RunOptions {
    write: bool,
    strict_errors: bool,
}

struct RunResult {
    summaries: Vec<output::Summary>,
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
    let history_root = bench_root.join("history");

    let config = match config::DatasetsConfig::load(&bench_root) {
        Ok(config) => config,
        Err(error) => {
            return Err(format!("Failed to load datasets.yaml: {error}"));
        }
    };

    if args.check {
        return run_check(&args, &config, &bench_root, &results_root, &history_root);
    }

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
        return Ok(());
    }

    run_datasets(
        &args,
        &selected,
        &bench_root,
        &results_root,
        &history_root,
        RunOptions {
            write: !args.dry_run,
            strict_errors: false,
        },
    )?;

    Ok(())
}

fn run_check(
    args: &Args,
    config: &config::DatasetsConfig,
    bench_root: &std::path::Path,
    results_root: &std::path::Path,
    history_root: &std::path::Path,
) -> Result<(), String> {
    if args.dry_run {
        return Err("--check cannot be combined with --dry-run".to_string());
    }
    if args.limit.is_some() {
        return Err("--check cannot be combined with --limit".to_string());
    }

    let selected = config.filter_by_slugs(&args.datasets);
    if selected.is_empty() {
        return Err(format!(
            "No datasets selected. Available: {:?}",
            config
                .datasets
                .iter()
                .map(|dataset| &dataset.slug)
                .collect::<Vec<_>>()
        ));
    }

    let probe = run_datasets(
        args,
        &selected,
        bench_root,
        results_root,
        history_root,
        RunOptions {
            write: false,
            strict_errors: true,
        },
    )?;

    let needs_refresh = output::summaries_need_refresh(results_root, &probe.summaries)
        .map_err(|error| format!("Failed to check stored summaries: {error}"))?;

    if !needs_refresh {
        return Ok(());
    }

    run_datasets(
        args,
        &config.datasets,
        bench_root,
        results_root,
        history_root,
        RunOptions {
            write: true,
            strict_errors: true,
        },
    )?;

    Ok(())
}

fn run_datasets(
    args: &Args,
    selected: &[config::DatasetEntry],
    bench_root: &std::path::Path,
    results_root: &std::path::Path,
    history_root: &std::path::Path,
    options: RunOptions,
) -> Result<RunResult, String> {
    let latest_baseline = if options.write {
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

    let commit_info = if options.write {
        Some(output::git_commit_info())
    } else {
        None
    };
    let mut summaries = Vec::new();
    let mut total_tasks = 0_usize;
    let mut total_strict_failed = 0_usize;
    let mut total_nonstrict_failed = 0_usize;
    let mut ran_datasets = 0_usize;

    for entry in selected {
        let data_path = bench_root.join(&entry.file);
        match data::check_data_file(&data_path) {
            data::DataFileStatus::Missing => {
                let message = format!(
                    "[{}] data file missing (run `git lfs pull` to fetch)",
                    entry.slug
                );
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}, skipping");
                continue;
            }
            data::DataFileStatus::LfsPointer => {
                let message = format!(
                    "[{}] LFS pointer not resolved (run `git lfs pull` to fetch)",
                    entry.slug
                );
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}, skipping");
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
                let message = format!("[{}] Failed to read parquet: {error}", entry.slug);
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}");
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

        if options.write {
            if let Err(error) = output::write_summary(&results_root, &entry.slug, summary) {
                let message = format!("[{}] Failed to write summary: {error}", entry.slug);
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}");
            }
            if let Some(ref commit) = commit_info {
                if let Err(error) = output::write_commit_results(
                    &history_root,
                    &entry.slug,
                    summary,
                    &records,
                    &results,
                    commit,
                ) {
                    let message =
                        format!("[{}] Failed to write commit results: {error}", entry.slug);
                    if options.strict_errors {
                        return Err(message);
                    }
                    eprintln!("{message}");
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
        if options.write {
            if let Err(error) = output::write_overall(&results_root, &overall) {
                let message = format!("Failed to write overall summary: {error}");
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}");
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

    Ok(RunResult { summaries })
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
