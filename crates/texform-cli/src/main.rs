//! The `texform` command-line tool.

mod build_info;
mod packages;
mod serve;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Parse and normalize LaTeX formulas with TeXForm.
#[derive(Parser)]
#[command(name = "texform", version = build_info::VERSION_TEXT)]
struct Cli {
    /// Knowledge packages to load, comma-separated [default: all built-in packages]
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
    /// Serve the normalizer protocol (JSON-RPC 2.0 over stdio)
    ///
    /// Reads one request per line from stdin and writes one response per line
    /// to stdout until stdin reaches end of file. `--packages` is the default
    /// for `configure` requests that omit `packages`.
    Serve,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let packages = cli.packages.unwrap_or_else(packages::all);
    match cli.command {
        Command::Serve => serve::run(packages),
    }
}
