use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use rayon::prelude::*;
use serde::Serialize;
use texform_bench::{config, data, output};
use texform_core::ast::Ast;
use texform_core::parse::ParseConfig;
use texform_core::serialize::serialize;

#[derive(Parser)]
#[command(
    name = "evaluate_flatten_groups",
    about = "Evaluate the FlattenGroups transform phase across bench datasets."
)]
struct Args {
    /// Dataset configuration YAML. Defaults to the texform repo bench/datasets.yaml.
    #[arg(long)]
    datasets_yaml: Option<PathBuf>,

    /// Result output directory. Defaults to results/flatten_groups next to datasets-yaml.
    #[arg(long)]
    results_root: Option<PathBuf>,

    /// Comma-separated dataset slugs.
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "linxy,unimer,wikipedia,lf80m-benchmarks"
    )]
    datasets: Vec<String>,

    /// Limit records per dataset.
    #[arg(long)]
    limit: Option<usize>,

    /// Number of highest-impact examples to keep per comparison.
    #[arg(long, default_value_t = 12)]
    sample_limit: usize,
}

#[derive(Clone, Copy, Debug)]
enum Comparison {
    NoTransformVsFlattenOnly,
    OtherPhasesVsFull,
}

impl Comparison {
    fn label(self) -> &'static str {
        match self {
            Self::NoTransformVsFlattenOnly => "no_transform_vs_flatten_only",
            Self::OtherPhasesVsFull => "other_phases_vs_full",
        }
    }
}

#[derive(Default)]
struct ComparisonStats {
    changed: usize,
    removed_empty: usize,
    replaced_single_child: usize,
    inlined_multi_child: usize,
    unwrapped_slot: usize,
    samples: TopSamples,
}

#[derive(Default)]
struct DatasetStats {
    records: usize,
    parsed: usize,
    parse_failed: usize,
    transform_failed: usize,
    flatten_only: ComparisonStats,
    full_delta: ComparisonStats,
}

#[derive(Default)]
struct RecordAnalysis {
    parsed: bool,
    transform_failures: usize,
    flatten_only: Option<ChangeRecord>,
    full_delta: Option<ChangeRecord>,
}

struct ChangeRecord {
    formula_id: String,
    source: String,
    before: String,
    after: String,
    impact: usize,
    formula_len: usize,
    report: texform_transform::FlattenGroupsReport,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize)]
struct Sample {
    impact: usize,
    formula_len: usize,
    dataset: String,
    formula_id: String,
    source: String,
    before: String,
    after: String,
    removed_empty: usize,
    replaced_single_child: usize,
    inlined_multi_child: usize,
    unwrapped_slot: usize,
}

#[derive(Default)]
struct TopSamples {
    heap: BinaryHeap<Reverse<Sample>>,
}

impl TopSamples {
    fn push(&mut self, sample: Sample, limit: usize) {
        if limit == 0 {
            return;
        }
        self.heap.push(Reverse(sample));
        if self.heap.len() > limit {
            self.heap.pop();
        }
    }

    fn sorted(&self) -> Vec<Sample> {
        let mut samples = self
            .heap
            .iter()
            .map(|Reverse(sample)| sample.clone())
            .collect::<Vec<_>>();
        samples.sort_by(|left, right| {
            right
                .impact
                .cmp(&left.impact)
                .then_with(|| right.formula_len.cmp(&left.formula_len))
                .then_with(|| left.dataset.cmp(&right.dataset))
                .then_with(|| left.formula_id.cmp(&right.formula_id))
                .then_with(|| left.source.cmp(&right.source))
        });
        samples
    }
}

#[derive(Serialize)]
struct EvaluationReport {
    datasets: Vec<DatasetOutput>,
    overall: DatasetOutput,
}

#[derive(Serialize)]
struct DatasetOutput {
    dataset: String,
    records: usize,
    parsed: usize,
    parse_failed: usize,
    transform_failed: usize,
    comparisons: Vec<ComparisonOutput>,
}

#[derive(Serialize)]
struct ComparisonOutput {
    comparison: &'static str,
    changed: usize,
    records_pct: f64,
    parsed_pct: f64,
    removed_empty: usize,
    replaced_single_child: usize,
    inlined_multi_child: usize,
    unwrapped_slot: usize,
}

#[derive(Serialize)]
struct DatasetSamplesOutput {
    dataset: String,
    comparisons: Vec<ComparisonSamplesOutput>,
}

#[derive(Serialize)]
struct ComparisonSamplesOutput {
    comparison: &'static str,
    samples: Vec<Sample>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let wanted = args.datasets.iter().cloned().collect::<HashSet<_>>();
    let datasets_yaml = args
        .datasets_yaml
        .clone()
        .unwrap_or_else(config::default_datasets_yaml);
    let results_root = args
        .results_root
        .clone()
        .unwrap_or_else(|| config::default_results_root(&datasets_yaml).join("flatten_groups"));
    let commit_root = results_root
        .join("commits")
        .join(output::git_commit_info().commit_dir_name());
    let datasets = config::DatasetsConfig::load_from_yaml(&datasets_yaml)?;
    let parse_cfg = ParseConfig {
        abort_on_error: true,
        ..Default::default()
    };
    let engine = texform::Engine::builder()
        .profile(texform::Profile::Equiv)
        .build()?;

    let start = Instant::now();
    let mut overall = DatasetStats::default();
    let mut dataset_outputs = Vec::new();

    for dataset in datasets
        .datasets
        .iter()
        .filter(|dataset| wanted.contains(&dataset.slug))
    {
        let data_path = config::resolve_dataset_file(&datasets_yaml, dataset);
        match data::check_data_file(&data_path) {
            data::DataFileStatus::Ready => {}
            other => {
                eprintln!(
                    "[{}] skipping {:?}: {}",
                    dataset.slug,
                    data_path,
                    data_file_status_label(other)
                );
                continue;
            }
        }

        let mut stats = DatasetStats::default();
        data::read_formula_record_batches(&data_path, 0, args.limit, |records| {
            stats.records += records.len();
            overall.records += records.len();

            let analyses = records
                .par_iter()
                .map(|record| analyze_record(record, &engine, &parse_cfg))
                .collect::<Vec<_>>();

            for analysis in analyses {
                if !analysis.parsed {
                    stats.parse_failed += 1;
                    overall.parse_failed += 1;
                    continue;
                }

                stats.parsed += 1;
                overall.parsed += 1;
                stats.transform_failed += analysis.transform_failures;
                overall.transform_failed += analysis.transform_failures;

                if let Some(change) = analysis.flatten_only {
                    record_change(
                        &mut stats.flatten_only,
                        Comparison::NoTransformVsFlattenOnly,
                        &dataset.slug,
                        &change,
                        args.sample_limit,
                    );
                    record_change(
                        &mut overall.flatten_only,
                        Comparison::NoTransformVsFlattenOnly,
                        &dataset.slug,
                        &change,
                        0,
                    );
                }
                if let Some(change) = analysis.full_delta {
                    record_change(
                        &mut stats.full_delta,
                        Comparison::OtherPhasesVsFull,
                        &dataset.slug,
                        &change,
                        args.sample_limit,
                    );
                    record_change(
                        &mut overall.full_delta,
                        Comparison::OtherPhasesVsFull,
                        &dataset.slug,
                        &change,
                        0,
                    );
                }
            }
            Ok(())
        })?;

        let dataset_summary = dataset_output(&dataset.slug, &stats);
        let dataset_samples = dataset_samples_output(&dataset.slug, &stats);
        print_dataset(&dataset.slug, &stats);
        output::write_json_file(
            &commit_root.join(&dataset.slug).join("summary.json"),
            &dataset_summary,
        )?;
        output::write_json_file(
            &commit_root.join(&dataset.slug).join("samples.json"),
            &dataset_samples,
        )?;
        dataset_outputs.push(dataset_summary);
    }

    let elapsed_sec = start.elapsed().as_secs_f64();
    let report = EvaluationReport {
        datasets: dataset_outputs,
        overall: dataset_output("overall", &overall),
    };
    let summary_path = results_root.join("summary.json");
    output::write_json_file(&summary_path, &report)?;

    print_dataset("overall", &overall);
    println!("elapsed_sec\t{elapsed_sec:.2}");
    println!("results_root\t{}", results_root.display());
    println!("summary_json\t{}", summary_path.display());

    Ok(())
}

fn flatten_only_config() -> texform::TransformConfig {
    texform::TransformConfig {
        rewrite_enabled: false,
        lower_attributes_enabled: false,
        flatten_groups: texform::FlattenGroupsConfig::ENABLED,
        max_iterations: 100,
    }
}

fn other_phases_config(engine: &texform::Engine) -> texform::TransformConfig {
    let mut config = *engine.default_transform_config();
    config.flatten_groups = texform::FlattenGroupsConfig::DISABLED;
    config
}

fn analyze_record(
    record: &data::FormulaRecord,
    engine: &texform::Engine,
    parse_cfg: &ParseConfig,
) -> RecordAnalysis {
    let Ok(ast) = engine
        .parser()
        .parse_to_ast_with(&record.formula, parse_cfg)
    else {
        return RecordAnalysis::default();
    };

    let mut analysis = RecordAnalysis {
        parsed: true,
        ..RecordAnalysis::default()
    };

    match compare_flatten_only(record, &ast, engine) {
        Ok(change) => analysis.flatten_only = change,
        Err(_) => analysis.transform_failures += 1,
    }

    match compare_full_delta(record, &ast, engine) {
        Ok(change) => analysis.full_delta = change,
        Err(_) => analysis.transform_failures += 1,
    }

    analysis
}

fn compare_flatten_only(
    record: &data::FormulaRecord,
    ast: &Ast,
    engine: &texform::Engine,
) -> Result<Option<ChangeRecord>, texform::Error> {
    let before = serialize(ast);
    let mut after_ast = ast.clone();
    let report = engine
        .transform_ast_with(&mut after_ast, &flatten_only_config())?
        .flatten_groups;
    let after = serialize(&after_ast);
    if before != after {
        let impact = levenshtein(&before, &after);
        return Ok(Some(ChangeRecord {
            formula_id: record.formula_id.clone(),
            source: record.formula.clone(),
            before,
            after,
            impact,
            formula_len: record.formula.len(),
            report,
        }));
    }
    Ok(None)
}

fn compare_full_delta(
    record: &data::FormulaRecord,
    ast: &Ast,
    engine: &texform::Engine,
) -> Result<Option<ChangeRecord>, texform::Error> {
    let mut other_ast = ast.clone();
    engine.transform_ast_with(&mut other_ast, &other_phases_config(engine))?;
    let before = serialize(&other_ast);

    let mut full_ast = ast.clone();
    let report = engine.transform_ast(&mut full_ast)?.flatten_groups;
    let after = serialize(&full_ast);

    if before != after {
        let impact = levenshtein(&before, &after);
        return Ok(Some(ChangeRecord {
            formula_id: record.formula_id.clone(),
            source: record.formula.clone(),
            before,
            after,
            impact,
            formula_len: record.formula.len(),
            report,
        }));
    }
    Ok(None)
}

fn record_change(
    stats: &mut ComparisonStats,
    comparison: Comparison,
    dataset: &str,
    change: &ChangeRecord,
    sample_limit: usize,
) {
    stats.changed += 1;
    stats.removed_empty += change.report.removed_empty;
    stats.replaced_single_child += change.report.replaced_single_child;
    stats.inlined_multi_child += change.report.inlined_multi_child;
    stats.unwrapped_slot += change.report.unwrapped_slot;
    stats.samples.push(
        Sample {
            impact: change.impact,
            formula_len: change.formula_len,
            dataset: dataset.to_string(),
            formula_id: format!("{}:{}", comparison.label(), change.formula_id),
            source: truncate(&change.source),
            before: truncate(&change.before),
            after: truncate(&change.after),
            removed_empty: change.report.removed_empty,
            replaced_single_child: change.report.replaced_single_child,
            inlined_multi_child: change.report.inlined_multi_child,
            unwrapped_slot: change.report.unwrapped_slot,
        },
        sample_limit,
    );
}

fn dataset_output(slug: &str, stats: &DatasetStats) -> DatasetOutput {
    DatasetOutput {
        dataset: slug.to_string(),
        records: stats.records,
        parsed: stats.parsed,
        parse_failed: stats.parse_failed,
        transform_failed: stats.transform_failed,
        comparisons: vec![
            comparison_output(
                Comparison::NoTransformVsFlattenOnly,
                stats.records,
                stats.parsed,
                &stats.flatten_only,
            ),
            comparison_output(
                Comparison::OtherPhasesVsFull,
                stats.records,
                stats.parsed,
                &stats.full_delta,
            ),
        ],
    }
}

fn comparison_output(
    comparison: Comparison,
    records: usize,
    parsed: usize,
    stats: &ComparisonStats,
) -> ComparisonOutput {
    ComparisonOutput {
        comparison: comparison.label(),
        changed: stats.changed,
        records_pct: pct(stats.changed, records),
        parsed_pct: pct(stats.changed, parsed),
        removed_empty: stats.removed_empty,
        replaced_single_child: stats.replaced_single_child,
        inlined_multi_child: stats.inlined_multi_child,
        unwrapped_slot: stats.unwrapped_slot,
    }
}

fn dataset_samples_output(slug: &str, stats: &DatasetStats) -> DatasetSamplesOutput {
    DatasetSamplesOutput {
        dataset: slug.to_string(),
        comparisons: vec![
            comparison_samples_output(Comparison::NoTransformVsFlattenOnly, &stats.flatten_only),
            comparison_samples_output(Comparison::OtherPhasesVsFull, &stats.full_delta),
        ],
    }
}

fn comparison_samples_output(
    comparison: Comparison,
    stats: &ComparisonStats,
) -> ComparisonSamplesOutput {
    ComparisonSamplesOutput {
        comparison: comparison.label(),
        samples: stats.samples.sorted(),
    }
}

fn print_dataset(slug: &str, stats: &DatasetStats) {
    println!("\n== {slug} ==");
    println!(
        "records\t{}\nparsed\t{}\nparse_failed\t{}\ntransform_failed\t{}",
        stats.records, stats.parsed, stats.parse_failed, stats.transform_failed
    );
    print_comparison(
        Comparison::NoTransformVsFlattenOnly,
        stats.records,
        stats.parsed,
        &stats.flatten_only,
    );
    print_comparison(
        Comparison::OtherPhasesVsFull,
        stats.records,
        stats.parsed,
        &stats.full_delta,
    );
}

fn data_file_status_label(status: data::DataFileStatus) -> &'static str {
    match status {
        data::DataFileStatus::Ready => "ready",
        data::DataFileStatus::Missing => "missing",
        data::DataFileStatus::LfsPointer => "lfs-pointer",
    }
}

fn print_comparison(
    comparison: Comparison,
    records: usize,
    parsed: usize,
    stats: &ComparisonStats,
) {
    let pct_records = pct(stats.changed, records);
    let pct_parsed = pct(stats.changed, parsed);
    println!(
        "{}\tchanged={} ({:.2}% of records, {:.2}% of parsed)\tremoved_empty={}\treplaced_single_child={}\tinlined_multi_child={}\tunwrapped_slot={}",
        comparison.label(),
        stats.changed,
        pct_records,
        pct_parsed,
        stats.removed_empty,
        stats.replaced_single_child,
        stats.inlined_multi_child,
        stats.unwrapped_slot
    );
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

fn truncate(value: &str) -> String {
    const MAX: usize = 420;
    let mut out = value.chars().take(MAX).collect::<String>();
    if value.chars().count() > MAX {
        out.push_str("...");
    }
    out.replace('\n', "\\n")
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_ch) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right_chars.iter().enumerate() {
            let cost = usize::from(left_ch != *right_ch);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}
