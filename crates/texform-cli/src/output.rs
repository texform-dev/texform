//! Text and `--json` rendering, exit statuses, and diagnostics on stderr.

use std::any::Any;
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
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

/// Report a usage, configuration, or I/O error and return its exit status.
pub fn usage_error(message: impl Display) -> ExitCode {
    eprint_line(format_args!("error: {message}"));
    ExitCode::from(USAGE)
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
    /// Text mode: the stdout text, plus parse diagnostics of the complete
    /// document to show as warnings.
    Text {
        line: String,
        warnings: Vec<ParseDiagnostic>,
    },
    /// JSON mode: one complete JSON value, including token arrays.
    Json(String),
}

#[derive(Clone, Copy)]
pub enum Format {
    Text,
    Tree { verbose: bool },
    Json,
}

impl Format {
    pub fn syntax(self, syntax: &SyntaxNode) -> String {
        if matches!(self, Self::Tree { verbose: true }) {
            serde_json::to_string_pretty(syntax).expect("syntax serializes to JSON")
        } else {
            syntax.to_string().trim_end_matches('\n').to_owned()
        }
    }

    fn is_block(self) -> bool {
        matches!(self, Self::Tree { .. })
    }
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
    /// Partial tree of an incomplete parse, shown in both text and JSON modes.
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

/// The CLI processes requests on one thread. Suppress the default panic hook
/// only inside the recovery boundary; the caller renders the failure once.
pub fn catch_processing_panic<T>(process: impl FnOnce() -> T) -> std::thread::Result<T> {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(process));
    panic::set_hook(hook);
    result
}

/// Writes per-formula results and remembers whether any formula failed.
pub struct Printer {
    format: Format,
    per_line: bool,
    color: bool,
    failed: bool,
}

impl Printer {
    pub fn new(format: Format, per_line: bool) -> Self {
        Self {
            format,
            per_line,
            color: stderr_color(),
            failed: false,
        }
    }

    /// Write the result of one formula.
    ///
    /// JSON, normalized text, and token lists retain line alignment.
    /// Human-readable trees use numbered blocks under `--lines`.
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
                if matches!(self.format, Format::Json) {
                    Some(failure_line(&failure))
                } else {
                    self.report_failure(formula, &failure.error);
                    failure
                        .syntax
                        .as_deref()
                        .map(|syntax| self.format.syntax(syntax))
                        .or_else(|| {
                            self.per_line.then(|| {
                                if self.format.is_block() {
                                    "(failed)".to_owned()
                                } else {
                                    String::new()
                                }
                            })
                        })
                }
            }
        };
        match line {
            Some(line) => {
                let mut stdout = io::stdout().lock();
                if self.format.is_block()
                    && let Origin::Line(number) = formula.origin
                {
                    writeln!(stdout, "<line {number}>\n{line}\n")
                } else {
                    writeln!(stdout, "{line}")
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn recovered_panics_are_silent_and_restore_the_previous_hook() {
        let previous = panic::take_hook();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        panic::set_hook(Box::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        }));
        let recovered = catch_processing_panic(|| panic!("formula failed"));
        let calls_inside = calls.load(Ordering::SeqCst);
        let outside = panic::catch_unwind(|| panic!("outside recovery boundary"));
        let calls_outside = calls.load(Ordering::SeqCst);
        panic::set_hook(previous);

        assert_eq!(
            panic_message(recovered.unwrap_err().as_ref()),
            "formula failed"
        );
        assert!(outside.is_err());
        assert_eq!(calls_inside, 0);
        assert_eq!(calls_outside, 1);
        assert_eq!(catch_processing_panic(|| 42).unwrap(), 42);
    }
}
