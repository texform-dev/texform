//! Contract tests for the `texform serve` normalizer protocol.

mod support;

use std::collections::BTreeSet;
use std::process::Command;

use serde_json::{Value, json};
use support::{BIN, Served, configure, initialize, normalize, request, serve, serve_bytes};
use texform::bindings::{NormalizeConfigInput, normalize_error_to_parts, read};
use texform::{Profile, TransformEngine};

const NOT_INITIALIZED: i64 = -32002;
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const NORMALIZE_FAILED: i64 = 1;

const UNCLOSED_FRACTION: &str = r"\frac{a";

fn all_packages() -> Vec<String> {
    texform::list_packages()
        .into_iter()
        .map(|package| package.name)
        .collect()
}

fn timing_keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("timing should be an object: {value}"))
        .keys()
        .map(String::as_str)
        .collect()
}

fn invalid_params_message(served: &Served, id: i64) -> &str {
    assert_eq!(served.error_code(id), INVALID_PARAMS);
    served.response(id)["error"]["data"]["message"]
        .as_str()
        .expect("-32602 carries data.message")
}

#[test]
fn requests_before_initialize_are_rejected() {
    let served = serve(
        &[],
        &[
            configure(0, "corpus", json!({ "profile": "corpus" })),
            normalize(1, "corpus", "x", false),
            initialize(2),
            configure(3, "corpus", json!({ "profile": "corpus" })),
        ],
    );

    assert_eq!(served.error_code(0), NOT_INITIALIZED);
    assert_eq!(served.error_code(1), NOT_INITIALIZED);
    assert_eq!(served.response(2)["result"]["protocolVersion"], 1);
    assert!(served.response(3).get("result").is_some());
}

#[test]
fn configure_rejects_invalid_configuration() {
    let served = serve(
        &[],
        &[
            initialize(0),
            configure(
                1,
                "typo",
                json!({ "profile": "corpus", "overrides": { "rewrite": { "enabld": false } } }),
            ),
            configure(2, "profile", json!({ "profile": "print" })),
            configure(
                3,
                "package",
                json!({ "profile": "corpus", "packages": ["base", "missing"] }),
            ),
            configure(4, "key", json!({ "profile": "corpus", "profle": "corpus" })),
            normalize(5, "typo", "x", false),
        ],
    );

    let typo = invalid_params_message(&served, 1);
    assert!(typo.contains("config.overrides.rewrite.enabld"), "{typo}");
    assert!(typo.contains("unknown field `enabld`"), "{typo}");
    assert!(invalid_params_message(&served, 2).contains("`print`"));
    assert!(invalid_params_message(&served, 3).contains("missing"));
    assert!(invalid_params_message(&served, 4).contains("`profle`"));
    // A rejected configure registers nothing under its id.
    assert!(invalid_params_message(&served, 5).contains("typo"));
}

#[test]
fn configure_resolves_the_effective_config() {
    let served = serve(
        &["--packages", "ams,base"],
        &[
            initialize(0),
            configure(
                1,
                "same",
                json!({ "profile": "authoring", "overrides": { "rewrite": { "enabled": false } } }),
            ),
            configure(
                2,
                "same",
                json!({ "profile": "equiv", "packages": ["physics", "base", "base"] }),
            ),
            normalize(3, "same", r"a \over b", false),
        ],
    );

    let first = &served.response(1)["result"]["resolved"];
    let engine = TransformEngine::builder()
        .packages(&["ams", "base"])
        .profile(Profile::Authoring)
        .build()
        .unwrap();
    let overrides = read::<NormalizeConfigInput>(json!({ "rewrite": { "enabled": false } }))
        .unwrap()
        .into_config(engine.default_normalize_config());
    let mut expected = serde_json::to_value(NormalizeConfigInput::from_config(overrides)).unwrap();
    expected["profile"] = json!("authoring");
    // The serve-level `--packages` applies when `packages` is omitted.
    expected["packages"] = json!(["ams", "base"]);
    assert_eq!(*first, expected);

    // Reconfiguring an id replaces its engine; packages are reported as the
    // loaded set in catalog order.
    let second = &served.response(2)["result"]["resolved"];
    assert_eq!(second["profile"], "equiv");
    assert_eq!(second["packages"], json!(["base", "physics"]));
    assert_eq!(served.response(3)["result"]["output"], r"\frac { a } { b }");
}

#[test]
fn configure_defaults_match_the_library_and_all_packages_remain_explicit() {
    let served = serve(
        &[],
        &[
            initialize(0),
            configure(1, "default", json!({"profile": "corpus"})),
            configure(
                2,
                "all",
                json!({"profile": "corpus", "packages": all_packages()}),
            ),
            normalize(3, "default", r"\Ket{x}", false),
            normalize(4, "all", r"\Ket{x}", false),
        ],
    );
    assert_eq!(
        served.response(1)["result"]["resolved"]["packages"],
        json!([
            "ams",
            "base",
            "bboldx",
            "boldsymbol",
            "physics",
            "textmacros"
        ])
    );
    assert_eq!(
        served.response(2)["result"]["resolved"]["packages"],
        json!(all_packages())
    );
    assert_eq!(
        served.response(3)["result"]["output"],
        support::engine(Profile::Corpus)
            .normalize(r"\Ket{x}")
            .unwrap()
    );
    assert_ne!(
        served.response(3)["result"]["output"],
        served.response(4)["result"]["output"]
    );
}

#[test]
fn invalid_iteration_limit_preserves_the_previous_configuration() {
    let served = serve(
        &[],
        &[
            initialize(0),
            configure(1, "c", json!({"profile":"corpus"})),
            configure(
                2,
                "c",
                json!({"profile":"corpus", "overrides":{"rewrite":{"max_iterations":0}}}),
            ),
            normalize(3, "c", r"a \over b", false),
        ],
    );
    assert!(invalid_params_message(&served, 2).contains("rewrite.max_iterations"));
    assert_eq!(served.response(3)["result"]["output"], r"\frac { a } { b }");
}

#[test]
fn normalize_failures_report_kind_and_executed_stages() {
    let served = serve(
        &[],
        &[
            initialize(0),
            configure(1, "corpus", json!({ "profile": "corpus" })),
            configure(
                2,
                "one-pass",
                json!({ "profile": "corpus", "overrides": { "rewrite": { "max_iterations": 1 } } }),
            ),
            normalize(3, "corpus", UNCLOSED_FRACTION, true),
            normalize(4, "one-pass", r"a \over b", true),
            normalize(5, "corpus", r"a \over b", true),
        ],
    );

    let parse_failure = &served.response(3)["error"];
    assert_eq!(parse_failure["code"], NORMALIZE_FAILED);
    assert_eq!(parse_failure["data"]["kind"], "parse");
    assert!(
        !parse_failure["data"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        timing_keys(&parse_failure["data"]["timing"]),
        BTreeSet::from(["parse_ns"])
    );

    let transform_failure = &served.response(4)["error"];
    assert_eq!(transform_failure["code"], NORMALIZE_FAILED);
    assert_eq!(transform_failure["data"]["kind"], "transform");
    assert_eq!(
        timing_keys(&transform_failure["data"]["timing"]),
        BTreeSet::from(["parse_ns", "transform_ns"])
    );

    assert_eq!(
        timing_keys(&served.response(5)["result"]["timing"]),
        BTreeSet::from(["parse_ns", "transform_ns", "serialize_ns"])
    );
}

#[test]
fn timing_does_not_change_results() {
    let formulas = [
        r"a \over b",
        r"{\bf x}^2_1 + \left( y \right)",
        r"\begin{matrix} a & b \\ c & d \end{matrix}",
        UNCLOSED_FRACTION,
        r"\notacommand{x}",
    ];
    let configs = [
        ("corpus", json!({ "profile": "corpus" })),
        (
            "one-pass",
            json!({ "profile": "corpus", "overrides": { "rewrite": { "max_iterations": 1 } } }),
        ),
        (
            "strict",
            json!({ "profile": "authoring", "overrides": { "reject_unknown": true } }),
        ),
    ];
    let mut lines = vec![initialize(0)];
    let mut pairs = Vec::new();
    let mut next_id = 1;
    for (config_id, config) in &configs {
        lines.push(configure(next_id, config_id, config.clone()));
        next_id += 1;
        for formula in formulas {
            lines.push(normalize(next_id, config_id, formula, false));
            lines.push(normalize(next_id + 1, config_id, formula, true));
            pairs.push((next_id, next_id + 1));
            next_id += 2;
        }
    }
    let served = serve(&[], &lines);

    for (untimed_id, timed_id) in pairs {
        let untimed = served.response(untimed_id);
        let timed = served.response(timed_id);
        if let Some(output) = untimed.get("result") {
            assert_eq!(timed["result"]["output"], output["output"]);
            assert!(output.get("timing").is_none());
        } else {
            let (untimed, timed) = (&untimed["error"], &timed["error"]);
            assert_eq!(timed["code"], untimed["code"]);
            assert_eq!(timed["message"], untimed["message"]);
            assert_eq!(timed["data"]["kind"], untimed["data"]["kind"]);
            assert_eq!(timed["data"]["diagnostics"], untimed["data"]["diagnostics"]);
            assert!(untimed["data"].get("timing").is_none());
        }
    }
}

#[test]
fn normalize_matches_facade_normalize_with() {
    let formulas = [
        r"a \over b",
        r"\frac12 + \dfrac{1}{2}",
        r"{\bf x} + \mathbf{y} + {\rm d}x",
        r"\left( \frac{a}{b} \right)^{2}_{i}",
        r"\begin{matrix} a & b \\ c & d \end{matrix}",
        r"\sqrt[3]{x} \cdot \binom{n}{k}",
        r"\bra{\psi} \ket{\phi}",
        r"\text{if } x' > 0",
        r"\notacommand{x}",
        UNCLOSED_FRACTION,
    ];
    let packages = vec![
        "base",
        "ams",
        "physics",
        "textmacros",
        "bboldx",
        "boldsymbol",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let configs = [
        (
            "authoring",
            "authoring",
            Profile::Authoring,
            None,
            json!({}),
        ),
        ("faithful", "faithful", Profile::Faithful, None, json!({})),
        ("corpus", "corpus", Profile::Corpus, None, json!({})),
        ("equiv", "equiv", Profile::Equiv, None, json!({})),
        (
            "authoring-no-rewrite",
            "authoring",
            Profile::Authoring,
            None,
            json!({ "rewrite": { "enabled": false } }),
        ),
        (
            "corpus-strict",
            "corpus",
            Profile::Corpus,
            None,
            json!({ "reject_unknown": true, "abort_on_error": true }),
        ),
        (
            "equiv-base-only",
            "equiv",
            Profile::Equiv,
            Some(vec!["base".to_owned()]),
            json!({ "flatten_groups": { "enabled": false }, "lower_attributes": { "enabled": false } }),
        ),
    ];

    let mut lines = vec![initialize(0)];
    let mut expectations = Vec::new();
    let mut next_id = 1;
    for (config_id, profile_name, profile, config_packages, overrides) in &configs {
        let mut config = json!({ "profile": profile_name, "overrides": overrides });
        if let Some(config_packages) = config_packages {
            config["packages"] = json!(config_packages);
        }
        lines.push(configure(next_id, config_id, config));
        next_id += 1;

        let selected = config_packages.as_ref().unwrap_or(&packages);
        let names: Vec<&str> = selected.iter().map(String::as_str).collect();
        let engine = TransformEngine::builder()
            .packages(&names)
            .profile(*profile)
            .build()
            .unwrap();
        let normalize_config = read::<NormalizeConfigInput>(overrides.clone())
            .unwrap()
            .into_config(engine.default_normalize_config());
        for formula in formulas {
            lines.push(normalize(next_id, config_id, formula, false));
            let expected = engine.normalize_with(formula, &normalize_config);
            expectations.push((next_id, *config_id, formula, expected));
            next_id += 1;
        }
    }
    let served = serve(&[], &lines);

    let mut failures = 0;
    for (id, config_id, formula, expected) in expectations {
        let response = served.response(id);
        let context = format!("config {config_id}, formula {formula:?}: {response}");
        match expected {
            Ok(output) => assert_eq!(response["result"]["output"], output, "{context}"),
            Err(error) => {
                failures += 1;
                let expected = normalize_error_to_parts(error).error;
                let error = &response["error"];
                assert_eq!(error["code"], NORMALIZE_FAILED, "{context}");
                assert_eq!(error["message"], expected.message, "{context}");
                assert_eq!(error["data"]["kind"], expected.kind, "{context}");
                assert_eq!(
                    error["data"]["diagnostics"],
                    serde_json::to_value(&expected.diagnostics).unwrap(),
                    "{context}"
                );
            }
        }
    }
    assert!(failures > 0, "the comparison should cover failing inputs");
}

#[test]
fn protocol_errors_follow_json_rpc() {
    let mut input = String::new();
    for line in [
        "not json".to_owned(),
        "[]".to_owned(),
        json!([{ "jsonrpc": "2.0", "id": 9, "method": "initialize" }]).to_string(),
        json!({ "id": 1, "method": "initialize" }).to_string(),
        json!({ "jsonrpc": "2.0", "id": "init", "method": "initialize" }).to_string(),
        json!({ "jsonrpc": "2.0", "id": null, "method": "missing" }).to_string(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "normalize", "params": ["corpus", "x"] })
            .to_string(),
        // Notifications are executed but never answered, even on failure.
        json!({ "jsonrpc": "2.0", "method": "missing" }).to_string(),
        json!({ "jsonrpc": "2.0", "method": "configure", "params": { "id": "corpus", "config": { "profile": "corpus" } } }).to_string(),
        normalize(3, "corpus", "x", false),
        // Blank lines are skipped.
        "  ".to_owned(),
    ] {
        input.push_str(&line);
        input.push('\n');
    }
    let mut input = input.into_bytes();
    input.extend_from_slice(b"\xff\xfe\n");
    let served = serve_bytes(&[], input);

    let codes: Vec<(Value, i64)> = served
        .responses
        .iter()
        .map(|response| {
            let code = response["error"]["code"].as_i64().unwrap_or(0);
            (response["id"].clone(), code)
        })
        .collect();
    assert_eq!(
        codes,
        vec![
            (Value::Null, PARSE_ERROR),
            (Value::Null, INVALID_REQUEST),
            (Value::Null, INVALID_REQUEST),
            (json!(1), INVALID_REQUEST),
            (json!("init"), 0),
            (Value::Null, METHOD_NOT_FOUND),
            (json!(2), INVALID_PARAMS),
            (json!(3), 0),
            (Value::Null, PARSE_ERROR),
        ]
    );
    assert_eq!(served.response(3)["result"]["output"], "x");
}

#[test]
fn shutdown_rejects_later_requests() {
    let served = serve(
        &[],
        &[
            initialize(0),
            request(1, "shutdown", json!({})),
            initialize(2),
            request(3, "shutdown", json!({})),
        ],
    );

    assert_eq!(served.response(1)["result"], Value::Null);
    assert_eq!(served.error_code(2), INVALID_REQUEST);
    assert_eq!(served.error_code(3), INVALID_REQUEST);
    assert!(served.status.success());
}

#[test]
fn stdin_eof_answers_pending_requests_and_exits_zero() {
    let served = serve(
        &[],
        &[
            initialize(0),
            configure(1, "corpus", json!({ "profile": "corpus" })),
            normalize(2, "corpus", r"a \over b", false),
        ],
    );

    assert_eq!(served.responses.len(), 3);
    assert_eq!(served.response(2)["result"]["output"], r"\frac { a } { b }");
    assert_eq!(served.status.code(), Some(0));
}

#[test]
fn unknown_startup_package_is_a_usage_error() {
    let output = Command::new(BIN)
        .args(["serve", "--packages", "base,missing"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing"));
}
