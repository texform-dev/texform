use crate::data::FormulaRecord;
use crate::runner::FormulaResults;
use crate::stats::{self, ModeStats};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime};
use texform_core::parse::ParseDiagnostic;

const HISTOGRAM_BUCKET_US: u128 = 10;
const HISTOGRAM_BUCKET_COUNT: usize = 100_001;

struct FixedPrecisionPrettyFormatter<'a> {
    inner: serde_json::ser::PrettyFormatter<'a>,
}

impl<'a> FixedPrecisionPrettyFormatter<'a> {
    fn new() -> Self {
        Self {
            inner: serde_json::ser::PrettyFormatter::with_indent(b"  "),
        }
    }
}

impl serde_json::ser::Formatter for FixedPrecisionPrettyFormatter<'_> {
    fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        write!(writer, "{value:.2}")
    }

    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        write!(writer, "{value:.2}")
    }

    fn begin_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_array(writer)
    }

    fn end_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_array(writer)
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_array_value(writer, first)
    }

    fn end_array_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_array_value(writer)
    }

    fn begin_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_object(writer)
    }

    fn end_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_object(writer)
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_object_key(writer, first)
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_object_value(writer)
    }

    fn end_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_object_value(writer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub dataset: String,
    pub dataset_row_count: usize,
    pub total_tasks: usize,
    pub completed: usize,
    #[serde(default)]
    pub max_formula_len: Option<usize>,
    #[serde(default)]
    pub length_filtered_count: usize,
    #[serde(default)]
    pub length_filtered_rate_pct: f64,
    pub strict: ModeStats,
    pub nonstrict: ModeStats,
    #[serde(skip, default)]
    strict_duration_buckets: Vec<u32>,
    #[serde(skip, default)]
    strict_duration_ms_sum: f64,
    #[serde(skip, default)]
    strict_count: usize,
    #[serde(skip, default)]
    strict_failed: usize,
    #[serde(skip, default)]
    nonstrict_duration_buckets: Vec<u32>,
    #[serde(skip, default)]
    nonstrict_duration_ms_sum: f64,
    #[serde(skip, default)]
    nonstrict_count: usize,
    #[serde(skip, default)]
    nonstrict_failed: usize,
}

#[derive(Debug, Default)]
pub struct SummaryAccumulator {
    max_formula_len: Option<usize>,
    dataset_row_count: usize,
    length_filtered_count: usize,
    total_tasks: usize,
    strict_duration_buckets: Vec<u32>,
    strict_duration_ms_sum: f64,
    strict_count: usize,
    strict_failed: usize,
    nonstrict_duration_buckets: Vec<u32>,
    nonstrict_duration_ms_sum: f64,
    nonstrict_count: usize,
    nonstrict_failed: usize,
    strict_max: Option<(Duration, String)>,
    nonstrict_max: Option<(Duration, String)>,
}

impl SummaryAccumulator {
    pub fn new(max_formula_len: Option<usize>) -> Self {
        Self {
            max_formula_len,
            ..Self::default()
        }
    }

    pub fn append_filtered(&mut self, count: usize) {
        self.dataset_row_count += count;
        self.length_filtered_count += count;
    }

    pub fn append(&mut self, records: &[FormulaRecord], results: &[FormulaResults]) {
        debug_assert_eq!(records.len(), results.len());

        self.dataset_row_count += records.len();
        self.total_tasks += records.len();
        ensure_histogram(&mut self.strict_duration_buckets);
        ensure_histogram(&mut self.nonstrict_duration_buckets);

        for (record, result) in records.iter().zip(results.iter()) {
            push_duration(&mut self.strict_duration_buckets, result.strict.duration);
            self.strict_duration_ms_sum += result.strict.duration.as_secs_f64() * 1_000.0;
            self.strict_count += 1;
            if !result.strict.ok {
                self.strict_failed += 1;
            }

            push_duration(
                &mut self.nonstrict_duration_buckets,
                result.nonstrict.duration,
            );
            self.nonstrict_duration_ms_sum += result.nonstrict.duration.as_secs_f64() * 1_000.0;
            self.nonstrict_count += 1;
            if !result.nonstrict.ok {
                self.nonstrict_failed += 1;
            }

            if self
                .strict_max
                .as_ref()
                .is_none_or(|(duration, _)| result.strict.duration > *duration)
            {
                self.strict_max = Some((result.strict.duration, record.formula_id.clone()));
            }
            if self
                .nonstrict_max
                .as_ref()
                .is_none_or(|(duration, _)| result.nonstrict.duration > *duration)
            {
                self.nonstrict_max = Some((result.nonstrict.duration, record.formula_id.clone()));
            }
        }
    }

    pub fn finish(self, slug: &str) -> Summary {
        let strict_max_duration = self.strict_max.as_ref().map(|(duration, _)| *duration);
        let strict_max_formula_id = self
            .strict_max
            .as_ref()
            .map(|(_, formula_id)| formula_id.clone());
        let mut strict = mode_stats_from_histogram(
            self.strict_count,
            self.strict_failed,
            self.strict_duration_ms_sum,
            &self.strict_duration_buckets,
            strict_max_duration,
        );
        strict.timing_ms.max_formula_id = strict_max_formula_id;

        let nonstrict_max_duration = self.nonstrict_max.as_ref().map(|(duration, _)| *duration);
        let nonstrict_max_formula_id = self
            .nonstrict_max
            .as_ref()
            .map(|(_, formula_id)| formula_id.clone());
        let mut nonstrict = mode_stats_from_histogram(
            self.nonstrict_count,
            self.nonstrict_failed,
            self.nonstrict_duration_ms_sum,
            &self.nonstrict_duration_buckets,
            nonstrict_max_duration,
        );
        nonstrict.timing_ms.max_formula_id = nonstrict_max_formula_id;

        Summary {
            dataset: slug.to_string(),
            dataset_row_count: self.dataset_row_count,
            total_tasks: self.total_tasks,
            completed: self.total_tasks,
            max_formula_len: self.max_formula_len,
            length_filtered_count: self.length_filtered_count,
            length_filtered_rate_pct: rate_pct(self.length_filtered_count, self.dataset_row_count),
            strict,
            nonstrict,
            strict_duration_buckets: self.strict_duration_buckets,
            strict_duration_ms_sum: self.strict_duration_ms_sum,
            strict_count: self.strict_count,
            strict_failed: self.strict_failed,
            nonstrict_duration_buckets: self.nonstrict_duration_buckets,
            nonstrict_duration_ms_sum: self.nonstrict_duration_ms_sum,
            nonstrict_count: self.nonstrict_count,
            nonstrict_failed: self.nonstrict_failed,
        }
    }
}

fn rate_pct(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64 * 100.0
    }
}

fn ensure_histogram(buckets: &mut Vec<u32>) {
    if buckets.is_empty() {
        buckets.resize(HISTOGRAM_BUCKET_COUNT, 0);
    }
}

fn push_duration(buckets: &mut [u32], duration: Duration) {
    let bucket = (duration.as_micros() / HISTOGRAM_BUCKET_US)
        .min(HISTOGRAM_BUCKET_COUNT.saturating_sub(1) as u128) as usize;
    buckets[bucket] = buckets[bucket].saturating_add(1);
}

fn mode_stats_from_histogram(
    total: usize,
    failed: usize,
    sum_ms: f64,
    buckets: &[u32],
    max_duration: Option<Duration>,
) -> ModeStats {
    let failed = failed.min(total);
    ModeStats {
        ok: total.saturating_sub(failed),
        failed,
        failure_rate_pct: if total == 0 {
            0.0
        } else {
            failed as f64 / total as f64 * 100.0
        },
        timing_ms: stats::TimingStats {
            mean: if total == 0 {
                0.0
            } else {
                sum_ms / total as f64
            },
            p50: histogram_percentile_ms(buckets, total, 50.0),
            p95: histogram_percentile_ms(buckets, total, 95.0),
            p99: histogram_percentile_ms(buckets, total, 99.0),
            max: max_duration
                .map(|duration| duration.as_secs_f64() * 1_000.0)
                .unwrap_or(0.0),
            max_formula_id: None,
        },
    }
}

fn histogram_percentile_ms(buckets: &[u32], total: usize, percentile: f64) -> f64 {
    if total == 0 {
        return 0.0;
    }

    let rank = ((percentile.clamp(0.0, 100.0) / 100.0) * total as f64).ceil() as usize;
    let mut seen = 0_usize;
    for (index, count) in buckets.iter().enumerate() {
        seen += *count as usize;
        if seen >= rank {
            return (index as f64 * HISTOGRAM_BUCKET_US as f64) / 1_000.0;
        }
    }
    ((buckets.len().saturating_sub(1)) as f64 * HISTOGRAM_BUCKET_US as f64) / 1_000.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallSummary {
    pub dataset_count: usize,
    pub dataset_row_count: usize,
    pub total_tasks: usize,
    #[serde(default)]
    pub max_formula_len: Option<usize>,
    #[serde(default)]
    pub length_filtered_count: usize,
    #[serde(default)]
    pub length_filtered_rate_pct: f64,
    pub strict: ModeStats,
    pub nonstrict: ModeStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredSummaryStatus {
    Missing,
    Match,
    Different,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct StableModeStats {
    ok: usize,
    failed: usize,
    failure_rate_pct: f64,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct StableSummary {
    dataset: String,
    dataset_row_count: usize,
    total_tasks: usize,
    completed: usize,
    #[serde(default)]
    max_formula_len: Option<usize>,
    #[serde(default)]
    length_filtered_count: usize,
    #[serde(default)]
    length_filtered_rate_pct: f64,
    strict: StableModeStats,
    nonstrict: StableModeStats,
}

#[derive(Serialize, Deserialize)]
struct StableOverallSummary {
    dataset_count: usize,
    #[serde(default)]
    dataset_row_count: usize,
    total_tasks: usize,
    #[serde(default)]
    max_formula_len: Option<usize>,
    #[serde(default)]
    length_filtered_count: usize,
    #[serde(default)]
    length_filtered_rate_pct: f64,
    strict: StableModeStats,
    nonstrict: StableModeStats,
}

#[derive(Serialize, Deserialize)]
struct StableRunSummary {
    datasets: Vec<StableSummary>,
    overall: StableOverallSummary,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    commit_hash: String,
    commit_full: String,
    dataset: String,
    dataset_row_count: usize,
    timestamp: String,
}

pub struct GitCommitInfo {
    pub short_hash: String,
    pub full_hash: String,
    pub date: String,
    pub dirty: bool,
}

impl GitCommitInfo {
    pub fn commit_dir_name(&self) -> String {
        if self.dirty {
            format!("{}-dirty", self.full_hash)
        } else {
            self.full_hash.clone()
        }
    }
}

pub struct CommitResultWriter {
    dir: std::path::PathBuf,
    dataset: String,
    errors: std::io::BufWriter<std::fs::File>,
}

#[derive(Serialize)]
struct ErrorEntry {
    dataset: String,
    formula_id: String,
    formula: String,
    mode: &'static str,
    strict: bool,
    diagnostic_count: usize,
    diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug)]
pub struct CommitBaseline {
    pub commit_hash: String,
    summaries: Vec<Summary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeanRegressionWarning {
    pub baseline_commit_hash: String,
    pub mode: &'static str,
    pub baseline_mean_ms: f64,
    pub current_mean_ms: f64,
}

impl MeanRegressionWarning {
    pub fn ratio_pct(&self) -> f64 {
        self.current_mean_ms / self.baseline_mean_ms * 100.0
    }
}

fn stable_rate(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

impl From<&ModeStats> for StableModeStats {
    fn from(stats: &ModeStats) -> Self {
        Self {
            ok: stats.ok,
            failed: stats.failed,
            failure_rate_pct: stable_rate(stats.failure_rate_pct),
        }
    }
}

impl From<&Summary> for StableSummary {
    fn from(summary: &Summary) -> Self {
        Self {
            dataset: summary.dataset.clone(),
            dataset_row_count: summary.dataset_row_count,
            total_tasks: summary.total_tasks,
            completed: summary.completed,
            max_formula_len: summary.max_formula_len,
            length_filtered_count: summary.length_filtered_count,
            length_filtered_rate_pct: stable_rate(summary.length_filtered_rate_pct),
            strict: StableModeStats::from(&summary.strict),
            nonstrict: StableModeStats::from(&summary.nonstrict),
        }
    }
}

impl From<&OverallSummary> for StableOverallSummary {
    fn from(overall: &OverallSummary) -> Self {
        Self {
            dataset_count: overall.dataset_count,
            dataset_row_count: overall.dataset_row_count,
            total_tasks: overall.total_tasks,
            max_formula_len: overall.max_formula_len,
            length_filtered_count: overall.length_filtered_count,
            length_filtered_rate_pct: stable_rate(overall.length_filtered_rate_pct),
            strict: StableModeStats::from(&overall.strict),
            nonstrict: StableModeStats::from(&overall.nonstrict),
        }
    }
}

impl StableRunSummary {
    fn from_parts(summaries: &[Summary], overall: &OverallSummary) -> Self {
        Self {
            datasets: summaries.iter().map(StableSummary::from).collect(),
            overall: StableOverallSummary::from(overall),
        }
    }
}

pub fn build_summary(slug: &str, records: &[FormulaRecord], results: &[FormulaResults]) -> Summary {
    let mut accumulator = SummaryAccumulator::new(None);
    accumulator.append(records, results);
    accumulator.finish(slug)
}

pub fn build_overall(summaries: &[Summary]) -> OverallSummary {
    let total_tasks = summaries.iter().map(|summary| summary.total_tasks).sum();
    let dataset_row_count = summaries
        .iter()
        .map(|summary| summary.dataset_row_count)
        .sum();
    let length_filtered_count = summaries
        .iter()
        .map(|summary| summary.length_filtered_count)
        .sum();
    let max_formula_len = summaries.iter().find_map(|summary| summary.max_formula_len);
    let mut strict_duration_buckets = vec![0_u32; HISTOGRAM_BUCKET_COUNT];
    let mut nonstrict_duration_buckets = vec![0_u32; HISTOGRAM_BUCKET_COUNT];
    let strict_count = summaries.iter().map(|summary| summary.strict_count).sum();
    let nonstrict_count = summaries
        .iter()
        .map(|summary| summary.nonstrict_count)
        .sum();
    let strict_failed = summaries.iter().map(|summary| summary.strict_failed).sum();
    let nonstrict_failed = summaries
        .iter()
        .map(|summary| summary.nonstrict_failed)
        .sum();
    let strict_duration_ms_sum = summaries
        .iter()
        .map(|summary| summary.strict_duration_ms_sum)
        .sum();
    let nonstrict_duration_ms_sum = summaries
        .iter()
        .map(|summary| summary.nonstrict_duration_ms_sum)
        .sum();
    let strict_max = summaries
        .iter()
        .map(|summary| summary.strict.timing_ms.max)
        .fold(0.0, f64::max);
    let nonstrict_max = summaries
        .iter()
        .map(|summary| summary.nonstrict.timing_ms.max)
        .fold(0.0, f64::max);

    for summary in summaries {
        for (target, source) in strict_duration_buckets
            .iter_mut()
            .zip(summary.strict_duration_buckets.iter())
        {
            *target = target.saturating_add(*source);
        }
        for (target, source) in nonstrict_duration_buckets
            .iter_mut()
            .zip(summary.nonstrict_duration_buckets.iter())
        {
            *target = target.saturating_add(*source);
        }
    }

    OverallSummary {
        dataset_count: summaries.len(),
        dataset_row_count,
        total_tasks,
        max_formula_len,
        length_filtered_count,
        length_filtered_rate_pct: rate_pct(length_filtered_count, dataset_row_count),
        strict: mode_stats_from_histogram(
            strict_count,
            strict_failed,
            strict_duration_ms_sum,
            &strict_duration_buckets,
            Some(Duration::from_secs_f64(strict_max / 1_000.0)),
        ),
        nonstrict: mode_stats_from_histogram(
            nonstrict_count,
            nonstrict_failed,
            nonstrict_duration_ms_sum,
            &nonstrict_duration_buckets,
            Some(Duration::from_secs_f64(nonstrict_max / 1_000.0)),
        ),
    }
}

pub fn stored_summary_status(
    results_root: &Path,
    slug: &str,
    summary: &Summary,
) -> Result<StoredSummaryStatus, Box<dyn std::error::Error>> {
    let path = results_root.join("summary.json");
    if !path.exists() {
        return Ok(StoredSummaryStatus::Missing);
    }

    let existing: StableRunSummary = serde_json::from_reader(std::fs::File::open(&path)?)?;
    let Some(existing_summary) = existing
        .datasets
        .iter()
        .find(|stored| stored.dataset == slug)
    else {
        return Ok(StoredSummaryStatus::Missing);
    };
    let current = StableSummary::from(summary);

    if *existing_summary == current {
        Ok(StoredSummaryStatus::Match)
    } else {
        Ok(StoredSummaryStatus::Different)
    }
}

pub fn summaries_need_refresh(
    results_root: &Path,
    summaries: &[Summary],
) -> Result<bool, Box<dyn std::error::Error>> {
    for summary in summaries {
        if stored_summary_status(results_root, &summary.dataset, summary)?
            != StoredSummaryStatus::Match
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn write_run_summary(
    results_root: &Path,
    summaries: &[Summary],
    overall: &OverallSummary,
) -> Result<(), Box<dyn std::error::Error>> {
    write_json_file(
        &results_root.join("summary.json"),
        &StableRunSummary::from_parts(summaries, overall),
    )
}

pub fn start_commit_results(
    commits_root: &Path,
    slug: &str,
    commit: &GitCommitInfo,
) -> Result<CommitResultWriter, Box<dyn std::error::Error>> {
    let dir = commits_root.join(commit.commit_dir_name()).join(slug);
    std::fs::create_dir_all(&dir)?;
    let errors = std::io::BufWriter::new(std::fs::File::create(dir.join("errors.jsonl"))?);
    Ok(CommitResultWriter {
        dir,
        dataset: slug.to_string(),
        errors,
    })
}

impl CommitResultWriter {
    pub fn write_batch_errors(
        &mut self,
        records: &[FormulaRecord],
        results: &[FormulaResults],
    ) -> Result<(), Box<dyn std::error::Error>> {
        append_error_entries(&mut self.errors, &self.dataset, records, results)?;
        Ok(())
    }

    pub fn finish(
        mut self,
        summary: &Summary,
        commit: &GitCommitInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.errors.flush()?;
        write_json_file(&self.dir.join("summary.json"), summary)?;

        let manifest = Manifest {
            commit_hash: commit.short_hash.clone(),
            commit_full: commit.full_hash.clone(),
            dataset: summary.dataset.clone(),
            dataset_row_count: summary.dataset_row_count,
            timestamp: now_timestamp(),
        };
        std::fs::write(
            self.dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        Ok(())
    }
}

/// Returns the short hash, full hash, and commit date for HEAD.
/// The date is formatted as `yyyy-mm-dd`.
pub fn git_commit_info() -> GitCommitInfo {
    let repo_root = crate::config::default_repo_root();

    let full = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let short = if full.len() >= 8 {
        full[..8].to_string()
    } else {
        full.clone()
    };

    let date = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .args(["log", "-1", "--format=%ci", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .and_then(|ci| ci.split_whitespace().next().map(|s| s.to_string()))
        .unwrap_or_else(|| "0000-00-00".to_string());
    let dirty = git_worktree_dirty(&repo_root);

    GitCommitInfo {
        short_hash: short,
        full_hash: full,
        date,
        dirty,
    }
}

fn git_worktree_dirty(repo_root: &Path) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .is_some_and(|output| !output.stdout.is_empty())
}

pub fn latest_commit_baseline(
    history_root: &Path,
) -> Result<Option<CommitBaseline>, Box<dyn std::error::Error>> {
    let Some((commit_hash, commit_dir)) = latest_commit_dir(history_root)? else {
        return Ok(None);
    };

    let summaries = load_commit_summaries(&commit_dir)?;
    if summaries.is_empty() {
        return Ok(None);
    }

    Ok(Some(CommitBaseline {
        commit_hash,
        summaries,
    }))
}

pub fn detect_mean_regressions(
    current_summaries: &[Summary],
    baseline: &CommitBaseline,
) -> Vec<MeanRegressionWarning> {
    let current_by_dataset: std::collections::HashMap<&str, &Summary> = current_summaries
        .iter()
        .map(|summary| (summary.dataset.as_str(), summary))
        .collect();

    let mut matched_current = Vec::new();
    let mut matched_baseline = Vec::new();
    for baseline_summary in &baseline.summaries {
        let Some(current_summary) = current_by_dataset.get(baseline_summary.dataset.as_str())
        else {
            continue;
        };

        if current_summary.total_tasks != baseline_summary.total_tasks {
            continue;
        }

        matched_current.push(*current_summary);
        matched_baseline.push(baseline_summary);
    }

    if matched_current.is_empty() {
        return Vec::new();
    }

    let current_strict_mean =
        weighted_mean_ms(&matched_current, |summary| summary.strict.timing_ms.mean);
    let baseline_strict_mean =
        weighted_mean_ms(&matched_baseline, |summary| summary.strict.timing_ms.mean);
    let current_nonstrict_mean =
        weighted_mean_ms(&matched_current, |summary| summary.nonstrict.timing_ms.mean);
    let baseline_nonstrict_mean = weighted_mean_ms(&matched_baseline, |summary| {
        summary.nonstrict.timing_ms.mean
    });

    let mut warnings = Vec::new();
    maybe_push_regression(
        &mut warnings,
        &baseline.commit_hash,
        "strict",
        baseline_strict_mean,
        current_strict_mean,
    );
    maybe_push_regression(
        &mut warnings,
        &baseline.commit_hash,
        "nonstrict",
        baseline_nonstrict_mean,
        current_nonstrict_mean,
    );
    warnings
}

fn now_timestamp() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    format!("{}ns-since-epoch", duration.as_nanos())
}

fn latest_commit_dir(
    history_root: &Path,
) -> Result<Option<(String, std::path::PathBuf)>, Box<dyn std::error::Error>> {
    if !history_root.exists() {
        return Ok(None);
    }

    let mut latest: Option<(u128, String, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(history_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let path = entry.path();
        let timestamp = latest_snapshot_marker(&path)?;
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let commit_hash = dir_name
            .get(11..)
            .filter(|_| looks_like_dated_snapshot_dir(&dir_name))
            .unwrap_or(&dir_name)
            .to_string();

        if latest
            .as_ref()
            .is_none_or(|(latest_ts, _, _)| timestamp > *latest_ts)
        {
            latest = Some((timestamp, commit_hash, path));
        }
    }

    Ok(latest.map(|(_, commit_hash, path)| (commit_hash, path)))
}

fn latest_snapshot_marker(path: &Path) -> Result<u128, Box<dyn std::error::Error>> {
    let metadata = std::fs::metadata(path)?;
    let mut latest = system_time_to_marker(metadata.created().or_else(|_| metadata.modified())?);

    if metadata.is_file() {
        if path.file_name().and_then(|name| name.to_str()) == Some("manifest.json")
            && let Some(marker) = read_manifest_marker(path)?
            && marker > latest
        {
            latest = marker;
        }

        return Ok(latest);
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let candidate = latest_snapshot_marker(&entry.path())?;
        if candidate > latest {
            latest = candidate;
        }
    }

    Ok(latest)
}

fn read_manifest_marker(path: &Path) -> Result<Option<u128>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let manifest: Manifest = serde_json::from_reader(reader)?;
    Ok(parse_timestamp_marker(&manifest.timestamp))
}

fn parse_timestamp_marker(timestamp: &str) -> Option<u128> {
    if let Some(raw) = timestamp.strip_suffix("ns-since-epoch") {
        return raw.parse().ok();
    }

    if let Some(raw) = timestamp.strip_suffix("s-since-epoch") {
        return raw
            .parse::<u128>()
            .ok()
            .map(|seconds| seconds * 1_000_000_000);
    }

    None
}

fn looks_like_dated_snapshot_dir(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 11
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b'-')
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn system_time_to_marker(timestamp: SystemTime) -> u128 {
    timestamp
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn load_commit_summaries(commit_dir: &Path) -> Result<Vec<Summary>, Box<dyn std::error::Error>> {
    let mut summaries = Vec::new();
    for entry in std::fs::read_dir(commit_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let summary_path = entry.path().join("summary.json");
        if !summary_path.exists() {
            continue;
        }

        let file = std::fs::File::open(summary_path)?;
        let reader = std::io::BufReader::new(file);
        summaries.push(serde_json::from_reader(reader)?);
    }
    Ok(summaries)
}

fn weighted_mean_ms<F>(summaries: &[&Summary], select_mean: F) -> f64
where
    F: Fn(&Summary) -> f64,
{
    let total_tasks: usize = summaries.iter().map(|summary| summary.total_tasks).sum();
    if total_tasks == 0 {
        return 0.0;
    }

    summaries
        .iter()
        .map(|summary| select_mean(summary) * summary.total_tasks as f64)
        .sum::<f64>()
        / total_tasks as f64
}

fn maybe_push_regression(
    warnings: &mut Vec<MeanRegressionWarning>,
    baseline_commit_hash: &str,
    mode: &'static str,
    baseline_mean_ms: f64,
    current_mean_ms: f64,
) {
    if baseline_mean_ms > 0.0 && current_mean_ms > baseline_mean_ms * 1.2 {
        warnings.push(MeanRegressionWarning {
            baseline_commit_hash: baseline_commit_hash.to_string(),
            mode,
            baseline_mean_ms,
            current_mean_ms,
        });
    }
}

pub fn write_json_file<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let formatter = FixedPrecisionPrettyFormatter::new();
    let mut serializer = serde_json::Serializer::with_formatter(writer, formatter);
    value.serialize(&mut serializer)?;
    Ok(())
}

/// Appends one JSON object per failed parse (strict and/or nonstrict) to `writer`.
fn append_error_entries<W: std::io::Write + ?Sized>(
    writer: &mut W,
    dataset: &str,
    records: &[FormulaRecord],
    results: &[FormulaResults],
) -> std::io::Result<()> {
    for (record, result) in records.iter().zip(results.iter()) {
        if !result.strict.ok {
            serde_json::to_writer(
                &mut *writer,
                &ErrorEntry {
                    dataset: dataset.to_string(),
                    formula_id: record.formula_id.clone(),
                    formula: record.formula.clone(),
                    mode: "strict",
                    strict: true,
                    diagnostic_count: result.strict.diagnostic_count,
                    diagnostics: result.strict.diagnostics.clone(),
                },
            )?;
            writer.write_all(b"\n")?;
        }

        if !result.nonstrict.ok {
            serde_json::to_writer(
                &mut *writer,
                &ErrorEntry {
                    dataset: dataset.to_string(),
                    formula_id: record.formula_id.clone(),
                    formula: record.formula.clone(),
                    mode: "nonstrict",
                    strict: false,
                    diagnostic_count: result.nonstrict.diagnostic_count,
                    diagnostics: result.nonstrict.diagnostics.clone(),
                },
            )?;
            writer.write_all(b"\n")?;
        }
    }
    Ok(())
}

/// Streams per-formula failure diagnostics to a single `errors.jsonl`, decoupled
/// from the per-commit snapshot tree.
///
/// Downstream consumers that only need the failure diagnostics (for example the
/// workspace studies analysis) can request this output via `--emit-errors`
/// without populating `commits/<hash>/`, keeping the results directory flat.
pub struct ErrorsEmitWriter {
    writer: std::io::BufWriter<std::fs::File>,
}

impl ErrorsEmitWriter {
    pub fn start(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        Ok(Self {
            writer: std::io::BufWriter::new(file),
        })
    }

    pub fn write_batch(
        &mut self,
        dataset: &str,
        records: &[FormulaRecord],
        results: &[FormulaResults],
    ) -> Result<(), Box<dyn std::error::Error>> {
        append_error_entries(&mut self.writer, dataset, records, results)?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FormulaRecord;
    use crate::runner::{FormulaResults, ParseResult};
    use std::ops::Deref;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime};
    use texform_core::parse::Span;

    #[test]
    fn build_summary_tracks_max_formula_ids() {
        let summary = build_summary("demo", &sample_records(), &sample_results());

        assert_eq!(
            summary.strict.timing_ms.max_formula_id.as_deref(),
            Some("beta")
        );
        assert_eq!(
            summary.nonstrict.timing_ms.max_formula_id.as_deref(),
            Some("beta")
        );
    }

    #[test]
    fn summary_accumulator_excludes_length_filtered_records_from_parse_rates() {
        let records = sample_records();
        let results = sample_results();
        let mut accumulator = SummaryAccumulator::new(Some(5));

        accumulator.append(&records[..1], &results[..1]);
        accumulator.append_filtered(1);
        let summary = accumulator.finish("demo");

        assert_eq!(summary.dataset_row_count, 2);
        assert_eq!(summary.total_tasks, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.max_formula_len, Some(5));
        assert_eq!(summary.length_filtered_count, 1);
        assert!((summary.length_filtered_rate_pct - 50.0).abs() < 1e-9);
        assert_eq!(summary.strict.ok, 1);
        assert_eq!(summary.strict.failed, 0);
        assert!((summary.strict.failure_rate_pct - 0.0).abs() < 1e-9);
    }

    #[test]
    fn build_overall_sums_length_filtered_counts() {
        let records = sample_records();
        let results = sample_results();
        let mut first = SummaryAccumulator::new(Some(5));
        first.append(&records[..1], &results[..1]);
        first.append_filtered(1);
        let first = first.finish("first");

        let mut second = SummaryAccumulator::new(Some(5));
        second.append(&records[..1], &results[..1]);
        let second = second.finish("second");

        let overall = build_overall(&[first, second]);

        assert_eq!(overall.dataset_row_count, 3);
        assert_eq!(overall.total_tasks, 2);
        assert_eq!(overall.max_formula_len, Some(5));
        assert_eq!(overall.length_filtered_count, 1);
        assert!((overall.length_filtered_rate_pct - 33.33333333333333).abs() < 1e-9);
    }

    #[test]
    fn write_run_summary_omits_unstable_timing_fields() {
        let dir = make_temp_dir("stable-run-summary");
        let summary = build_summary("demo", &sample_records(), &sample_results());
        let summaries = vec![summary];
        let overall = build_overall(&summaries);

        write_run_summary(&dir, &summaries, &overall).unwrap();

        let summary_json: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(dir.join("summary.json")).unwrap())
                .unwrap();
        assert!(summary_json.get("datasets").is_some());
        assert!(summary_json.get("overall").is_some());
        assert!(
            summary_json["datasets"][0]["strict"]
                .get("timing_ms")
                .is_none()
        );
        assert!(
            summary_json["datasets"][0]["strict"]
                .get("max_formula_id")
                .is_none()
        );
        assert_eq!(summary_json["datasets"][0]["length_filtered_count"], 0);
        assert_eq!(summary_json["overall"]["length_filtered_count"], 0);
        assert!(!dir.join("overall.json").exists());
    }

    #[test]
    fn stored_summary_status_reports_missing_when_file_absent() {
        let dir = make_temp_dir("summary-status-missing");
        let summary = build_summary("demo", &sample_records(), &sample_results());

        let status = stored_summary_status(&dir, "demo", &summary).unwrap();

        assert_eq!(status, StoredSummaryStatus::Missing);
    }

    #[test]
    fn stored_summary_status_reports_match_for_identical_summary() {
        let dir = make_temp_dir("summary-status-match");
        let summary = build_summary("demo", &sample_records(), &sample_results());
        let summaries = vec![summary.clone()];
        let overall = build_overall(&summaries);

        write_run_summary(&dir, &summaries, &overall).unwrap();

        let status = stored_summary_status(&dir, "demo", &summary).unwrap();

        assert_eq!(status, StoredSummaryStatus::Match);
    }

    #[test]
    fn stored_summary_status_uses_stable_failure_rate_precision() {
        let dir = make_temp_dir("summary-status-stable-rate");
        let mut records = sample_records();
        records.push(FormulaRecord {
            formula_id: "gamma".to_string(),
            formula: "z".to_string(),
        });
        let mut results = sample_results();
        results.push(FormulaResults {
            strict: ParseResult {
                duration: Duration::from_micros(20),
                ok: true,
                diagnostic_count: 0,
                diagnostics: Vec::new(),
            },
            nonstrict: ParseResult {
                duration: Duration::from_micros(18),
                ok: true,
                diagnostic_count: 0,
                diagnostics: Vec::new(),
            },
        });
        let summary = build_summary("demo", &records, &results);
        let summaries = vec![summary.clone()];
        let overall = build_overall(&summaries);

        write_run_summary(&dir, &summaries, &overall).unwrap();

        let status = stored_summary_status(&dir, "demo", &summary).unwrap();

        assert_eq!(status, StoredSummaryStatus::Match);
    }

    #[test]
    fn stored_summary_status_reports_different_for_changed_summary() {
        let dir = make_temp_dir("summary-status-different");
        let original = build_summary("demo", &sample_records(), &sample_results());
        let summaries = vec![original.clone()];
        let overall = build_overall(&summaries);
        write_run_summary(&dir, &summaries, &overall).unwrap();

        let mut changed = original.clone();
        changed.strict.ok = 2;
        changed.strict.failed = 0;
        changed.strict.failure_rate_pct = 0.0;

        let status = stored_summary_status(&dir, "demo", &changed).unwrap();

        assert_eq!(status, StoredSummaryStatus::Different);
    }

    #[test]
    fn summaries_need_refresh_returns_false_when_all_match() {
        let dir = make_temp_dir("summaries-refresh-all-match");
        let first = build_summary("demo-a", &sample_records(), &sample_results());
        let second = build_summary("demo-b", &sample_records(), &sample_results());
        let summaries = vec![first.clone(), second.clone()];
        let overall = build_overall(&summaries);

        write_run_summary(&dir, &summaries, &overall).unwrap();

        let needs_refresh = summaries_need_refresh(&dir, &[first, second]).unwrap();

        assert!(!needs_refresh);
    }

    #[test]
    fn summaries_need_refresh_returns_true_when_any_summary_is_missing_or_different() {
        let dir = make_temp_dir("summaries-refresh-needed");
        let matching = build_summary("demo-a", &sample_records(), &sample_results());
        let mut different = build_summary("demo-b", &sample_records(), &sample_results());
        let stored_summaries = vec![matching.clone()];
        let overall = build_overall(&stored_summaries);

        write_run_summary(&dir, &stored_summaries, &overall).unwrap();
        different.nonstrict.ok = 1;
        different.nonstrict.failed = 1;
        different.nonstrict.failure_rate_pct = 50.0;

        let needs_refresh = summaries_need_refresh(&dir, &[matching, different]).unwrap();

        assert!(needs_refresh);
    }

    #[test]
    fn errors_emit_writer_streams_failures_with_dataset_and_formula_id() {
        let dir = make_temp_dir("errors-emit");
        let records = sample_records();
        let results = sample_results();

        let mut writer = ErrorsEmitWriter::start(&dir.join("errors.jsonl")).unwrap();
        writer.write_batch("demo", &records, &results).unwrap();
        writer.finish().unwrap();

        let errors = std::fs::read_to_string(dir.join("errors.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = errors
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        // sample_results: only `beta` fails (strict); `alpha` is clean in both modes.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["dataset"], "demo");
        assert_eq!(rows[0]["formula_id"], "beta");
        assert_eq!(rows[0]["mode"], "strict");
        assert_eq!(rows[0]["diagnostic_count"], 2);
    }

    #[test]
    fn start_commit_results_suffixes_dirty_commit_directory() {
        let dir = make_temp_dir("dirty-commit-results");
        let commit = GitCommitInfo {
            short_hash: "abc12345".to_string(),
            full_hash: "abc12345full".to_string(),
            date: "2024-01-01".to_string(),
            dirty: true,
        };

        let writer = start_commit_results(&dir, "demo", &commit).unwrap();
        drop(writer);

        assert!(dir.join("abc12345full-dirty").join("demo").exists());
        assert!(!dir.join("abc12345full").exists());
    }

    #[test]
    fn latest_commit_baseline_prefers_most_recent_commit_directory() {
        let dir = make_temp_dir("latest-baseline");
        write_test_commit_summary(
            &dir,
            "older",
            sample_summary_with_means("demo", 10, 0.10, 0.20),
        );
        write_test_commit_summary(
            &dir,
            "newer",
            sample_summary_with_means("demo", 10, 0.11, 0.21),
        );

        let baseline = latest_commit_baseline(&dir).unwrap().unwrap();

        assert_eq!(baseline.commit_hash, "newer");
        assert_eq!(baseline.summaries.len(), 1);
        assert_eq!(baseline.summaries[0].dataset, "demo");
    }

    #[test]
    fn latest_commit_baseline_tracks_refreshed_existing_commit_directory() {
        let dir = make_temp_dir("refreshed-baseline");
        write_test_commit_summary(
            &dir,
            "older",
            sample_summary_with_means("demo", 10, 0.10, 0.20),
        );
        write_test_commit_summary(
            &dir,
            "newer",
            sample_summary_with_means("demo", 10, 0.11, 0.21),
        );

        let baseline = latest_commit_baseline(&dir).unwrap().unwrap();
        assert_eq!(baseline.commit_hash, "newer");

        write_test_commit_summary(
            &dir,
            "older",
            sample_summary_with_means("demo", 10, 0.12, 0.22),
        );

        let refreshed = latest_commit_baseline(&dir).unwrap().unwrap();
        assert_eq!(refreshed.commit_hash, "older");
    }

    #[test]
    fn detect_mean_regressions_warns_only_for_modes_over_threshold() {
        let current = vec![sample_summary_with_means("demo", 10, 0.13, 0.23)];
        let baseline = CommitBaseline {
            commit_hash: "prev12345".to_string(),
            summaries: vec![sample_summary_with_means("demo", 10, 0.10, 0.20)],
        };

        let warnings = detect_mean_regressions(&current, &baseline);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].mode, "strict");
        assert_eq!(warnings[0].baseline_commit_hash, "prev12345");
        assert!((warnings[0].baseline_mean_ms - 0.10).abs() < 1e-9);
        assert!((warnings[0].current_mean_ms - 0.13).abs() < 1e-9);
    }

    fn sample_records() -> Vec<FormulaRecord> {
        vec![
            FormulaRecord {
                formula_id: "alpha".to_string(),
                formula: "x^2 + y".to_string(),
            },
            FormulaRecord {
                formula_id: "beta".to_string(),
                formula: "\\foo".to_string(),
            },
        ]
    }

    fn sample_results() -> Vec<FormulaResults> {
        vec![
            FormulaResults {
                strict: ParseResult {
                    duration: Duration::from_micros(10),
                    ok: true,
                    diagnostic_count: 0,
                    diagnostics: Vec::new(),
                },
                nonstrict: ParseResult {
                    duration: Duration::from_micros(12),
                    ok: true,
                    diagnostic_count: 0,
                    diagnostics: Vec::new(),
                },
            },
            FormulaResults {
                strict: ParseResult {
                    duration: Duration::from_micros(30),
                    ok: false,
                    diagnostic_count: 2,
                    diagnostics: vec![ParseDiagnostic::new(
                        "invalid token",
                        Span { start: 0, end: 1 },
                        vec!["group".to_string()],
                        Some("\\foo".to_string()),
                        Vec::new(),
                    )],
                },
                nonstrict: ParseResult {
                    duration: Duration::from_micros(25),
                    ok: true,
                    diagnostic_count: 0,
                    diagnostics: Vec::new(),
                },
            },
        ]
    }

    fn sample_summary_with_means(
        dataset: &str,
        total_tasks: usize,
        strict_mean_ms: f64,
        nonstrict_mean_ms: f64,
    ) -> Summary {
        Summary {
            dataset: dataset.to_string(),
            dataset_row_count: total_tasks,
            total_tasks,
            completed: total_tasks,
            max_formula_len: None,
            length_filtered_count: 0,
            length_filtered_rate_pct: 0.0,
            strict: ModeStats {
                ok: total_tasks,
                failed: 0,
                failure_rate_pct: 0.0,
                timing_ms: stats::TimingStats {
                    mean: strict_mean_ms,
                    p50: strict_mean_ms,
                    p95: strict_mean_ms,
                    p99: strict_mean_ms,
                    max: strict_mean_ms,
                    max_formula_id: Some("alpha".to_string()),
                },
            },
            nonstrict: ModeStats {
                ok: total_tasks,
                failed: 0,
                failure_rate_pct: 0.0,
                timing_ms: stats::TimingStats {
                    mean: nonstrict_mean_ms,
                    p50: nonstrict_mean_ms,
                    p95: nonstrict_mean_ms,
                    p99: nonstrict_mean_ms,
                    max: nonstrict_mean_ms,
                    max_formula_id: Some("beta".to_string()),
                },
            },
            strict_duration_buckets: Vec::new(),
            strict_duration_ms_sum: strict_mean_ms * total_tasks as f64,
            strict_count: total_tasks,
            strict_failed: 0,
            nonstrict_duration_buckets: Vec::new(),
            nonstrict_duration_ms_sum: nonstrict_mean_ms * total_tasks as f64,
            nonstrict_count: total_tasks,
            nonstrict_failed: 0,
        }
    }

    fn write_test_commit_summary(history_root: &Path, commit_hash: &str, summary: Summary) {
        static NEXT_TEST_MARKER: AtomicU64 = AtomicU64::new(1);

        let dataset_dir = history_root
            .join(format!("2024-01-01-{commit_hash}"))
            .join(summary.dataset.as_str());
        std::fs::create_dir_all(&dataset_dir).unwrap();
        write_json_file(&dataset_dir.join("summary.json"), &summary).unwrap();
        let marker = system_time_to_marker(SystemTime::now())
            + u128::from(NEXT_TEST_MARKER.fetch_add(1, Ordering::Relaxed));
        let manifest = Manifest {
            commit_hash: commit_hash.to_string(),
            commit_full: format!("{commit_hash}-full"),
            dataset: summary.dataset.clone(),
            dataset_row_count: summary.dataset_row_count,
            timestamp: format!("{marker}ns-since-epoch"),
        };
        std::fs::write(
            dataset_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    struct TestDir(tempfile::TempDir);

    impl Deref for TestDir {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            self.0.path()
        }
    }

    fn make_temp_dir(name: &str) -> TestDir {
        TestDir(
            tempfile::Builder::new()
                .prefix(&format!("texform-regression-{name}-"))
                .tempdir()
                .unwrap(),
        )
    }
}
