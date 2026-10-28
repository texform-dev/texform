//! Round-trip report: counts transformed-tree round-trip failures over the
//! regression datasets and the adversarial boundary set.
//!
//! The report is an observation, not a merge condition. It never changes the
//! `transform_contract` exit status or verdict. Each run compares itself with
//! the report the previous run left in the same result root.

use crate::{config, data, output};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::Instant;
use texform_core::parse::ParseContext;
use texform_core::serialize::Style;
use texform_transform::TransformContext;
use texform_transform::round_trip::{
    self, Check, Failure, Outcome, PACKAGE_SETS, PROFILES, STYLES, adversarial,
};

const SCHEMA_VERSION: u32 = 1;
pub const ADVERSARIAL_DATASET: &str = "adversarial-boundary";
pub const REPORT_FILE: &str = "round_trip_report.json";
pub const FAILURES_FILE: &str = "round_trip_failures.jsonl";
const SAMPLES_PER_CELL: usize = 3;
const BATCH_SIZE: usize = 8_192;

#[derive(Debug, Clone)]
pub struct ReportConfig {
    pub datasets_yaml: PathBuf,
    pub results_root: PathBuf,
    /// Regression dataset slugs; empty selects all. The adversarial boundary
    /// set is always included.
    pub datasets: Vec<String>,
    pub limit: Option<usize>,
    pub dry_run: bool,
    pub skip_commit_results: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellCounts {
    /// Complete inputs whose transform succeeded.
    pub checked: usize,
    pub skipped: Skipped,
    pub failures: Failures,
}

/// Inputs not checked, by reason.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub incomplete: usize,
    pub transform_failure: usize,
}

/// Failures per transformed-tree check, keyed by [`Check::label`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failures {
    pub reparse: usize,
    pub reparse_error: usize,
    pub retransform: usize,
    pub reparse_retransform: usize,
}

impl CellCounts {
    fn add(&mut self, other: &Self) {
        self.checked += other.checked;
        self.skipped.incomplete += other.skipped.incomplete;
        self.skipped.transform_failure += other.skipped.transform_failure;
        self.failures.reparse += other.failures.reparse;
        self.failures.reparse_error += other.failures.reparse_error;
        self.failures.retransform += other.failures.retransform;
        self.failures.reparse_retransform += other.failures.reparse_retransform;
    }

    fn record(&mut self, check: Check) {
        let failures = &mut self.failures;
        match check {
            Check::Reparse => failures.reparse += 1,
            Check::ReparseError => failures.reparse_error += 1,
            Check::Retransform => failures.retransform += 1,
            Check::ReparseRetransform => failures.reparse_retransform += 1,
            Check::ParserRoundTrip => unreachable!("counted per parser cell"),
        }
    }

    /// Labeled counts compared between runs; `checked` is shown for context.
    fn compared(&self) -> [(&'static str, usize); 5] {
        let failures = &self.failures;
        [
            ("transform failures", self.skipped.transform_failure),
            (Check::Reparse.label(), failures.reparse),
            (Check::ReparseError.label(), failures.reparse_error),
            (Check::Retransform.label(), failures.retransform),
            (
                Check::ReparseRetransform.label(),
                failures.reparse_retransform,
            ),
        ]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserCounts {
    /// Complete inputs.
    pub checked: usize,
    pub failures: ParserFailures,
}

/// Failures of the parser check, keyed by [`Check::label`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserFailures {
    pub parser_round_trip: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub dataset: String,
    pub packages: String,
    pub profile: String,
    pub style: String,
    #[serde(flatten)]
    pub counts: CellCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserCell {
    pub dataset: String,
    pub packages: String,
    pub style: String,
    #[serde(flatten)]
    pub counts: ParserCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalCell {
    pub packages: String,
    pub profile: String,
    pub style: String,
    #[serde(flatten)]
    pub counts: CellCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub texform_commit_short: String,
    pub texform_dirty: bool,
    /// Covers the inputs only: dataset selection, limit, package sets, and the
    /// adversarial generator. Transform code and configuration are excluded,
    /// since the report measures their effect.
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundTripReport {
    pub schema_version: u32,
    pub provenance: Provenance,
    pub datasets: Vec<String>,
    pub limit: Option<usize>,
    pub package_sets: BTreeMap<String, Vec<String>>,
    pub cells: Vec<Cell>,
    pub parser: Vec<ParserCell>,
    pub totals: Vec<TotalCell>,
}

#[derive(Debug, Serialize)]
struct FailureDetail<'a> {
    dataset: &'a str,
    formula_id: &'a str,
    formula: &'a str,
    packages: &'a str,
    profile: Option<&'static str>,
    style: Option<&'static str>,
    check: &'static str,
    output: Option<&'a str>,
    difference: Option<&'a round_trip::Difference>,
    detail: Option<&'a str>,
}

/// Results for one formula under one package set.
struct FormulaOutcome {
    incomplete: bool,
    parser: [Option<Failure>; 2],
    profiles: Vec<Outcome>,
}

type CellKey = (usize, usize, usize);

#[derive(Default)]
struct DatasetAccumulator {
    cells: BTreeMap<CellKey, CellCounts>,
    parser: BTreeMap<(usize, usize), ParserCounts>,
    /// Sample lines per cell, keyed with a check label.
    samples: BTreeMap<CellKey, Vec<String>>,
}

struct Lane {
    name: &'static str,
    packages: &'static [&'static str],
    parse_ctx: ParseContext,
    engines: Vec<TransformContext>,
}

pub fn run(config: &ReportConfig) -> Result<RoundTripReport, Box<dyn std::error::Error>> {
    let datasets_config = config::DatasetsConfig::load_from_yaml(&config.datasets_yaml)?;
    let selected = datasets_config.filter_by_slugs(&config.datasets);
    let lanes: Vec<Lane> = PACKAGE_SETS
        .iter()
        .map(|(name, packages)| {
            let parse_ctx = ParseContext::from_packages(packages);
            let engines = round_trip::engines(&parse_ctx);
            Lane {
                name,
                packages,
                parse_ctx,
                engines,
            }
        })
        .collect();

    let commit_info = output::git_commit_info();
    let mut failures = if !config.dry_run && !config.skip_commit_results {
        let dir = config
            .results_root
            .join("commits")
            .join(commit_info.commit_dir_name());
        std::fs::create_dir_all(&dir)?;
        Some(std::io::BufWriter::new(std::fs::File::create(
            dir.join(FAILURES_FILE),
        )?))
    } else {
        None
    };

    let mut accumulators: Vec<(String, DatasetAccumulator)> = Vec::new();
    for entry in &selected {
        let path = config::resolve_dataset_file(&config.datasets_yaml, entry);
        if data::check_data_file(&path) != data::DataFileStatus::Ready {
            eprintln!(
                "[{}] round-trip report: data file unavailable (run `git lfs pull`), skipping",
                entry.slug
            );
            continue;
        }
        let start = Instant::now();
        let mut accumulator = DatasetAccumulator::default();
        data::read_formula_record_batches(&path, 0, config.limit, |records| {
            for (lane_index, lane) in lanes.iter().enumerate() {
                check_batch(
                    &entry.slug,
                    lane_index,
                    lane,
                    &records,
                    &mut accumulator,
                    failures.as_mut(),
                )?;
            }
            Ok(())
        })?;
        eprintln!(
            "[{}] round-trip report in {:.1}s",
            entry.slug,
            start.elapsed().as_secs_f64()
        );
        accumulators.push((entry.slug.clone(), accumulator));
    }

    let start = Instant::now();
    let mut accumulator = DatasetAccumulator::default();
    for (lane_index, lane) in lanes.iter().enumerate() {
        let mut records: Vec<_> = adversarial::full(&lane.parse_ctx)
            .into_iter()
            .map(|input| data::FormulaRecord {
                formula_id: input.source.clone(),
                formula: input.source,
            })
            .collect();
        if let Some(limit) = config.limit {
            records.truncate(limit);
        }
        for batch in records.chunks(BATCH_SIZE) {
            check_batch(
                ADVERSARIAL_DATASET,
                lane_index,
                lane,
                batch,
                &mut accumulator,
                failures.as_mut(),
            )?;
        }
    }
    eprintln!(
        "[{ADVERSARIAL_DATASET}] round-trip report in {:.1}s",
        start.elapsed().as_secs_f64()
    );
    accumulators.push((ADVERSARIAL_DATASET.to_string(), accumulator));

    if let Some(mut writer) = failures {
        writer.flush()?;
    }

    let report = build_report(config, &commit_info, &lanes, &accumulators)?;
    let report_path = config.results_root.join(REPORT_FILE);
    let previous = std::fs::read_to_string(&report_path)
        .ok()
        .and_then(|text| serde_json::from_str::<RoundTripReport>(&text).ok());
    for line in comparison_lines(&report, previous.as_ref(), &accumulators) {
        println!("{line}");
    }

    if !config.dry_run {
        std::fs::create_dir_all(&config.results_root)?;
        let file = std::fs::File::create(&report_path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &report)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    Ok(report)
}

fn check_batch(
    dataset: &str,
    lane_index: usize,
    lane: &Lane,
    records: &[data::FormulaRecord],
    accumulator: &mut DatasetAccumulator,
    failures: Option<&mut std::io::BufWriter<std::fs::File>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let outcomes: Vec<FormulaOutcome> = records
        .par_iter()
        .map(|record| check_formula(lane, &record.formula))
        .collect();
    tally(
        dataset,
        lane_index,
        lane.name,
        records,
        outcomes,
        accumulator,
        failures.map(|writer| writer as &mut dyn Write),
    )?;
    Ok(())
}

/// Adds the outcomes of one batch to the counts and writes failure details.
fn tally(
    dataset: &str,
    lane_index: usize,
    lane_name: &str,
    records: &[data::FormulaRecord],
    outcomes: Vec<FormulaOutcome>,
    accumulator: &mut DatasetAccumulator,
    mut failures: Option<&mut dyn Write>,
) -> Result<(), std::io::Error> {
    for (record, outcome) in records.iter().zip(outcomes) {
        if outcome.incomplete {
            for profile in 0..PROFILES.len() {
                for style in 0..STYLES.len() {
                    accumulator
                        .cells
                        .entry((lane_index, profile, style))
                        .or_default()
                        .skipped
                        .incomplete += 1;
                }
            }
            continue;
        }
        let mut write = |detail: FailureDetail<'_>| -> Result<(), std::io::Error> {
            if let Some(writer) = failures.as_mut() {
                serde_json::to_writer(&mut **writer, &detail)?;
                writer.write_all(b"\n")?;
            }
            Ok(())
        };
        let base = |profile, style, check| FailureDetail {
            dataset,
            formula_id: &record.formula_id,
            formula: &record.formula,
            packages: lane_name,
            profile,
            style,
            check,
            output: None,
            difference: None,
            detail: None,
        };
        for (style_index, failure) in outcome.parser.iter().enumerate() {
            let counts = accumulator
                .parser
                .entry((lane_index, style_index))
                .or_default();
            counts.checked += 1;
            if let Some(failure) = failure {
                counts.failures.parser_round_trip += 1;
                write(FailureDetail {
                    output: Some(&failure.output),
                    difference: failure.difference.as_ref(),
                    detail: failure.detail.as_deref(),
                    ..base(
                        None,
                        failure.style.map(round_trip::style_name),
                        "parser_round_trip",
                    )
                })?;
            }
        }
        for (profile_index, (profile, result)) in PROFILES.iter().zip(&outcome.profiles).enumerate()
        {
            let profile_name = round_trip::profile_name(*profile);
            match result {
                Outcome::TransformFailed(error) => {
                    for style in 0..STYLES.len() {
                        let key = (lane_index, profile_index, style);
                        accumulator
                            .cells
                            .entry(key)
                            .or_default()
                            .skipped
                            .transform_failure += 1;
                        push_sample(
                            accumulator,
                            key,
                            format!("transform failure: {} [{error}]", record.formula),
                        );
                    }
                    write(FailureDetail {
                        detail: Some(error),
                        ..base(Some(profile_name), None, "transform_failure")
                    })?;
                }
                Outcome::Checked(failed) => {
                    for style in 0..STYLES.len() {
                        accumulator
                            .cells
                            .entry((lane_index, profile_index, style))
                            .or_default()
                            .checked += 1;
                    }
                    for failure in failed {
                        // `Retransform` does not serialize; it counts in both style cells.
                        let styles: Vec<usize> = match failure.style {
                            Some(style) => vec![style_index(style)],
                            None => (0..STYLES.len()).collect(),
                        };
                        for style in styles {
                            let key = (lane_index, profile_index, style);
                            accumulator
                                .cells
                                .entry(key)
                                .or_default()
                                .record(failure.check);
                            push_sample(
                                accumulator,
                                key,
                                format!("{}: {} => {failure}", failure.check, record.formula),
                            );
                        }
                        write(FailureDetail {
                            output: Some(&failure.output),
                            difference: failure.difference.as_ref(),
                            detail: failure.detail.as_deref(),
                            ..base(
                                Some(profile_name),
                                failure.style.map(round_trip::style_name),
                                failure.check.label(),
                            )
                        })?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn push_sample(accumulator: &mut DatasetAccumulator, key: CellKey, line: String) {
    let samples = accumulator.samples.entry(key).or_default();
    if samples.len() < SAMPLES_PER_CELL {
        samples.push(line);
    }
}

fn check_formula(lane: &Lane, formula: &str) -> FormulaOutcome {
    let parse_ctx = &lane.parse_ctx;
    let parsed = catch_unwind(AssertUnwindSafe(|| round_trip::parse(parse_ctx, formula)));
    let Ok(Some(original)) = parsed else {
        return FormulaOutcome {
            incomplete: true,
            parser: [None, None],
            profiles: Vec::new(),
        };
    };
    let parser = STYLES.map(|style| {
        catch_unwind(AssertUnwindSafe(|| {
            round_trip::check_parser(parse_ctx, &original, style)
        }))
        .unwrap_or_else(|_| {
            Some(Failure {
                check: Check::ParserRoundTrip,
                style: Some(style),
                output: String::new(),
                difference: None,
                detail: Some("panic".into()),
            })
        })
    });
    let profiles = lane
        .engines
        .iter()
        .map(|engine| {
            catch_unwind(AssertUnwindSafe(|| {
                round_trip::check_transform(parse_ctx, engine, &original)
            }))
            .unwrap_or_else(|_| Outcome::TransformFailed("panic".into()))
        })
        .collect();
    FormulaOutcome {
        incomplete: false,
        parser,
        profiles,
    }
}

fn style_index(style: Style) -> usize {
    STYLES
        .iter()
        .position(|candidate| *candidate == style)
        .expect("checks use the listed styles")
}

fn build_report(
    config: &ReportConfig,
    commit_info: &output::GitCommitInfo,
    lanes: &[Lane],
    accumulators: &[(String, DatasetAccumulator)],
) -> Result<RoundTripReport, Box<dyn std::error::Error>> {
    let mut cells = Vec::new();
    let mut parser = Vec::new();
    let mut totals: BTreeMap<CellKey, CellCounts> = BTreeMap::new();
    for (dataset, accumulator) in accumulators {
        for (lane_index, lane) in lanes.iter().enumerate() {
            for (style_index, style) in STYLES.iter().enumerate() {
                parser.push(ParserCell {
                    dataset: dataset.clone(),
                    packages: lane.name.to_string(),
                    style: round_trip::style_name(*style).to_string(),
                    counts: accumulator
                        .parser
                        .get(&(lane_index, style_index))
                        .copied()
                        .unwrap_or_default(),
                });
            }
            for (profile_index, profile) in PROFILES.iter().enumerate() {
                for (style_index, style) in STYLES.iter().enumerate() {
                    let key = (lane_index, profile_index, style_index);
                    let counts = accumulator.cells.get(&key).copied().unwrap_or_default();
                    totals.entry(key).or_default().add(&counts);
                    cells.push(Cell {
                        dataset: dataset.clone(),
                        packages: lane.name.to_string(),
                        profile: round_trip::profile_name(*profile).to_string(),
                        style: round_trip::style_name(*style).to_string(),
                        counts,
                    });
                }
            }
        }
    }
    let totals = totals
        .into_iter()
        .map(|((lane, profile, style), counts)| TotalCell {
            packages: lanes[lane].name.to_string(),
            profile: round_trip::profile_name(PROFILES[profile]).to_string(),
            style: round_trip::style_name(STYLES[style]).to_string(),
            counts,
        })
        .collect();
    Ok(RoundTripReport {
        schema_version: SCHEMA_VERSION,
        provenance: Provenance {
            texform_commit_short: commit_info.short_hash.clone(),
            texform_dirty: commit_info.dirty,
            config_hash: config_hash(config)?,
        },
        datasets: accumulators.iter().map(|(name, _)| name.clone()).collect(),
        limit: config.limit,
        package_sets: lanes
            .iter()
            .map(|lane| {
                (
                    lane.name.to_string(),
                    lane.packages.iter().map(|name| name.to_string()).collect(),
                )
            })
            .collect(),
        cells,
        parser,
        totals,
    })
}

fn comparison_lines(
    report: &RoundTripReport,
    previous: Option<&RoundTripReport>,
    accumulators: &[(String, DatasetAccumulator)],
) -> Vec<String> {
    let mut lines = Vec::new();
    let previous = previous.filter(|previous| {
        previous.provenance.config_hash == report.provenance.config_hash
            && previous.schema_version == report.schema_version
    });
    lines.push("\nRound-trip report (an observation; it does not affect the verdict)".to_string());
    match previous {
        Some(previous) => lines.push(format!(
            "Compared with the previous report from {}{}",
            previous.provenance.texform_commit_short,
            if previous.provenance.texform_dirty {
                " (dirty)"
            } else {
                ""
            }
        )),
        None => lines.push(
            "No comparable previous report (missing or different config hash); showing this run only"
                .to_string(),
        ),
    }
    let old_cells: BTreeMap<_, _> = previous
        .map(|previous| {
            previous
                .cells
                .iter()
                .map(|cell| {
                    (
                        (&cell.dataset, &cell.packages, &cell.profile, &cell.style),
                        cell.counts,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let samples: BTreeMap<_, _> = accumulators
        .iter()
        .map(|(dataset, accumulator)| (dataset.as_str(), &accumulator.samples))
        .collect();
    for (index, cell) in report.cells.iter().enumerate() {
        let old = old_cells.get(&(&cell.dataset, &cell.packages, &cell.profile, &cell.style));
        let current = cell.counts.compared();
        let changes: Vec<String> = current
            .iter()
            .enumerate()
            .filter_map(|(field, (label, now))| {
                let before = old.map(|old| old.compared()[field].1);
                match before {
                    Some(before) if before != *now => Some(format!(
                        "{label} {before} → {now} ({:+})",
                        *now as i64 - before as i64
                    )),
                    Some(_) | None if *now > 0 => Some(format!("{label} {now}")),
                    _ => None,
                }
            })
            .collect();
        if changes.is_empty() {
            continue;
        }
        lines.push(format!(
            "  {} {} {} {}: checked {}; {}",
            cell.dataset,
            cell.packages,
            cell.profile,
            cell.style,
            cell.counts.checked,
            changes.join("; ")
        ));
        let increased = old.is_some_and(|old| {
            old.compared()
                .iter()
                .zip(current)
                .any(|((_, before), (_, now))| now > *before)
        });
        if increased {
            let lane = index / (PROFILES.len() * STYLES.len()) % PACKAGE_SETS.len();
            let profile = index / STYLES.len() % PROFILES.len();
            let style = index % STYLES.len();
            if let Some(cell_samples) = samples
                .get(cell.dataset.as_str())
                .and_then(|samples| samples.get(&(lane, profile, style)))
            {
                lines.extend(cell_samples.iter().map(|line| format!("      {line}")));
            }
        }
    }
    for cell in &report.parser {
        if cell.counts.failures.parser_round_trip > 0 {
            lines.push(format!(
                "  {} {} {}: {} {} of {}",
                cell.dataset,
                cell.packages,
                cell.style,
                Check::ParserRoundTrip.label(),
                cell.counts.failures.parser_round_trip,
                cell.counts.checked
            ));
        }
    }
    lines
}

#[derive(Serialize)]
struct ConfigHashInput<'a> {
    datasets_yaml_hash_fnv1a64: String,
    datasets: &'a [String],
    limit: Option<usize>,
    package_sets: Vec<(&'static str, &'static [&'static str])>,
    generator_version: u32,
    schema_version: u32,
}

fn config_hash(config: &ReportConfig) -> Result<String, Box<dyn std::error::Error>> {
    let mut datasets = config.datasets.clone();
    datasets.sort();
    let yaml = std::fs::read(&config.datasets_yaml)?;
    let input = ConfigHashInput {
        datasets_yaml_hash_fnv1a64: format!("{:016x}", fnv1a64(&yaml)),
        datasets: &datasets,
        limit: config.limit,
        package_sets: PACKAGE_SETS.to_vec(),
        generator_version: adversarial::GENERATOR_VERSION,
        schema_version: SCHEMA_VERSION,
    };
    Ok(format!("{:016x}", fnv1a64(&serde_json::to_vec(&input)?)))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(check: Check, style: Option<Style>) -> Failure {
        Failure {
            check,
            style,
            output: "out".into(),
            difference: None,
            detail: None,
        }
    }

    fn record(formula: &str) -> data::FormulaRecord {
        data::FormulaRecord {
            formula_id: formula.into(),
            formula: formula.into(),
        }
    }

    #[test]
    fn tally_counts_each_check_and_both_skip_reasons() {
        let records = [record("checked"), record("incomplete")];
        let outcomes = vec![
            FormulaOutcome {
                incomplete: false,
                parser: [
                    None,
                    Some(failure(Check::ParserRoundTrip, Some(Style::Compact))),
                ],
                profiles: vec![
                    Outcome::Checked(Vec::new()),
                    Outcome::Checked(vec![
                        failure(Check::Reparse, Some(Style::Spaced)),
                        failure(Check::ReparseError, Some(Style::Spaced)),
                    ]),
                    Outcome::TransformFailed("boom".into()),
                    Outcome::Checked(vec![failure(Check::Retransform, None)]),
                ],
            },
            FormulaOutcome {
                incomplete: true,
                parser: [None, None],
                profiles: Vec::new(),
            },
        ];
        let mut accumulator = DatasetAccumulator::default();
        let mut details = Vec::new();
        tally(
            "fixture",
            0,
            "default",
            &records,
            outcomes,
            &mut accumulator,
            Some(&mut details),
        )
        .unwrap();

        let cell = |profile, style| accumulator.cells[&(0, profile, style)];
        assert_eq!(cell(0, 0).checked, 1);
        assert_eq!(cell(0, 0).skipped.incomplete, 1);
        assert_eq!(
            (
                cell(1, 0).failures.reparse,
                cell(1, 0).failures.reparse_error
            ),
            (1, 1)
        );
        assert_eq!(
            (
                cell(1, 1).failures.reparse,
                cell(1, 1).failures.reparse_error
            ),
            (0, 0)
        );
        for style in 0..2 {
            assert_eq!(cell(2, style).checked, 0);
            assert_eq!(cell(2, style).skipped.transform_failure, 1);
            // `Retransform` does not serialize, so it counts under both styles.
            assert_eq!(cell(3, style).failures.retransform, 1);
        }
        assert_eq!(
            accumulator.parser[&(0, 0)],
            ParserCounts {
                checked: 1,
                failures: ParserFailures {
                    parser_round_trip: 0
                }
            }
        );
        assert_eq!(
            accumulator.parser[&(0, 1)],
            ParserCounts {
                checked: 1,
                failures: ParserFailures {
                    parser_round_trip: 1
                }
            }
        );

        let details = String::from_utf8(details).unwrap();
        let checks: Vec<String> = details
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["check"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            checks,
            [
                "parser_round_trip",
                "reparse",
                "reparse_error",
                "transform_failure",
                "retransform"
            ]
        );
    }

    fn report(hash: &str, reparse: usize) -> RoundTripReport {
        RoundTripReport {
            schema_version: SCHEMA_VERSION,
            provenance: Provenance {
                texform_commit_short: "abc".into(),
                texform_dirty: false,
                config_hash: hash.into(),
            },
            datasets: vec!["fixture".into()],
            limit: None,
            package_sets: BTreeMap::new(),
            cells: vec![Cell {
                dataset: "fixture".into(),
                packages: "default".into(),
                profile: "authoring".into(),
                style: "spaced".into(),
                counts: CellCounts {
                    checked: 10,
                    failures: Failures {
                        reparse,
                        ..Failures::default()
                    },
                    ..CellCounts::default()
                },
            }],
            parser: Vec::new(),
            totals: Vec::new(),
        }
    }

    #[test]
    fn comparison_reports_changes_only_against_the_same_inputs() {
        let mut accumulator = DatasetAccumulator::default();
        accumulator
            .samples
            .insert((0, 0, 0), vec!["reparse: sample".into()]);
        let accumulators = vec![("fixture".to_string(), accumulator)];
        let current = report("same", 2);

        let lines = comparison_lines(&current, Some(&report("same", 1)), &accumulators).join("\n");
        assert!(
            lines.contains("Compared with the previous report from abc"),
            "{lines}"
        );
        assert!(lines.contains("reparse 1 → 2 (+1)"), "{lines}");
        assert!(lines.contains("reparse: sample"), "{lines}");

        let lines = comparison_lines(&current, Some(&report("other", 1)), &accumulators).join("\n");
        assert!(lines.contains("No comparable previous report"), "{lines}");
        assert!(lines.contains("reparse 2"), "{lines}");
        assert!(!lines.contains("reparse: sample"), "{lines}");
    }

    /// The next run reads the report this run writes.
    #[test]
    fn report_survives_a_json_round_trip() {
        let report = report("same", 1);
        let text = serde_json::to_string_pretty(&report).unwrap();
        let read: RoundTripReport = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string_pretty(&read).unwrap(), text);
    }
}
