//! Embeds the build identity reported by `texform --version` and by the serve
//! protocol's `serverInfo`.
//!
//! The identity must describe the source tree that was actually compiled, so
//! the script reruns whenever build inputs or the checked-out commit change.
//! Missing git information (crates.io sources, `git archive` trees, no `git`
//! executable) is not an error: the identity then carries only the version.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace paths that feed the build. `dirty` is computed over exactly these
/// paths, and they are also watched for reruns, so a change that alters the
/// build always refreshes the identity.
const BUILD_INPUTS: [&str; 3] = ["crates", "Cargo.toml", "Cargo.lock"];

struct GitIdentity {
    commit: String,
    commit_date: String,
    dirty: bool,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let version = env::var("CARGO_PKG_VERSION").expect("set by Cargo");
    // The crate lives at `<workspace>/crates/texform-cli`.
    let root = manifest_dir.join("..").join("..");
    let root = fs::canonicalize(&root).unwrap_or(root);

    println!("cargo:rerun-if-changed=build.rs");
    // Cargo treats a missing `rerun-if-changed` path as always stale, so only
    // existing paths are watched.
    for input in BUILD_INPUTS {
        watch_if_exists(&root.join(input));
    }

    let identity = if is_repository_root(&root) {
        for path in git_state_paths(&root) {
            watch_if_exists(&path);
        }
        git_identity(&root)
    } else {
        None
    };

    let (commit, commit_date, dirty, version_text) = match &identity {
        Some(identity) => {
            let short = &identity.commit[..identity.commit.len().min(12)];
            let suffix = if identity.dirty { ", dirty" } else { "" };
            (
                identity.commit.as_str(),
                identity.commit_date.as_str(),
                identity.dirty,
                format!("{version} ({short} {}{suffix})", identity.commit_date),
            )
        }
        None => ("", "", false, version),
    };
    println!("cargo:rustc-env=TEXFORM_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=TEXFORM_BUILD_COMMIT_DATE={commit_date}");
    println!("cargo:rustc-env=TEXFORM_BUILD_DIRTY={dirty}");
    println!("cargo:rustc-env=TEXFORM_BUILD_VERSION_TEXT={version_text}");
}

fn watch_if_exists(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Whether `root` is the top level of a git work tree.
///
/// A packaged crate unpacked below some unrelated repository (for example a
/// dotfiles repository containing `~/.cargo`, or `target/package` during
/// `cargo package`) must not pick up that repository's commit.
fn is_repository_root(root: &Path) -> bool {
    let Some(toplevel) = git(root, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    fs::canonicalize(toplevel).is_ok_and(|toplevel| toplevel == root)
}

/// Git files whose modification signals a new commit, branch switch, or
/// staging change. Paths come from `git rev-parse --git-path` so linked
/// worktrees and submodules, whose `.git` is a file, resolve correctly.
fn git_state_paths(root: &Path) -> Vec<PathBuf> {
    // `logs/HEAD` is rewritten by every commit in this work tree, which covers
    // a first commit on a branch whose ref exists only in `packed-refs`.
    let mut names = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
        "logs/HEAD".to_owned(),
    ];
    if let Some(branch_ref) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        names.push(branch_ref);
    }
    names
        .iter()
        .filter_map(|name| git(root, &["rev-parse", "--git-path", name]))
        .map(|path| root.join(path))
        .collect()
}

fn git_identity(root: &Path) -> Option<GitIdentity> {
    let head = git(root, &["log", "-1", "--format=%H %cs", "HEAD"])?;
    let (commit, commit_date) = head.split_once(' ')?;
    let mut status_args = vec!["status", "--porcelain", "--"];
    status_args.extend(BUILD_INPUTS);
    let status = git(root, &status_args)?;
    Some(GitIdentity {
        commit: commit.to_owned(),
        commit_date: commit_date.to_owned(),
        dirty: !status.is_empty(),
    })
}

/// Run `git` in `root` and return trimmed stdout, or `None` on any failure.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        // Without this, `git status` refreshes and rewrites the index, which
        // would change a watched file and make every build rerun this script.
        .arg("--no-optional-locks")
        .args(args)
        // Discover the repository from the source tree rather than from an
        // inherited environment such as a git hook of an enclosing repository.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(stdout.trim().to_owned())
}
