use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use texform_argspec::parse_arg_specs;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let specs_dir = manifest_dir.join("../../resources/specs");
    let out_path = manifest_dir.join("src/builtin/generated.rs");

    println!("cargo:rerun-if-changed={}", specs_dir.display());

    let mut entries = fs::read_dir(&specs_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", specs_dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("yaml"))
        .collect::<Vec<_>>();
    entries.sort();

    let packages = entries
        .iter()
        .map(|path| load_package(path.as_path()))
        .collect::<Vec<_>>();

    fs::write(out_path, generate_builtin_source(packages.as_slice()))
        .unwrap_or_else(|err| panic!("failed to write generated builtin source: {err}"));
}

#[derive(Debug)]
struct BuiltinPackageSource {
    name: String,
    commands: Vec<CommandRecordSource>,
    environments: Vec<EnvironmentRecordSource>,
    characters: Vec<CharacterRecordSource>,
    delimiter_controls: Vec<String>,
}

#[derive(Debug)]
struct CommandRecordSource {
    name: String,
    kind: CommandKindYaml,
    allowed_mode: AllowedModeYaml,
    tags: Vec<String>,
    spec_string: String,
}

#[derive(Debug)]
struct EnvironmentRecordSource {
    name: String,
    allowed_mode: AllowedModeYaml,
    body_mode: ContentModeYaml,
    tags: Vec<String>,
    spec_string: String,
}

#[derive(Debug)]
struct CharacterRecordSource {
    name: String,
    allowed_mode: AllowedModeYaml,
    unicode_value: String,
    attributes: CharacterAttributesYaml,
}

#[derive(Debug, Default, Deserialize)]
struct PackageSpecsYaml {
    #[serde(default)]
    characters: Vec<CharacterSpecYaml>,
    #[serde(default)]
    commands: Vec<CommandSpecYaml>,
    #[serde(default)]
    environments: Vec<EnvironmentSpecYaml>,
    #[serde(default)]
    delimiter_controls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CharacterSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
    unicode_value: String,
    attributes: CharacterAttributesYaml,
}

#[derive(Debug, Default, Deserialize)]
struct CharacterAttributesYaml {
    #[serde(default)]
    mathvariant: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CommandSpecYaml {
    name: String,
    kind: CommandKindYaml,
    #[serde(default)]
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    spec: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CommandKindYaml {
    Prefix,
    Infix,
    Declarative,
}

impl CommandKindYaml {
    fn code(self) -> &'static str {
        match self {
            CommandKindYaml::Prefix => "CommandKind::Prefix",
            CommandKindYaml::Infix => "CommandKind::Infix",
            CommandKindYaml::Declarative => "CommandKind::Declarative",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum AllowedModeYaml {
    Math,
    Text,
    #[default]
    Both,
}

impl AllowedModeYaml {
    fn code(self) -> &'static str {
        match self {
            AllowedModeYaml::Math => "AllowedMode::Math",
            AllowedModeYaml::Text => "AllowedMode::Text",
            AllowedModeYaml::Both => "AllowedMode::Both",
        }
    }
}

#[derive(Debug, Deserialize)]
struct EnvironmentSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    spec: String,
    body_mode: ContentModeYaml,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContentModeYaml {
    Math,
    Text,
}

impl ContentModeYaml {
    fn code(self) -> &'static str {
        match self {
            ContentModeYaml::Math => "ContentMode::Math",
            ContentModeYaml::Text => "ContentMode::Text",
        }
    }
}

fn load_package(path: &Path) -> BuiltinPackageSource {
    let package_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_else(|| panic!("invalid spec filename: {}", path.display()))
        .to_string();
    let yaml = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let parsed: PackageSpecsYaml = serde_yaml::from_str(&yaml)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));

    let commands = parsed
        .commands
        .into_iter()
        .map(|command| {
            let context = format!("command {}", command.name);
            parse_arg_specs(command.spec.as_str(), context.as_str()).unwrap_or_else(|err| {
                panic!("{err}");
            });
            CommandRecordSource {
                name: command.name,
                kind: command.kind,
                allowed_mode: command.allowed_mode,
                tags: command.tags,
                spec_string: command.spec,
            }
        })
        .collect::<Vec<_>>();
    let environments = parsed
        .environments
        .into_iter()
        .map(|environment| {
            let context = format!("environment {}", environment.name);
            parse_arg_specs(environment.spec.as_str(), context.as_str()).unwrap_or_else(|err| {
                panic!("{err}");
            });
            EnvironmentRecordSource {
                name: environment.name,
                allowed_mode: environment.allowed_mode,
                body_mode: environment.body_mode,
                tags: environment.tags,
                spec_string: environment.spec,
            }
        })
        .collect::<Vec<_>>();
    let characters = parsed
        .characters
        .into_iter()
        .map(|character| CharacterRecordSource {
            name: character.name,
            allowed_mode: character.allowed_mode,
            unicode_value: character.unicode_value,
            attributes: character.attributes,
        })
        .collect::<Vec<_>>();

    assert_unique_names(
        package_name.as_str(),
        "command",
        commands.iter().map(|record| record.name.as_str()),
    );
    assert_unique_names(
        package_name.as_str(),
        "environment",
        environments.iter().map(|record| record.name.as_str()),
    );
    assert_unique_names(
        package_name.as_str(),
        "character",
        characters.iter().map(|record| record.name.as_str()),
    );
    assert_unique_names(
        package_name.as_str(),
        "delimiter control",
        parsed.delimiter_controls.iter().map(String::as_str),
    );
    BuiltinPackageSource {
        name: package_name,
        commands,
        environments,
        characters,
        delimiter_controls: parsed.delimiter_controls,
    }
}

fn assert_unique_names<'a>(
    package: &str,
    record_kind: &str,
    names: impl IntoIterator<Item = &'a str>,
) {
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name.to_string()) {
            panic!("duplicate {record_kind} `{name}` in package `{package}`");
        }
    }
}

fn generate_builtin_source(packages: &[BuiltinPackageSource]) -> String {
    let mut out = String::new();
    out.push_str("// @generated by build.rs\n");

    for package in packages {
        emit_package_module(&mut out, package);
    }

    out.push_str("pub static ALL_PACKAGES: &[BuiltinPackage] = &[\n");
    for package in packages {
        writeln!(
            out,
            "    BuiltinPackage {{ name: {:?}, commands: {}::cmd::ALL, environments: {}::env::ALL, characters: {}::chars::ALL, delimiter_controls: {}::DELIMITER_CONTROLS }},",
            package.name,
            package_module_ident(package.name.as_str()),
            package_module_ident(package.name.as_str()),
            package_module_ident(package.name.as_str()),
            package_module_ident(package.name.as_str()),
        )
        .unwrap();
    }
    out.push_str("];\n");

    out
}

fn emit_package_module(out: &mut String, package: &BuiltinPackageSource) {
    let package_ident = package_module_ident(package.name.as_str());
    writeln!(out, "pub mod {package_ident} {{").unwrap();
    out.push_str("    use super::generated_prelude::*;\n");
    out.push_str("    use crate::argspec;\n");
    emit_command_module(out, package.commands.as_slice());
    emit_environment_module(out, package.environments.as_slice());
    emit_character_module(out, package.characters.as_slice());
    writeln!(
        out,
        "    pub static DELIMITER_CONTROLS: &[&str] = {};",
        render_string_slice(package.delimiter_controls.as_slice())
    )
    .unwrap();
    out.push_str("}\n");
}

fn emit_command_module(out: &mut String, records: &[CommandRecordSource]) {
    out.push_str("    pub mod cmd {\n");
    out.push_str("        use super::*;\n");
    let facades = resolve_facade_idents(
        records
            .iter()
            .map(|record| record.name.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let refs = emit_named_records(
        out,
        records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                (
                    facades[index].clone(),
                    format!("COMMAND_{index}"),
                    render_command_record(record),
                )
            })
            .collect::<Vec<_>>()
            .as_slice(),
        "BuiltinCommandRecord",
        8,
    );
    writeln!(
        out,
        "        pub static ALL: &[&BuiltinCommandRecord] = &[{}];",
        refs.join(", ")
    )
    .unwrap();
    out.push_str("    }\n");
}

fn emit_environment_module(out: &mut String, records: &[EnvironmentRecordSource]) {
    out.push_str("    pub mod env {\n");
    out.push_str("        use super::*;\n");
    let facades = resolve_facade_idents(
        records
            .iter()
            .map(|record| record.name.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let refs = emit_named_records(
        out,
        records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                (
                    facades[index].clone(),
                    format!("ENVIRONMENT_{index}"),
                    render_environment_record(record),
                )
            })
            .collect::<Vec<_>>()
            .as_slice(),
        "BuiltinEnvironmentRecord",
        8,
    );
    writeln!(
        out,
        "        pub static ALL: &[&BuiltinEnvironmentRecord] = &[{}];",
        refs.join(", ")
    )
    .unwrap();
    out.push_str("    }\n");
}

fn emit_character_module(out: &mut String, records: &[CharacterRecordSource]) {
    out.push_str("    pub mod chars {\n");
    out.push_str("        use super::*;\n");
    let facades = resolve_facade_idents(
        records
            .iter()
            .map(|record| record.name.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let refs = emit_named_records(
        out,
        records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                (
                    facades[index].clone(),
                    format!("CHARACTER_{index}"),
                    render_character_record(record),
                )
            })
            .collect::<Vec<_>>()
            .as_slice(),
        "BuiltinCharacterRecord",
        8,
    );
    writeln!(
        out,
        "        pub static ALL: &[&BuiltinCharacterRecord] = &[{}];",
        refs.join(", ")
    )
    .unwrap();
    out.push_str("    }\n");
}

fn emit_named_records(
    out: &mut String,
    records: &[(Option<String>, String, String)],
    ty: &str,
    indent: usize,
) -> Vec<String> {
    let mut refs = Vec::with_capacity(records.len());
    let pad = " ".repeat(indent);
    for (facade, fallback, literal) in records {
        let name = facade.as_deref().unwrap_or(fallback.as_str());
        let visibility = if facade.is_some() { "pub " } else { "" };
        writeln!(out, "{pad}{visibility}static {name}: {ty} = {literal};").unwrap();
        refs.push(format!("&{name}"));
    }
    refs
}

fn render_command_record(record: &CommandRecordSource) -> String {
    format!(
        "BuiltinCommandRecord {{ name: {:?}, kind: {}, allowed_mode: {}, argspec: {}, tags: {} }}",
        record.name,
        record.kind.code(),
        record.allowed_mode.code(),
        render_argspec_macro(record.spec_string.as_str()),
        render_string_slice(record.tags.as_slice()),
    )
}

fn render_environment_record(record: &EnvironmentRecordSource) -> String {
    format!(
        "BuiltinEnvironmentRecord {{ name: {:?}, allowed_mode: {}, argspec: {}, body_mode: {}, tags: {} }}",
        record.name,
        record.allowed_mode.code(),
        render_argspec_macro(record.spec_string.as_str()),
        record.body_mode.code(),
        render_string_slice(record.tags.as_slice()),
    )
}

fn render_character_record(record: &CharacterRecordSource) -> String {
    format!(
        "BuiltinCharacterRecord {{ name: {:?}, allowed_mode: {}, unicode_value: {:?}, attributes: {} }}",
        record.name,
        record.allowed_mode.code(),
        record.unicode_value,
        render_character_attributes(&record.attributes),
    )
}

fn render_character_attributes(attributes: &CharacterAttributesYaml) -> String {
    match attributes.mathvariant.as_deref() {
        Some(value) => format!("char_attrs!({value:?})"),
        None => "char_attrs!()".to_string(),
    }
}

fn render_argspec_macro(spec: &str) -> String {
    format!("argspec!({spec:?})")
}

fn render_string_slice(values: &[impl AsRef<str>]) -> String {
    if values.is_empty() {
        return "&[]".to_string();
    }

    let rendered = values
        .iter()
        .map(|value| format!("{:?}", value.as_ref()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{rendered}]")
}

fn package_module_ident(name: &str) -> String {
    let mut ident = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            ident.push(ch.to_ascii_lowercase());
        } else {
            ident.push('_');
        }
    }
    ident
}

fn facade_ident(name: &str) -> Option<String> {
    let mut ident = String::new();
    let mut previous_is_alnum = false;
    let mut previous_is_lower_or_digit = false;

    for (index, ch) in name.chars().enumerate() {
        if ch == '*' {
            if !ident.is_empty() && !ident.ends_with('_') {
                ident.push('_');
            }
            ident.push_str("STAR");
            previous_is_alnum = false;
            previous_is_lower_or_digit = false;
            continue;
        }

        if !ch.is_ascii_alphanumeric() {
            return None;
        }
        if index == 0 && ch.is_ascii_digit() {
            return None;
        }

        let is_upper = ch.is_ascii_uppercase();
        let is_lower = ch.is_ascii_lowercase();
        let is_digit = ch.is_ascii_digit();

        if previous_is_alnum && is_upper && previous_is_lower_or_digit && !ident.ends_with('_') {
            ident.push('_');
        }

        ident.push(ch.to_ascii_uppercase());
        previous_is_alnum = true;
        previous_is_lower_or_digit = is_lower || is_digit;
    }

    if ident.is_empty() { None } else { Some(ident) }
}

fn resolve_facade_idents(names: &[&str]) -> Vec<Option<String>> {
    let base = names
        .iter()
        .map(|name| facade_ident(name))
        .collect::<Vec<_>>();
    let mut total = HashMap::new();
    for ident in base.iter().flatten() {
        *total.entry(ident.clone()).or_insert(0usize) += 1;
    }

    let mut used = HashMap::new();
    base.into_iter()
        .map(|ident| {
            ident.map(|ident| {
                if total.get(&ident).copied().unwrap_or(0) <= 1 {
                    return ident;
                }

                let seen = used.entry(ident.clone()).or_insert(0usize);
                *seen += 1;
                if *seen == 1 {
                    ident
                } else {
                    format!("{ident}_{seen}")
                }
            })
        })
        .collect()
}
