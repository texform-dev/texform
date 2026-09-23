//! Text and `--json` rendering, exit statuses, and diagnostics on stderr.

use std::any::Any;
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::ops::Range;
use std::process::ExitCode;

use ariadne::{Config, IndexType, Label, Report, ReportKind, Source};
use serde::Serialize;
use texform::bindings::{BindingErrorDto, normalize_error_to_parts};
use texform::{ParseDiagnostic, Span, SyntaxNode};

use crate::input::{Formula, Origin};

/// Exit status when at least one formula or query failed.
pub const FAILURE: u8 = 1;
/// Exit status for usage, configuration, and I/O errors: the request could
/// not be carried out, as opposed to formulas that failed.
pub const USAGE: u8 = 2;

/// Report a usage or configuration error and return its exit status.
pub fn usage_error(message: impl Display) -> ExitCode {
    eprint_line(format_args!("error: {message}"));
    ExitCode::from(USAGE)
}

/// Report a failure to read input or write output. Output may be incomplete,
/// so this is not a per-formula failure.
pub fn io_error(message: impl Display) -> ExitCode {
    usage_error(message)
}

/// Write one line to stderr. stderr is best effort: there is nowhere left to
/// report a failure to write it.
pub fn eprint_line(line: impl Display) {
    let _ = writeln!(io::stderr().lock(), "{line}");
}

/// Write one line to stdout, treating a closed pipe as success because the
/// reader stopped on purpose (for example `texform packages | head -1`).
pub fn print_line(line: impl Display) -> io::Result<()> {
    ignore_broken_pipe(writeln!(io::stdout().lock(), "{line}"))
}

fn ignore_broken_pipe(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

/// Serialize a value as one line of JSON.
pub fn json_line(value: &impl Serialize) -> String {
    // Output values are built from strings, numbers, booleans, options,
    // sequences, and string-keyed structs, which cannot fail to serialize.
    serde_json::to_string(value).expect("output values serialize to JSON")
}

/// A formula that was processed successfully, shaped for the output format.
pub enum Success {
    /// Text mode: the stdout line, plus parse diagnostics of the complete
    /// document to show as warnings.
    Text {
        line: String,
        warnings: Vec<ParseDiagnostic>,
    },
    /// JSON mode: the complete `{"ok":true,...}` line, built with [`ok_line`].
    Json(String),
}

/// `{"ok":true}` followed by the members of `fields`, as one JSON line.
pub fn ok_line(fields: impl Serialize) -> String {
    #[derive(Serialize)]
    struct OkLine<T> {
        ok: bool,
        #[serde(flatten)]
        fields: T,
    }
    json_line(&OkLine { ok: true, fields })
}

/// A formula that failed.
pub struct Failure {
    pub error: BindingErrorDto,
    /// Partial tree of an incomplete parse, reported by `parse --json`.
    /// Boxed to keep the `Err` variant of per-formula results small.
    pub syntax: Option<Box<SyntaxNode>>,
}

impl From<texform::Error> for Failure {
    fn from(error: texform::Error) -> Self {
        Self {
            error: normalize_error_to_parts(error).error,
            syntax: None,
        }
    }
}

impl Failure {
    /// `internal` failure for a formula whose processing panicked.
    pub fn panicked(payload: &(dyn Any + Send)) -> Self {
        Self {
            error: BindingErrorDto {
                kind: "internal",
                message: format!("internal panic: {}", panic_message(payload)),
                diagnostics: Vec::new(),
            },
            syntax: None,
        }
    }
}

/// Text of a panic payload.
pub fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

/// Writes per-formula results and remembers whether any formula failed.
pub struct Printer {
    json: bool,
    per_line: bool,
    color: bool,
    failed: bool,
}

impl Printer {
    pub fn new(json: bool, per_line: bool) -> Self {
        Self {
            json,
            per_line,
            color: stderr_color(),
            failed: false,
        }
    }

    /// Write the result of one formula.
    ///
    /// Every formula produces exactly one stdout line under `--lines` or
    /// `--json`, so output lines stay aligned with input lines. In text mode a
    /// failure writes an empty placeholder line and reports the error on
    /// stderr; a single formula that fails writes nothing to stdout.
    pub fn emit(
        &mut self,
        formula: &Formula<'_>,
        outcome: Result<Success, Failure>,
    ) -> io::Result<()> {
        let line = match outcome {
            Ok(Success::Text { line, warnings }) => {
                let mut stderr = io::stderr().lock();
                let _ = render_diagnostics(
                    &mut stderr,
                    formula,
                    &warnings,
                    ReportKind::Warning,
                    self.color,
                );
                Some(line)
            }
            Ok(Success::Json(line)) => Some(line),
            Err(failure) => {
                self.failed = true;
                if self.json {
                    Some(failure_line(&failure))
                } else {
                    self.report_failure(formula, &failure.error);
                    self.per_line.then(String::new)
                }
            }
        };
        match line {
            Some(line) => ignore_broken_pipe(writeln!(io::stdout().lock(), "{line}")),
            None => Ok(()),
        }
    }

    fn report_failure(&self, formula: &Formula<'_>, error: &BindingErrorDto) {
        let mut stderr = io::stderr().lock();
        let _ = match formula.origin {
            Origin::Line(number) => writeln!(stderr, "error: line {number}: {}", error.message),
            Origin::Argument | Origin::Stdin => writeln!(stderr, "error: {}", error.message),
        };
        let _ = render_diagnostics(
            &mut stderr,
            formula,
            &error.diagnostics,
            ReportKind::Error,
            self.color,
        );
    }

    /// Exit status for the formulas emitted so far.
    pub fn exit_code(&self) -> ExitCode {
        if self.failed {
            ExitCode::from(FAILURE)
        } else {
            ExitCode::SUCCESS
        }
    }
}

fn failure_line(failure: &Failure) -> String {
    #[derive(Serialize)]
    struct ErrorLine<'a> {
        ok: bool,
        error: &'a BindingErrorDto,
        #[serde(skip_serializing_if = "Option::is_none")]
        syntax: Option<&'a SyntaxNode>,
    }
    json_line(&ErrorLine {
        ok: false,
        error: &failure.error,
        syntax: failure.syntax.as_deref(),
    })
}

/// Colors are for people: only use them on a terminal, and honor `NO_COLOR`.
fn stderr_color() -> bool {
    io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
}

/// Render parse diagnostics against the formula source with `ariadne`.
fn render_diagnostics(
    out: &mut impl Write,
    formula: &Formula<'_>,
    diagnostics: &[ParseDiagnostic],
    kind: ReportKind<'_>,
    color: bool,
) -> io::Result<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    let name = formula.origin.to_string();
    let name = name.as_str();
    let source = Source::from(formula.latex);
    let config = Config::default()
        .with_index_type(IndexType::Byte)
        .with_color(color);
    let range = |span: &Span| clamp(span, formula.latex.len());
    for diagnostic in diagnostics {
        let span = range(&diagnostic.span);
        let mut report = Report::build(kind, (name, span.clone()))
            .with_config(config)
            .with_message(&diagnostic.message);
        if let Some(code) = diagnostic.kind {
            report = report.with_code(code.as_str());
        }
        let label = match &diagnostic.found {
            Some(found) => format!("found {found}"),
            None => "here".to_owned(),
        };
        report.add_label(Label::new((name, span)).with_message(label));
        for context in &diagnostic.contexts {
            report.add_label(
                Label::new((name, range(&context.span)))
                    .with_message(format!("while parsing {}", context.label)),
            );
        }
        if !diagnostic.expected.is_empty() {
            report.set_note(format!("expected {}", diagnostic.expected.join(", ")));
        }
        report.finish().write((name, &source), &mut *out)?;
    }
    Ok(())
}

/// Keep a span inside the source so rendering cannot go out of bounds.
fn clamp(span: &Span, len: usize) -> Range<usize> {
    let end = span.end.min(len);
    span.start.min(end)..end
}
