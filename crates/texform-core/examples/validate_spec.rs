use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        print_usage(&args[0]);
        return;
    }

    let spec = args[1].as_str();
    println!("=== TeXForm validate_spec Example ===");
    println!("Spec: {}", spec);

    let result =
        std::panic::catch_unwind(|| texform_specs::specs::parse_arg_specs(spec, "validate_spec"));
    match result {
        Ok(parsed) => {
            println!("valid: true");
            println!("arg_count: {}", parsed.len());
            println!("parsed: {:#?}", parsed);
        }
        Err(payload) => {
            println!("valid: false");
            println!("error: {}", panic_payload_to_string(payload));
            std::process::exit(1);
        }
    }
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "validate_spec panic".to_string()
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <spec>", program);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} 'm o'", program);
    eprintln!("  {} 's m'", program);
}
