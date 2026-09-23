//! One module per subcommand, except `serve`.

pub mod argspec;
pub mod info;
pub mod normalize;
pub mod packages;
pub mod parse;
pub mod tokenize;

use std::ops::ControlFlow;
use std::panic::{self, AssertUnwindSafe};
use std::process::ExitCode;

use crate::input::FormulaInput;
use crate::output::{Failure, Printer, Success, io_error};

/// Process every input formula and write one result per formula.
///
/// All formulas are processed even after a failure, so `--lines` output stays
/// aligned with the input; the exit status reports whether any failed.
fn run_formulas(
    input: &FormulaInput,
    json: bool,
    mut process: impl FnMut(&str) -> Result<Success, Failure>,
) -> ExitCode {
    let mut printer = Printer::new(json, input.per_line());
    let mut write_error = None;
    let read = input.for_each(|formula| {
        // A panic is a bug, but it must not end a run over many formulas or
        // break line alignment, so it fails only this formula.
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| process(formula.latex)))
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
        return io_error(format_args!("cannot write stdout: {error}"));
    }
    match read {
        Ok(()) => printer.exit_code(),
        Err(message) => io_error(message),
    }
}
