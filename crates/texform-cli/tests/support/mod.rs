//! Helpers for driving the built `texform` binary.

#![allow(dead_code)]

use std::io::{ErrorKind, Write};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;

use serde_json::{Value, json};
use texform::{Parser, Profile, TransformEngine};

pub const BIN: &str = env!("CARGO_BIN_EXE_texform");

/// Facade engine using the library's default package selection.
pub fn engine(profile: Profile) -> TransformEngine {
    TransformEngine::builder().profile(profile).build().unwrap()
}

/// Facade parser using the library's default package selection.
pub fn parser() -> Parser {
    Parser::builder().build().unwrap()
}

/// Everything one `texform` invocation wrote before exiting.
pub struct Run {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn code(&self) -> i32 {
        self.status
            .code()
            .expect("texform exited with a status code")
    }

    pub fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    pub fn json_lines(&self) -> Vec<Value> {
        self.stdout
            .lines()
            .map(|line| serde_json::from_str(line).expect("each stdout line is JSON"))
            .collect()
    }
}

/// Run `texform` with `args`, feed it `stdin`, and close stdin.
pub fn texform(args: &[&str], stdin: &str) -> Run {
    let output = run_with_stdin(args, stdin.as_bytes().to_vec());
    Run {
        status: output.status,
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

fn run_with_stdin(args: &[&str], input: Vec<u8>) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn texform");
    let mut stdin = child.stdin.take().expect("piped stdin");
    // Write from another thread so a full stdout pipe cannot deadlock the test.
    let writer = thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().expect("wait for texform");
    // The CLI may exit before reading stdin, for example on a usage error.
    match writer.join().expect("stdin writer thread") {
        Err(error) if error.kind() != ErrorKind::BrokenPipe => panic!("write stdin: {error}"),
        _ => {}
    }
    output
}

/// One JSON-RPC request line.
pub fn request(id: impl Into<Value>, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id.into(), "method": method, "params": params }).to_string()
}

pub fn initialize(id: i64) -> String {
    request(id, "initialize", json!({}))
}

pub fn configure(id: i64, config_id: &str, config: Value) -> String {
    request(
        id,
        "configure",
        json!({ "id": config_id, "config": config }),
    )
}

pub fn normalize(id: i64, config_id: &str, latex: &str, timing: bool) -> String {
    request(
        id,
        "normalize",
        json!({ "config": config_id, "latex": latex, "timing": timing }),
    )
}

/// Everything a `texform serve` process wrote before exiting.
pub struct Served {
    pub responses: Vec<Value>,
    pub status: ExitStatus,
}

impl Served {
    /// The response carrying `id`; ids are unique within a test session.
    pub fn response(&self, id: impl Into<Value>) -> &Value {
        let id = id.into();
        self.responses
            .iter()
            .find(|response| response["id"] == id)
            .unwrap_or_else(|| panic!("no response with id {id}: {:#?}", self.responses))
    }

    pub fn error_code(&self, id: impl Into<Value>) -> i64 {
        let response = self.response(id);
        response["error"]["code"]
            .as_i64()
            .unwrap_or_else(|| panic!("expected an error response: {response}"))
    }
}

/// Run `texform serve`, feed it `lines`, and close stdin.
pub fn serve(args: &[&str], lines: &[String]) -> Served {
    let mut input = lines.join("\n").into_bytes();
    input.push(b'\n');
    serve_bytes(args, input)
}

/// Run `texform serve` over raw stdin bytes and close stdin.
pub fn serve_bytes(args: &[&str], input: Vec<u8>) -> Served {
    let mut command = vec!["serve"];
    command.extend_from_slice(args);
    let output = run_with_stdin(&command, input);
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("each stdout line is JSON"))
        .collect();
    Served {
        responses,
        status: output.status,
    }
}
