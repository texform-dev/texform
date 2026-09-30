//! Opt-in whole-tree corpus audits, kept outside parser timing and stored baselines.

use crate::{data::FormulaRecord, runner};
use std::sync::Mutex;
use texform_core::document::Document;

#[derive(Clone, Copy, Debug, Default)]
pub struct AuditCounts {
    pub checked: usize,
    pub skipped: usize,
    pub violations: usize,
    pub transform_errors: usize,
}

impl AuditCounts {
    pub fn append(&mut self, other: Self) {
        self.checked += other.checked;
        self.skipped += other.skipped;
        self.violations += other.violations;
        self.transform_errors += other.transform_errors;
    }

    pub fn failed(self) -> bool {
        self.violations != 0 || self.transform_errors != 0
    }

    pub fn report(self, dataset: &str, stage: &str) {
        println!(
            "[{dataset}] conformance {stage}: {} checked, {} incomplete skipped, {} violation(s), {} transform error(s)",
            self.checked, self.skipped, self.violations, self.transform_errors,
        );
    }
}

pub fn check_document(
    dataset: &str,
    record: &FormulaRecord,
    stage: &str,
    document: Option<&Document>,
) -> AuditCounts {
    let Some(document) = document.filter(|document| !document.has_errors()) else {
        return AuditCounts {
            skipped: 1,
            ..Default::default()
        };
    };
    let violation = document.__validate_conformance().err();
    if let Some(error) = &violation {
        eprintln!(
            "{}",
            serde_json::json!({
                "dataset": dataset,
                "formula_id": record.formula_id,
                "formula": record.formula,
                "stage": stage,
                "path": error.path,
                "rule": error.rule.to_string(),
                "message": error.message,
            })
        );
    }
    AuditCounts {
        checked: 1,
        violations: usize::from(violation.is_some()),
        ..Default::default()
    }
}

pub fn run_parser_audit(
    dataset: &str,
    records: &[FormulaRecord],
) -> (Vec<runner::FormulaResults>, [AuditCounts; 2]) {
    let counts = Mutex::new([AuditCounts::default(); 2]);
    let results = runner::run_parser_regression_with(records, |record, strict, nonstrict| {
        let strict = check_document(dataset, record, "strict", strict.document());
        let nonstrict = check_document(dataset, record, "nonstrict", nonstrict.document());
        let mut counts = counts.lock().expect("audit observer must not panic");
        counts[0].append(strict);
        counts[1].append(nonstrict);
    });
    (
        results,
        counts.into_inner().expect("audit observer must not panic"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use texform_core::ast::Node;
    use texform_core::parse::{ParseConfig, ParseContext};

    #[test]
    fn parser_audit_checks_complete_trees_and_counts_aborted_results() {
        let records = [FormulaRecord {
            formula_id: "unknown".to_string(),
            formula: r"\thisCommandIsUnknown".to_string(),
        }];
        let (results, counts) = run_parser_audit("fixture", &records);
        assert!(!results[0].strict.ok);
        assert!(results[0].nonstrict.ok);
        assert_eq!(counts[0].checked, 0);
        assert_eq!(counts[0].skipped, 1);
        assert_eq!(counts[1].checked, 1);
        assert!(!counts[1].failed());
    }

    #[test]
    fn complete_but_nonconforming_tree_fails_audit() {
        let record = FormulaRecord {
            formula_id: "invalid".to_string(),
            formula: "x".to_string(),
        };
        let (mut document, _) = ParseContext::shared()
            .parse(&record.formula, &ParseConfig::default())
            .try_into_document()
            .unwrap();
        let ast = document.__texform_engine_ast_mut();
        let Node::Root { children, .. } = ast.node(ast.root()) else {
            unreachable!();
        };
        let child = children[0];
        *ast.node_opt_mut(child).unwrap() = Node::Prime { count: 0 };
        let counts = check_document("fixture", &record, "strict", Some(&document));
        assert_eq!(counts.checked, 1);
        assert_eq!(counts.violations, 1);
        assert!(counts.failed());
    }
}
