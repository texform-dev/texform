//! The `texform` command-line tool.
//!
//! [`run`] is the whole tool behind one call. The `texform` binary and the
//! `texform` command installed by the Python package both call it; it is not a
//! stable API for other embedders.

mod build_info;
mod commands;
mod input;
mod normalizer;
mod output;
mod packages;
mod serve;

use std::ffi::OsString;
use std::io::{self, Write};

use clap::{Parser, Subcommand};

use crate::commands::{argspec, info, normalize, parse, tokenize};
use crate::output::USAGE;

/// Parse and normalize LaTeX formulas with TeXForm.
///
/// Formula commands read one formula from the LATEX argument, all of stdin
/// when it is omitted, or one formula per stdin line with `--lines`. Exit
/// status is 0 when everything succeeded, 1 when at least one formula or
/// lookup failed, and 2 for usage, configuration, or I/O errors.
#[derive(Parser)]
#[command(name = "texform", version = build_info::VERSION_TEXT)]
struct Cli {
    /// Knowledge packages to load, comma-separated [default: same as the library; excludes physics]
    #[arg(
        long,
        global = true,
        value_name = "NAMES",
        value_delimiter = ',',
        value_parser = packages::parse_name
    )]
    packages: Option<Vec<String>>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Normalize formulas with a transform profile
    Normalize(normalize::Args),
    /// Parse formulas and print their syntax trees
    Parse(parse::Args),
    /// Split the serialization of formulas into tokens
    Tokenize(tokenize::Args),
    /// Show knowledge-base records for a control sequence or an environment
    Info(info::Args),
    /// List the built-in knowledge packages
    Packages(commands::packages::Args),
    /// Argument-specification tools
    #[command(subcommand)]
    Argspec(argspec::Command),
    /// Serve the normalizer protocol (JSON-RPC 2.0 over stdio)
    ///
    /// Reads one request per line from stdin and writes one response per line
    /// to stdout until stdin reaches end of file. `--packages` is the default
    /// for `configure` requests that omit `packages`.
    Serve,
}

/// Run the tool on `args`, whose first item is the program name, and return
/// the exit status.
pub fn run<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` and `--version` also arrive here, with exit status 0.
            let _ = error.print();
            return u8::try_from(error.exit_code()).unwrap_or(USAGE);
        }
    };
    let packages = cli.packages.unwrap_or_else(packages::defaults);
    let status = match cli.command {
        Command::Normalize(args) => normalize::run(args, &packages),
        Command::Parse(args) => parse::run(args, &packages),
        Command::Tokenize(args) => tokenize::run(args, &packages),
        Command::Info(args) => info::run(args, &packages),
        Command::Packages(args) => commands::packages::run(args),
        Command::Argspec(command) => argspec::run(command),
        Command::Serve => serve::run(packages),
    };
    // Only a Rust `main` flushes stdout on return; an embedding process such
    // as the Python interpreter would drop a final line without a newline.
    let _ = io::stdout().flush();
    status
}
