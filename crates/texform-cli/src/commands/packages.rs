//! `texform packages`: list the built-in knowledge packages.

use std::process::ExitCode;

use texform::bindings::list_packages_to_dto;

use crate::output::{io_error, json_line, print_line};

#[derive(clap::Args)]
pub struct Args {
    /// Print the package list as one JSON array
    #[arg(long)]
    json: bool,
}

pub fn run(args: Args) -> ExitCode {
    let packages = list_packages_to_dto();
    let text = if args.json {
        json_line(&packages)
    } else {
        let width = packages
            .iter()
            .map(|package| package.name.len())
            .max()
            .unwrap_or(0);
        packages
            .iter()
            .map(|package| {
                format!(
                    "{:width$}  {}, {}",
                    package.name,
                    count(package.commands, "command"),
                    count(package.environments, "environment")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    match print_line(text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => io_error(format_args!("cannot write stdout: {error}")),
    }
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
