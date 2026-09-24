//! Contract tests for the formula commands: `normalize`, `parse`, and
//! `tokenize`.

mod support;

use serde_json::{Value, json};
use support::{engine, parser, texform};
use texform::Profile;
use texform::bindings::{
    NormalizeConfigInput, normalize_error_to_parts, read, tokenized_latex_to_dto,
    transform_report_to_dto,
};

const UNCLOSED_FRACTION: &str = r"\frac{a";
const FAILURE: i32 = 1;
const USAGE: i32 = 2;

/// One formula per line, as `--lines` reads them.
fn lines(formulas: &[&str]) -> String {
    let mut input = formulas.join("\n");
    input.push('\n');
    input
}

#[test]
fn normalize_matches_facade_normalize_with() {
    let formulas = [
        r"a \over b",
        r"\frac12 + \dfrac{1}{2}",
        r"{\bf x} + \mathbf{y} + {\rm d}x",
        r"\left( \frac{a}{b} \right)^{2}_{i}",
        r"\begin{matrix} a & b \\ c & d \end{matrix}",
        r"\bra{\psi} \ket{\phi}",
        r"\text{if } x' > 0",
        "",
        r"\notacommand{x}",
        UNCLOSED_FRACTION,
        r"a \over b \over c",
    ];
    let configs = [
        ("corpus", Profile::Corpus, json!({})),
        (
            "authoring",
            Profile::Authoring,
            json!({ "rewrite": { "enabled": false } }),
        ),
        ("equiv", Profile::Equiv, json!({ "reject_unknown": true })),
    ];
    let input = lines(&formulas);

    for (profile_name, profile, overrides) in configs {
        let config_arg = overrides.to_string();
        let args = [
            "normalize",
            "--profile",
            profile_name,
            "--config",
            &config_arg,
        ];
        let text = texform(&[&args[..], &["--lines"]].concat(), &input);
        let json = texform(&[&args[..], &["--lines", "--json"]].concat(), &input);

        let engine = engine(profile);
        let config = read::<NormalizeConfigInput>(overrides)
            .unwrap()
            .into_config(engine.default_normalize_config());
        let text_lines = text.lines();
        let json_lines = json.json_lines();
        assert_eq!(text_lines.len(), formulas.len(), "{profile_name}");
        assert_eq!(json_lines.len(), formulas.len(), "{profile_name}");
        for (index, formula) in formulas.iter().enumerate() {
            let context = format!("{profile_name}, formula {formula:?}");
            match engine.normalize_with(formula, &config) {
                Ok(output) => {
                    assert_eq!(text_lines[index], output, "{context}");
                    assert_eq!(
                        json_lines[index],
                        json!({ "ok": true, "output": output }),
                        "{context}"
                    );
                }
                Err(error) => {
                    // Text mode keeps an empty placeholder line and reports on stderr.
                    assert_eq!(text_lines[index], "", "{context}");
                    assert!(
                        text.stderr.contains(&format!("error: line {}:", index + 1)),
                        "{context}"
                    );
                    let error = normalize_error_to_parts(error).error;
                    assert_eq!(
                        json_lines[index],
                        json!({ "ok": false, "error": serde_json::to_value(&error).unwrap() }),
                        "{context}"
                    );
                }
            }
        }
        // Every configuration has failing formulas; each run still finishes the input.
        assert_eq!(text.code(), FAILURE, "{profile_name}");
        assert_eq!(json.code(), FAILURE, "{profile_name}");
    }
}

#[test]
fn single_formula_comes_from_the_argument_or_all_of_stdin() {
    // Stdin is one formula, so newlines inside an environment are kept and
    // only the final newline is dropped.
    let formula = "\\begin{matrix}\na & b \\\\\nc & d\n\\end{matrix}";
    let expected = engine(Profile::Corpus).normalize(formula).unwrap();

    let argument = texform(&["normalize", "--profile", "corpus", formula], "ignored");
    let stdin = texform(
        &["normalize", "--profile", "corpus"],
        &format!("{formula}\n"),
    );
    for run in [&argument, &stdin] {
        assert_eq!(run.code(), 0, "{}", run.stderr);
        assert_eq!(run.stdout, format!("{expected}\n"));
    }

    // Without `--lines`, a failing formula writes nothing to stdout.
    let failed = texform(&["normalize", "--profile", "corpus", UNCLOSED_FRACTION], "");
    assert_eq!(failed.code(), FAILURE);
    assert_eq!(failed.stdout, "");
    assert!(
        failed
            .stderr
            .contains("error: parse produced an incomplete document")
    );
    assert!(
        failed.stderr.contains("unclosed brace argument"),
        "{}",
        failed.stderr
    );
}

#[test]
fn normalize_report_is_json_only() {
    let formula = r"{\bf x} \over b";
    let run = texform(
        &[
            "normalize",
            "--profile",
            "corpus",
            "--report",
            "--json",
            formula,
        ],
        "",
    );

    let engine = engine(Profile::Corpus);
    let expected = engine
        .normalize_with_report(formula, &engine.default_normalize_config())
        .unwrap();
    assert_eq!(run.code(), 0);
    assert_eq!(
        run.json_lines(),
        vec![json!({
            "ok": true,
            "output": expected.normalized,
            "report": serde_json::to_value(transform_report_to_dto(&expected.report)).unwrap(),
        })]
    );

    let text = texform(
        &["normalize", "--profile", "corpus", "--report", formula],
        "",
    );
    assert_eq!(text.code(), USAGE);
    assert_eq!(text.stdout, "");
}

#[test]
fn config_is_validated_against_the_command_shape() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("strict-parse.json");
    std::fs::write(&path, r#"{ "reject_unknown": true }"#).unwrap();
    let from_file = format!("@{}", path.display());

    // Parse options apply to every formula command.
    for command in [
        &["normalize", "--profile", "corpus"][..],
        &["parse"],
        &["tokenize"],
        &["tokenize", "--profile", "corpus"],
    ] {
        let strict = texform(
            &[command, &["--config", &from_file, r"\notacommand"]].concat(),
            "",
        );
        assert_eq!(strict.code(), FAILURE, "{command:?}: {}", strict.stderr);
        assert!(
            strict.stderr.contains("unknown-command"),
            "{command:?}: {}",
            strict.stderr
        );

        let typo = texform(
            &[command, &["--config", r#"{"reject_unknwn":true}"#, "x"]].concat(),
            "",
        );
        assert_eq!(typo.code(), USAGE, "{command:?}");
        assert_eq!(typo.stdout, "", "{command:?}");
        assert!(
            typo.stderr.contains("unknown field `reject_unknwn`"),
            "{command:?}: {}",
            typo.stderr
        );
    }

    // Transform overrides exist only where the command normalizes.
    let transform = r#"{"rewrite":{"enabled":false}}"#;
    assert_eq!(
        texform(&["parse", "--config", transform, "x"], "").code(),
        USAGE
    );
    assert_eq!(
        texform(&["tokenize", "--config", transform, "x"], "").code(),
        USAGE
    );
    let tokenized = texform(
        &[
            "tokenize",
            "--profile",
            "corpus",
            "--config",
            transform,
            r"a \over b",
        ],
        "",
    );
    assert!(tokenized.stdout.contains("control_seq"));
    assert!(tokenized.stdout.contains(r#""\\over""#));

    let nested = texform(
        &[
            "normalize",
            "--profile",
            "corpus",
            "--config",
            r#"{"rewrite":{"enabld":false}}"#,
            "x",
        ],
        "",
    );
    assert_eq!(nested.code(), USAGE);
    assert!(
        nested.stderr.contains("rewrite.enabld"),
        "{}",
        nested.stderr
    );
    assert_eq!(
        texform(
            &["normalize", "--profile", "corpus", "--config", "{", "x"],
            ""
        )
        .code(),
        USAGE
    );
}

#[test]
fn parse_reports_complete_partial_and_missing_documents() {
    let formulas = [r"\frac12 + x", UNCLOSED_FRACTION, "}", ""];
    let input = lines(&formulas);
    let json = texform(&["parse", "--lines", "--json"], &input);
    let text = texform(&["parse", "--lines"], &input);

    let parser = parser();
    let (complete, _) = parser.parse(formulas[0]).try_into_document().unwrap();
    let partial = parser.parse(formulas[1]).into_parts().0.unwrap();
    let results = json.json_lines();
    assert_eq!(results.len(), formulas.len());

    assert_eq!(
        results[0],
        json!({ "ok": true, "syntax": serde_json::to_value(complete.to_syntax()).unwrap(), "diagnostics": [] })
    );
    // An incomplete document fails like it does for normalize, but its
    // partial tree is still reported.
    assert_eq!(results[1]["ok"], false);
    assert_eq!(results[1]["error"]["kind"], "parse");
    assert_eq!(
        results[1]["error"]["message"],
        "parse produced an incomplete document"
    );
    assert_eq!(
        results[1]["syntax"],
        serde_json::to_value(partial.to_syntax()).unwrap()
    );
    assert_eq!(results[2]["ok"], false);
    assert_eq!(results[2]["error"]["message"], "parse produced no document");
    assert!(results[2].get("syntax").is_none());
    assert!(
        !results[2]["error"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(results[3]["ok"], true);
    assert_eq!(json.code(), FAILURE);

    assert!(text.stdout.contains(&complete.to_syntax().to_string()));
    assert!(text.stdout.contains(&partial.to_syntax().to_string()));
    for number in 1..=4 {
        assert!(text.stdout.contains(&format!("<line {number}>")));
    }
    assert!(text.stdout.contains("<line 3>\n(failed)"));
    assert_eq!(text.code(), FAILURE);
    assert!(
        text.stderr
            .contains("error: line 2: parse produced an incomplete document")
    );
    assert!(
        text.stderr
            .contains("error: line 3: parse produced no document")
    );
    // Diagnostics are rendered against the failing line's source.
    assert!(text.stderr.contains("<line 2>"), "{}", text.stderr);
    assert!(text.stderr.contains(UNCLOSED_FRACTION), "{}", text.stderr);
}

#[test]
fn tokenize_splits_the_parsed_or_normalized_serialization() {
    let formula = r"a \over b";
    let parser = parser();
    let (document, _) = parser.parse(formula).try_into_document().unwrap();
    let parsed = document.to_tokenized_latex().unwrap();
    let engine = engine(Profile::Corpus);
    let (mut normalized, _) = engine.parser().parse(formula).try_into_document().unwrap();
    engine.transform(&mut normalized).unwrap();
    let normalized = normalized.to_tokenized_latex().unwrap();
    assert_eq!(normalized.latex, engine.normalize(formula).unwrap());

    assert_eq!(
        texform(&["tokenize", "--json", formula], "").json_lines(),
        vec![serde_json::to_value(tokenized_latex_to_dto(parsed).tokens).unwrap()]
    );
    assert_eq!(
        texform(&["tokenize", "--profile", "corpus", "--json", formula], "").json_lines(),
        vec![serde_json::to_value(tokenized_latex_to_dto(normalized).tokens).unwrap()]
    );

    let mixed = texform(
        &["tokenize", "--lines", "--json"],
        &lines(&["x", UNCLOSED_FRACTION, ""]),
    );
    let results = mixed.json_lines();
    assert_eq!(results[0][0]["text"], "x");
    assert_eq!(results[1]["error"]["kind"], "parse");
    assert_eq!(results[2], json!([]));
    assert_eq!(mixed.code(), FAILURE);
}

#[test]
fn parse_displays_compact_and_verbose_trees_including_partial_trees() {
    for formula in [r"\sqrt{a+b}", UNCLOSED_FRACTION] {
        let tree = parser().parse(formula).document().unwrap().to_syntax();
        let compact = texform(&["parse", formula], "");
        assert_eq!(compact.stdout, tree.to_string());
        let verbose = texform(&["parse", "--verbose", formula], "");
        assert_eq!(
            serde_json::from_str::<Value>(&verbose.stdout).unwrap(),
            serde_json::to_value(tree).unwrap()
        );
        assert_eq!(verbose.code(), compact.code());
    }
    assert_eq!(
        texform(&["parse", "--verbose", "--json", "x"], "").code(),
        USAGE
    );
}

#[test]
fn token_list_preserves_text_boundaries_modes_and_line_alignment() {
    let run = texform(&["tokenize", r"\text{a b}"], "");
    assert_eq!(run.code(), 0);
    assert_eq!(
        run.stdout,
        concat!(
            r#"[control_seq("\\text"), delim("{"), text("a b", mode=text), delim("}")]"#,
            "\n"
        )
    );
    assert_eq!(texform(&["tokenize", ""], "").stdout, "[]\n");
    let quoted = texform(&["tokenize", "\\text{say \"hi\"}"], "");
    assert!(quoted.stdout.contains(r#"text("say \"hi\"", mode=text)"#));
    assert_eq!(quoted.lines().len(), 1);
    let json = texform(&["tokenize", "--json", r"\text{a b}"], "");
    let tokens = json.json_lines().remove(0);
    assert_eq!(tokens.as_array().unwrap().len(), 4);
    assert_eq!(tokens[0]["kind"], "control_sequence");
    assert_eq!(tokens[1]["kind"], "delimiter");
    assert_eq!(tokens[2]["text"], "a b");
    assert_eq!(tokens[2]["mode"], "text");
    let lines = texform(
        &["tokenize", "--lines"],
        &lines(&["x", UNCLOSED_FRACTION, ""]),
    );
    assert_eq!(lines.lines(), vec![r#"[char("x")]"#, "", "[]"]);
    assert_eq!(lines.code(), FAILURE);
}

#[test]
fn unknown_options_fail_and_negative_formulas_use_the_option_separator() {
    for command in [
        &["parse"][..],
        &["tokenize"],
        &["normalize", "--profile", "corpus"],
        &["info"],
        &["argspec", "validate"],
    ] {
        let typo = texform(&[command, &["--jsno"]].concat(), "x+y\n");
        assert_eq!(typo.code(), USAGE, "{command:?}");
        assert_eq!(typo.stdout, "");
    }
    let negative = texform(&["normalize", "--profile", "corpus", "--", "-x^2"], "");
    assert_eq!(negative.code(), 0);
    assert_eq!(negative.stdout, "- x ^ { 2 }\n");
}

#[test]
fn zero_rewrite_iterations_fail_before_reading_formulas() {
    for command in [
        &["normalize", "--profile", "corpus"][..],
        &["tokenize", "--profile", "corpus"],
    ] {
        let run = texform(
            &[
                command,
                &[
                    "--json",
                    "--lines",
                    "--config",
                    r#"{"rewrite":{"max_iterations":0}}"#,
                ],
            ]
            .concat(),
            "",
        );
        assert_eq!(run.code(), USAGE);
        assert_eq!(run.stdout, "");
        assert!(
            run.stderr
                .contains("rewrite.max_iterations must be greater than zero")
        );
        assert!(!run.stderr.contains("panicked"));
    }
}

#[test]
fn closed_stdout_stops_without_waiting_for_stdin_eof() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    for command in [
        &["parse"][..],
        &["tokenize"],
        &["normalize", "--profile", "corpus"],
    ] {
        let mut child = Command::new(support::BIN)
            .args(command)
            .arg("--lines")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"x\ny\n").unwrap();
        stdin.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "{command:?}: {status}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{command:?} kept waiting for stdin after stdout closed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
