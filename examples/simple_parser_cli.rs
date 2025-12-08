use logos::Logos;
use std::env;
use texform::lexer::Token;
use texform::parser;

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

    // Step 1: Lexical analysis
    println!("--- Step 1: Lexical Analysis ---");
    let mut lexer = Token::lexer(&input);
    let mut all_tokens = Vec::new();

    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => {
                all_tokens.push(token.clone());
                println!("  {:?}", token);
            }
            Err(err) => {
                eprintln!(
                    "Lexical error at position {}: {:?}",
                    lexer.span().start,
                    err
                );
                return;
            }
        }
    }
    println!("Total tokens: {}", all_tokens.len());
    println!();

    // Step 2: Parsing (comments are skipped by the lexer)
    println!("--- Step 2: Parsing ---");
    match parser::parse(&all_tokens, strict) {
        Ok(syntax_node) => {
            println!("Parse successful!");
            println!();
            println!("--- Syntax Tree ---");
            println!("{}", syntax_node);
        }
        Err(errors) => {
            eprintln!("Parse error(s):");
            for error in &errors {
                eprintln!("  {:?}", error);
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
