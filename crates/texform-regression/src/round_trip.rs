//! Round-trip check: runs the transformed-tree round-trip checks over the
//! regression datasets and the adversarial boundary set.
//!
//! Every check must pass. Any failure, including a transform failure, fails
//! the `transform_contract` run.

use crate::{config, data, output};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::Instant;
use texform_core::parse::ParseContext;
use texform_transform::TransformContext;
use texform_transform::round_trip::{
    self, Check, Difference, Failure, Outcome, PACKAGE_SETS, PROFILES, STYLES, adversarial,
};

pub const ADVERSARIAL_DATASET: &str = "adversarial-boundary";
pub const FAILURES_FILE: &str = "round_trip_failures.jsonl";
const BATCH_SIZE: usize = 8_192;

#[derive(Debug, Clone)]
pub struct RoundTripConfig {
    pub datasets_yaml: PathBuf,
    pub results_root: PathBuf,
    /// Regression dataset slugs; empty selects all. The adversarial boundary
    /// set is always included.
    pub datasets: Vec<String>,
    pub limit: Option<usize>,
    pub dry_run: bool,
    pub skip_commit_results: bool,
}

/// One failure; `profile` is `None` for `parser_round_trip`, and `style` is
/// `None` for `retransform` and transform failures.
#[derive(Debug, Serialize)]
struct FailureRecord {
    profile: Option<&'static str>,
    style: Option<&'static str>,
    check: &'static str,
    output: Option<String>,
    difference: Option<Difference>,
    detail: Option<String>,
}

impl FailureRecord {
    fn from_failure(profile: Option<&'static str>, failure: Failure) -> Self {
        Self {
            profile,
            style: failure.style.map(round_trip::style_name),
            check: failure.check.label(),
            output: Some(failure.output),
            difference: failure.difference,
            detail: failure.detail,
        }
    }
}

#[derive(Serialize)]
struct FailureDetail<'a> {
    dataset: &'a str,
    formula_id: &'a str,
    formula: &'a str,
    packages: &'a str,
    #[serde(flatten)]
    failure: &'a FailureRecord,
}

/// Failure counts keyed by dataset, package set, profile, style, and check;
/// `-` stands for a profile or style the check does not depend on.
type Counts = BTreeMap<
    (
        String,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ),
    usize,
>;

struct Lane {
    name: &'static str,
    parse_ctx: ParseContext,
    engines: Vec<TransformContext>,
}

/// Runs the checks and returns the number of failures.
pub fn run(config: &RoundTripConfig) -> Result<usize, Box<dyn std::error::Error>> {
    let datasets_config = config::DatasetsConfig::load_from_yaml(&config.datasets_yaml)?;
    let lanes: Vec<Lane> = PACKAGE_SETS
        .iter()
        .map(|(name, packages)| {
            let parse_ctx = ParseContext::from_packages(packages);
            let engines = round_trip::engines(&parse_ctx);
            Lane {
                name,
                parse_ctx,
                engines,
            }
        })
        .collect();

    let mut details = if !config.dry_run && !config.skip_commit_results {
        let dir = config
            .results_root
            .join("commits")
            .join(output::git_commit_info().commit_dir_name());
        std::fs::create_dir_all(&dir)?;
        Some(std::io::BufWriter::new(std::fs::File::create(
            dir.join(FAILURES_FILE),
        )?))
    } else {
        None
    };
    let mut counts = Counts::new();

    for entry in datasets_config.filter_by_slugs(&config.datasets) {
        let path = config::resolve_dataset_file(&config.datasets_yaml, &entry);
        if data::check_data_file(&path) != data::DataFileStatus::Ready {
            eprintln!(
                "[{}] round trip: data file unavailable (run `git lfs pull`), skipping",
                entry.slug
            );
            continue;
        }
        let start = Instant::now();
        let (mut checked, mut incomplete) = (0, 0);
        data::read_formula_record_batches(&path, 0, config.limit, |records| {
            for lane in &lanes {
                let (c, i) = check_batch(&entry.slug, lane, &records, &mut counts, &mut details)?;
                checked += c;
                incomplete += i;
            }
            Ok(())
        })?;
        print_dataset(&entry.slug, checked, incomplete, start);
    }

    let start = Instant::now();
    let (mut checked, mut incomplete) = (0, 0);
    for lane in &lanes {
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
            let (c, i) = check_batch(ADVERSARIAL_DATASET, lane, batch, &mut counts, &mut details)?;
            checked += c;
            incomplete += i;
        }
    }
    print_dataset(ADVERSARIAL_DATASET, checked, incomplete, start);

    if let Some(mut writer) = details {
        writer.flush()?;
    }
    let total: usize = counts.values().sum();
    for ((dataset, packages, profile, style, check), count) in &counts {
        println!("  {dataset} {packages} {profile} {style} {check}: {count}");
    }
    println!("Round trip: {total} failure(s)");
    Ok(total)
}

fn print_dataset(dataset: &str, checked: usize, incomplete: usize, start: Instant) {
    eprintln!(
        "[{dataset}] round trip: {checked} complete and {incomplete} incomplete input(s) across package sets in {:.1}s",
        start.elapsed().as_secs_f64()
    );
}

/// Checks one batch under one package set, adds its failures to `counts` and
/// `details`, and returns the numbers of complete and incomplete inputs.
fn check_batch(
    dataset: &str,
    lane: &Lane,
    records: &[data::FormulaRecord],
    counts: &mut Counts,
    details: &mut Option<impl Write>,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let results: Vec<Option<Vec<FailureRecord>>> = records
        .par_iter()
        .map(|record| check_formula(lane, &record.formula))
        .collect();
    let mut incomplete = 0;
    for (record, result) in records.iter().zip(results) {
        let Some(failures) = result else {
            incomplete += 1;
            continue;
        };
        for failure in &failures {
            *counts
                .entry((
                    dataset.to_string(),
                    lane.name,
                    failure.profile.unwrap_or("-"),
                    failure.style.unwrap_or("-"),
                    failure.check,
                ))
                .or_default() += 1;
            if let Some(writer) = details.as_mut() {
                serde_json::to_writer(
                    &mut *writer,
                    &FailureDetail {
                        dataset,
                        formula_id: &record.formula_id,
                        formula: &record.formula,
                        packages: lane.name,
                        failure,
                    },
                )?;
                writer.write_all(b"\n")?;
            }
        }
    }
    Ok((records.len() - incomplete, incomplete))
}

/// Every failure of `formula` under `lane`, or `None` when its parse is
/// incomplete.
fn check_formula(lane: &Lane, formula: &str) -> Option<Vec<FailureRecord>> {
    let parse_ctx = &lane.parse_ctx;
    let original = catch_unwind(AssertUnwindSafe(|| round_trip::parse(parse_ctx, formula)))
        .ok()
        .flatten()?;
    let mut failures = Vec::new();
    for style in STYLES {
        let failure = catch_unwind(AssertUnwindSafe(|| {
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
        });
        failures.extend(failure.map(|failure| FailureRecord::from_failure(None, failure)));
    }
    for (profile, engine) in PROFILES.into_iter().zip(&lane.engines) {
        let profile = Some(round_trip::profile_name(profile));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            round_trip::check_transform(parse_ctx, engine, &original)
        }))
        .unwrap_or_else(|_| Outcome::TransformFailed("panic".into()));
        match outcome {
            Outcome::TransformFailed(error) => failures.push(FailureRecord {
                profile,
                style: None,
                check: "transform_failure",
                output: None,
                difference: None,
                detail: Some(error),
            }),
            Outcome::Checked(checked) => failures.extend(
                checked
                    .into_iter()
                    .map(|failure| FailureRecord::from_failure(profile, failure)),
            ),
        }
    }
    Some(failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_batch_counts_failures_and_incomplete_inputs() {
        let parse_ctx = ParseContext::from_packages(round_trip::DEFAULT_PACKAGES);
        let lane = Lane {
            name: "default",
            engines: round_trip::engines(&parse_ctx),
            parse_ctx,
        };
        let records: Vec<_> = [r"\frac{a}{b}", r"\frac{a"]
            .into_iter()
            .map(|formula| data::FormulaRecord {
                formula_id: formula.into(),
                formula: formula.into(),
            })
            .collect();
        let mut counts = Counts::new();
        let mut details = Some(Vec::new());
        let (checked, incomplete) =
            check_batch("fixture", &lane, &records, &mut counts, &mut details).unwrap();
        assert_eq!((checked, incomplete), (1, 1));
        assert!(counts.is_empty(), "{counts:?}");
        assert!(details.unwrap().is_empty());
    }
}
