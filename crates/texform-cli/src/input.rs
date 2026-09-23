//! Input sources: formulas from an argument, stdin, or stdin lines, and
//! `--config` overlays.

use std::fmt;
use std::fs;
use std::io::{self, BufRead, Read};
use std::ops::ControlFlow;

use serde::de::DeserializeOwned;
use texform::bindings::{format_read_error, read};

/// Where the formulas of `normalize`, `parse`, and `tokenize` come from.
#[derive(clap::Args)]
pub struct FormulaInput {
    /// Formula to process [default: all of stdin, minus one trailing newline]
    #[arg(value_name = "LATEX", allow_hyphen_values = true)]
    latex: Option<String>,

    /// Read one formula per stdin line; empty lines are empty formulas
    #[arg(long, conflicts_with = "latex")]
    lines: bool,
}

/// One formula and where it came from.
pub struct Formula<'a> {
    pub latex: &'a str,
    pub origin: Origin,
}

/// Formula origin, used to name the source in diagnostics.
#[derive(Clone, Copy)]
pub enum Origin {
    Argument,
    Stdin,
    /// One-based stdin line number under `--lines`.
    Line(usize),
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument => f.write_str("<argument>"),
            Self::Stdin => f.write_str("<stdin>"),
            Self::Line(number) => write!(f, "<line {number}>"),
        }
    }
}

impl FormulaInput {
    /// Whether every stdin line is a separate formula.
    pub fn per_line(&self) -> bool {
        self.lines
    }

    /// Call `visit` for each formula in input order until it breaks.
    ///
    /// Lines are streamed, so results appear while later input is still
    /// being read. An error means stdin could not be read or was not UTF-8;
    /// formulas before it have already been visited.
    pub fn for_each(
        &self,
        mut visit: impl FnMut(Formula<'_>) -> ControlFlow<()>,
    ) -> Result<(), String> {
        if let Some(latex) = &self.latex {
            let _ = visit(Formula {
                latex,
                origin: Origin::Argument,
            });
            return Ok(());
        }
        let mut stdin = io::stdin().lock();
        if !self.lines {
            let mut bytes = Vec::new();
            stdin
                .read_to_end(&mut bytes)
                .map_err(|error| format!("cannot read stdin: {error}"))?;
            let text =
                String::from_utf8(bytes).map_err(|_| "stdin is not valid UTF-8".to_owned())?;
            let _ = visit(Formula {
                latex: strip_newline(&text),
                origin: Origin::Stdin,
            });
            return Ok(());
        }
        let mut bytes = Vec::new();
        for number in 1.. {
            bytes.clear();
            let read = stdin
                .read_until(b'\n', &mut bytes)
                .map_err(|error| format!("cannot read stdin: {error}"))?;
            if read == 0 {
                break;
            }
            let line = std::str::from_utf8(&bytes)
                .map_err(|_| format!("stdin line {number} is not valid UTF-8"))?;
            let formula = Formula {
                latex: strip_newline(line),
                origin: Origin::Line(number),
            };
            if visit(formula).is_break() {
                break;
            }
        }
        Ok(())
    }
}

/// Remove one trailing `\n` or `\r\n`.
fn strip_newline(text: &str) -> &str {
    text.strip_suffix('\n')
        .map(|text| text.strip_suffix('\r').unwrap_or(text))
        .unwrap_or(text)
}

/// Read a `--config` value: inline JSON, or `@FILE` for a JSON file.
///
/// Unknown fields are rejected, and the error names the offending path.
pub fn read_config<T: DeserializeOwned + Default>(arg: Option<&str>) -> Result<T, String> {
    let Some(arg) = arg else {
        return Ok(T::default());
    };
    let text = match arg.strip_prefix('@') {
        Some(path) => fs::read_to_string(path)
            .map_err(|error| format!("cannot read --config file `{path}`: {error}"))?,
        None => arg.to_owned(),
    };
    let value = serde_json::from_str(&text)
        .map_err(|error| format!("invalid --config: not valid JSON: {error}"))?;
    read(value).map_err(|error| format_read_error(&error, "--config", str::to_owned))
}
