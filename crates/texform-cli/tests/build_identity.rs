//! `--version` and the serve `serverInfo` report the same build identity.

mod support;

use std::process::Command;

use support::{BIN, initialize, serve};

#[test]
fn version_output_matches_server_info() {
    let output = Command::new(BIN).arg("--version").output().unwrap();
    assert!(output.status.success());
    let version_line = String::from_utf8(output.stdout).unwrap();

    let served = serve(&[], &[initialize(0)]);
    let info = &served.response(0)["result"]["serverInfo"];
    assert_eq!(info["name"], "texform");
    let version = info["version"].as_str().unwrap();
    assert_eq!(version, env!("CARGO_PKG_VERSION"));

    let expected = match info["commit"].as_str() {
        Some(commit) => {
            assert_eq!(commit.len(), 40, "full commit hash: {commit}");
            let date = info["commitDate"].as_str().expect("commitDate with commit");
            let dirty = if info["dirty"].as_bool().unwrap() {
                ", dirty"
            } else {
                ""
            };
            format!("texform {version} ({} {date}{dirty})\n", &commit[..12])
        }
        None => {
            assert!(info["commitDate"].is_null());
            assert_eq!(info["dirty"], false);
            format!("texform {version}\n")
        }
    };
    assert_eq!(version_line, expected);
}
