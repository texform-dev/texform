use ariadne::{Config, IndexType, Label, Report, ReportKind, Source};
use std::env;
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, ContextItem, DelimiterControlItem,
    EnvironmentItem, ParseConfig, ParseContextBuilder, ParseDiagnostic, ParseOutput,
};

struct CliOptions {
    input: String,
    strict: bool,
    verbose: bool,
    packages: Option<Vec<String>>,
    items: Vec<ContextItem>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let options = match parse_args(args.as_slice()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("Error: {message}");
            print_usage(&args[0]);
            std::process::exit(1);
        }
    };

    let output = match parse_with_options(&options) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("Error: {message}");
            std::process::exit(1);
        }
    };

    print_summary(&options);
    let success = print_output(&options.input, &output, options.verbose);
    if !success {
        std::process::exit(1);
    }
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut input: Option<String> = None;
    let mut strict = false;
    let mut verbose = false;
    let mut packages: Option<Vec<String>> = None;
    let mut items = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--strict" => {
                strict = parse_bool(required_arg(args, i + 1, "--strict")?)?;
                i += 2;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            "--packages" => {
                packages = Some(parse_packages(required_arg(args, i + 1, "--packages")?));
                i += 2;
            }
            "--command" => {
                let name = required_arg(args, i + 1, "--command <name>")?;
                let kind = parse_command_kind(required_arg(args, i + 2, "--command <kind>")?)?;
                let allowed_mode =
                    parse_allowed_mode(required_arg(args, i + 3, "--command <mode>")?)?;
                let spec = required_arg(args, i + 4, "--command <spec>")?;
                items.push(CommandItem::new(name, kind, allowed_mode, spec).into());
                i += 5;
            }
            "--environment" => {
                let name = required_arg(args, i + 1, "--environment <name>")?;
                let allowed_mode =
                    parse_allowed_mode(required_arg(args, i + 2, "--environment <mode>")?)?;
                let body_mode =
                    parse_content_mode(required_arg(args, i + 3, "--environment <body_mode>")?)?;
                let spec = required_arg(args, i + 4, "--environment <spec>")?;
                items.push(EnvironmentItem::new(name, allowed_mode, body_mode, spec).into());
                i += 5;
            }
            "--delimiter" => {
                let name = required_arg(args, i + 1, "--delimiter <name>")?;
                items.push(DelimiterControlItem::new(name).into());
                i += 2;
            }
            arg if arg.starts_with("--") => {
                return Err(format!("unknown option {}", args[i]));
            }
            value => {
                if input.replace(value.to_string()).is_some() {
                    return Err("multiple input values provided".to_string());
                }
                i += 1;
            }
        }
    }

    let input = input.ok_or_else(|| "no input provided".to_string())?;
    Ok(CliOptions {
        input,
        strict,
        verbose,
        packages,
        items,
    })
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_with_options(options: &CliOptions) -> Result<ParseOutput, String> {
    let base_ctx = match options.packages.as_ref() {
        Some(packages) => {
            let refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            ParseContextBuilder::empty().packages(refs.as_slice())
        }
        None => ParseContextBuilder::default(),
    };
    let mut builder = base_ctx;
    for item in &options.items {
        builder = builder.insert_item(item.clone());
    }
    let ctx = builder
        .build()
        .map_err(|error| format!("failed to build parse context: {error:?}"))?;
    let config = if options.strict {
        ParseConfig::STRICT_NO_RECOVER
    } else {
        ParseConfig::NONSTRICT_RECOVER
    };
    Ok(ctx.parse(options.input.as_str(), &config))
}

fn print_summary(options: &CliOptions) {
    println!("=== TeXForm Parse Example ===");
    println!("Input: {}", options.input);
    println!("Strict mode: {}", options.strict);
    println!("Verbose: {}", options.verbose);
    println!(
        "Packages: {}",
        options
            .packages
            .as_ref()
            .map(|values| {
                if values.is_empty() {
                    "<empty>".to_string()
                } else {
                    values.join(",")
                }
            })
            .unwrap_or_else(|| "default packages".to_string())
    );
    println!("Custom items: {}", options.items.len());
    println!();
}

fn print_output(input: &str, output: &ParseOutput, verbose: bool) -> bool {
    if output.diagnostics.is_empty() {
        match &output.result {
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
                true
            }
            None => {
                eprintln!("Parse produced no result and no diagnostics");
                false
            }
        }
    } else {
        eprintln!("Parse diagnostics:");
        render_diagnostics(input, output.diagnostics.as_slice());

        if let Some(result) = &output.result {
            eprintln!();
            eprintln!("Partial parse available:");
            if verbose {
                eprintln!("{}", serde_json::to_string_pretty(&result.node).unwrap());
            } else {
                eprintln!("{}", result.node);
            }
        }

        false
    }
}

fn render_diagnostics(input: &str, diagnostics: &[ParseDiagnostic]) {
    let config = Config::default().with_index_type(IndexType::Byte);
    let source = Source::from(input);

    for diagnostic in diagnostics {
        let range = diagnostic.span.start..diagnostic.span.end;
        let mut label_message = diagnostic.message.clone();

        if !diagnostic.expected.is_empty() {
            label_message.push_str(&format!(" | expected: {}", diagnostic.expected.join(", ")));
        }
        if let Some(found) = &diagnostic.found {
            label_message.push_str(&format!(" | found: {}", found));
        }

        Report::build(ReportKind::Error, range.clone())
            .with_config(config)
            .with_message(diagnostic.message.clone())
            .with_label(Label::new(range).with_message(label_message))
            .finish()
            .eprint(source.clone())
            .unwrap();

        for context in &diagnostic.contexts {
            eprintln!(
                "  context: {} @ {}..{}",
                context.label, context.span.start, context.span.end
            );
        }
    }
}

fn parse_command_kind(value: &str) -> Result<CommandKind, String> {
    match value {
        "prefix" => Ok(CommandKind::Prefix),
        "infix" => Ok(CommandKind::Infix),
        "declarative" => Ok(CommandKind::Declarative),
        _ => Err(format!("invalid command kind {value}")),
    }
}

fn parse_allowed_mode(value: &str) -> Result<AllowedMode, String> {
    match value {
        "math" => Ok(AllowedMode::Math),
        "text" => Ok(AllowedMode::Text),
        "both" => Ok(AllowedMode::Both),
        _ => Err(format!("invalid allowed mode {value}")),
    }
}

fn parse_content_mode(value: &str) -> Result<ContentMode, String> {
    match value {
        "math" => Ok(ContentMode::Math),
        "text" => Ok(ContentMode::Text),
        _ => Err(format!("invalid body mode {value}")),
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid bool {value}")),
    }
}

fn parse_packages(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn print_usage(program: &str) {
    eprintln!("Usage:");
    eprintln!(
        "  {} <input> [--strict true|false] [--verbose] [--packages base,ams]",
        program
    );
    eprintln!("           [--command <name> <kind> <mode> <spec>]",);
    eprintln!("           [--environment <name> <mode> <body_mode> <spec>]",);
    eprintln!("           [--delimiter <name>]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} '\\\\frac{{a}}{{b}}'", program);
    eprintln!(
        "  {} '\\\\probe{{a}}' --command probe prefix math 'm' --strict true",
        program
    );
    eprintln!(
        "  {} '\\\\begin{{probeenv}}a\\\\end{{probeenv}}' --environment probeenv math math ''",
        program
    );
    eprintln!(
        "  {} '\\\\left\\\\langle x\\\\right\\\\rangle' --delimiter langle --delimiter rangle",
        program
    );
    eprintln!();
    eprintln!("Notes:");
    eprintln!("  - Without custom items, this behaves like the normal parse CLI.");
    eprintln!("  - Without --packages, this example loads default packages.");
    eprintln!("  - Repeat --command / --environment / --delimiter to inject multiple items.");
}
