//! One module per subcommand, except `serve`.

pub mod argspec;
pub mod info;
pub mod normalize;
pub mod packages;
pub mod parse;
pub mod tokenize;

use std::ops::ControlFlow;
use std::process::ExitCode;

use crate::input::FormulaInput;
use crate::output::{Failure, Format, Printer, Success, catch_processing_panic, usage_error};

/// Continue after formula failures, but stop reading when stdout closes.
fn run_formulas(
    input: &FormulaInput,
    format: Format,
    mut process: impl FnMut(&str) -> Result<Success, Failure>,
) -> ExitCode {
    let mut printer = Printer::new(format, input.per_line());
    let mut write_error = None;
    let read = input.for_each(|formula| {
        // A panic is a bug, but it must not end a run over many formulas or
        // break line alignment, so it fails only this formula.
        let outcome = catch_processing_panic(|| process(formula.latex))
            .unwrap_or_else(|payload| Err(Failure::panicked(payload.as_ref())));
        match printer.emit(&formula, outcome) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => {
                write_error = Some(error);
                ControlFlow::Break(())
            }
        }
    });
    if let Some(error) = write_error {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            return ExitCode::SUCCESS;
        }
        return usage_error(format_args!("cannot write stdout: {error}"));
    }
    match read {
        Ok(()) => printer.exit_code(),
        Err(message) => usage_error(message),
    }
}
