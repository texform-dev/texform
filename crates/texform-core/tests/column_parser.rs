use texform_core::column_parser::{ColumnParseError, parse_column_template};
use texform_interface::column::{ColumnAlign, LineStyle, VerticalAlign};

// MathJax references:
// - testsuite: references/mathjax/testsuite/tests/input/tex/Tex.test.ts:604
// - testsuite: references/mathjax/testsuite/tests/input/tex/Base.test.ts:3493
// - testsuite: references/mathjax/testsuite/tests/input/tex/Base.test.ts:3563
// - testsuite: references/mathjax/testsuite/tests/input/tex/Base.test.ts:13775

#[test]
fn mjx_ref_columnlines_solid_none() {
    let spec = parse_column_template("c|cc").unwrap();
    assert_eq!(
        spec.column_align,
        vec![
            ColumnAlign::Center,
            ColumnAlign::Center,
            ColumnAlign::Center
        ]
    );
    assert_eq!(spec.column_lines, vec![LineStyle::Solid, LineStyle::None]);
}

#[test]
fn mjx_ref_column_gt_space() {
    let spec = parse_column_template("> {x} c").unwrap();
    assert_eq!(spec.column_align, vec![ColumnAlign::Center]);
    assert_eq!(spec.column_start[0], "x");
}

#[test]
fn mjx_ref_bad_pream_token() {
    let err = parse_column_template("a").unwrap_err();
    assert_eq!(err, ColumnParseError::BadPreamToken('a'));
    assert_eq!(err.to_string(), "Illegal pream-token (a)");
}

#[test]
fn mjx_ref_missing_close_brace() {
    let err = parse_column_template("@{x").unwrap_err();
    assert_eq!(err, ColumnParseError::MissingCloseBrace);
    assert_eq!(err.to_string(), "Missing close brace");
}

#[test]
fn mjx_ref_bad_dimension() {
    let err = parse_column_template("cp{xyz}c").unwrap_err();
    assert_eq!(err, ColumnParseError::MissingColumnDimOrUnits('p'));
}

#[test]
fn dimension_columns_accept_shared_unit_set_and_reject_unknown_units() {
    for unit in ["em", "ex", "pt", "pc", "px", "in", "cm", "mm", "mu"] {
        let template = format!("p{{1{unit}}}");
        let spec = parse_column_template(&template).unwrap();
        let expected = format!("1{unit}");
        assert_eq!(spec.column_width[0], expected);
    }

    let err = parse_column_template("p{1zz}").unwrap_err();
    assert_eq!(err, ColumnParseError::MissingColumnDimOrUnits('p'));
}

#[test]
fn mjx_ref_missing_argument() {
    let err = parse_column_template("c@").unwrap_err();
    assert_eq!(err, ColumnParseError::MissingArgForColumn('@'));
}

#[test]
fn mjx_ref_bad_star_argument() {
    let err = parse_column_template("*{x}{x}").unwrap_err();
    assert_eq!(err, ColumnParseError::ColArgNotNum);
}

#[test]
fn repeat_and_macro_column() {
    let repeated = parse_column_template("*{2}{c|}").unwrap();
    assert_eq!(repeated.column_align.len(), 2);
    assert_eq!(repeated.column_lines, vec![LineStyle::Solid]);

    let macro_p = parse_column_template("P{1em}").unwrap();
    assert_eq!(macro_p.column_align, vec![ColumnAlign::Left]);
    assert_eq!(macro_p.row_align.len(), 1);
    let row = macro_p.row_align[0].as_ref().unwrap();
    assert_eq!(row.vertical, VerticalAlign::Top);
    assert_eq!(row.width, "1em");
    assert_eq!(row.align, ColumnAlign::Left);
    assert_eq!(macro_p.column_start[0], "$");
    assert_eq!(macro_p.column_end[0], "$");
    assert_eq!(macro_p.to_string(), ">{$}p{1em}<{$}");
}

#[test]
fn too_many_columns_guard() {
    let err = parse_column_template("*{10001}{c}").unwrap_err();
    assert_eq!(err, ColumnParseError::MaxColumns);
}

#[test]
fn at_and_bang_columns_preserve_spacing_metadata() {
    let spec = parse_column_template("c@{x}!{y}c").unwrap();

    assert_eq!(spec.column_align.len(), 4);
    assert_eq!(spec.column_spacing, vec!["0", "0", ".5em"]);
    assert_eq!(spec.column_start, vec!["", "x", "y", ""]);
    assert_eq!(spec.column_extra, vec![false, true, true, false]);
}
