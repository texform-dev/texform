//! `texform serve`: the normalizer protocol over stdio.
//!
//! One JSON-RPC 2.0 message per line in each direction. Requests are handled
//! strictly in arrival order, and every response is flushed immediately
//! because clients typically wait for each response before sending the next
//! request. The crate README is the protocol reference.

mod rpc;
mod server;

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use serde_json::Value;

use self::rpc::{Request, Response, RpcError};
use self::server::Server;
use crate::output::catch_processing_panic;

/// Serve requests from stdin until end of file.
///
/// `default_packages` applies to `configure` requests that omit `packages`.
pub fn run(default_packages: Vec<String>) -> ExitCode {
    let mut server = Server::new(default_packages);
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut line = Vec::new();
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) => return ExitCode::SUCCESS,
            Ok(_) => {}
            Err(error) => {
                eprintln!("texform serve: cannot read stdin: {error}");
                return ExitCode::FAILURE;
            }
        }
        let Some(response) = handle_line(&mut server, &line) else {
            continue;
        };
        if let Err(error) = write_response(&mut output, &response) {
            eprintln!("texform serve: cannot write stdout: {error}");
            return ExitCode::FAILURE;
        }
    }
}

/// Handle one input line; `None` means nothing is written back (a blank line
/// or a notification).
fn handle_line(server: &mut Server, line: &[u8]) -> Option<Response> {
    let Ok(text) = std::str::from_utf8(line) else {
        let error = RpcError::parse_error("message is not valid UTF-8");
        return Some(Response::new(Value::Null, Err(error)));
    };
    if text.trim().is_empty() {
        return None;
    }
    let value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            return Some(Response::new(
                Value::Null,
                Err(RpcError::parse_error(error)),
            ));
        }
    };
    let request = match Request::from_value(value) {
        Ok(request) => request,
        Err(invalid) => return Some(Response::new(invalid.id, Err(invalid.error))),
    };
    // A panic is a bug, but one bad formula must not end a long-running
    // session, so it becomes an `internal` failure of this request only.
    let outcome = catch_processing_panic(|| server.dispatch(&request.method, request.params))
        .unwrap_or_else(|payload| Err(server::panic_error(payload.as_ref())));
    request.id.map(|id| Response::new(id, outcome))
}

fn write_response(output: &mut impl Write, response: &Response) -> io::Result<()> {
    serde_json::to_writer(&mut *output, response)?;
    output.write_all(b"\n")?;
    output.flush()
}
