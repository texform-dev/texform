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

    while state.i < state.template.len() {
        if n > MAX_COLUMNS {
            return Err(ColumnParseError::MaxColumns);
        }
        n += 1;
        let c = state
            .next_char()
            .ok_or(ColumnParseError::MissingCloseBrace)?;
        state.c = c;
        handle_column_char(c, &mut state)?;
    }

    let mut spec = ColumnSpec::new(template.to_string(), state.template.clone());
    set_column_align(&state, &mut spec);
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
    i: usize,
    c: char,
    j: usize,
    calign: Vec<Option<ColumnAlign>>,
    cwidth: Vec<Option<String>>,
    cspace: Vec<Option<String>>,
    clines: Vec<Option<LineStyle>>,
    cstart: Vec<Option<String>>,
    cend: Vec<Option<String>>,
    cextra: Vec<bool>,
    ralign: Vec<Option<RowAlign>>,
}

impl ColumnState {
    fn new(template: &str) -> Self {
        ColumnState {
            template: template.to_string(),
            i: 0,
            c: '\0',
            j: 0,
            calign: Vec::new(),
            cwidth: Vec::new(),
            cspace: Vec::new(),
            clines: Vec::new(),
            cstart: Vec::new(),
            cend: Vec::new(),
            cextra: Vec::new(),
            ralign: Vec::new(),
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let rest = self.template.get(self.i..)?;
        let mut chars = rest.chars();
        let c = chars.next()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn peek_char(&self) -> Option<char> {
        self.template.get(self.i..)?.chars().next()
    }
}

fn handle_column_char(c: char, state: &mut ColumnState) -> Result<(), ColumnParseError> {
    match c {
        'l' => {
            set_calign(state, state.j, ColumnAlign::Left);
            state.j += 1;
            Ok(())
        }
        'c' => {
            set_calign(state, state.j, ColumnAlign::Center);
            state.j += 1;
            Ok(())
        }
        'r' => {
            set_calign(state, state.j, ColumnAlign::Right);
            state.j += 1;
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
            append_cstart(state, state.j, &value);
            Ok(())
        }
        '<' => {
            let idx = state.j.saturating_sub(1);
            let value = get_braces(state)?;
            append_cend(state, idx, &value);
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
    set_calign(state, state.j, align);
    set_option_string(&mut state.cwidth, state.j, width.clone());
    set_option(
        &mut state.ralign,
        state.j,
        RowAlign {
            vertical,
            width,
            align,
        },
    );
    state.j += 1;
    Ok(())
}

fn get_dimen(state: &mut ColumnState) -> Result<String, ColumnParseError> {
    let dim = get_braces(state)?;
    if !is_valid_dimension(&dim) {
        return Err(ColumnParseError::MissingColumnDimOrUnits(state.c));
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

    if state.i >= state.template.len() {
        return Err(ColumnParseError::MissingArgForColumn(state.c));
    }

    if state.peek_char() != Some('{') {
        return Ok(state.next_char().unwrap().to_string());
    }

    state.next_char(); // consume '{'
    let start = state.i;
    let mut braces = 1usize;

    while state.i < state.template.len() {
        let ch = state.next_char().unwrap();
        match ch {
            '\\' => {
                // Keep escaped content verbatim while skipping brace matching.
                if state.i < state.template.len() {
                    state.next_char();
                }
            }
            '{' => braces += 1,
            '}' => {
                braces -= 1;
                if braces == 0 {
                    let end = state.i - 1; // consumed '}' is one byte
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
    let rest = state.template[state.i..].to_string();
    state.template = format!("{expansion}{rest}");
    state.i = 0;
    Ok(())
}

fn add_rule(state: &mut ColumnState, style: LineStyle) {
    if get_option(&state.clines, state.j).is_some() {
        add_at(state, r"\,".to_string());
    }
    set_option(&mut state.clines, state.j, style);
    if get_option(&state.cspace, state.j).as_deref() == Some("0") {
        set_option_string(&mut state.cstart, state.j, r"\hspace{.5em}".to_string());
    }
}

fn add_at(state: &mut ColumnState, macro_text: String) {
    let j = state.j;
    set_cextra(state, j, true);
    set_calign(state, j, ColumnAlign::Center);

    if get_option(&state.clines, j).is_some() {
        if get_option(&state.cspace, j).as_deref() == Some(".5em") {
            if j > 0 {
                append_cstart(state, j - 1, r"\hspace{.25em}");
            }
        } else if get_option(&state.cspace, j).is_none() && j > 0 {
            append_cend(state, j - 1, r"\hspace{.5em}");
        }
    }

    set_option_string(&mut state.cstart, j, macro_text);
    set_option_string(&mut state.cspace, j, "0".to_string());
    state.j += 1;
    set_option_string(&mut state.cspace, state.j, "0".to_string());
}

fn add_bang(state: &mut ColumnState, macro_text: String) {
    let j = state.j;
    set_cextra(state, j, true);
    set_calign(state, j, ColumnAlign::Center);

    let prefix = if get_option(&state.cspace, j).as_deref() == Some("0")
        && get_option(&state.clines, j).is_some()
    {
        r"\hspace{.25em}"
    } else {
        ""
    };
    set_option_string(&mut state.cstart, j, format!("{prefix}{macro_text}"));
    if get_option(&state.cspace, j).is_none() {
        set_option_string(&mut state.cspace, j, ".5em".to_string());
    }

    state.j += 1;
    set_option_string(&mut state.cspace, state.j, ".5em".to_string());
}

fn repeat(state: &mut ColumnState) -> Result<(), ColumnParseError> {
    let num = get_braces(state)?;
    let cols = get_braces(state)?;
    let parsed = num.parse::<isize>().ok();
    if parsed.is_none() || parsed.unwrap() < 0 || parsed.unwrap().to_string() != num {
        return Err(ColumnParseError::ColArgNotNum);
    }
    let n = parsed.unwrap() as usize;
    let rest = state.template[state.i..].to_string();
    state.template = format!("{}{}", cols.repeat(n), rest);
    state.i = 0;
    Ok(())
}

fn substitute_args(args: &[String], text: &str) -> Result<String, ColumnParseError> {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            out.push(c);
            i += 1;
            if i < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c == '#' {
            i += 1;
            if i >= chars.len() {
                return Err(ColumnParseError::ColArgNotNum);
            }
            let k = chars[i];
            if k == '#' {
                out.push('#');
                i += 1;
                continue;
            }
            if !k.is_ascii_digit() || k == '0' {
                return Err(ColumnParseError::ColArgNotNum);
            }
            let idx = (k as u8 - b'1') as usize;
            if idx >= args.len() {
                return Err(ColumnParseError::ColArgNotNum);
            }
            out.push_str(&args[idx]);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }

    Ok(out)
}

fn set_column_align(state: &ColumnState, spec: &mut ColumnSpec) {
    spec.column_align = state
        .calign
        .iter()
        .map(|a| a.unwrap_or(ColumnAlign::Center))
        .collect();
}

fn set_column_widths(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.cwidth.iter().any(|w| w.is_some()) {
        return;
    }
    let mut widths = state.cwidth.clone();
    if widths.len() < state.calign.len() {
        widths.push(Some("auto".to_string()));
    }
    spec.column_width = widths
        .into_iter()
        .map(|w| w.unwrap_or_else(|| "auto".to_string()))
        .collect();
}

fn set_column_spacing(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.cspace.iter().any(|s| s.is_some()) {
        return;
    }
    let mut spacing = state.cspace.clone();
    if spacing.len() < state.calign.len() {
        spacing.push(Some("1em".to_string()));
    }
    spec.column_spacing = spacing
        .into_iter()
        .skip(1)
        .map(|s| s.unwrap_or_else(|| "1em".to_string()))
        .collect();
}

fn set_column_lines(state: &ColumnState, spec: &mut ColumnSpec) {
    if !state.clines.iter().any(|l| l.is_some()) {
        return;
    }
    let mut lines = state.clines.clone();
    if let Some(Some(style)) = lines.first().copied() {
        spec.frame.push(FrameLine {
            side: FrameSide::Left,
            style,
        });
    }
    if lines.len() > state.calign.len() {
        if let Some(Some(style)) = lines.pop() {
            spec.frame.push(FrameLine {
                side: FrameSide::Right,
                style,
            });
        }
    } else if lines.len() < state.calign.len() {
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
    if state.calign.is_empty() {
        return;
    }
    let left_extra = state.cextra.first().copied().unwrap_or(false);
    let i = state.calign.len() - 1;
    let right_extra = state.cextra.get(i).copied().unwrap_or(false);
    if !left_extra && !right_extra {
        return;
    }

    let left = get_option(&state.cspace, 0).unwrap_or_else(|| ".5em".to_string());
    let right = if right_extra {
        get_option(&state.cspace, i).unwrap_or_else(|| ".5em".to_string())
    } else {
        ".5em".to_string()
    };
    spec.array_padding = Some(ArrayPadding { left, right });
}

fn set_column_extras(state: &ColumnState, spec: &mut ColumnSpec) {
    let n = [
        state.calign.len(),
        state.cstart.len(),
        state.cend.len(),
        state.cextra.len(),
        state.ralign.len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    spec.column_start = (0..n)
        .map(|i| get_option(&state.cstart, i).unwrap_or_default())
        .collect();
    spec.column_end = (0..n)
        .map(|i| get_option(&state.cend, i).unwrap_or_default())
        .collect();
    spec.column_extra = (0..n)
        .map(|i| state.cextra.get(i).copied().unwrap_or(false))
        .collect();
    spec.row_align = (0..n).map(|i| get_option(&state.ralign, i)).collect();
}

fn is_valid_dimension(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;

    if matches!(chars.get(i), Some('+') | Some('-')) {
        i += 1;
    }

    let mut int_digits = 0usize;
    while matches!(chars.get(i), Some(c) if c.is_ascii_digit()) {
        i += 1;
        int_digits += 1;
    }

    let mut frac_digits = 0usize;
    if matches!(chars.get(i), Some('.') | Some(',')) {
        i += 1;
        while matches!(chars.get(i), Some(c) if c.is_ascii_digit()) {
            i += 1;
            frac_digits += 1;
        }
    }

    if int_digits == 0 && frac_digits == 0 {
        return false;
    }

    while matches!(chars.get(i), Some(c) if c.is_whitespace()) {
        i += 1;
    }

    let unit_start = i;
    while matches!(chars.get(i), Some(c) if c.is_ascii_alphabetic()) {
        i += 1;
    }
    if unit_start == i {
        return false;
    }
    let unit: String = chars[unit_start..i].iter().collect();
    if !matches!(
        unit.as_str(),
        "em" | "ex" | "pt" | "pc" | "px" | "in" | "cm" | "mm" | "mu"
    ) {
        return false;
    }

    while matches!(chars.get(i), Some(c) if c.is_whitespace()) {
        i += 1;
    }

    i == chars.len()
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

fn set_calign(state: &mut ColumnState, index: usize, value: ColumnAlign) {
    set_option(&mut state.calign, index, value);
}

fn set_cextra(state: &mut ColumnState, index: usize, value: bool) {
    if state.cextra.len() <= index {
        state.cextra.resize(index + 1, false);
    }
    state.cextra[index] = value;
}

fn get_option<T: Clone>(vec: &[Option<T>], index: usize) -> Option<T> {
    vec.get(index).and_then(|v| v.clone())
}

fn append_cstart(state: &mut ColumnState, index: usize, value: &str) {
    let mut cur = get_option(&state.cstart, index).unwrap_or_default();
    cur.push_str(value);
    set_option_string(&mut state.cstart, index, cur);
}

fn append_cend(state: &mut ColumnState, index: usize, value: &str) {
    let mut cur = get_option(&state.cend, index).unwrap_or_default();
    cur.push_str(value);
    set_option_string(&mut state.cend, index, cur);
}
