use arrow_array::cast::AsArray;
use arrow_array::{Array, GenericStringArray, OffsetSizeTrait};
use arrow_schema::DataType;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const LFS_POINTER_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1";
const FORMULA_ID_COLUMN: &str = "formula_id";
const FORMULA_COLUMN: &str = "formula";
const RECORD_BATCH_SIZE: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaRecord {
    pub formula_id: String,
    pub formula: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DataFileStatus {
    Ready,
    Missing,
    LfsPointer,
}

pub fn check_data_file(path: &Path) -> DataFileStatus {
    if !path.exists() {
        return DataFileStatus::Missing;
    }

    let mut buffer = [0_u8; 128];
    if let Ok(mut file) = File::open(path)
        && let Ok(read) = file.read(&mut buffer)
        && buffer[..read].starts_with(LFS_POINTER_PREFIX)
    {
        return DataFileStatus::LfsPointer;
    }

    DataFileStatus::Ready
}

pub fn formula_row_count(path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    Ok(builder.metadata().file_metadata().num_rows() as usize)
}

pub fn read_formula_record_batches<F>(
    path: &Path,
    offset: usize,
    limit: Option<usize>,
    mut on_batch: F,
) -> Result<usize, Box<dyn std::error::Error>>
where
    F: FnMut(Vec<FormulaRecord>) -> Result<(), Box<dyn std::error::Error>>,
{
    // The parquet reader honors with_offset/with_limit exactly, so callers do not need
    // to re-check or truncate.
    let file = File::open(path)?;
    let mut builder =
        ParquetRecordBatchReaderBuilder::try_new(file)?.with_batch_size(RECORD_BATCH_SIZE);
    if offset > 0 {
        builder = builder.with_offset(offset);
    }
    if let Some(limit) = limit {
        builder = builder.with_limit(limit);
    }
    let reader = builder.build()?;

    let mut total_records = 0_usize;
    for batch in reader {
        let batch = batch?;
        let formula_id_column = batch.column_by_name(FORMULA_ID_COLUMN).ok_or_else(|| {
            format!(
                "column '{FORMULA_ID_COLUMN}' not found in {}",
                path.display()
            )
        })?;
        let formula_column = batch
            .column_by_name(FORMULA_COLUMN)
            .ok_or_else(|| format!("column '{FORMULA_COLUMN}' not found in {}", path.display()))?;

        let mut records = Vec::with_capacity(batch.num_rows());
        match (formula_id_column.data_type(), formula_column.data_type()) {
            (DataType::Utf8, DataType::Utf8) => collect_records(
                formula_id_column.as_string::<i32>(),
                formula_column.as_string::<i32>(),
                &mut records,
            ),
            (DataType::LargeUtf8, DataType::LargeUtf8) => collect_records(
                formula_id_column.as_string::<i64>(),
                formula_column.as_string::<i64>(),
                &mut records,
            ),
            (formula_id_type, formula_type) => {
                return Err(format!(
                    "columns '{FORMULA_ID_COLUMN}' and '{FORMULA_COLUMN}' in {} must both be Utf8 or LargeUtf8, got {formula_id_type:?} and {formula_type:?}",
                    path.display()
                )
                .into())
            }
        }

        if !records.is_empty() {
            total_records += records.len();
            on_batch(records)?;
        }
    }

    Ok(total_records)
}

fn collect_records<O: OffsetSizeTrait>(
    formula_ids: &GenericStringArray<O>,
    formulas: &GenericStringArray<O>,
    records: &mut Vec<FormulaRecord>,
) {
    debug_assert_eq!(formula_ids.len(), formulas.len());

    for index in 0..formula_ids.len() {
        if formula_ids.is_null(index) || formulas.is_null(index) {
            continue;
        }

        records.push(FormulaRecord {
            formula_id: formula_ids.value(index).to_string(),
            formula: formulas.value(index).to_string(),
        });
    }
}
