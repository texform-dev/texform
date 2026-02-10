use ariadne::{Config, IndexType, Label, Report, ReportKind, Source};
use std::env;
use texform_core::parser;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
    let mut input = String::new();
    let mut strict = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--strict" => {
                if i + 1 < args.len() {
                    strict = args[i + 1].parse().unwrap_or(false);
                    i += 2;
                } else {
                    eprintln!("Error: --strict requires a value (true/false)");
                    print_usage(&args[0]);
                    return;
                }
            }
            arg if !arg.starts_with("--") => {
                input = arg.to_string();
                i += 1;
            }
            _ => {
                eprintln!("Error: unknown option {}", args[i]);
                print_usage(&args[0]);
                return;
            }
        }
    }

    if input.is_empty() {
        eprintln!("Error: no input provided");
        print_usage(&args[0]);
        return;
    }

    println!("=== TeXForm Simple Parser CLI ===");
    println!("Input: {}", input);
    println!("Strict mode: {}", strict);
    println!();

    // Parse directly from source string
    println!("--- Parsing ---");
    match parser::parse(&input, strict) {
        Ok((syntax_node, span)) => {
            println!("Parse successful!");
            println!("Root span: {}..{}", span.start, span.end);
            println!();
            println!("--- Syntax Tree ---");
            println!("{}", syntax_node);
        }
        Err(errors) => {
            eprintln!("Parse error(s):");
            let config = Config::default().with_index_type(IndexType::Byte);
            let source = Source::from(input.as_str());

            for error in &errors {
                let span = error.span();
                let range = span.start..span.end;

                let mut builder = Report::build(ReportKind::Error, range.clone())
                    .with_config(config)
                    .with_message(format!("{:?}", error));

                builder = builder.with_label(
                    Label::new(range).with_message(format!("{:?}", error.reason())),
                );

                builder.finish().eprint(source.clone()).unwrap();
            }
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <input> [--strict true|false]", program);
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <input>              LaTeX formula to parse");
    eprintln!("  --strict true|false  Enable strict mode (default: false)");
    eprintln!();
    eprintln!("Example:");
    eprintln!("  {} 'a \\\\over b'", program);
    eprintln!("  {} '\\\\frac{{a}}{{b}}' --strict false", program);
}
