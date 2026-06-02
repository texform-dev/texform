use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use texform_regression::{config, transform_contract};

#[derive(Parser)]
#[command(
    name = "transform_contract",
    about = "Run the transform eliminated-form contract across configured datasets."
)]
struct Args {
    /// Dataset configuration YAML. Defaults to the texform repo regression/datasets.yaml.
    #[arg(long)]
    datasets_yaml: Option<PathBuf>,

    /// Result output directory. Defaults to results/transform_contract next to datasets-yaml.
    #[arg(long)]
    results_root: Option<PathBuf>,

    #[arg(long = "dataset")]
    datasets: Vec<String>,

    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, help = "Run without writing any result files")]
    dry_run: bool,

    #[arg(long, hide = true)]
    skip_commit_results: bool,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<ExitCode, String> {
    let datasets_yaml = args
        .datasets_yaml
        .clone()
        .unwrap_or_else(config::default_datasets_yaml);
    let results_root = args
        .results_root
        .clone()
        .unwrap_or_else(|| config::default_results_root(&datasets_yaml).join("transform_contract"));

    let outcome = transform_contract::run(transform_contract::RunConfig {
        datasets_yaml,
        results_root,
        datasets: args.datasets,
        limit: args.limit,
        dry_run: args.dry_run,
        skip_commit_results: args.skip_commit_results,
    })
    .map_err(|error| error.to_string())?;

    if outcome.summary.checked_formulas > 0 {
        println!(
            "\nTotal: {} formulas across {} dataset(s); {} violating formula(s), {} violation(s), {:.4}% formula rate",
            outcome.summary.checked_formulas,
            outcome.summary.metadata.datasets.len(),
            outcome.summary.violating_formulas,
            outcome.summary.violations,
            outcome.summary.violating_formulas_pct,
        );
    }

    if outcome.unallowed_violations > 0 {
        eprintln!(
            "Transform contract failed: {} violation(s) are not covered by contract_exceptions.yaml",
            outcome.unallowed_violations
        );
        return Ok(ExitCode::FAILURE);
    }

    Ok(ExitCode::SUCCESS)
}
