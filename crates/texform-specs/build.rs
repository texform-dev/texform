use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

mod specs_yaml {
    include!("src/specs_yaml.rs");
}

use specs_yaml::{
    AllowedModeYaml, CharacterAttributesYaml, CommandKindYaml, ContentModeYaml, PackageSpecsYaml,
};
use texform_argspec::parse_arg_specs;

const MANAGED_PACKAGE_IMPORT_ORDER: [&str; 7] = [
    "base",
    "ams",
    "braket",
    "physics",
    "textmacros",
    "bboldx",
    "boldsymbol",
];

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

    assert_managed_order_matches_specs(packages.as_slice());

    fs::write(out_path, generate_builtin_source(packages.as_slice()))
        .unwrap_or_else(|err| panic!("failed to write generated builtin source: {err}"));
}

#[derive(Debug)]
struct BuiltinPackageSource {
    name: String,
    commands: Vec<CommandRecordSource>,
    environments: Vec<EnvironmentRecordSource>,
    characters: Vec<CharacterRecordSource>,
    delimiters: Vec<DelimiterRecordSource>,
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

#[derive(Debug)]
struct DelimiterRecordSource {
    name: String,
    is_control_sequence: bool,
    allowed_mode: AllowedModeYaml,
    unicode_value: String,
    attributes: CharacterAttributesYaml,
}

fn assert_managed_order_matches_specs(packages: &[BuiltinPackageSource]) {
    let package_names = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>();

    for managed in MANAGED_PACKAGE_IMPORT_ORDER {
        assert!(
            package_names.contains(&managed),
            "managed package `{managed}` is missing from resources/specs"
        );
    }

    for name in package_names {
        assert!(
            MANAGED_PACKAGE_IMPORT_ORDER.contains(&name),
            "package `{name}` exists in resources/specs but is missing from MANAGED_PACKAGE_IMPORT_ORDER"
        );
    }
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

impl AllowedModeYaml {
    fn code(self) -> &'static str {
        match self {
            AllowedModeYaml::Math => "AllowedMode::Math",
            AllowedModeYaml::Text => "AllowedMode::Text",
            AllowedModeYaml::Both => "AllowedMode::Both",
        }
    }
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
            parse_arg_specs(command.argspec.as_str(), context.as_str()).unwrap_or_else(|err| {
                panic!("{err}");
            });
            CommandRecordSource {
                name: command.name,
                kind: command.kind,
                allowed_mode: command.allowed_mode,
                tags: command.tags,
                spec_string: command.argspec,
            }
        })
        .collect::<Vec<_>>();
    let environments = parsed
        .environments
        .into_iter()
        .map(|environment| {
            let context = format!("environment {}", environment.name);
            parse_arg_specs(environment.argspec.as_str(), context.as_str()).unwrap_or_else(|err| {
                panic!("{err}");
            });
            EnvironmentRecordSource {
                name: environment.name,
                allowed_mode: environment.allowed_mode,
                body_mode: environment.body_mode,
                tags: environment.tags,
                spec_string: environment.argspec,
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
    let delimiters = parsed
        .delimiters
        .into_iter()
        .map(|delimiter| DelimiterRecordSource {
            name: delimiter.name,
            is_control_sequence: delimiter.is_control_sequence,
            allowed_mode: delimiter.allowed_mode,
            unicode_value: delimiter.unicode_value,
            attributes: delimiter.attributes,
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
    assert_unique_delimiter_keys(package_name.as_str(), delimiters.as_slice());
    BuiltinPackageSource {
        name: package_name,
        commands,
        environments,
        characters,
        delimiters,
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

fn assert_unique_delimiter_keys(package: &str, delimiters: &[DelimiterRecordSource]) {
    let mut seen = HashSet::new();
    for delimiter in delimiters {
        let key = (delimiter.name.clone(), delimiter.is_control_sequence);
        if !seen.insert(key.clone()) {
            panic!(
                "duplicate delimiter `{}::{}` in package `{package}`",
                key.0, key.1
            );
        }
    }
}

fn generate_builtin_source(packages: &[BuiltinPackageSource]) -> String {
    let mut out = String::new();
    out.push_str("// @generated by build.rs\n");

    for package in packages {
        emit_package_module(&mut out, package);
    }

    emit_package_name_enum(&mut out);

    out.push_str("pub static ALL_PACKAGES: &[BuiltinPackage] = &[\n");
    for package in packages {
        writeln!(
            out,
            "    BuiltinPackage {{ name: {:?}, commands: {}::cmd::ALL, environments: {}::env::ALL, characters: {}::chars::ALL, delimiters: {}::delims::ALL }},",
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

fn emit_package_name_enum(out: &mut String) {
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    out.push_str("pub enum PackageName {\n");
    for package_name in MANAGED_PACKAGE_IMPORT_ORDER {
        writeln!(out, "    {},", package_variant_ident(package_name)).unwrap();
    }
    out.push_str("}\n");

    out.push_str("pub static MANAGED_PACKAGE_IMPORT_ORDER: &[PackageName] = &[\n");
    for package_name in MANAGED_PACKAGE_IMPORT_ORDER {
        writeln!(
            out,
            "    PackageName::{},",
            package_variant_ident(package_name)
        )
        .unwrap();
    }
    out.push_str("];\n");

    out.push_str("impl PackageName {\n");
    out.push_str("    pub const fn as_str(self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for package_name in MANAGED_PACKAGE_IMPORT_ORDER {
        writeln!(
            out,
            "            PackageName::{} => {:?},",
            package_variant_ident(package_name),
            package_name
        )
        .unwrap();
    }
    out.push_str("        }\n");
    out.push_str("    }\n");

    out.push_str("    pub const fn import_order(self) -> usize {\n");
    out.push_str("        match self {\n");
    for (index, package_name) in MANAGED_PACKAGE_IMPORT_ORDER.iter().enumerate() {
        writeln!(
            out,
            "            PackageName::{} => {},",
            package_variant_ident(package_name),
            index
        )
        .unwrap();
    }
    out.push_str("        }\n");
    out.push_str("    }\n");

    out.push_str("    pub fn package(self) -> &'static BuiltinPackage {\n");
    out.push_str(
        "        lookup_package(self.as_str()).expect(\"generated package name must exist\")\n",
    );
    out.push_str("    }\n");

    out.push_str("    #[allow(clippy::should_implement_trait)]\n");
    out.push_str("    pub fn from_str(name: &str) -> Option<Self> {\n");
    out.push_str("        match name {\n");
    for package_name in MANAGED_PACKAGE_IMPORT_ORDER {
        writeln!(
            out,
            "            {:?} => Some(PackageName::{}),",
            package_name,
            package_variant_ident(package_name)
        )
        .unwrap();
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_package_module(out: &mut String, package: &BuiltinPackageSource) {
    let package_ident = package_module_ident(package.name.as_str());
    writeln!(out, "pub mod {package_ident} {{").unwrap();
    out.push_str("    use super::generated_prelude::*;\n");
    out.push_str("    use crate::argspec;\n");
    emit_command_module(out, package.commands.as_slice());
    emit_environment_module(out, package.environments.as_slice());
    emit_character_module(out, package.characters.as_slice());
    emit_delimiter_module(out, package.delimiters.as_slice());
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

fn emit_delimiter_module(out: &mut String, records: &[DelimiterRecordSource]) {
    out.push_str("    pub mod delims {\n");
    out.push_str("        use super::*;\n");
    let refs = emit_named_records(
        out,
        records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                (
                    None,
                    format!("DELIMITER_{index}"),
                    render_delimiter_record(record),
                )
            })
            .collect::<Vec<_>>()
            .as_slice(),
        "BuiltinDelimiterRecord",
        8,
    );
    writeln!(
        out,
        "        pub static ALL: &[&BuiltinDelimiterRecord] = &[{}];",
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
        render_character_like_attributes(&record.attributes),
    )
}

fn render_delimiter_record(record: &DelimiterRecordSource) -> String {
    format!(
        "BuiltinDelimiterRecord {{ name: {:?}, is_control_sequence: {}, allowed_mode: {}, unicode_value: {:?}, attributes: {} }}",
        record.name,
        record.is_control_sequence,
        record.allowed_mode.code(),
        record.unicode_value,
        render_character_like_attributes(&record.attributes),
    )
}

fn render_character_like_attributes(attributes: &CharacterAttributesYaml) -> String {
    format!(
        "BuiltinCharacterAttributes {{ mathvariant: {}, tex_class: {}, stretchy: {}, move_sup_sub: {}, large_op: {} }}",
        render_option_string(attributes.mathvariant.as_deref()),
        render_option_string(attributes.tex_class.as_deref()),
        render_option_bool(attributes.stretchy),
        render_option_bool(attributes.move_sup_sub),
        render_option_bool(attributes.large_op),
    )
}

fn render_argspec_macro(spec: &str) -> String {
    format!("argspec!({spec:?})")
}

fn render_option_string(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("Some({value:?})"),
        None => "None".to_string(),
    }
}

fn render_option_bool(value: Option<bool>) -> String {
    match value {
        Some(value) => format!("Some({value})"),
        None => "None".to_string(),
    }
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

fn package_variant_ident(name: &str) -> String {
    name.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut out = String::new();
            out.push(first.to_ascii_uppercase());
            out.extend(chars.map(|ch| ch.to_ascii_lowercase()));
            out
        })
        .collect::<String>()
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
