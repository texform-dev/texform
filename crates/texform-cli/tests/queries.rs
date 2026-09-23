//! Contract tests for the query commands: `info`, `packages`, and
//! `argspec validate`.

mod support;

use serde_json::{Value, json};
use support::{parser, texform};
use texform::ContentMode;
use texform::bindings::{
    character_info_to_dto, command_info_to_dto, env_info_to_dto, list_packages_to_dto,
};

const FAILURE: i32 = 1;
const USAGE: i32 = 2;

fn to_json(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn info_reports_knowledge_records_like_the_bindings() {
    let parser = parser();
    let math = ContentMode::Math;
    let lookup = |args: &[&str]| {
        let run = texform(&[&["info", "--json"], args].concat(), "");
        assert_eq!(run.code(), 0, "{args:?}: {}", run.stderr);
        run.json_lines().remove(0)
    };

    assert_eq!(
        lookup(&[r"\frac"]),
        json!({ "command": to_json(command_info_to_dto(parser.lookup_command("frac", math).unwrap())) })
    );
    // A character command has a character record next to its command record.
    assert_eq!(
        lookup(&[r"\alpha"]),
        json!({
            "command": to_json(command_info_to_dto(parser.lookup_command("alpha", math).unwrap())),
            "character": to_json(character_info_to_dto(parser.lookup_character("alpha", math).unwrap())),
        })
    );
    assert_eq!(
        lookup(&["--env", "align"]),
        json!({ "environment": to_json(env_info_to_dto(parser.lookup_env("align", math).unwrap())) })
    );

    let text = texform(&["info", r"\frac"], "");
    assert_eq!(text.code(), 0);
    assert!(
        text.stdout.starts_with("command:  \\frac\n"),
        "{}",
        text.stdout
    );
    assert!(text.stdout.contains("argspec:  m m\n"), "{}", text.stdout);
}

#[test]
fn info_distinguishes_missing_entries_from_usage_errors() {
    let missing = texform(&["info", r"\notacommand"], "");
    assert_eq!(missing.code(), FAILURE);
    assert_eq!(missing.stdout, "");
    assert!(missing.stderr.contains(r"\notacommand"));

    let missing_json = texform(&["info", "--json", "--env", "notanenv"], "");
    assert_eq!(missing_json.code(), FAILURE);
    assert_eq!(missing_json.stdout, "null\n");

    // Lookups use the global package selection.
    assert_eq!(texform(&["info", r"\ket"], "").code(), 0);
    assert_eq!(
        texform(&["--packages", "base", "info", r"\ket"], "").code(),
        FAILURE
    );

    let bare = texform(&["info", "frac"], "");
    assert_eq!(bare.code(), USAGE);
    assert!(
        bare.stderr.contains("not a control sequence"),
        "{}",
        bare.stderr
    );
}

#[test]
fn packages_lists_builtin_packages() {
    let expected = list_packages_to_dto();

    let json = texform(&["packages", "--json"], "");
    assert_eq!(json.code(), 0);
    assert_eq!(json.json_lines(), vec![to_json(&expected)]);

    let text = texform(&["packages"], "");
    let lines = text.lines();
    assert_eq!(lines.len(), expected.len());
    for (line, package) in lines.iter().zip(&expected) {
        assert_eq!(line.split_whitespace().next(), Some(package.name.as_str()));
        assert!(
            line.contains(&format!("{} command", package.commands)),
            "{line}"
        );
    }
}

#[test]
fn argspec_validate_reports_slots_or_the_error() {
    let spec = "s m O{default}";
    let json = texform(&["argspec", "validate", "--json", spec], "");
    assert_eq!(json.code(), 0);
    assert_eq!(
        json.json_lines(),
        vec![to_json(texform::validate_argspec(spec))]
    );

    let text = texform(&["argspec", "validate", spec], "");
    assert_eq!(text.code(), 0);
    assert_eq!(
        text.lines(),
        vec![
            "valid: 3 arguments",
            "  1. optional, star",
            "  2. required, math content",
            "  3. optional, math content",
        ]
    );

    let invalid = "m {";
    let json = texform(&["argspec", "validate", "--json", invalid], "");
    assert_eq!(json.code(), FAILURE);
    assert_eq!(
        json.json_lines(),
        vec![to_json(texform::validate_argspec(invalid))]
    );
    let text = texform(&["argspec", "validate", invalid], "");
    assert_eq!(text.code(), FAILURE);
    assert_eq!(text.stdout, "");
    assert!(text.stderr.contains("invalid argspec"), "{}", text.stderr);
}
