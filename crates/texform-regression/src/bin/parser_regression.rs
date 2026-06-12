use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};
use texform_regression::stats::ModeStats;
use texform_regression::{config, data, output, runner};

#[derive(Parser)]
#[command(
    name = "parser_regression",
    about = "Run the parser corpus regression suite across configured datasets."
)]
struct Args {
    /// Dataset configuration YAML. Defaults to the texform repo regression/datasets.yaml.
    #[arg(long)]
    datasets_yaml: Option<PathBuf>,

    /// Result output directory. Defaults to results/parser_regression next to datasets-yaml.
    #[arg(long)]
    results_root: Option<PathBuf>,

    #[arg(long = "dataset")]
    datasets: Vec<String>,

    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, hide = true, default_value_t = 0)]
    offset: usize,

    #[arg(long, help = "Run without writing any result files")]
    dry_run: bool,

    #[arg(long, hide = true)]
    skip_commit_results: bool,

    #[arg(
        long,
        help = "Pre-commit probe for selected datasets; refresh all regression results if any summary changed"
    )]
    check: bool,
}

struct RunOptions {
    write: bool,
    strict_errors: bool,
}

struct RunResult {
    summaries: Vec<output::Summary>,
}

#[derive(Clone)]
struct SlowSample {
    duration: Duration,
    formula_id: String,
    formula_len: usize,
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
        .unwrap_or_else(|| config::default_results_root(&datasets_yaml).join("parser_regression"));
    let commits_root = results_root.join("commits");

    let config = match config::DatasetsConfig::load_from_yaml(&datasets_yaml) {
        Ok(config) => config,
        Err(error) => {
            return Err(format!("Failed to load datasets.yaml: {error}"));
        }
    };

    if args.check {
        return run_check(&args, &config, &datasets_yaml, &results_root, &commits_root);
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
        &datasets_yaml,
        &results_root,
        &commits_root,
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
    datasets_yaml: &Path,
    results_root: &Path,
    commits_root: &Path,
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
        datasets_yaml,
        results_root,
        commits_root,
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
        datasets_yaml,
        results_root,
        commits_root,
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
    datasets_yaml: &Path,
    results_root: &Path,
    commits_root: &Path,
    options: RunOptions,
) -> Result<RunResult, String> {
    let latest_baseline = if options.write {
        match output::latest_commit_baseline(commits_root) {
            Ok(baseline) => baseline,
            Err(error) => {
                eprintln!("Failed to load latest regression baseline: {error}");
                None
            }
        }
    } else {
        None
    };

    let commit_info = if options.write && !args.skip_commit_results {
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
        let data_path = config::resolve_dataset_file(datasets_yaml, entry);
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
            "[{}] Reading formulas from {}...",
            entry.slug,
            data_path.display()
        );

        let start = Instant::now();
        let mut accumulator = output::SummaryAccumulator::new();
        let mut slow_nonstrict = Vec::new();
        let mut slow_strict = Vec::new();
        let mut commit_writer = if options.write {
            commit_info
                .as_ref()
                .map(|commit| output::start_commit_results(commits_root, &entry.slug, commit))
                .transpose()
                .map_err(|error| {
                    format!("[{}] Failed to create commit results: {error}", entry.slug)
                })?
        } else {
            None
        };
        let records_read = match data::read_formula_record_batches(
            &data_path,
            args.offset,
            args.limit,
            |records| {
                let results = runner::run_parser_regression(&records);
                collect_slow_samples(&mut slow_nonstrict, &records, &results, false, 5, None);
                collect_slow_samples(
                    &mut slow_strict,
                    &records,
                    &results,
                    true,
                    10,
                    Some(Duration::from_millis(100)),
                );
                if let Some(writer) = commit_writer.as_mut() {
                    writer.write_batch_errors(&records, &results)?;
                }
                accumulator.append(&records, &results);
                drop(results);
                trim_allocator();
                Ok(())
            },
        ) {
            Ok(records_read) => records_read,
            Err(error) => {
                let message = format!("[{}] Failed to read parquet: {error}", entry.slug);
                if options.strict_errors {
                    return Err(message);
                }
                eprintln!("{message}");
                continue;
            }
        };
        let elapsed = start.elapsed();
        let summary = accumulator.finish(&entry.slug);

        total_tasks += summary.total_tasks;
        total_strict_failed += summary.strict.failed;
        total_nonstrict_failed += summary.nonstrict.failed;
        ran_datasets += 1;
        summaries.push(summary);

        let summary = summaries
            .last()
            .expect("summary was just pushed and must exist");

        if options.write
            && let Some(ref commit) = commit_info
            && let Some(writer) = commit_writer
            && let Err(error) = writer.finish(summary, commit)
        {
            let message = format!("[{}] Failed to write commit results: {error}", entry.slug);
            if options.strict_errors {
                return Err(message);
            }
            eprintln!("{message}");
        }

        println!(
            "[{}] {} formulas in {:.1}s\n  {}\n  {}",
            entry.slug,
            records_read,
            elapsed.as_secs_f64(),
            format_mode_stats("strict", &summary.strict),
            format_mode_stats("nonstrict", &summary.nonstrict),
        );
        print_slow_samples(&entry.slug, "nonstrict", &slow_nonstrict);
        print_slow_samples(&entry.slug, "strict", &slow_strict);
    }

    if !summaries.is_empty() {
        let overall = output::build_overall(&summaries);
        if options.write
            && let Err(error) = output::write_run_summary(results_root, &summaries, &overall)
        {
            let message = format!("Failed to write run summary: {error}");
            if options.strict_errors {
                return Err(message);
            }
            eprintln!("{message}");
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

fn collect_slow_samples(
    samples: &mut Vec<SlowSample>,
    records: &[data::FormulaRecord],
    results: &[runner::FormulaResults],
    strict: bool,
    limit: usize,
    threshold: Option<Duration>,
) {
    for (record, result) in records.iter().zip(results.iter()) {
        let duration = if strict {
            result.strict.duration
        } else {
            result.nonstrict.duration
        };
        if threshold.is_some_and(|threshold| duration <= threshold) {
            continue;
        }
        samples.push(SlowSample {
            duration,
            formula_id: record.formula_id.clone(),
            formula_len: record.formula.len(),
        });
    }
    samples.sort_by_key(|sample| std::cmp::Reverse(sample.duration));
    samples.truncate(limit);
}

fn print_slow_samples(slug: &str, mode: &str, samples: &[SlowSample]) {
    if samples.is_empty() {
        return;
    }
    eprintln!("[{slug}] top-{} slow {mode}:", samples.len());
    for sample in samples {
        eprintln!(
            "  {:>10.2}ms  {}  {} chars",
            sample.duration.as_secs_f64() * 1_000.0,
            sample.formula_id,
            sample.formula_len
        );
    }
}

#[cfg(target_os = "linux")]
fn trim_allocator() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }

    unsafe {
        malloc_trim(0);
    }
}

#[cfg(not(target_os = "linux"))]
fn trim_allocator() {}

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
