//! Build script that auto-discovers builtin rewrite rule modules and emits
//! the LowerAttributes phase data tables.
//!
//! Rule files live under `src/rewrite/rules/` and declare exactly one
//! `pub static UPPER_SNAKE: SomeRuleType`, where the constant name is the
//! UPPER_SNAKE_CASE form of the file stem. Support files named `helpers.rs` or
//! `_*.rs` are emitted as normal modules but are not added to `ALL_RULES`. The
//! generated registry file itself is ignored by discovery.
//!
//! LowerAttributes data is generated from
//! `src/lower_attributes/data.yaml` via the codegen module included
//! below.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

#[path = "src/lower_attributes/codegen.rs"]
mod lower_attributes_codegen;

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

fn is_valid_directory_module_component(component: &str) -> bool {
    if component.is_empty() || component == "_" {
        return false;
    }

    let mut chars = component.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_lowercase()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn is_valid_file_module_component(component: &str) -> bool {
    if component.is_empty() || component == "_" {
        return false;
    }

    let mut chars = component.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_alphabetic() || ch.is_ascii_digit())
}

fn validate_directory_module_component(component: &str, path: &Path) {
    if !is_valid_directory_module_component(component) {
        panic!(
            "rule directory path component '{}' in '{}' must be a standard snake_case Rust module name",
            component,
            path.display()
        );
    }
}

fn validate_file_module_component(component: &str, path: &Path) {
    if !is_valid_file_module_component(component) {
        panic!(
            "rule file stem '{}' in '{}' must be a valid Rust module name; use '_' instead of '-' and preserve rule-id case",
            component,
            path.display()
        );
    }
}

fn module_path(relative_path: &Path) -> Vec<String> {
    let components: Vec<String> = relative_path
        .with_extension("")
        .iter()
        .map(|component| {
            let component = component.to_str().unwrap();
            component.to_owned()
        })
        .collect();
    let last_index = components
        .len()
        .checked_sub(1)
        .expect("rule module paths are never empty");
    for (index, component) in components.iter().enumerate() {
        if index == last_index {
            validate_file_module_component(component, relative_path);
        } else {
            validate_directory_module_component(component, relative_path);
        }
    }
    components
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
        if file_name == "mod.rs"
            || file_name == "generated.rs"
            || path.extension().and_then(|ext| ext.to_str()) != Some("rs")
        {
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

fn needs_non_snake_case_allow(module_name: &str) -> bool {
    module_name.chars().any(|ch| ch.is_ascii_uppercase())
}

fn write_module_tree(code: &mut String, entries: &[ModuleEntry], prefix: &[String], indent: usize) {
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

        if file_entries
            .iter()
            .any(|entry| entry.module_path == child_prefix)
        {
            if prefix.last() == Some(&child) {
                writeln!(
                    code,
                    "{:indent$}#[allow(clippy::module_inception)]",
                    "",
                    indent = indent
                )
                .unwrap();
            }
            if needs_non_snake_case_allow(&child) {
                writeln!(
                    code,
                    "{:indent$}#[allow(non_snake_case)]",
                    "",
                    indent = indent
                )
                .unwrap();
            }
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

        if prefix.last() == Some(&child) {
            writeln!(
                code,
                "{:indent$}#[allow(clippy::module_inception)]",
                "",
                indent = indent
            )
            .unwrap();
        }
        if needs_non_snake_case_allow(&child) {
            writeln!(
                code,
                "{:indent$}#[allow(non_snake_case)]",
                "",
                indent = indent
            )
            .unwrap();
        }
        writeln!(
            code,
            "{:indent$}pub(crate) mod {} {{",
            "",
            child,
            indent = indent
        )
        .unwrap();
        write_module_tree(code, entries, &child_prefix, indent + 4);
        writeln!(code, "{:indent$}}}", "", indent = indent).unwrap();
    }
}

fn rule_path(entry: &ModuleEntry) -> String {
    entry.module_path.join("::")
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir_path = Path::new(&manifest_dir);
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    lower_attributes_codegen::generate(manifest_dir_path, &out_dir);

    let rules_dir = manifest_dir_path.join("src/rewrite/rules");
    println!("cargo:rerun-if-changed={}", rules_dir.display());
    println!("cargo:rerun-if-changed=src/lower_attributes/data.yaml");
    println!("cargo:rerun-if-changed=src/lower_attributes/codegen.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let mut entries = Vec::new();
    collect_module_entries(&rules_dir, &rules_dir, &mut entries);
    entries.sort();
    validate_module_paths(&entries);

    let mut code = String::new();

    writeln!(
        code,
        "// Auto-generated by build.rs - do not edit.\n\
         //\n\
         // Builtin rewrite rule registry. Rule `.rs` files under\n\
         // `src/rewrite/rules/` are automatically registered here.\n\
         // Support modules named `helpers.rs` or `_*.rs` are emitted but not\n\
         // added to `ALL_RULES`.\n"
    )
    .unwrap();

    writeln!(code, "use crate::rewrite::rule::RewriteRule;\n").unwrap();
    write_module_tree(&mut code, &entries, &[], 0);

    writeln!(code).unwrap();
    writeln!(
        code,
        "pub(crate) static ALL_RULES: &[&dyn RewriteRule] = &["
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

    let out_path = rules_dir.join("generated.rs");
    let should_write = fs::read_to_string(&out_path).map_or(true, |existing| existing != code);
    if should_write {
        fs::write(&out_path, code).expect("failed to write rewrite/rules/generated.rs");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        constant_name, is_support_module, is_valid_directory_module_component,
        is_valid_file_module_component, module_path, needs_non_snake_case_allow,
    };
    use std::path::Path;

    #[test]
    fn directory_names_are_standard_snake_case_modules() {
        assert!(is_valid_directory_module_component("derivative_expand"));
        assert!(is_valid_directory_module_component("trace_alias"));
        assert!(is_valid_directory_module_component("_shared"));
        assert!(!is_valid_directory_module_component("derivative-expand"));
        assert!(!is_valid_directory_module_component("trace_capital_to_Tr"));
        assert!(!is_valid_directory_module_component("123_group"));
    }

    #[test]
    fn file_names_may_preserve_rule_id_case() {
        assert!(is_valid_file_module_component("trace_capital_to_Tr"));
        assert!(is_valid_file_module_component("Bqty_to_brace_fence"));
        assert!(is_valid_file_module_component("implies_to_Longrightarrow"));
        assert!(!is_valid_file_module_component("implies-to-Longrightarrow"));
        assert!(!is_valid_file_module_component("123_rule"));
    }

    #[test]
    fn module_path_preserves_directory_shape() {
        assert_eq!(
            module_path(Path::new(
                "physics/faithful/ams_operator_alias/implies_to_Longrightarrow.rs"
            )),
            vec![
                "physics",
                "faithful",
                "ams_operator_alias",
                "implies_to_Longrightarrow"
            ]
        );
    }

    #[test]
    fn uppercase_file_modules_get_non_snake_case_allow() {
        assert!(needs_non_snake_case_allow("implies_to_Longrightarrow"));
        assert!(!needs_non_snake_case_allow("dv_to_frac_d"));
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
                "physics/faithful/derivative_expand/dv_to_frac_d.rs"
            )),
            "DV_TO_FRAC_D"
        );
    }
}
