//! Counter aggregation for proposal impact evaluation.
//!
//! Walks a parsed `SyntaxNode` and counts occurrences of each `(kind, mode, name)`
//! tuple where `kind` is "cmd", "env", or "char". Command-like nodes
//! contribute as command or character targets according to the builtin specs;
//! environment nodes contribute as environment targets.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use texform_interface::syntax_node::ContentMode;

pub use texform_core::target_counter::{
    TargetCounter as FormulaCounter, TargetCounterKey, TargetKind as Kind, count_node,
};

fn mode_str(mode: ContentMode) -> &'static str {
    match mode {
        ContentMode::Math => "math",
        ContentMode::Text => "text",
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
    mode: Vec<&'static str>,
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
        for (key, count) in &counter.counts {
            self.dataset.push(dataset.to_string());
            self.formula_id.push(formula_id.to_string());
            self.name.push(key.name.clone());
            self.kind.push(key.kind.as_str());
            self.mode.push(mode_str(key.mode));
            self.count.push(*count);
        }
    }

    pub fn merge(&mut self, other: RowBuffer) {
        self.dataset.extend(other.dataset);
        self.formula_id.extend(other.formula_id);
        self.name.extend(other.name);
        self.kind.extend(other.kind);
        self.mode.extend(other.mode);
        self.count.extend(other.count);
    }

    fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("dataset", DataType::Utf8, false),
            Field::new("formula_id", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, false),
            Field::new("mode", DataType::Utf8, false),
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
        let mode: ArrayRef = Arc::new(StringArray::from(
            self.mode.into_iter().map(String::from).collect::<Vec<_>>(),
        ));
        let count: ArrayRef = Arc::new(UInt32Array::from(self.count));
        Ok(RecordBatch::try_new(
            schema,
            vec![dataset, formula_id, name, kind, mode, count],
        )?)
    }
}

/// Streaming parquet writer for `RowBuffer` chunks.
pub struct ParquetRowWriter {
    writer: ArrowWriter<File>,
    rows_written: usize,
}

impl ParquetRowWriter {
    pub fn try_new(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let schema = RowBuffer::schema();
        let file = File::create(path)?;
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
            .build();
        let writer = ArrowWriter::try_new(file, schema, Some(props))?;
        Ok(Self {
            writer,
            rows_written: 0,
        })
    }

    pub fn write_buffer(&mut self, buffer: RowBuffer) -> Result<(), Box<dyn std::error::Error>> {
        if buffer.is_empty() {
            return Ok(());
        }
        let batch = buffer.into_record_batch()?;
        self.write_batch(&batch)
    }

    // ArrowWriter cuts row groups at its configured `max_row_group_size` (default 1M rows),
    // so callers only need to feed batches.
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), Box<dyn std::error::Error>> {
        self.rows_written += batch.num_rows();
        self.writer.write(batch)?;
        Ok(())
    }

    pub fn finish(self) -> Result<usize, Box<dyn std::error::Error>> {
        self.writer.close()?;
        Ok(self.rows_written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use texform_core::parse::{ParseConfig, ParseContext};

    fn key(kind: Kind, mode: ContentMode, name: &str) -> TargetCounterKey {
        TargetCounterKey {
            kind,
            mode,
            name: name.to_string(),
        }
    }

    fn count_formula(src: &str) -> FormulaCounter {
        let output = ParseContext::shared().parse(src, &ParseConfig::default());
        let result = output
            .try_into_document()
            .expect("parser returned no result")
            .0;
        let mut counter = FormulaCounter::default();
        count_node(&result.to_syntax(), &mut counter);
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
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Cmd, ContentMode::Math, "frac")),
            Some(&1)
        );
    }

    #[test]
    fn counts_infix_command() {
        let counter = count_formula(r"{a \over b}");
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Cmd, ContentMode::Math, "over")),
            Some(&1)
        );
    }

    #[test]
    fn counts_environment() {
        let counter = count_formula(r"\begin{matrix}a & b\\c & d\end{matrix}");
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Env, ContentMode::Math, "matrix")),
            Some(&1)
        );
    }

    #[test]
    fn counts_nested_args_recursively() {
        let counter = count_formula(r"\sqrt{\frac{a}{b}}");
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Cmd, ContentMode::Math, "sqrt")),
            Some(&1)
        );
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Cmd, ContentMode::Math, "frac")),
            Some(&1)
        );
    }

    #[test]
    fn aggregates_repeated_names() {
        let counter = count_formula(r"\alpha + \alpha + \alpha");
        assert_eq!(
            counter
                .counts
                .get(&key(Kind::Char, ContentMode::Math, "alpha")),
            Some(&3)
        );
    }

    #[test]
    fn separates_same_command_by_content_mode() {
        let counter = count_formula(r"\bf{x} + \text{\bf y}");
        assert_eq!(
            counter.counts.get(&key(Kind::Cmd, ContentMode::Math, "bf")),
            Some(&1)
        );
        assert_eq!(
            counter.counts.get(&key(Kind::Cmd, ContentMode::Text, "bf")),
            Some(&1)
        );
    }

    #[test]
    fn row_buffer_extends_from_counter() {
        let mut counter = FormulaCounter::default();
        counter.bump(Kind::Cmd, ContentMode::Math, "frac");
        counter.bump(Kind::Cmd, ContentMode::Math, "frac");
        counter.bump(Kind::Env, ContentMode::Math, "matrix");

        let mut buf = RowBuffer::new();
        buf.extend_from_counter("linxy", "abc123def456", &counter);
        assert_eq!(buf.len(), 2);

        let frac_row = buf
            .name
            .iter()
            .position(|name| name == "frac")
            .expect("expected frac row");
        assert_eq!(buf.dataset[frac_row], "linxy");
        assert_eq!(buf.formula_id[frac_row], "abc123def456");
        assert_eq!(buf.kind[frac_row], "cmd");
        assert_eq!(buf.mode[frac_row], "math");
        assert_eq!(buf.count[frac_row], 2);

        let matrix_row = buf
            .name
            .iter()
            .position(|name| name == "matrix")
            .expect("expected matrix row");
        assert_eq!(buf.dataset[matrix_row], "linxy");
        assert_eq!(buf.formula_id[matrix_row], "abc123def456");
        assert_eq!(buf.kind[matrix_row], "env");
        assert_eq!(buf.mode[matrix_row], "math");
        assert_eq!(buf.count[matrix_row], 1);
    }

    #[test]
    fn row_buffer_round_trips_via_parquet() {
        let tmp = tempfile::Builder::new()
            .suffix(".parquet")
            .tempfile()
            .unwrap();

        let mut counter = FormulaCounter::default();
        counter.bump(Kind::Cmd, ContentMode::Math, "frac");
        counter.bump(Kind::Env, ContentMode::Math, "matrix");

        let mut buf = RowBuffer::new();
        buf.extend_from_counter("linxy", "abc123def456", &counter);
        let mut writer = ParquetRowWriter::try_new(tmp.path()).unwrap();
        writer.write_buffer(buf).unwrap();
        writer.finish().unwrap();

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
                vec!["dataset", "formula_id", "name", "kind", "mode", "count"],
            );
        }
        assert_eq!(total_rows, 2);
    }
}
