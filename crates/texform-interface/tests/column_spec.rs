use texform_interface::column::{ColumnAlign, ColumnSpec, LineStyle};

#[test]
fn column_spec_to_string_uses_normalized_template() {
    let spec = ColumnSpec::new(" c|c ".to_string(), "c|c".to_string());
    assert_eq!(spec.to_string(), "c|c");
}

#[test]
fn column_spec_json_roundtrip() {
    let mut spec = ColumnSpec::new("c|c".to_string(), "c|c".to_string());
    spec.column_align = vec![ColumnAlign::Center, ColumnAlign::Center];
    spec.column_lines = vec![LineStyle::Solid];
    spec.column_start = vec!["".to_string(), "".to_string()];
    spec.column_end = vec!["".to_string(), "".to_string()];
    spec.column_extra = vec![false, false];
    spec.row_align = vec![None, None];

    let json = serde_json::to_string(&spec).unwrap();
    let parsed: ColumnSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, spec);
}
