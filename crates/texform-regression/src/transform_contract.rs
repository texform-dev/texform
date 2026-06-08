use crate::{config, data, output};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use texform_core::document::Document;
use texform_core::parse::{ParseConfig, ParseContext, ParseDiagnostic};
use texform_transform::{
    BuildConfig, ContractViolation, Profile, RewriteError, RuleTarget, RuleTargetKey,
    RuleTargetKind, TransformContext, TransformError, collect_eliminated_violations,
};

const SCHEMA_VERSION: u32 = 1;
const PROFILE_NAME: &str = "corpus";
const PARSE_REJECT_UNKNOWN: bool = false;
const PARSE_ABORT_ON_ERROR: bool = true;

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub datasets_yaml: PathBuf,
    pub results_root: PathBuf,
    pub datasets: Vec<String>,
    pub limit: Option<usize>,
    pub dry_run: bool,
    pub skip_commit_results: bool,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub summary: TransformContractSummary,
    pub unallowed_violations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformContractSummary {
    pub schema_version: u32,
    pub metadata: SummaryMetadata,
    pub checked_formulas: usize,
    pub parse_errors: usize,
    pub transform_errors: usize,
    pub contract_errors: usize,
    pub violating_formulas: usize,
    pub violating_formulas_pct: f64,
    pub violations: usize,
    pub configured_exceptions: usize,
    pub matched_exceptions: usize,
    pub unmatched_exceptions: usize,
    pub allowed_exceptions: usize,
    pub unexpected_violations: usize,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetadata {
    pub profile: String,
    pub datasets: Vec<String>,
    pub limit: Option<usize>,
    pub provenance: Provenance,
    #[serde(skip)]
    pub elapsed_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRunSummary {
    pub slug: String,
    pub file: String,
    pub formulas: usize,
    pub parse_errors: usize,
    pub transform_errors: usize,
    pub contract_errors: usize,
    pub violating_formulas: usize,
    pub violations: usize,
    #[serde(skip)]
    pub elapsed_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub texform_commit_short: String,
    pub texform_dirty: bool,
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExceptionsFile {
    #[serde(default)]
    exceptions: Vec<ContractException>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContractException {
    dataset: String,
    formula_id: String,
    #[serde(default = "default_occurrence")]
    occurrence: usize,
    target_kind: String,
    target: String,
    #[serde(default)]
    node_name: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExceptionKey {
    dataset: String,
    formula_id: String,
    occurrence: usize,
    target_kind: String,
    target: String,
    node_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ViolationDetail {
    dataset: String,
    formula_id: String,
    occurrence: usize,
    target_kind: String,
    target: String,
    node_name: Option<String>,
    rule_keys: Vec<String>,
    allowed: bool,
    exception_reason: Option<String>,
    formula: String,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorDetail {
    dataset: String,
    formula_id: String,
    stage: &'static str,
    error: String,
    diagnostic_count: usize,
    diagnostics: Vec<ParseDiagnostic>,
    formula: String,
}

#[derive(Debug)]
struct FormulaOutcome {
    parse_error: Option<ParseFailure>,
    transform_error: Option<String>,
    violations: Vec<ViolationRecord>,
}

#[derive(Debug)]
struct ParseFailure {
    error: String,
    diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug)]
struct ViolationRecord {
    occurrence: usize,
    target_kind: String,
    target: String,
    node_name: Option<String>,
    rule_keys: Vec<String>,
}

#[derive(Default)]
struct RunAccumulator {
    total_formulas: usize,
    parse_errors: usize,
    transform_errors: usize,
    contract_errors: usize,
    violating_formulas: usize,
    violations: usize,
    allowed_violations: usize,
    unallowed_violations: usize,
}

struct DetailWriter {
    violations: std::io::BufWriter<std::fs::File>,
    errors: std::io::BufWriter<std::fs::File>,
}

impl DetailWriter {
    fn new(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(path)?;
        Ok(Self {
            violations: std::io::BufWriter::new(std::fs::File::create(
                path.join("violations.jsonl"),
            )?),
            errors: std::io::BufWriter::new(std::fs::File::create(path.join("errors.jsonl"))?),
        })
    }

    fn write_violation(&mut self, detail: &ViolationDetail) -> Result<(), std::io::Error> {
        serde_json::to_writer(&mut self.violations, detail)?;
        self.violations.write_all(b"\n")
    }

    fn write_error(&mut self, detail: &ErrorDetail) -> Result<(), std::io::Error> {
        serde_json::to_writer(&mut self.errors, detail)?;
        self.errors.write_all(b"\n")
    }

    fn flush(mut self) -> Result<(), std::io::Error> {
        self.violations.flush()?;
        self.errors.flush()
    }
}

pub fn run(config: RunConfig) -> Result<RunOutcome, Box<dyn std::error::Error>> {
    let datasets_config = config::DatasetsConfig::load_from_yaml(&config.datasets_yaml)?;
    validate_requested_datasets(&datasets_config, &config.datasets)?;
    let selected = datasets_config.filter_by_slugs(&config.datasets);
    if selected.is_empty() {
        eprintln!(
            "No datasets selected. Available: {:?}",
            datasets_config
                .datasets
                .iter()
                .map(|dataset| &dataset.slug)
                .collect::<Vec<_>>()
        );
        return Err("No datasets selected for transform_contract".into());
    }

    let exceptions_path = config
        .datasets_yaml
        .parent()
        .expect("datasets yaml should have a parent directory")
        .join("contract_exceptions.yaml");
    let exceptions = load_exceptions(&exceptions_path)?;
    let mut matched_exceptions = BTreeSet::new();

    let parse_ctx = ParseContext::shared();
    let transform_ctx =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), parse_ctx)?;
    let attribution = build_rule_attribution(&transform_ctx);
    ensure_unique_eliminated_owners(&attribution)?;
    let commit_info = output::git_commit_info();
    let commit_results_root = config
        .results_root
        .join("commits")
        .join(commit_info.commit_dir_name());
    let mut detail_writer = if !config.dry_run && !config.skip_commit_results {
        Some(DetailWriter::new(&commit_results_root)?)
    } else {
        None
    };

    let run_start = Instant::now();
    let mut run_accumulator = RunAccumulator::default();
    let mut dataset_summaries = Vec::new();

    for entry in selected {
        let data_path = config::resolve_dataset_file(&config.datasets_yaml, &entry);
        match data::check_data_file(&data_path) {
            data::DataFileStatus::Missing => {
                eprintln!(
                    "[{}] data file missing (run `git lfs pull` to fetch), skipping",
                    entry.slug
                );
                continue;
            }
            data::DataFileStatus::LfsPointer => {
                eprintln!(
                    "[{}] LFS pointer not resolved (run `git lfs pull` to fetch), skipping",
                    entry.slug
                );
                continue;
            }
            data::DataFileStatus::Ready => {}
        }

        eprintln!(
            "[{}] Reading formulas from {}...",
            entry.slug,
            data_path.display()
        );

        let dataset_start = Instant::now();
        let before = run_accumulator.snapshot();
        let records_read =
            data::read_formula_record_batches(&data_path, 0, config.limit, |records| {
                let outcomes = run_batch(&records, parse_ctx, &transform_ctx, &attribution);
                collect_batch(
                    &entry.slug,
                    &records,
                    &outcomes,
                    &exceptions,
                    &mut matched_exceptions,
                    &mut run_accumulator,
                    detail_writer.as_mut(),
                )?;
                drop(outcomes);
                trim_allocator();
                Ok(())
            })?;

        let elapsed = dataset_start.elapsed().as_secs_f64();
        let summary = run_accumulator.dataset_delta(&before, &entry.slug, &entry.file, elapsed);
        println!(
            "[{}] {} formulas in {:.1}s; {} violating formula(s), {} violation(s)",
            entry.slug, records_read, elapsed, summary.violating_formulas, summary.violations
        );
        dataset_summaries.push(summary);
    }

    if let Some(writer) = detail_writer {
        writer.flush()?;
    }

    if dataset_summaries.is_empty() {
        return Err("No dataset files were available to run transform_contract".into());
    }

    let elapsed_seconds = run_start.elapsed().as_secs_f64();
    let summary = finish_summary(
        &config,
        &commit_info,
        dataset_summaries,
        run_accumulator,
        &exceptions,
        &matched_exceptions,
        elapsed_seconds,
    )?;
    let unallowed_violations = summary.unexpected_violations;

    if !config.dry_run {
        write_json_pretty(&config.results_root.join("summary.json"), &summary)?;
    }

    Ok(RunOutcome {
        summary,
        unallowed_violations,
    })
}

fn validate_requested_datasets(
    datasets_config: &config::DatasetsConfig,
    requested: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if requested.is_empty() {
        return Ok(());
    }

    let available = datasets_config
        .datasets
        .iter()
        .map(|dataset| dataset.slug.as_str())
        .collect::<HashSet<_>>();
    let missing = requested
        .iter()
        .filter(|slug| !available.contains(slug.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    Err(format!(
        "Unknown dataset slug(s): {:?}. Available: {:?}",
        missing,
        datasets_config
            .datasets
            .iter()
            .map(|dataset| &dataset.slug)
            .collect::<Vec<_>>()
    )
    .into())
}

fn run_batch(
    records: &[data::FormulaRecord],
    parse_ctx: &ParseContext,
    transform_ctx: &TransformContext,
    attribution: &HashMap<String, Vec<String>>,
) -> Vec<FormulaOutcome> {
    records
        .par_iter()
        .map(|record| run_formula(record, parse_ctx, transform_ctx, attribution))
        .collect()
}

fn run_formula(
    record: &data::FormulaRecord,
    parse_ctx: &ParseContext,
    transform_ctx: &TransformContext,
    attribution: &HashMap<String, Vec<String>>,
) -> FormulaOutcome {
    let parse_output = parse_ctx.parse(&record.formula, &parse_config());
    let mut document = match parse_output.try_into_document() {
        Ok((document, _)) => document,
        Err(error) => {
            return FormulaOutcome {
                parse_error: Some(ParseFailure {
                    error: error.to_string(),
                    diagnostics: error.into_diagnostics(),
                }),
                transform_error: None,
                violations: Vec::new(),
            };
        }
    };

    let transform_result = transform_ctx.run(document.__texform_engine_ast_mut(), parse_ctx);
    match transform_result {
        Ok(_) => FormulaOutcome {
            parse_error: None,
            transform_error: None,
            violations: Vec::new(),
        },
        Err(error) if is_contract_error(&error) => {
            let violations = violation_records(
                collect_formula_violations(&mut document, parse_ctx, transform_ctx),
                attribution,
            );
            FormulaOutcome {
                parse_error: None,
                transform_error: None,
                violations,
            }
        }
        Err(error) => FormulaOutcome {
            parse_error: None,
            transform_error: Some(error.to_string()),
            violations: Vec::new(),
        },
    }
}

fn collect_formula_violations(
    document: &mut Document,
    parse_ctx: &ParseContext,
    transform_ctx: &TransformContext,
) -> Vec<ContractViolation> {
    collect_eliminated_violations(
        document.__texform_engine_ast_mut(),
        parse_ctx,
        transform_ctx.rewrite_plan().eliminated_forms(),
    )
}

fn is_contract_error(error: &TransformError) -> bool {
    matches!(
        error,
        TransformError::Rewrite(RewriteError::ContractViolation { .. })
    )
}

fn violation_record(
    violation: ContractViolation,
    occurrence: usize,
    attribution: &HashMap<String, Vec<String>>,
) -> ViolationRecord {
    let target_key = target_key_string(violation.target);
    ViolationRecord {
        occurrence,
        target_kind: violation.target.kind_label().to_string(),
        target: violation.target.name.to_string(),
        node_name: violation.node_name,
        rule_keys: attribution
            .get(&target_key)
            .cloned()
            .unwrap_or_else(|| vec!["unattributed".to_string()]),
    }
}

fn violation_records(
    violations: Vec<ContractViolation>,
    attribution: &HashMap<String, Vec<String>>,
) -> Vec<ViolationRecord> {
    let mut occurrences: HashMap<(String, String, Option<String>), usize> = HashMap::new();
    violations
        .into_iter()
        .map(|violation| {
            let key = (
                violation.target.kind_label().to_string(),
                violation.target.name.to_string(),
                violation.node_name.clone(),
            );
            let occurrence = occurrences.entry(key).or_default();
            *occurrence += 1;
            violation_record(violation, *occurrence, attribution)
        })
        .collect()
}

fn collect_batch(
    dataset: &str,
    records: &[data::FormulaRecord],
    outcomes: &[FormulaOutcome],
    exceptions: &[ContractException],
    matched_exceptions: &mut BTreeSet<usize>,
    accumulator: &mut RunAccumulator,
    mut detail_writer: Option<&mut DetailWriter>,
) -> Result<(), Box<dyn std::error::Error>> {
    debug_assert_eq!(records.len(), outcomes.len());

    let exceptions_by_key = exception_lookup(exceptions);

    for (record, outcome) in records.iter().zip(outcomes.iter()) {
        accumulator.total_formulas += 1;
        if let Some(parse_error) = &outcome.parse_error {
            accumulator.parse_errors += 1;
            if let Some(writer) = detail_writer.as_mut() {
                writer.write_error(&ErrorDetail {
                    dataset: dataset.to_string(),
                    formula_id: record.formula_id.clone(),
                    stage: "parse",
                    error: parse_error.error.clone(),
                    diagnostic_count: parse_error.diagnostics.len(),
                    diagnostics: parse_error.diagnostics.clone(),
                    formula: record.formula.clone(),
                })?;
            }
            continue;
        }

        if let Some(error) = &outcome.transform_error {
            accumulator.transform_errors += 1;
            if let Some(writer) = detail_writer.as_mut() {
                writer.write_error(&ErrorDetail {
                    dataset: dataset.to_string(),
                    formula_id: record.formula_id.clone(),
                    stage: "transform",
                    error: error.clone(),
                    diagnostic_count: 0,
                    diagnostics: Vec::new(),
                    formula: record.formula.clone(),
                })?;
            }
            continue;
        }

        if outcome.violations.is_empty() {
            continue;
        }

        accumulator.contract_errors += 1;
        accumulator.violating_formulas += 1;

        for violation in &outcome.violations {
            accumulator.violations += 1;
            let key = ExceptionKey {
                dataset: dataset.to_string(),
                formula_id: record.formula_id.clone(),
                occurrence: violation.occurrence,
                target_kind: violation.target_kind.clone(),
                target: violation.target.clone(),
                node_name: violation.node_name.clone(),
            };
            let matched = exceptions_by_key.get(&key).copied();
            let allowed = matched.is_some();
            if let Some(index) = matched {
                matched_exceptions.insert(index);
                accumulator.allowed_violations += 1;
            } else {
                accumulator.unallowed_violations += 1;
            }

            if let Some(writer) = detail_writer.as_mut() {
                writer.write_violation(&ViolationDetail {
                    dataset: dataset.to_string(),
                    formula_id: record.formula_id.clone(),
                    occurrence: violation.occurrence,
                    target_kind: violation.target_kind.clone(),
                    target: violation.target.clone(),
                    node_name: violation.node_name.clone(),
                    rule_keys: violation.rule_keys.clone(),
                    allowed,
                    exception_reason: matched.map(|index| exceptions[index].reason.clone()),
                    formula: record.formula.clone(),
                })?;
            }
        }
    }

    Ok(())
}

fn finish_summary(
    config: &RunConfig,
    commit_info: &output::GitCommitInfo,
    dataset_summaries: Vec<DatasetRunSummary>,
    accumulator: RunAccumulator,
    exceptions: &[ContractException],
    matched_exceptions: &BTreeSet<usize>,
    elapsed_seconds: f64,
) -> Result<TransformContractSummary, Box<dyn std::error::Error>> {
    let dataset_slugs = dataset_summaries
        .iter()
        .map(|dataset| dataset.slug.clone())
        .collect();
    let unexpected_violations = accumulator.unallowed_violations;

    Ok(TransformContractSummary {
        schema_version: SCHEMA_VERSION,
        metadata: SummaryMetadata {
            profile: PROFILE_NAME.to_string(),
            datasets: dataset_slugs,
            limit: config.limit,
            provenance: Provenance {
                texform_commit_short: commit_info.short_hash.clone(),
                texform_dirty: commit_info.dirty,
                config_hash: config_hash(config)?,
            },
            elapsed_seconds,
        },
        checked_formulas: accumulator.total_formulas,
        parse_errors: accumulator.parse_errors,
        transform_errors: accumulator.transform_errors,
        contract_errors: accumulator.contract_errors,
        violating_formulas: accumulator.violating_formulas,
        violating_formulas_pct: pct(accumulator.violating_formulas, accumulator.total_formulas),
        violations: accumulator.violations,
        configured_exceptions: exceptions.len(),
        matched_exceptions: matched_exceptions.len(),
        unmatched_exceptions: exceptions.len().saturating_sub(matched_exceptions.len()),
        allowed_exceptions: accumulator.allowed_violations,
        unexpected_violations,
        verdict: if unexpected_violations == 0 {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
    })
}

impl RunAccumulator {
    fn snapshot(&self) -> RunAccumulatorSnapshot {
        RunAccumulatorSnapshot {
            total_formulas: self.total_formulas,
            parse_errors: self.parse_errors,
            transform_errors: self.transform_errors,
            contract_errors: self.contract_errors,
            violating_formulas: self.violating_formulas,
            violations: self.violations,
        }
    }

    fn dataset_delta(
        &self,
        before: &RunAccumulatorSnapshot,
        slug: &str,
        file: &str,
        elapsed_seconds: f64,
    ) -> DatasetRunSummary {
        DatasetRunSummary {
            slug: slug.to_string(),
            file: file.to_string(),
            formulas: self.total_formulas - before.total_formulas,
            parse_errors: self.parse_errors - before.parse_errors,
            transform_errors: self.transform_errors - before.transform_errors,
            contract_errors: self.contract_errors - before.contract_errors,
            violating_formulas: self.violating_formulas - before.violating_formulas,
            violations: self.violations - before.violations,
            elapsed_seconds,
        }
    }
}

struct RunAccumulatorSnapshot {
    total_formulas: usize,
    parse_errors: usize,
    transform_errors: usize,
    contract_errors: usize,
    violating_formulas: usize,
    violations: usize,
}

fn parse_config() -> ParseConfig {
    ParseConfig {
        reject_unknown: PARSE_REJECT_UNKNOWN,
        abort_on_error: PARSE_ABORT_ON_ERROR,
        ..Default::default()
    }
}

fn build_rule_attribution(transform_ctx: &TransformContext) -> HashMap<String, Vec<String>> {
    let mut by_target: HashMap<String, Vec<String>> = HashMap::new();
    for rule in transform_ctx.rewrite_plan().rules() {
        let key = rule.meta().key.to_string();
        for target in rule.meta().consumes.eliminates {
            by_target
                .entry(target_key_string(target.key()))
                .or_default()
                .push(key.clone());
        }
    }
    for keys in by_target.values_mut() {
        keys.sort();
        keys.dedup();
    }
    by_target
}

fn ensure_unique_eliminated_owners(
    attribution: &HashMap<String, Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let duplicates = attribution
        .iter()
        .filter(|(_, rule_keys)| rule_keys.len() > 1)
        .map(|(target, rule_keys)| format!("{target}: {}", rule_keys.join(", ")))
        .collect::<Vec<_>>();

    if duplicates.is_empty() {
        return Ok(());
    }

    Err(format!(
        "Duplicate consumes.eliminates owner(s) in transform_contract profile:\n{}",
        duplicates.join("\n")
    )
    .into())
}

fn target_key_string(target: RuleTargetKey) -> String {
    format!("{}:{}", target.kind_label(), target.name)
}

fn exception_lookup(exceptions: &[ContractException]) -> HashMap<ExceptionKey, usize> {
    exceptions
        .iter()
        .enumerate()
        .map(|(index, exception)| (exception_key(exception), index))
        .collect()
}

fn exception_key(exception: &ContractException) -> ExceptionKey {
    ExceptionKey {
        dataset: exception.dataset.clone(),
        formula_id: exception.formula_id.clone(),
        occurrence: exception.occurrence,
        target_kind: exception.target_kind.clone(),
        target: exception.target.clone(),
        node_name: exception.node_name.clone(),
    }
}

fn default_occurrence() -> usize {
    1
}

fn load_exceptions(path: &Path) -> Result<Vec<ContractException>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let file: ExceptionsFile = serde_yaml::from_str(&content)?;
    for (index, exception) in file.exceptions.iter().enumerate() {
        if exception.reason.trim().is_empty() {
            return Err(format!(
                "exception #{index} in {} has an empty reason",
                path.display()
            )
            .into());
        }
        if !matches!(
            exception.target_kind.as_str(),
            "command" | "environment" | "character"
        ) {
            return Err(format!(
                "exception #{index} in {} has invalid target_kind {:?}",
                path.display(),
                exception.target_kind
            )
            .into());
        }
        if exception.occurrence == 0 {
            return Err(format!(
                "exception #{index} in {} has occurrence 0; occurrence is 1-based",
                path.display()
            )
            .into());
        }
    }
    let mut seen = HashSet::new();
    for (index, exception) in file.exceptions.iter().enumerate() {
        if !seen.insert(exception_key(exception)) {
            return Err(format!(
                "exception #{index} in {} duplicates an earlier exception key",
                path.display()
            )
            .into());
        }
    }
    Ok(file.exceptions)
}

fn hash_file_fnv1a64(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = fnv1a64_initial();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        fnv1a64_update(&mut hash, &buffer[..read]);
    }
    Ok(hash)
}

#[derive(Serialize)]
struct ConfigHashInput<'a> {
    datasets_yaml_hash_fnv1a64: String,
    contract_exceptions_hash_fnv1a64: String,
    datasets: &'a [String],
    limit: Option<usize>,
    profile: &'static str,
    parse_reject_unknown: bool,
    parse_abort_on_error: bool,
    schema_version: u32,
}

fn config_hash(config: &RunConfig) -> Result<String, Box<dyn std::error::Error>> {
    let exceptions_path = config
        .datasets_yaml
        .parent()
        .expect("datasets yaml should have a parent directory")
        .join("contract_exceptions.yaml");
    let mut datasets = config.datasets.clone();
    datasets.sort();
    let input = ConfigHashInput {
        datasets_yaml_hash_fnv1a64: hash_optional_file(&config.datasets_yaml)?,
        contract_exceptions_hash_fnv1a64: hash_optional_file(&exceptions_path)?,
        datasets: datasets.as_slice(),
        limit: config.limit,
        profile: PROFILE_NAME,
        parse_reject_unknown: PARSE_REJECT_UNKNOWN,
        parse_abort_on_error: PARSE_ABORT_ON_ERROR,
        schema_version: SCHEMA_VERSION,
    };
    let bytes = serde_json::to_vec(&input)?;
    Ok(format!("{:016x}", hash_bytes_fnv1a64(&bytes)))
}

fn hash_optional_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok("missing".to_string());
    }
    Ok(format!("{:016x}", hash_file_fnv1a64(path)?))
}

fn hash_bytes_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = fnv1a64_initial();
    fnv1a64_update(&mut hash, bytes);
    hash
}

fn fnv1a64_initial() -> u64 {
    0xcbf29ce484222325
}

fn fnv1a64_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

fn write_json_pretty<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(writer, value)?;
    Ok(())
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

#[allow(dead_code)]
fn _target_kind_for_match(kind: RuleTargetKind) -> &'static str {
    match kind {
        RuleTargetKind::Command => "command",
        RuleTargetKind::Environment => "environment",
        RuleTargetKind::Character => "character",
    }
}

#[allow(dead_code)]
fn _target_name_for_match(target: RuleTarget) -> &'static str {
    target.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exception_lookup_matches_full_formula_violation_key() {
        let exceptions = vec![ContractException {
            dataset: "sample".to_string(),
            formula_id: "abc123".to_string(),
            occurrence: 1,
            target_kind: "command".to_string(),
            target: "over".to_string(),
            node_name: Some("over".to_string()),
            reason: "Intentional fixture exception.".to_string(),
        }];
        let lookup = exception_lookup(&exceptions);

        assert!(lookup.contains_key(&ExceptionKey {
            dataset: "sample".to_string(),
            formula_id: "abc123".to_string(),
            occurrence: 1,
            target_kind: "command".to_string(),
            target: "over".to_string(),
            node_name: Some("over".to_string()),
        }));
    }

    #[test]
    fn config_hash_changes_when_limit_changes() {
        let base = RunConfig {
            datasets_yaml: PathBuf::from("regression/datasets.yaml"),
            results_root: PathBuf::from("regression/results/transform_contract"),
            datasets: vec!["sample".to_string()],
            limit: None,
            dry_run: false,
            skip_commit_results: false,
        };
        let mut limited = base.clone();
        limited.limit = Some(100);

        assert_ne!(config_hash(&base).unwrap(), config_hash(&limited).unwrap());
    }

    #[test]
    fn config_hash_ignores_dataset_argument_order() {
        let base = RunConfig {
            datasets_yaml: PathBuf::from("regression/datasets.yaml"),
            results_root: PathBuf::from("regression/results/transform_contract"),
            datasets: vec!["wikipedia".to_string(), "linxy".to_string()],
            limit: None,
            dry_run: false,
            skip_commit_results: false,
        };
        let mut reversed = base.clone();
        reversed.datasets.reverse();

        assert_eq!(config_hash(&base).unwrap(), config_hash(&reversed).unwrap());
    }

    #[test]
    fn duplicate_eliminated_owners_are_rejected() {
        let attribution = HashMap::from([(
            "command:eval".to_string(),
            vec!["physics/eval-a".to_string(), "physics/eval-b".to_string()],
        )]);

        let error = ensure_unique_eliminated_owners(&attribution).unwrap_err();
        assert!(error.to_string().contains("command:eval"));
        assert!(error.to_string().contains("physics/eval-a"));
        assert!(error.to_string().contains("physics/eval-b"));
    }

    #[test]
    fn single_eliminated_owner_is_accepted() {
        let attribution = HashMap::from([(
            "command:eval".to_string(),
            vec!["physics/eval-expand".to_string()],
        )]);

        ensure_unique_eliminated_owners(&attribution).unwrap();
    }

    #[test]
    fn corpus_profile_eliminated_owners_are_unique() {
        let parse_ctx = ParseContext::shared();
        let transform_ctx =
            TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), parse_ctx)
                .unwrap();
        let attribution = build_rule_attribution(&transform_ctx);

        ensure_unique_eliminated_owners(&attribution).unwrap();
    }

    #[test]
    fn violation_records_assign_one_based_occurrences_per_target() {
        let target = RuleTargetKey {
            kind: RuleTargetKind::Command,
            name: "buildrel",
        };
        let records = violation_records(
            vec![
                ContractViolation {
                    target,
                    node_name: Some("buildrel".to_string()),
                },
                ContractViolation {
                    target,
                    node_name: Some("buildrel".to_string()),
                },
            ],
            &HashMap::new(),
        );

        assert_eq!(records[0].occurrence, 1);
        assert_eq!(records[1].occurrence, 2);
    }

    #[test]
    fn validate_requested_datasets_rejects_unknown_slug() {
        let datasets_config = config::DatasetsConfig {
            datasets: vec![config::DatasetEntry {
                slug: "linxy".to_string(),
                file: "data/linxy.parquet".to_string(),
            }],
        };
        let requested = vec!["linxy".to_string(), "typo".to_string()];

        let error = validate_requested_datasets(&datasets_config, &requested).unwrap_err();

        assert!(error.to_string().contains("typo"));
    }
}
