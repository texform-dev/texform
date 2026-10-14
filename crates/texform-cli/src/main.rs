//! The `texform` binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(texform_cli::run(std::env::args_os()))
}
