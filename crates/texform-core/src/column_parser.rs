//! Column template parser aligned with MathJax ColumnParser semantics.
//!
//! This module parses array column templates like `c|c|c` into a structured
//! `ColumnSpec` value.
//!
//! Note:
//! - Built-in column handlers are implemented.
//! - `\newcolumntype` runtime extensions are intentionally not supported.

use std::fmt;

use texform_interface::column::{
    ArrayPadding, ColumnAlign, ColumnSpec, FrameLine, FrameSide, LineStyle, RowAlign, VerticalAlign,
};

const MAX_COLUMNS: usize = 10000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnParseError {
    MaxColumns,
    BadPreamToken(char),
    MissingColumnDimOrUnits(char),
    MissingArgForColumn(char),
    MissingCloseBrace,
    ColArgNotNum,
}

impl fmt::Display for ColumnParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnParseError::MaxColumns => {
                write!(
                    f,
                    "Too many column specifiers (perhaps looping column definitions?)"
                )
            }
            ColumnParseError::BadPreamToken(c) => write!(f, "Illegal pream-token ({c})"),
            ColumnParseError::MissingColumnDimOrUnits(c) => write!(
                f,
                "Missing dimension or its units for {c} column declaration"
            ),
            ColumnParseError::MissingArgForColumn(c) => {
                write!(f, "Missing argument for {c} column declaration")
            }
            ColumnParseError::MissingCloseBrace => write!(f, "Missing close brace"),
            ColumnParseError::ColArgNotNum => {
                write!(f, "First argument to * column specifier must be a number")
            }
        }
    }
}

pub fn parse_column_template(template: &str) -> Result<ColumnSpec, ColumnParseError> {
    let mut state = ColumnState::new(template);
    let mut n = 0usize;

    while state.cursor < state.template.len() {
        if n > MAX_COLUMNS {
            return Err(ColumnParseError::MaxColumns);
        }
        n += 1;
        let current_char = state
            .next_char()
            .ok_or(ColumnParseError::MissingCloseBrace)?;
        state.current_char = current_char;
        handle_column_char(current_char, &mut state)?;
    }

    let mut spec = ColumnSpec::new(template.to_string(), state.template.clone());
    set_column_aligns(&state, &mut spec);
    set_column_widths(&state, &mut spec);
    set_column_spacing(&state, &mut spec);
    set_column_lines(&state, &mut spec);
    set_padding(&state, &mut spec);
    set_column_extras(&state, &mut spec);

    Ok(spec)
}

#[derive(Clone)]
struct ColumnState {
    template: String,
    cursor: usize,
    current_char: char,
    column_index: usize,
    column_aligns: Vec<Option<ColumnAlign>>,
    column_widths: Vec<Option<String>>,
    column_spacing: Vec<Option<String>>,
    column_lines: Vec<Option<LineStyle>>,
    column_starts: Vec<Option<String>>,
    column_ends: Vec<Option<String>>,
    column_extras: Vec<bool>,
    row_aligns: Vec<Option<RowAlign>>,
}

impl ColumnState {
    fn new(template: &str) -> Self {
        ColumnState {
            template: template.to_string(),
            cursor: 0,
            current_char: '\0',
            column_index: 0,
            column_aligns: Vec::new(),
            column_widths: Vec::new(),
            column_spacing: Vec::new(),
            column_lines: Vec::new(),
            column_starts: Vec::new(),
            column_ends: Vec::new(),
            column_extras: Vec::new(),
            row_aligns: Vec::new(),
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let rest = self.template.get(self.cursor..)?;
        let mut chars = rest.chars();
        let current_char = chars.next()?;
        self.cursor += current_char.len_utf8();
        Some(current_char)
    }

    fn peek_char(&self) -> Option<char> {
        self.template.get(self.cursor..)?.chars().next()
    }
}

fn handle_column_char(c: char, state: &mut ColumnState) -> Result<(), ColumnParseError> {
    match c {
        'l' => {
            set_column_align(state, state.column_index, ColumnAlign::Left);
            state.column_index += 1;
            Ok(())
        }
        'c' => {
            set_column_align(state, state.column_index, ColumnAlign::Center);
            state.column_index += 1;
            Ok(())
        }
        'r' => {
            set_column_align(state, state.column_index, ColumnAlign::Right);
            state.column_index += 1;
            Ok(())
        }
        'p' => get_column(state, VerticalAlign::Top, Some(ColumnAlign::Left)),
        'm' => get_column(state, VerticalAlign::Middle, Some(ColumnAlign::Left)),
        'b' => get_column(state, VerticalAlign::Bottom, Some(ColumnAlign::Left)),
        'w' | 'W' => get_column(state, VerticalAlign::Top, None),
        '|' => {
            add_rule(state, LineStyle::Solid);
            Ok(())
        }
        ':' => {
            add_rule(state, LineStyle::Dashed);
            Ok(())
        }
        '>' => {
            let value = get_braces(state)?;
            append_column_start(state, state.column_index, &value);
            Ok(())
        }
        '<' => {
            let idx = state.column_index.saturating_sub(1);
            let value = get_braces(state)?;
            append_column_end(state, idx, &value);
            Ok(())
        }
        '@' => {
            let value = get_braces(state)?;
            add_at(state, value);
            Ok(())
        }
        '!' => {
            let value = get_braces(state)?;
            add_bang(state, value);
            Ok(())
        }
        '*' => repeat(state),
        'P' => macro_column(state, ">{$}p{#1}<{$}", 1),
        'M' => macro_column(state, ">{$}m{#1}<{$}", 1),
        'B' => macro_column(state, ">{$}b{#1}<{$}", 1),
        ' ' => Ok(()),
        _ => Err(ColumnParseError::BadPreamToken(c)),
    }
}

fn get_column(
    state: &mut ColumnState,
    vertical: VerticalAlign,
    default_align: Option<ColumnAlign>,
) -> Result<(), ColumnParseError> {
    let align = if let Some(align) = default_align {
        align
    } else {
        get_align(state)?
    };
    let width = get_dimen(state)?;
    set_column_align(state, state.column_index, align);
    set_option_string(&mut state.column_widths, state.column_index, width.clone());
    set_option(
        &mut state.row_aligns,
        state.column_index,
        RowAlign {
            vertical,
            width,
            align,
        },
    );
    state.column_index += 1;
    Ok(())
}

fn get_dimen(state: &mut ColumnState) -> Result<String, ColumnParseError> {
    let dim = get_braces(state)?;
    if !is_valid_dimension(&dim) {
        return Err(ColumnParseError::MissingColumnDimOrUnits(
            state.current_char,
        ));
    }
    Ok(dim)
}

fn get_align(state: &mut ColumnState) -> Result<ColumnAlign, ColumnParseError> {
    let align = get_braces(state)?;
    let lowered = align.to_lowercase();
    Ok(match lowered.as_str() {
        "l" => ColumnAlign::Left,
        "c" => ColumnAlign::Center,
        "r" => ColumnAlign::Right,
        _ => ColumnAlign::Empty,
    })
}

fn get_braces(state: &mut ColumnState) -> Result<String, ColumnParseError> {
    while matches!(state.peek_char(), Some(' ')) {
        state.next_char();
    }

    if state.cursor >= state.template.len() {
        return Err(ColumnParseError::MissingArgForColumn(state.current_char));
    }

    if state.peek_char() != Some('{') {
        return Ok(state.next_char().unwrap().to_string());
    }

    state.next_char(); // consume '{'
    let start = state.cursor;
    let mut braces = 1usize;

    while state.cursor < state.template.len() {
        let ch = state.next_char().unwrap();
        match ch {
            '\\' => {
                // Keep escaped content verbatim while skipping brace matching.
                if state.cursor < state.template.len() {
                    state.next_char();
                }
            }
            '{' => braces += 1,
            '}' => {
                braces -= 1;
                if braces == 0 {
                    let end = state.cursor - 1; // consumed '}' is one byte
                    return Ok(state.template[start..end].to_string());
                }
            }
            _ => {}
        }
    }

    Err(ColumnParseError::MissingCloseBrace)
}

fn macro_column(
    state: &mut ColumnState,
    macro_template: &str,
    n: usize,
) -> Result<(), ColumnParseError> {
    let mut args = Vec::with_capacity(n);
    for _ in 0..n {
        args.push(get_braces(state)?);
    }
    let expansion = substitute_args(&args, macro_template)?;
    let rest = state.template[state.cursor..].to_string();
    state.template = format!("{expansion}{rest}");
    state.cursor = 0;
    Ok(())
}

fn add_rule(state: &mut ColumnState, style: LineStyle) {
    if get_option(&state.column_lines, state.column_index).is_some() {
        add_at(state, r"\,".to_string());
    }
    set_option(&mut state.column_lines, state.column_index, style);
    if get_option(&state.column_spacing, state.column_index).as_deref() == Some("0") {
        set_option_string(
            &mut state.column_starts,
            state.column_index,
            r"\hspace{.5em}".to_string(),
        );
    }
}

fn add_at(state: &mut ColumnState, macro_text: String) {
    let column_index = state.column_index;
    set_column_extra(state, column_index, true);
    set_column_align(state, column_index, ColumnAlign::Center);

    if get_option(&state.column_lines, column_index).is_some() {
        if get_option(&state.column_spacing, column_index).as_deref() == Some(".5em") {
            if column_index > 0 {
                append_column_start(state, column_index - 1, r"\hspace{.25em}");
            }
        } else if get_option(&state.column_spacing, column_index).is_none() && column_index > 0 {
            append_column_end(state, column_index - 1, r"\hspace{.5em}");
        }
    }

    set_option_string(&mut state.column_starts, column_index, macro_text);
    set_option_string(&mut state.column_spacing, column_index, "0".to_string());
    state.column_index += 1;
    set_option_string(
        &mut state.column_spacing,
        state.column_index,
        "0".to_string(),
    );
}

fn add_bang(state: &mut ColumnState, macro_text: String) {
    let column_index = state.column_index;
    set_column_extra(state, column_index, true);
    set_column_align(state, column_index, ColumnAlign::Center);

    let prefix = if get_option(&state.column_spacing, column_index).as_deref() == Some("0")
        && get_option(&state.column_lines, column_index).is_some()
    {
        r"\hspace{.25em}"
    } else {
        ""
    };
    set_option_string(
        &mut state.column_starts,
        column_index,
        format!("{prefix}{macro_text}"),
    );
    if get_option(&state.column_spacing, column_index).is_none() {
        set_option_string(&mut state.column_spacing, column_index, ".5em".to_string());
    }

    state.column_index += 1;
    set_option_string(
        &mut state.column_spacing,
        state.column_index,
        ".5em".to_string(),
    );
}

fn repeat(state: &mut ColumnState) -> Result<(), ColumnParseError> {
    let num = get_braces(state)?;
    let cols = get_braces(state)?;
    let parsed = num.parse::<isize>().ok();
    if parsed.is_none() || parsed.unwrap() < 0 || parsed.unwrap().to_string() != num {
        return Err(ColumnParseError::ColArgNotNum);
    }
    let n = parsed.unwrap() as usize;
    let rest = state.template[state.cursor..].to_string();
    state.template = format!("{}{}", cols.repeat(n), rest);
    state.cursor = 0;
    Ok(())
}

fn substitute_args(args: &[String], text: &str) -> Result<String, ColumnParseError> {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut cursor = 0usize;

    while cursor < chars.len() {
        let current_char = chars[cursor];
        if current_char == '\\' {
            out.push(current_char);
            cursor += 1;
            if cursor < chars.len() {
                out.push(chars[cursor]);
                cursor += 1;
            }
            continue;
        }
        if current_char == '#' {
            cursor += 1;
            if cursor >= chars.len() {
                return Err(ColumnParseError::ColArgNotNum);
            }
            let marker = chars[cursor];
            if marker == '#' {
                out.push('#');
                cursor += 1;
                continue;
            }
            if !marker.is_ascii_digit() || marker == '0' {
                return Err(ColumnParseError::ColArgNotNum);
            }
            let idx = (marker as u8 - b'1') as usize;
            if idx >= args.len() {
                return Err(ColumnParseError::ColArgNotNum);
            }
            out.push_str(&args[idx]);
            cursor += 1;
            continue;
        }
        out.push(current_char);
        cursor += 1;
    }

    Ok(out)
}

fn set_column_aligns(state: &ColumnState, spec: &mut ColumnSpec) {
    spec.column_align = state
        .column_aligns
        .iter()
        .map(|a| a.unwrap_or(ColumnAlign::Center))
        .collect();
}

fn set_column_widths(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.column_widths.iter().any(|w| w.is_some()) {
        return;
    }
    let mut widths = state.column_widths.clone();
    if widths.len() < state.column_aligns.len() {
        widths.push(Some("auto".to_string()));
    }
    spec.column_width = widths
        .into_iter()
        .map(|w| w.unwrap_or_else(|| "auto".to_string()))
        .collect();
}

fn set_column_spacing(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.column_spacing.iter().any(|s| s.is_some()) {
        return;
    }
    let mut spacing = state.column_spacing.clone();
    if spacing.len() < state.column_aligns.len() {
        spacing.push(Some("1em".to_string()));
    }
    spec.column_spacing = spacing
        .into_iter()
        .skip(1)
        .map(|s| s.unwrap_or_else(|| "1em".to_string()))
        .collect();
}

fn set_column_lines(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.column_lines.iter().any(|l| l.is_some()) {
        return;
    }
    let mut lines = state.column_lines.clone();
    if let Some(Some(style)) = lines.first().copied() {
        spec.frame.push(FrameLine {
            side: FrameSide::Left,
            style,
        });
    }
    if lines.len() > state.column_aligns.len() {
        if let Some(Some(style)) = lines.pop() {
            spec.frame.push(FrameLine {
                side: FrameSide::Right,
                style,
            });
        }
    } else if lines.len() < state.column_aligns.len() {
        lines.push(Some(LineStyle::None));
    }
    if lines.len() > 1 {
        spec.column_lines = lines
            .into_iter()
            .skip(1)
            .map(|l| l.unwrap_or(LineStyle::None))
            .collect();
    }
}

fn set_padding(state: &ColumnState, spec: &mut ColumnSpec) {
    if state.column_aligns.is_empty() {
        return;
    }
    let left_extra = state.column_extras.first().copied().unwrap_or(false);
    let last_column_index = state.column_aligns.len() - 1;
    let right_extra = state
        .column_extras
        .get(last_column_index)
        .copied()
        .unwrap_or(false);
    if !left_extra && !right_extra {
        return;
    }

    let left = get_option(&state.column_spacing, 0).unwrap_or_else(|| ".5em".to_string());
    let right = if right_extra {
        get_option(&state.column_spacing, last_column_index).unwrap_or_else(|| ".5em".to_string())
    } else {
        ".5em".to_string()
    };
    spec.array_padding = Some(ArrayPadding { left, right });
}

fn set_column_extras(state: &ColumnState, spec: &mut ColumnSpec) {
    let n = [
        state.column_aligns.len(),
        state.column_starts.len(),
        state.column_ends.len(),
        state.column_extras.len(),
        state.row_aligns.len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    spec.column_start = (0..n)
        .map(|index| get_option(&state.column_starts, index).unwrap_or_default())
        .collect();
    spec.column_end = (0..n)
        .map(|index| get_option(&state.column_ends, index).unwrap_or_default())
        .collect();
    spec.column_extra = (0..n)
        .map(|index| state.column_extras.get(index).copied().unwrap_or(false))
        .collect();
    spec.row_align = (0..n)
        .map(|index| get_option(&state.row_aligns, index))
        .collect();
}

fn is_valid_dimension(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }

    let chars: Vec<char> = s.chars().collect();
    let mut cursor = 0usize;

    if matches!(chars.get(cursor), Some('+') | Some('-')) {
        cursor += 1;
    }

    let mut int_digits = 0usize;
    while matches!(chars.get(cursor), Some(ch) if ch.is_ascii_digit()) {
        cursor += 1;
        int_digits += 1;
    }

    let mut frac_digits = 0usize;
    if matches!(chars.get(cursor), Some('.') | Some(',')) {
        cursor += 1;
        while matches!(chars.get(cursor), Some(ch) if ch.is_ascii_digit()) {
            cursor += 1;
            frac_digits += 1;
        }
    }

    if int_digits == 0 && frac_digits == 0 {
        return false;
    }

    while matches!(chars.get(cursor), Some(ch) if ch.is_whitespace()) {
        cursor += 1;
    }

    let unit_start = cursor;
    while matches!(chars.get(cursor), Some(ch) if ch.is_ascii_alphabetic()) {
        cursor += 1;
    }
    if unit_start == cursor {
        return false;
    }
    let unit: String = chars[unit_start..cursor].iter().collect();
    if !matches!(
        unit.as_str(),
        "em" | "ex" | "pt" | "pc" | "px" | "in" | "cm" | "mm" | "mu"
    ) {
        return false;
    }

    while matches!(chars.get(cursor), Some(ch) if ch.is_whitespace()) {
        cursor += 1;
    }

    cursor == chars.len()
}

fn set_option<T: Clone>(vec: &mut Vec<Option<T>>, index: usize, value: T) {
    if vec.len() <= index {
        vec.resize(index + 1, None);
    }
    vec[index] = Some(value);
}

fn set_option_string(vec: &mut Vec<Option<String>>, index: usize, value: String) {
    set_option(vec, index, value);
}

fn set_column_align(state: &mut ColumnState, index: usize, value: ColumnAlign) {
    set_option(&mut state.column_aligns, index, value);
}

fn set_column_extra(state: &mut ColumnState, index: usize, value: bool) {
    if state.column_extras.len() <= index {
        state.column_extras.resize(index + 1, false);
    }
    state.column_extras[index] = value;
}

fn get_option<T: Clone>(vec: &[Option<T>], index: usize) -> Option<T> {
    vec.get(index).and_then(|v| v.clone())
}

fn append_column_start(state: &mut ColumnState, index: usize, value: &str) {
    let mut cur = get_option(&state.column_starts, index).unwrap_or_default();
    cur.push_str(value);
    set_option_string(&mut state.column_starts, index, cur);
}

fn append_column_end(state: &mut ColumnState, index: usize, value: &str) {
    let mut cur = get_option(&state.column_ends, index).unwrap_or_default();
    cur.push_str(value);
    set_option_string(&mut state.column_ends, index, cur);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_state_uses_descriptive_field_names() {
        let state = ColumnState::new("c");

        assert_eq!(state.template, "c");
        assert_eq!(state.cursor, 0);
        assert_eq!(state.current_char, '\0');
        assert_eq!(state.column_index, 0);
        assert!(state.column_aligns.is_empty());
        assert!(state.column_widths.is_empty());
        assert!(state.column_spacing.is_empty());
        assert!(state.column_lines.is_empty());
        assert!(state.column_starts.is_empty());
        assert!(state.column_ends.is_empty());
        assert!(state.column_extras.is_empty());
        assert!(state.row_aligns.is_empty());
    }

    #[test]
    fn descriptive_setters_fill_column_spec() {
        let mut state = ColumnState::new("c");
        state.column_aligns.push(Some(ColumnAlign::Center));
        state.column_widths.push(Some("2em".to_string()));

        let mut spec = ColumnSpec::new("c".to_string(), "c".to_string());
        set_column_aligns(&state, &mut spec);
        set_column_widths(&state, &mut spec);

        assert_eq!(spec.column_align, vec![ColumnAlign::Center]);
        assert_eq!(spec.column_width, vec!["2em"]);
    }
}
