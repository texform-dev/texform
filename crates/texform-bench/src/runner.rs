use crate::data::FormulaRecord;
use rayon::prelude::*;
use std::time::{Duration, Instant};
use texform_core::parse::{ParseConfig, ParseContext, ParseDiagnostic};

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub duration: Duration,
    pub ok: bool,
    pub diagnostic_count: usize,
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct FormulaResults {
    pub strict: ParseResult,
    pub nonstrict: ParseResult,
}

pub fn run_bench(records: &[FormulaRecord]) -> Vec<FormulaResults> {
    let strict_cfg = ParseConfig::STRICT;
    let nonstrict_cfg = ParseConfig {
        abort_on_error: true,
        ..Default::default()
    };
    let parser = ParseContext::shared();
    let _ = parser.parse("", &nonstrict_cfg);

    records
        .par_iter()
        .map(|record| {
            let strict_start = Instant::now();
            let strict_output = parser.parse(&record.formula, &strict_cfg);
            let strict_duration = strict_start.elapsed();

            let nonstrict_start = Instant::now();
            let nonstrict_output = parser.parse(&record.formula, &nonstrict_cfg);
            let nonstrict_duration = nonstrict_start.elapsed();

            FormulaResults {
                strict: ParseResult {
                    duration: strict_duration,
                    ok: strict_output.diagnostics.is_empty(),
                    diagnostic_count: strict_output.diagnostics.len(),
                    diagnostics: strict_output.diagnostics,
                },
                nonstrict: ParseResult {
                    duration: nonstrict_duration,
                    ok: nonstrict_output.diagnostics.is_empty(),
                    diagnostic_count: nonstrict_output.diagnostics.len(),
                    diagnostics: nonstrict_output.diagnostics,
                },
            }
        })
        .collect()
}
