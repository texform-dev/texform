//! Counter aggregation for proposal impact evaluation.
//!
//! Walks a parsed `SyntaxNode` and counts occurrences of each `(kind, name)`
//! pair where `kind` is "cmd", "env", or "char". Command-like nodes
//! contribute as command or character targets according to the builtin specs;
//! environment nodes contribute as environment targets.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use texform_interface::syntax_node::{Argument, ArgumentValue, SyntaxNode};
use texform_specs::builtin::ALL_PACKAGES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Cmd,
    Env,
    Char,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Cmd => "cmd",
            Kind::Env => "env",
            Kind::Char => "char",
        }
    }
}

/// Counter aggregated from a single formula's AST.
#[derive(Debug, Default)]
pub struct FormulaCounter {
    pub counts: HashMap<(Kind, String), u32>,
}

impl FormulaCounter {
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    fn bump(&mut self, kind: Kind, name: &str) {
        *self.counts.entry((kind, name.to_string())).or_insert(0) += 1;
    }
}

/// Walk a `SyntaxNode` and accumulate target counts into `out`.
pub fn count_node(node: &SyntaxNode, out: &mut FormulaCounter) {
    match node {
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            for child in children {
                count_node(child, out);
            }
        }
        SyntaxNode::Command { name, args, .. } => {
            bump_cmd_like(out, name);
            count_args(args, out);
        }
        SyntaxNode::Infix {
            name,
            args,
            left,
            right,
        } => {
            bump_cmd_like(out, name);
            count_args(args, out);
            count_node(left, out);
            count_node(right, out);
        }
        SyntaxNode::Declarative { name, args } => {
            bump_cmd_like(out, name);
            count_args(args, out);
        }
        SyntaxNode::Environment {
            name, args, body, ..
        } => {
            out.bump(Kind::Env, name);
            count_args(args, out);
            count_node(body, out);
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            count_node(base, out);
            if let Some(sub) = subscript {
                count_node(sub, out);
            }
            if let Some(sup) = superscript {
                count_node(sup, out);
            }
        }
        SyntaxNode::Text(_)
        | SyntaxNode::Char(_)
        | SyntaxNode::ActiveSpace
        | SyntaxNode::Error { .. } => {}
    }
}

fn bump_cmd_like(out: &mut FormulaCounter, name: &str) {
    let has_cmd = ALL_PACKAGES
        .iter()
        .any(|pkg| pkg.commands.iter().any(|record| record.name == name));
    let has_char = ALL_PACKAGES
        .iter()
        .any(|pkg| pkg.characters.iter().any(|record| record.name == name));

    if has_cmd || !has_char {
        out.bump(Kind::Cmd, name);
    }
    if has_char {
        out.bump(Kind::Char, name);
    }
}

fn count_args(args: &[Option<Argument>], out: &mut FormulaCounter) {
    for slot in args {
        let Some(arg) = slot else { continue };
        match &arg.value {
            ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                count_node(node, out);
            }
            ArgumentValue::Delimiter(_)
            | ArgumentValue::CSName(_)
            | ArgumentValue::Dimension(_)
            | ArgumentValue::Integer(_)
            | ArgumentValue::KeyVal(_)
            | ArgumentValue::Column(_)
            | ArgumentValue::Boolean(_) => {}
        }
    }
}

/// Append-only row buffer used by the dump bin. `dataset` and `formula_id`
/// are supplied by the caller; target entries come from a `FormulaCounter`.
#[derive(Default)]
pub struct RowBuffer {
    dataset: Vec<String>,
    formula_id: Vec<String>,
    name: Vec<String>,
    kind: Vec<&'static str>,
    count: Vec<u32>,
}

impl RowBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.dataset.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dataset.is_empty()
    }

    /// Append all target counts under the given dataset and formula id.
    pub fn extend_from_counter(
        &mut self,
        dataset: &str,
        formula_id: &str,
        counter: &FormulaCounter,
    ) {
        for ((kind, name), count) in &counter.counts {
            self.dataset.push(dataset.to_string());
            self.formula_id.push(formula_id.to_string());
            self.name.push(name.clone());
            self.kind.push(kind.as_str());
            self.count.push(*count);
        }
    }

    pub fn merge(&mut self, other: RowBuffer) {
        self.dataset.extend(other.dataset);
        self.formula_id.extend(other.formula_id);
        self.name.extend(other.name);
        self.kind.extend(other.kind);
        self.count.extend(other.count);
    }

    fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("dataset", DataType::Utf8, false),
            Field::new("formula_id", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, false),
            Field::new("count", DataType::UInt32, false),
        ]))
    }

    fn into_record_batch(self) -> Result<RecordBatch, Box<dyn std::error::Error>> {
        let schema = Self::schema();
        let dataset: ArrayRef = Arc::new(StringArray::from(self.dataset));
        let formula_id: ArrayRef = Arc::new(StringArray::from(self.formula_id));
        let name: ArrayRef = Arc::new(StringArray::from(self.name));
        let kind: ArrayRef = Arc::new(StringArray::from(
            self.kind.into_iter().map(String::from).collect::<Vec<_>>(),
        ));
        let count: ArrayRef = Arc::new(UInt32Array::from(self.count));
        Ok(RecordBatch::try_new(
            schema,
            vec![dataset, formula_id, name, kind, count],
        )?)
    }

    /// Write the buffer to `path` as a single zstd-compressed parquet file.
    pub fn write_parquet(self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let schema = Self::schema();
        let batch = self.into_record_batch()?;
        let file = File::create(path)?;
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
            .build();
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
        writer.write(&batch)?;
        writer.close()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use texform_core::api;

    fn count_formula(src: &str) -> FormulaCounter {
        let output = api::parse_latex(src, false);
        let result = output.result.expect("parse_latex returned no result");
        let mut counter = FormulaCounter::default();
        count_node(&result.node, &mut counter);
        counter
    }

    #[test]
    fn empty_for_pure_chars() {
        let counter = count_formula("1 + 2");
        assert!(
            counter.is_empty(),
            "expected empty, got {:?}",
            counter.counts
        );
    }

    #[test]
    fn counts_command_in_args() {
        let counter = count_formula(r"\frac{a}{b}");
        assert_eq!(counter.counts.get(&(Kind::Cmd, "frac".into())), Some(&1));
    }

    #[test]
    fn counts_infix_command() {
        let counter = count_formula(r"{a \over b}");
        assert_eq!(counter.counts.get(&(Kind::Cmd, "over".into())), Some(&1));
    }

    #[test]
    fn counts_environment() {
        let counter = count_formula(r"\begin{matrix}a & b\\c & d\end{matrix}");
        assert_eq!(counter.counts.get(&(Kind::Env, "matrix".into())), Some(&1));
    }

    #[test]
    fn counts_nested_args_recursively() {
        let counter = count_formula(r"\sqrt{\frac{a}{b}}");
        assert_eq!(counter.counts.get(&(Kind::Cmd, "sqrt".into())), Some(&1));
        assert_eq!(counter.counts.get(&(Kind::Cmd, "frac".into())), Some(&1));
    }

    #[test]
    fn aggregates_repeated_names() {
        let counter = count_formula(r"\alpha + \alpha + \alpha");
        assert_eq!(counter.counts.get(&(Kind::Char, "alpha".into())), Some(&3));
    }

    #[test]
    fn row_buffer_extends_from_counter() {
        let mut counter = FormulaCounter::default();
        counter.bump(Kind::Cmd, "frac");
        counter.bump(Kind::Cmd, "frac");
        counter.bump(Kind::Env, "matrix");

        let mut buf = RowBuffer::new();
        buf.extend_from_counter("linxy", "abc123def456", &counter);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn row_buffer_round_trips_via_parquet() {
        let tmp = tempfile::Builder::new()
            .suffix(".parquet")
            .tempfile()
            .unwrap();

        let mut counter = FormulaCounter::default();
        counter.bump(Kind::Cmd, "frac");
        counter.bump(Kind::Env, "matrix");

        let mut buf = RowBuffer::new();
        buf.extend_from_counter("linxy", "abc123def456", &counter);
        buf.write_parquet(tmp.path()).unwrap();

        let file = std::fs::File::open(tmp.path()).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .unwrap()
            .build()
            .unwrap();
        let mut total_rows = 0_usize;
        for batch in reader {
            let batch = batch.unwrap();
            total_rows += batch.num_rows();
            let schema = batch.schema();
            let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
            assert_eq!(
                names,
                vec!["dataset", "formula_id", "name", "kind", "count"],
            );
        }
        assert_eq!(total_rows, 2);
    }
}
