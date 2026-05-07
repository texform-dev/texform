//! Build script that auto-discovers builtin transform rule modules.
//!
//! Rule files live under `src/transform/rules/` and declare exactly one
//! `pub static UPPER_SNAKE: SomeRuleType`, where the constant name is the
//! UPPER_SNAKE_CASE form of the file stem. Support files named `helpers.rs`
//! or `_*.rs` are emitted as normal modules but are not added to `ALL_RULES`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModuleEntry {
    relative_path: PathBuf,
    module_path: Vec<String>,
    kind: ModuleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ModuleKind {
    Rule,
    Support,
}

fn rust_module_name(component: &str) -> String {
    let mut module_name = String::new();
    for ch in component.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            module_name.push(ch.to_ascii_lowercase());
        } else {
            module_name.push('_');
        }
    }

    if module_name.is_empty()
        || !module_name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        module_name.insert(0, '_');
    }

    module_name
}

fn module_path(relative_path: &Path) -> Vec<String> {
    relative_path
        .with_extension("")
        .iter()
        .map(|component| rust_module_name(component.to_str().unwrap()))
        .collect()
}

fn is_support_module(file_name: &str) -> bool {
    file_name == "helpers.rs" || file_name.starts_with('_')
}

fn constant_name(relative_path: &Path) -> String {
    relative_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap()
        .to_ascii_uppercase()
}

fn collect_module_entries(base_dir: &Path, dir: &Path, entries: &mut Vec<ModuleEntry>) {
    let mut children: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    children.sort();

    for path in children {
        if path.is_dir() {
            collect_module_entries(base_dir, &path, entries);
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name == "mod.rs" || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let relative_path = path.strip_prefix(base_dir).unwrap().to_path_buf();
        let module_path = module_path(&relative_path);
        let kind = if is_support_module(file_name) {
            ModuleKind::Support
        } else {
            ModuleKind::Rule
        };

        entries.push(ModuleEntry {
            relative_path,
            module_path,
            kind,
        });
    }
}

fn validate_module_paths(entries: &[ModuleEntry]) {
    let mut seen_files = BTreeMap::new();
    for entry in entries {
        if let Some(existing_path) = seen_files.insert(&entry.module_path, &entry.relative_path) {
            panic!(
                "duplicate generated module path '{}' for '{}' and '{}'",
                entry.module_path.join("::"),
                existing_path.display(),
                entry.relative_path.display()
            );
        }
    }

    let file_modules: BTreeSet<Vec<String>> = entries
        .iter()
        .map(|entry| entry.module_path.clone())
        .collect();
    for entry in entries {
        let mut parent = entry.module_path.clone();
        parent.pop();
        while !parent.is_empty() {
            if file_modules.contains(&parent) {
                panic!(
                    "generated module path '{}' conflicts with ancestor module '{}'",
                    entry.module_path.join("::"),
                    parent.join("::")
                );
            }
            parent.pop();
        }
    }
}

fn write_module_tree(
    code: &mut String,
    rules_dir: &Path,
    entries: &[ModuleEntry],
    prefix: &[String],
    indent: usize,
) {
    let mut child_modules = BTreeSet::new();
    let mut file_entries = Vec::new();

    for entry in entries {
        if !entry.module_path.starts_with(prefix) {
            continue;
        }
        match entry.module_path.get(prefix.len()) {
            Some(child) if entry.module_path.len() == prefix.len() + 1 => {
                file_entries.push(entry);
                child_modules.insert(child.clone());
            }
            Some(child) => {
                child_modules.insert(child.clone());
            }
            None => {}
        }
    }

    for child in child_modules {
        let child_prefix = {
            let mut path = prefix.to_vec();
            path.push(child.clone());
            path
        };

        if let Some(entry) = file_entries
            .iter()
            .find(|entry| entry.module_path == child_prefix)
        {
            let abs_path = rules_dir.join(&entry.relative_path);
            writeln!(
                code,
                "{:indent$}#[path = \"{}\"]",
                "",
                abs_path.display().to_string().replace('\\', "/"),
                indent = indent
            )
            .unwrap();
            writeln!(
                code,
                "{:indent$}pub(crate) mod {};",
                "",
                child,
                indent = indent
            )
            .unwrap();
            continue;
        }

        writeln!(
            code,
            "{:indent$}pub(crate) mod {} {{",
            "",
            child,
            indent = indent
        )
        .unwrap();
        write_module_tree(code, rules_dir, entries, &child_prefix, indent + 4);
        writeln!(code, "{:indent$}}}", "", indent = indent).unwrap();
    }
}

fn rule_path(entry: &ModuleEntry) -> String {
    entry.module_path.join("::")
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let rules_dir = Path::new(&manifest_dir).join("src/transform/rules");
    let out_dir = env::var("OUT_DIR").unwrap();

    // Re-run if any file in the rules directory changes.
    println!("cargo:rerun-if-changed={}", rules_dir.display());

    let mut entries = Vec::new();
    collect_module_entries(&rules_dir, &rules_dir, &mut entries);
    entries.sort();
    validate_module_paths(&entries);

    let mut code = String::new();

    writeln!(
        code,
        "// Auto-generated by build.rs - do not edit.\n\
         //\n\
         // Builtin transform rule registry. Rule `.rs` files under\n\
         // `src/transform/rules/` are automatically registered here.\n\
         // Support modules named `helpers.rs` or `_*.rs` are emitted but not\n\
         // added to `ALL_RULES`.\n"
    )
    .unwrap();

    writeln!(code, "use crate::transform::rule::TransformRule;\n").unwrap();
    write_module_tree(&mut code, &rules_dir, &entries, &[], 0);

    writeln!(code).unwrap();
    writeln!(
        code,
        "pub(crate) static ALL_RULES: &[&dyn TransformRule] = &["
    )
    .unwrap();
    for entry in entries
        .iter()
        .filter(|entry| entry.kind == ModuleKind::Rule)
    {
        writeln!(
            code,
            "    &{}::{},",
            rule_path(entry),
            constant_name(&entry.relative_path)
        )
        .unwrap();
    }
    writeln!(code, "];").unwrap();

    let out_path = Path::new(&out_dir).join("rules_registry.rs");
    fs::write(&out_path, code).expect("failed to write rules_registry.rs");
}

#[cfg(test)]
mod tests {
    use super::{constant_name, is_support_module, module_path, rust_module_name};
    use std::path::Path;

    #[test]
    fn module_names_are_plain_rust_identifiers() {
        assert_eq!(rust_module_name("derivative-expand"), "derivative_expand");
        assert_eq!(rust_module_name("trace_alias"), "trace_alias");
        assert_eq!(rust_module_name("123-group"), "_123_group");
    }

    #[test]
    fn module_path_preserves_directory_shape() {
        assert_eq!(
            module_path(Path::new(
                "physics/expand/derivative-expand/dv_to_frac_d.rs"
            )),
            vec!["physics", "expand", "derivative_expand", "dv_to_frac_d"]
        );
    }

    #[test]
    fn support_modules_are_not_rules() {
        assert!(is_support_module("helpers.rs"));
        assert!(is_support_module("_shared.rs"));
        assert!(!is_support_module("dv_to_frac_d.rs"));
    }

    #[test]
    fn rule_constant_uses_file_stem() {
        assert_eq!(
            constant_name(Path::new(
                "physics/expand/derivative-expand/dv_to_frac_d.rs"
            )),
            "DV_TO_FRAC_D"
        );
    }
}
