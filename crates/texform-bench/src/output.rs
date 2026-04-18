use crate::data::FormulaRecord;
use crate::runner::FormulaResults;
use crate::stats::{self, ModeStats};
use arrow_array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use texform_core::parse::ParseDiagnostic;

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

#[derive(Serialize)]
pub struct Summary {
    pub dataset: String,
    pub dataset_row_count: usize,
    pub total_tasks: usize,
    pub completed: usize,
    pub strict: ModeStats,
    pub nonstrict: ModeStats,
    #[serde(skip)]
    strict_durations: Vec<Duration>,
    #[serde(skip)]
    strict_oks: Vec<bool>,
    #[serde(skip)]
    nonstrict_durations: Vec<Duration>,
    #[serde(skip)]
    nonstrict_oks: Vec<bool>,
}

#[derive(Serialize)]
pub struct OverallSummary {
    pub dataset_count: usize,
    pub total_tasks: usize,
    pub strict: ModeStats,
    pub nonstrict: ModeStats,
}

#[derive(Serialize)]
struct Manifest {
    commit_hash: String,
    commit_full: String,
    dataset: String,
    dataset_row_count: usize,
    timestamp: String,
}

#[derive(Serialize)]
struct ErrorEntry {
    formula: String,
    strict: bool,
    diagnostic_count: usize,
    diagnostics: Vec<ParseDiagnostic>,
}

pub fn build_summary(slug: &str, records: &[FormulaRecord], results: &[FormulaResults]) -> Summary {
    let strict_durations: Vec<Duration> = results
        .iter()
        .map(|result| result.strict.duration)
        .collect();
    let strict_oks: Vec<bool> = results.iter().map(|result| result.strict.ok).collect();
    let nonstrict_durations: Vec<Duration> = results
        .iter()
        .map(|result| result.nonstrict.duration)
        .collect();
    let nonstrict_oks: Vec<bool> = results.iter().map(|result| result.nonstrict.ok).collect();
    let mut strict = stats::compute_mode_stats(&strict_durations, &strict_oks);
    strict.timing_ms.max_formula_id = max_formula_id(records, results, true);

    let mut nonstrict = stats::compute_mode_stats(&nonstrict_durations, &nonstrict_oks);
    nonstrict.timing_ms.max_formula_id = max_formula_id(records, results, false);

    Summary {
        dataset: slug.to_string(),
        dataset_row_count: records.len(),
        total_tasks: records.len(),
        completed: records.len(),
        strict,
        nonstrict,
        strict_durations,
        strict_oks,
        nonstrict_durations,
        nonstrict_oks,
    }
}

pub fn build_overall(summaries: &[Summary]) -> OverallSummary {
    let total_tasks = summaries.iter().map(|summary| summary.total_tasks).sum();
    let mut strict_durations = Vec::with_capacity(total_tasks);
    let mut strict_oks = Vec::with_capacity(total_tasks);
    let mut nonstrict_durations = Vec::with_capacity(total_tasks);
    let mut nonstrict_oks = Vec::with_capacity(total_tasks);

    for summary in summaries {
        strict_durations.extend(summary.strict_durations.iter().copied());
        strict_oks.extend(summary.strict_oks.iter().copied());
        nonstrict_durations.extend(summary.nonstrict_durations.iter().copied());
        nonstrict_oks.extend(summary.nonstrict_oks.iter().copied());
    }

    OverallSummary {
        dataset_count: summaries.len(),
        total_tasks,
        strict: stats::compute_mode_stats(&strict_durations, &strict_oks),
        nonstrict: stats::compute_mode_stats(&nonstrict_durations, &nonstrict_oks),
    }
}

pub fn write_summary(
    results_root: &Path,
    slug: &str,
    summary: &Summary,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = results_root.join(slug);
    std::fs::create_dir_all(&dir)?;
    write_summary_file(&dir.join("summary.json"), summary)?;
    Ok(())
}

pub fn write_overall(
    results_root: &Path,
    summaries: &[Summary],
) -> Result<(), Box<dyn std::error::Error>> {
    let overall = build_overall(summaries);
    write_overall_file(&results_root.join("overall.json"), &overall)
}

pub fn write_commit_results(
    results_root: &Path,
    slug: &str,
    summary: &Summary,
    records: &[FormulaRecord],
    results: &[FormulaResults],
    commit_hash: &str,
    commit_full: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = results_root.join("commits").join(commit_hash).join(slug);
    std::fs::create_dir_all(&dir)?;
    write_summary_file(&dir.join("summary.json"), summary)?;

    let manifest = Manifest {
        commit_hash: commit_hash.to_string(),
        commit_full: commit_full.to_string(),
        dataset: slug.to_string(),
        dataset_row_count: records.len(),
        timestamp: now_timestamp(),
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    write_results_parquet(&dir.join("results.parquet"), records, results)?;
    write_errors_jsonl(&dir.join("errors.jsonl"), records, results)?;

    Ok(())
}

pub fn git_hash() -> (String, String) {
    let bench_root = crate::config::resolve_bench_root();
    let repo_root = bench_root
        .parent()
        .expect("bench root should live inside the texform repo");
    let full = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
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

    (short, full)
}

fn now_timestamp() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    format!("{}s-since-epoch", duration.as_secs())
}

fn write_summary_file(path: &Path, summary: &Summary) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let formatter = FixedPrecisionPrettyFormatter::new();
    let mut serializer = serde_json::Serializer::with_formatter(writer, formatter);
    summary.serialize(&mut serializer)?;
    Ok(())
}

fn write_overall_file(
    path: &Path,
    overall: &OverallSummary,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let formatter = FixedPrecisionPrettyFormatter::new();
    let mut serializer = serde_json::Serializer::with_formatter(writer, formatter);
    overall.serialize(&mut serializer)?;
    Ok(())
}

fn write_results_parquet(
    path: &Path,
    records: &[FormulaRecord],
    results: &[FormulaResults],
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("formula_id", DataType::Utf8, false),
        Field::new("formula", DataType::Utf8, false),
        Field::new("strict_ok", DataType::Boolean, false),
        Field::new("strict_duration_ms", DataType::Float64, false),
        Field::new("strict_diagnostic_count", DataType::UInt32, false),
        Field::new("nonstrict_ok", DataType::Boolean, false),
        Field::new("nonstrict_duration_ms", DataType::Float64, false),
        Field::new("nonstrict_diagnostic_count", DataType::UInt32, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(
                records
                    .iter()
                    .map(|record| record.formula_id.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                records
                    .iter()
                    .map(|record| record.formula.clone())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BooleanArray::from(
                results
                    .iter()
                    .map(|result| result.strict.ok)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                results
                    .iter()
                    .map(|result| result.strict.duration.as_secs_f64() * 1_000.0)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                results
                    .iter()
                    .map(|result| result.strict.diagnostic_count as u32)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BooleanArray::from(
                results
                    .iter()
                    .map(|result| result.nonstrict.ok)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                results
                    .iter()
                    .map(|result| result.nonstrict.duration.as_secs_f64() * 1_000.0)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                results
                    .iter()
                    .map(|result| result.nonstrict.diagnostic_count as u32)
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;

    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build();
    let file = std::fs::File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn write_errors_jsonl(
    path: &Path,
    records: &[FormulaRecord],
    results: &[FormulaResults],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    for (record, result) in records.iter().zip(results.iter()) {
        if !result.strict.ok {
            serde_json::to_writer(
                &mut file,
                &ErrorEntry {
                    formula: record.formula.clone(),
                    strict: true,
                    diagnostic_count: result.strict.diagnostic_count,
                    diagnostics: result.strict.diagnostics.clone(),
                },
            )?;
            file.write_all(b"\n")?;
        }

        if !result.nonstrict.ok {
            serde_json::to_writer(
                &mut file,
                &ErrorEntry {
                    formula: record.formula.clone(),
                    strict: false,
                    diagnostic_count: result.nonstrict.diagnostic_count,
                    diagnostics: result.nonstrict.diagnostics.clone(),
                },
            )?;
            file.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn max_formula_id(
    records: &[FormulaRecord],
    results: &[FormulaResults],
    strict: bool,
) -> Option<String> {
    let mut current: Option<(Duration, &str)> = None;

    for (record, result) in records.iter().zip(results.iter()) {
        let duration = if strict {
            result.strict.duration
        } else {
            result.nonstrict.duration
        };

        if current.is_none_or(|(max_duration, _)| duration > max_duration) {
            current = Some((duration, record.formula_id.as_str()));
        }
    }

    current.map(|(_, formula_id)| formula_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FormulaRecord;
    use crate::runner::{FormulaResults, ParseResult};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    fn write_commit_results_persists_diagnostics_for_both_modes() {
        let dir = make_temp_dir("commit-results");
        let records = sample_records();
        let results = vec![
            FormulaResults {
                strict: ParseResult {
                    duration: Duration::from_micros(10),
                    ok: false,
                    diagnostic_count: 1,
                    diagnostics: vec![ParseDiagnostic {
                        message: "strict failed".to_string(),
                        span: Span { start: 0, end: 1 },
                        expected: vec!["group".to_string()],
                        found: Some("\\foo".to_string()),
                        contexts: Vec::new(),
                    }],
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
                    ok: true,
                    diagnostic_count: 0,
                    diagnostics: Vec::new(),
                },
                nonstrict: ParseResult {
                    duration: Duration::from_micros(25),
                    ok: false,
                    diagnostic_count: 1,
                    diagnostics: vec![ParseDiagnostic {
                        message: "nonstrict failed".to_string(),
                        span: Span { start: 2, end: 3 },
                        expected: vec!["delimiter".to_string()],
                        found: Some("\\bar".to_string()),
                        contexts: Vec::new(),
                    }],
                },
            },
        ];
        let summary = build_summary("demo", &records, &results);

        write_commit_results(
            &dir,
            "demo",
            &summary,
            &records,
            &results,
            "abc12345",
            "abc12345full",
        )
        .unwrap();

        let commit_dir = dir.join("commits").join("abc12345").join("demo");
        assert!(commit_dir.join("summary.json").exists());
        assert!(commit_dir.join("manifest.json").exists());
        assert!(commit_dir.join("results.parquet").exists());
        assert!(commit_dir.join("errors.jsonl").exists());

        let summary_json = std::fs::read_to_string(commit_dir.join("summary.json")).unwrap();
        assert!(summary_json.contains("\"max_formula_id\": \"beta\""));

        let errors = std::fs::read_to_string(commit_dir.join("errors.jsonl")).unwrap();
        assert_eq!(errors.lines().count(), 2);
        assert!(errors.contains("\"diagnostics\""));
        assert!(errors.contains("\"strict\":true"));
        assert!(errors.contains("\"strict\":false"));
        assert!(errors.contains("strict failed"));
        assert!(errors.contains("nonstrict failed"));

        std::fs::remove_dir_all(dir).unwrap();
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
                    diagnostics: vec![ParseDiagnostic {
                        message: "invalid token".to_string(),
                        span: Span { start: 0, end: 1 },
                        expected: vec!["group".to_string()],
                        found: Some("\\foo".to_string()),
                        contexts: Vec::new(),
                    }],
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

    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("texform-bench-{name}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
