use std::env;
use texform_argspec::parse_arg_specs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        print_usage(&args[0]);
        return;
    }

    let spec = args[1].as_str();
    println!("=== TeXForm validate_spec Example ===");
    println!("Spec: {}", spec);

    match parse_arg_specs(spec, "validate_spec") {
        Ok(parsed) => {
            println!("valid: true");
            println!("arg_count: {}", parsed.len());
            println!("parsed: {:#?}", parsed);
        }
        Err(error) => {
            println!("valid: false");
            println!("error: {}", error);
            std::process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <spec>", program);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} 'm o'", program);
    eprintln!("  {} 's m'", program);
}
