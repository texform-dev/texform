use std::env;

use texform_core::api::{self, SpecTarget};
use texform_core::knowledge::{AllowedMode, CommandKind};
use texform_interface::syntax_node::ContentMode;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut strict = false;
    let mut verbose = false;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--strict" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --strict requires true or false");
                    print_usage(&args[0]);
                    return;
                }
                strict = parse_bool(args[i + 1].as_str()).unwrap_or(false);
                i += 2;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            value => {
                positional.push(value.to_string());
                i += 1;
            }
        }
    }

    if positional.is_empty() {
        print_usage(&args[0]);
        return;
    }

    let parsed = match positional[0].as_str() {
        "command" => parse_command_args(&positional, &args[0]),
        "environment" => parse_environment_args(&positional, &args[0]),
        other => {
            eprintln!("Error: unsupported target {}", other);
            print_usage(&args[0]);
            None
        }
    };

    let Some((name, target, spec, input, target_label)) = parsed else {
        return;
    };

    let output = api::parse_once_with_spec(
        name.as_str(),
        target,
        spec.as_str(),
        input.as_str(),
        strict,
        None,
    );

    println!("=== TeXForm parse_once_with_spec Example ===");
    println!("Target: {}", target_label);
    println!("Name: {}", name);
    println!("Spec: {}", spec);
    println!("Input: {}", input);
    println!("Strict mode: {}", strict);
    println!("Verbose: {}", verbose);
    println!();

    if output.diagnostics.is_empty() {
        match output.result {
            Some(result) => {
                println!("Parse successful!");
                println!("Root span: {}..{}", result.span.start, result.span.end);
                println!();
                println!("--- Syntax Tree ---");
                if verbose {
                    println!("{}", serde_json::to_string_pretty(&result.node).unwrap());
                } else {
                    println!("{}", result.node);
                }
            }
            None => {
                eprintln!("Parse produced no result and no diagnostics");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Parse diagnostics:");
        for (idx, diag) in output.diagnostics.iter().enumerate() {
            eprintln!("{}. {}", idx + 1, diag.message);
            eprintln!("   span: {}..{}", diag.span.start, diag.span.end);
            if !diag.expected.is_empty() {
                eprintln!("   expected: {}", diag.expected.join(", "));
            }
            if let Some(found) = &diag.found {
                eprintln!("   found: {}", found);
            }
        }
        std::process::exit(1);
    }
}

type ParsedArgs = (String, SpecTarget, String, String, &'static str);

fn parse_command_args(positional: &[String], program: &str) -> Option<ParsedArgs> {
    if positional.len() != 6 {
        eprintln!("Error: command target expects 5 positional arguments");
        print_usage(program);
        return None;
    }

    let name = positional[1].clone();
    let kind = match parse_command_kind(positional[2].as_str()) {
        Some(value) => value,
        None => {
            eprintln!("Error: invalid command kind {}", positional[2]);
            return None;
        }
    };
    let mode = match parse_allowed_mode(positional[3].as_str()) {
        Some(value) => value,
        None => {
            eprintln!("Error: invalid allowed mode {}", positional[3]);
            return None;
        }
    };

    Some((
        name,
        SpecTarget::Command { kind, mode },
        positional[4].clone(),
        positional[5].clone(),
        "command",
    ))
}

fn parse_environment_args(positional: &[String], program: &str) -> Option<ParsedArgs> {
    if positional.len() != 7 {
        eprintln!("Error: environment target expects 6 positional arguments");
        print_usage(program);
        return None;
    }

    let name = positional[1].clone();
    let mode = match parse_allowed_mode(positional[2].as_str()) {
        Some(value) => value,
        None => {
            eprintln!("Error: invalid allowed mode {}", positional[2]);
            return None;
        }
    };
    let body_mode = match parse_content_mode(positional[3].as_str()) {
        Some(value) => value,
        None => {
            eprintln!("Error: invalid body mode {}", positional[3]);
            return None;
        }
    };
    let has_star_variant = match parse_bool(positional[4].as_str()) {
        Some(value) => value,
        None => {
            eprintln!("Error: invalid has_star_variant value {}", positional[4]);
            return None;
        }
    };

    Some((
        name,
        SpecTarget::Environment {
            has_star_variant,
            mode,
            body_mode,
        },
        positional[5].clone(),
        positional[6].clone(),
        "environment",
    ))
}

fn parse_command_kind(value: &str) -> Option<CommandKind> {
    match value {
        "prefix" => Some(CommandKind::Prefix),
        "infix" => Some(CommandKind::Infix),
        "declarative" => Some(CommandKind::Declarative),
        _ => None,
    }
}

fn parse_allowed_mode(value: &str) -> Option<AllowedMode> {
    match value {
        "math" => Some(AllowedMode::Math),
        "text" => Some(AllowedMode::Text),
        "both" => Some(AllowedMode::Both),
        _ => None,
    }
}

fn parse_content_mode(value: &str) -> Option<ContentMode> {
    match value {
        "math" => Some(ContentMode::Math),
        "text" => Some(ContentMode::Text),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage:");
    eprintln!(
        "  {} command <name> <kind> <mode> <spec> <input> [--strict true|false] [--verbose]",
        program
    );
    eprintln!(
        "  {} environment <name> <mode> <body_mode> <has_star_variant> <spec> <input> [--strict true|false] [--verbose]",
        program
    );
    eprintln!();
    eprintln!("Examples:");
    eprintln!(
        "  {} command probe prefix math 'm' '\\probe{{a}}' --strict true",
        program
    );
    eprintln!(
        "  {} command probe prefix math 's m' '\\probe*{{a}}' --verbose",
        program
    );
    eprintln!(
        "  {} environment probeenv math math false '' '\\begin{{probeenv}}a\\end{{probeenv}}'",
        program
    );
}
