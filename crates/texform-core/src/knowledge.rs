//! Knowledge base: the backing store behind [`ParseContext`](crate::parse::ParseContext).
//!
//! A [`KnowledgeBase`] holds indexed command, environment, character, and
//! delimiter-control metadata loaded from `texform-specs` package definitions.
//! It is the single source of truth the parser consults when recognizing
//! control sequences and environments.
//!
//! # Architecture
//!
//! The KB separates *raw storage* from the *parser-facing active view*:
//!
//! - **Explicit commands** are definitions with concrete argument specs
//!   (`\frac`, `\text`, etc.).
//! - **Character entries** are zero-arg symbols (`\alpha`, `\div`, etc.)
//!   that the KB projects into synthetic command views so the parser can
//!   recognize them uniformly.
//! - **Active index** maps each name to whichever source (explicit or
//!   character) is currently authoritative. Explicit commands always win.
//!
//! # Package import order
//!
//! Managed packages (base, ams, physics, …) are always imported in a
//! fixed canonical order regardless of the caller-supplied order. This
//! keeps merge results and `from_packages` arrays stable.
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use crate::ast::Node;
use std::collections::{HashMap, HashSet};
use texform_argspec::parse_arg_specs;
use texform_interface::syntax_node::ContentMode;
use texform_specs::builtin::BuiltinPackage;

use crate::parse::{CommandItem, ContextItem, DelimiterControlItem, EnvironmentItem};

pub use texform_argspec::{
    ArgForm, ArgSpec, ArgSpecParseError, DelimiterToken, ParsedArgSpec, ValueKind,
};
pub use texform_specs::specs::{
    AllowedMode, BuiltinCharacterRecord, BuiltinCommandRecord, BuiltinEnvironmentRecord,
    CharacterMeta, CommandKind, CommandMeta, EnvMeta,
};
#[cfg(test)]
use texform_specs::specs::{
    CharacterAttributes, CharacterSpec, CommandSpec, EnvironmentSpec, PackageSpecs,
};

const RUNTIME_PACKAGE_NAME: &str = "runtime";
#[cfg(test)]
const UNKNOWN_PACKAGE_NAME: &str = "unknown";
const MANAGED_PACKAGE_IMPORT_ORDER: [&str; 6] = [
    "base",
    "ams",
    "physics",
    "textmacros",
    "bboldx",
    "boldsymbol",
];
const PHYSICS_COMMAND_MERGE_DENYLIST: [&str; 3] = ["Pr", "det", "exp"];

/// Error returned when a requested package name is not found in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLoadError {
    /// The named package does not exist in the `texform-specs` registry.
    UnknownPackage { name: String },
}

impl std::fmt::Display for PackageLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageLoadError::UnknownPackage { name } => {
                write!(f, "unknown package: {name}")
            }
        }
    }
}

impl std::error::Error for PackageLoadError {}

/// The knowledge base separates raw storage from the parser-facing view:
///
/// - `commands` / `command_idx_by_name`: raw explicit command store.
/// - `characters` / `character_idx_by_name`: raw character store.
/// - `character_command_views`: zero-arg Prefix commands projected from characters,
///   so the parser can still recognize character control sequences as commands.
/// - `active_command_idx_by_name`: the single parser-facing index that tells
///   `lookup_command()` whether the active entry for a name comes from an
///   explicit command or a character-derived view.
/// - `suppressed_command_names`: names removed via `remove_item(Command)`.
///   Prevents a deleted name from "reviving" through a character fallback.
#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    characters: Vec<CharacterMeta>,
    character_idx_by_name: HashMap<String, usize>,
    character_command_views: Vec<CommandMeta>,
    active_command_idx_by_name: HashMap<String, ActiveCommandSource>,
    suppressed_command_names: HashSet<String>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    delimiter_controls: HashSet<&'static str>,
}

/// Tracks whether the parser-facing active command for a name points to
/// an explicit command or a character-derived command view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveCommandSource {
    Explicit(usize),
    Character(usize),
}

impl KnowledgeBase {
    fn new() -> Self {
        Self {
            commands: Vec::new(),
            command_idx_by_name: HashMap::new(),
            characters: Vec::new(),
            character_idx_by_name: HashMap::new(),
            character_command_views: Vec::new(),
            active_command_idx_by_name: HashMap::new(),
            suppressed_command_names: HashSet::new(),
            envs: Vec::new(),
            env_idx_by_name: HashMap::new(),
            delimiter_controls: HashSet::new(),
        }
    }

    /// Return the active command for `name`, respecting suppression.
    ///
    /// The active entry may be an explicit command or a character-derived
    /// zero-arg view. Suppressed names always return `None`.
    pub fn empty() -> Self {
        Self::new()
    }

    pub fn core_only() -> Self {
        new_with_core()
    }

    pub fn core_only_for_mode(target_mode: ContentMode) -> Self {
        new_with_core_for_mode(target_mode)
    }

    pub fn build_from_packages(packages: &[&str]) -> Self {
        Self::try_build_from_packages(packages).unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn try_build_from_packages(packages: &[&str]) -> Result<Self, PackageLoadError> {
        let mut kb = new_with_core();
        let to_load = canonical_package_import_order(packages);
        import_package_names(&mut kb, to_load.as_slice())?;
        Ok(kb)
    }

    pub fn try_build_from_packages_for_mode(
        packages: &[&str],
        target_mode: ContentMode,
    ) -> Result<Self, PackageLoadError> {
        let mut kb = new_with_core_for_mode(target_mode);
        let to_load = canonical_package_import_order(packages);
        import_package_names_for_mode(&mut kb, to_load.as_slice(), target_mode)?;
        Ok(kb)
    }

    pub fn all_packages() -> Self {
        let package_names = texform_specs::builtin::all_package_names();
        Self::build_from_packages(package_names.as_slice())
    }

    pub fn lookup_command(&self, name: &str) -> Option<&CommandMeta> {
        if self.suppressed_command_names.contains(name) {
            return None;
        }

        match self.active_command_idx_by_name.get(name).copied()? {
            ActiveCommandSource::Explicit(idx) => Some(&self.commands[idx]),
            ActiveCommandSource::Character(idx) => Some(&self.character_command_views[idx]),
        }
    }

    /// Look up only the explicit (non-character-derived) command for `name`.
    pub fn lookup_explicit_command(&self, name: &str) -> Option<&CommandMeta> {
        self.command_idx_by_name
            .get(name)
            .copied()
            .map(|idx| &self.commands[idx])
    }

    /// Look up raw character metadata by control-sequence name.
    pub fn lookup_character(&self, name: &str) -> Option<&CharacterMeta> {
        self.character_idx_by_name
            .get(name)
            .copied()
            .map(|idx| &self.characters[idx])
    }

    /// Look up environment metadata by name.
    pub fn lookup_env(&self, name: &str) -> Option<&EnvMeta> {
        self.env_idx_by_name
            .get(name)
            .copied()
            .map(|idx| &self.envs[idx])
    }

    /// Check whether `name` is registered as a delimiter control sequence.
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.delimiter_controls.contains(name)
    }

    /// Look up a delimiter control, returning the interned `&'static str` name.
    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.delimiter_controls.get(name).copied()
    }

    /// Insert a context item, dispatching to the appropriate typed inserter.
    pub fn insert_item(&mut self, item: impl Into<ContextItem>) -> Result<(), ArgSpecParseError> {
        match item.into() {
            ContextItem::Command(item) => self.insert_command(item),
            ContextItem::Environment(item) => self.insert_environment(item),
            ContextItem::DelimiterControl(item) => {
                self.insert_delimiter_control(item);
                Ok(())
            }
        }
    }

    /// Runtime insertion: clears any prior suppression so a previously
    /// removed name can be re-activated by explicit command injection.
    pub(crate) fn insert_command(&mut self, item: CommandItem) -> Result<(), ArgSpecParseError> {
        let meta = command_item_into_meta(item, vec![RUNTIME_PACKAGE_NAME.to_string()])?;
        let name = meta.name;
        let idx = self.upsert_command_meta(meta);
        self.suppressed_command_names.remove(name);
        self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
        Ok(())
    }

    /// Remove a previously inserted item. Returns `true` if found.
    pub fn remove_item(&mut self, item: impl Into<ContextItem>) -> bool {
        match item.into() {
            ContextItem::Command(item) => self.remove_command(item.name.as_str()),
            ContextItem::Environment(item) => {
                self.env_idx_by_name.remove(item.name.as_str()).is_some()
            }
            ContextItem::DelimiterControl(item) => {
                self.delimiter_controls.remove(item.name.as_str())
            }
        }
    }

    pub fn insert_environment(&mut self, item: EnvironmentItem) -> Result<(), ArgSpecParseError> {
        let meta = environment_item_into_meta(item, vec![RUNTIME_PACKAGE_NAME.to_string()])?;
        self.upsert_env_meta(meta);
        Ok(())
    }

    pub fn insert_delimiter_control(&mut self, item: DelimiterControlItem) {
        let name = item.name;
        if self.delimiter_controls.contains(name.as_str()) {
            return;
        }
        let name: &'static str = Box::leak(name.into_boxed_str());
        self.delimiter_controls.insert(name);
    }

    /// Removes a command name from both raw and active indices, then adds it
    /// to the suppression set. This prevents `lookup_command()` from falling
    /// back to a character-derived view after the name is explicitly removed.
    fn remove_command(&mut self, name: &str) -> bool {
        let explicit_removed = self.command_idx_by_name.remove(name).is_some();
        let active_removed = self.active_command_idx_by_name.remove(name).is_some();

        if explicit_removed || active_removed {
            self.suppressed_command_names.insert(name.to_string());
            return true;
        }

        false
    }

    fn set_active_command_source(&mut self, name: impl Into<String>, source: ActiveCommandSource) {
        self.active_command_idx_by_name.insert(name.into(), source);
    }

    fn upsert_command_meta(&mut self, meta: CommandMeta) -> usize {
        let idx = self.commands.len();
        let name = meta.name;
        self.commands.push(meta);
        self.command_idx_by_name.insert(name, idx);
        idx
    }

    fn upsert_env_meta(&mut self, meta: EnvMeta) {
        let idx = self.envs.len();
        let name = meta.name;
        self.envs.push(meta);
        self.env_idx_by_name.insert(name, idx);
    }

    fn upsert_character_meta(&mut self, meta: CharacterMeta) -> usize {
        let idx = self.characters.len();
        let name = meta.name.clone();
        self.characters.push(meta);
        self.character_idx_by_name.insert(name, idx);
        idx
    }

    fn upsert_character_command_view(&mut self, meta: CommandMeta) -> usize {
        let idx = self.character_command_views.len();
        self.character_command_views.push(meta);
        idx
    }

    /// Writes raw character metadata and creates a zero-arg Prefix command view
    /// so the parser can recognize the character as a command head. Does NOT
    /// write into the explicit command raw store.
    #[cfg(test)]
    fn insert_character_with_package(&mut self, character: CharacterSpec, package: &str) {
        let CharacterSpec {
            name,
            allowed_mode,
            unicode_value,
            attributes,
        } = character;

        self.upsert_character_meta(CharacterMeta {
            name: name.clone(),
            allowed_mode,
            unicode_value,
            attributes,
            package: package.to_string(),
        });

        let view_idx = self.upsert_character_command_view(make_command_meta(
            name.clone(),
            CommandKind::Prefix,
            allowed_mode,
            vec![],
            vec![],
            String::new(),
            vec![package.to_string()],
        ));
        self.set_active_command_source(name, ActiveCommandSource::Character(view_idx));
    }

    fn insert_builtin_character_with_package(
        &mut self,
        character: &'static BuiltinCharacterRecord,
        package: &str,
    ) {
        self.upsert_character_meta(CharacterMeta {
            name: character.name.to_string(),
            allowed_mode: character.allowed_mode,
            unicode_value: character.unicode_value.to_string(),
            attributes: character.attributes.into(),
            package: package.to_string(),
        });

        let view_idx = self.upsert_character_command_view(CommandMeta {
            name: character.name,
            kind: CommandKind::Prefix,
            allowed_mode: character.allowed_mode,
            argspec: texform_specs::argspec!(""),
            tags: &[],
            from_packages: leak_string_array(vec![package.to_string()]),
        });
        self.set_active_command_source(character.name, ActiveCommandSource::Character(view_idx));
    }

    #[cfg(test)]
    pub(crate) fn insert_or_override_command(&mut self, spec: CommandSpec) {
        self.insert_or_override_command_with_package(spec, UNKNOWN_PACKAGE_NAME);
    }

    #[cfg(test)]
    fn insert_or_override_command_with_package(&mut self, spec: CommandSpec, package: &str) {
        let meta = command_spec_into_meta(spec, vec![package.to_string()]);
        let idx = self.upsert_command_meta(meta);
        let name = self.commands[idx].name;
        self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
    }

    /// Package import path: merges the incoming command with an existing one
    /// if they share the same name/kind/spec and both come from managed packages;
    /// otherwise falls back to override (last-writer-wins).
    #[cfg(test)]
    fn import_or_merge_command_with_package(&mut self, spec: CommandSpec, package: &str) {
        let incoming = command_spec_into_meta(spec, vec![package.to_string()]);
        if let Some(existing_idx) = self.command_idx_by_name.get(incoming.name).copied() {
            let existing = &self.commands[existing_idx];
            if should_merge_command(existing, &incoming) {
                let merged = merge_command_meta(existing, &incoming);
                let idx = self.upsert_command_meta(merged);
                let name = self.commands[idx].name;
                self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
                return;
            }
        }

        let idx = self.upsert_command_meta(incoming);
        let name = self.commands[idx].name;
        self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
    }

    fn import_or_merge_builtin_command_with_package(
        &mut self,
        record: &'static BuiltinCommandRecord,
        package: &str,
    ) {
        let incoming = builtin_command_into_meta(record, vec![package.to_string()]);
        if let Some(existing_idx) = self.command_idx_by_name.get(incoming.name).copied() {
            let existing = &self.commands[existing_idx];
            if should_merge_command(existing, &incoming) {
                let merged = merge_command_meta(existing, &incoming);
                let idx = self.upsert_command_meta(merged);
                let name = self.commands[idx].name;
                self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
                return;
            }
        }

        let idx = self.upsert_command_meta(incoming);
        let name = self.commands[idx].name;
        self.set_active_command_source(name, ActiveCommandSource::Explicit(idx));
    }

    #[cfg(test)]
    fn insert_or_override_environment(&mut self, spec: EnvironmentSpec) {
        self.insert_or_override_environment_with_package(spec, UNKNOWN_PACKAGE_NAME);
    }

    #[cfg(test)]
    fn insert_or_override_environment_with_package(
        &mut self,
        spec: EnvironmentSpec,
        package: &str,
    ) {
        let meta = environment_spec_into_meta(spec, vec![package.to_string()]);
        self.upsert_env_meta(meta);
    }

    /// Same merge-or-override logic as commands, but for environments.
    /// Merge requires matching name, argspec source, and body_mode.
    #[cfg(test)]
    fn import_or_merge_environment_with_package(&mut self, spec: EnvironmentSpec, package: &str) {
        let incoming = environment_spec_into_meta(spec, vec![package.to_string()]);
        if let Some(existing_idx) = self.env_idx_by_name.get(incoming.name).copied() {
            let existing = &self.envs[existing_idx];
            if should_merge_environment(existing, &incoming) {
                self.upsert_env_meta(merge_environment_meta(existing, &incoming));
                return;
            }
        }

        self.upsert_env_meta(incoming);
    }

    fn import_or_merge_builtin_environment_with_package(
        &mut self,
        record: &'static BuiltinEnvironmentRecord,
        package: &str,
    ) {
        let incoming = builtin_environment_into_meta(record, vec![package.to_string()]);
        if let Some(existing_idx) = self.env_idx_by_name.get(incoming.name).copied() {
            let existing = &self.envs[existing_idx];
            if should_merge_environment(existing, &incoming) {
                self.upsert_env_meta(merge_environment_meta(existing, &incoming));
                return;
            }
        }

        self.upsert_env_meta(incoming);
    }

    #[cfg(test)]
    pub(crate) fn import_package(&mut self, specs: PackageSpecs) {
        self.import_package_with_name(UNKNOWN_PACKAGE_NAME, specs);
    }

    #[cfg(test)]
    fn import_package_with_name(&mut self, package: &str, specs: PackageSpecs) {
        for character in specs.characters {
            self.insert_character_with_package(character, package);
        }
        for cmd in specs.commands {
            self.import_or_merge_command_with_package(cmd, package);
        }
        for env in specs.environments {
            self.import_or_merge_environment_with_package(env, package);
        }
        for name in specs.delimiter_controls {
            self.insert_delimiter_control(DelimiterControlItem::new(name));
        }
    }

    fn import_builtin_package(&mut self, package: &'static BuiltinPackage) {
        for character in package.characters {
            self.insert_builtin_character_with_package(character, package.name);
        }
        for command in package.commands {
            self.import_or_merge_builtin_command_with_package(command, package.name);
        }
        for environment in package.environments {
            self.import_or_merge_builtin_environment_with_package(environment, package.name);
        }
        for &name in package.delimiter_controls {
            self.delimiter_controls.insert(name);
        }
    }

    fn import_builtin_package_for_mode(
        &mut self,
        package: &'static BuiltinPackage,
        target_mode: ContentMode,
    ) {
        for character in package.characters {
            if character.allowed_mode.allows(target_mode) {
                self.insert_builtin_character_with_package(character, package.name);
            }
        }
        for command in package.commands {
            if command.allowed_mode.allows(target_mode) {
                self.import_or_merge_builtin_command_with_package(command, package.name);
            }
        }
        for environment in package.environments {
            if environment.allowed_mode.allows(target_mode) {
                self.import_or_merge_builtin_environment_with_package(environment, package.name);
            }
        }
        for &name in package.delimiter_controls {
            self.delimiter_controls.insert(name);
        }
    }
}

fn make_command_meta(
    name: String,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    args: Vec<ArgSpec>,
    tags: Vec<String>,
    source: String,
    from_packages: Vec<String>,
) -> CommandMeta {
    CommandMeta {
        name: leak_string(name),
        kind,
        allowed_mode,
        argspec: ParsedArgSpec {
            args: leak_arg_specs(args),
            source: leak_string(source),
        },
        tags: leak_tags(tags),
        from_packages: leak_string_array(from_packages),
    }
}

fn command_item_into_meta(
    item: CommandItem,
    from_packages: Vec<String>,
) -> Result<CommandMeta, ArgSpecParseError> {
    let context = format!("command {}", item.name);
    let args = parse_arg_specs(item.spec.as_str(), context.as_str())?;
    Ok(make_command_meta(
        item.name,
        item.kind,
        item.allowed_mode,
        args,
        item.tags,
        item.spec,
        from_packages,
    ))
}

fn make_env_meta(
    name: String,
    allowed_mode: AllowedMode,
    args: Vec<ArgSpec>,
    body_mode: ContentMode,
    tags: Vec<String>,
    source: String,
    from_packages: Vec<String>,
) -> EnvMeta {
    EnvMeta {
        name: leak_string(name),
        allowed_mode,
        argspec: ParsedArgSpec {
            args: leak_arg_specs(args),
            source: leak_string(source),
        },
        body_mode,
        tags: leak_tags(tags),
        from_packages: leak_string_array(from_packages),
    }
}

fn environment_item_into_meta(
    item: EnvironmentItem,
    from_packages: Vec<String>,
) -> Result<EnvMeta, ArgSpecParseError> {
    let context = format!("environment {}", item.name);
    let args = parse_arg_specs(item.spec.as_str(), context.as_str())?;
    Ok(make_env_meta(
        item.name,
        item.allowed_mode,
        args,
        item.body_mode,
        item.tags,
        item.spec,
        from_packages,
    ))
}

#[cfg(test)]
fn command_spec_into_meta(spec: CommandSpec, from_packages: Vec<String>) -> CommandMeta {
    make_command_meta(
        spec.name,
        spec.kind,
        spec.allowed_mode,
        spec.argspec.args,
        spec.tags,
        spec.argspec.source,
        from_packages,
    )
}

fn builtin_command_into_meta(
    record: &'static BuiltinCommandRecord,
    from_packages: Vec<String>,
) -> CommandMeta {
    CommandMeta {
        name: record.name,
        kind: record.kind,
        allowed_mode: record.allowed_mode,
        argspec: record.argspec,
        tags: record.tags,
        from_packages: leak_string_array(from_packages),
    }
}

#[cfg(test)]
fn environment_spec_into_meta(spec: EnvironmentSpec, from_packages: Vec<String>) -> EnvMeta {
    make_env_meta(
        spec.name,
        spec.allowed_mode,
        spec.argspec.args,
        spec.body_mode,
        spec.tags,
        spec.argspec.source,
        from_packages,
    )
}

fn builtin_environment_into_meta(
    record: &'static BuiltinEnvironmentRecord,
    from_packages: Vec<String>,
) -> EnvMeta {
    EnvMeta {
        name: record.name,
        allowed_mode: record.allowed_mode,
        argspec: record.argspec,
        body_mode: record.body_mode,
        tags: record.tags,
        from_packages: leak_string_array(from_packages),
    }
}

/// Leak a `String` into a `&'static str` for arena-style storage.
///
/// Metadata structs (`CommandMeta`, `EnvMeta`, …) use `&'static` references
/// so they can be cheaply shared. The leaked memory lives for the process
/// lifetime, which is acceptable for a knowledge base that is built once.
fn leak_string(value: impl Into<String>) -> &'static str {
    Box::leak(value.into().into_boxed_str())
}

fn leak_arg_specs(args: Vec<ArgSpec>) -> &'static [ArgSpec] {
    Box::leak(args.into_boxed_slice())
}

fn leak_tags(tags: Vec<String>) -> &'static [&'static str] {
    let tags: Vec<&'static str> = tags
        .into_iter()
        .map(|tag| Box::leak(tag.into_boxed_str()) as &'static str)
        .collect();
    Box::leak(tags.into_boxed_slice())
}

fn leak_string_array(values: Vec<String>) -> &'static [&'static str] {
    let leaked: Vec<&'static str> = values.into_iter().map(leak_string).collect();
    Box::leak(leaked.into_boxed_slice())
}

fn dedup_names_in_request_order<'a>(requested: &[&'a str]) -> Vec<&'a str> {
    let mut unique = Vec::new();
    for &name in requested {
        if !unique.contains(&name) {
            unique.push(name);
        }
    }
    unique
}

fn is_managed_package(name: &str) -> bool {
    MANAGED_PACKAGE_IMPORT_ORDER.contains(&name)
}

/// Reorders requested package names so that managed packages always appear in
/// the fixed order defined by `MANAGED_PACKAGE_IMPORT_ORDER`, followed by any
/// unmanaged packages in their original request order. This ensures merge
/// results and `from_packages` arrays are stable regardless of caller order.
fn canonical_package_import_order<'a>(requested: &[&'a str]) -> Vec<&'a str> {
    let unique = dedup_names_in_request_order(requested);
    let mut normalized = Vec::new();

    // Managed packages first, in the fixed canonical order.
    for managed in MANAGED_PACKAGE_IMPORT_ORDER {
        if let Some(&name) = unique.iter().find(|&&candidate| candidate == managed) {
            normalized.push(name);
        }
    }

    // Non-managed packages follow, preserving the caller's request order.
    for &name in &unique {
        if !is_managed_package(name) {
            normalized.push(name);
        }
    }

    normalized
}

/// Merge is only attempted between records that both originate from managed
/// packages. This prevents merge rules from accidentally affecting runtime-
/// injected commands or unknown-source test fixtures.
fn from_packages_are_managed(packages: &[&str]) -> bool {
    !packages.is_empty() && packages.iter().all(|package| is_managed_package(package))
}

fn is_physics_denylisted_command(name: &str) -> bool {
    PHYSICS_COMMAND_MERGE_DENYLIST.contains(&name)
}

fn merge_tags(existing: &[&str], incoming: &[&str]) -> Vec<String> {
    let mut merged = Vec::new();
    for &tag in existing.iter().chain(incoming.iter()) {
        if !merged.iter().any(|existing_tag| existing_tag == tag) {
            merged.push(tag.to_string());
        }
    }
    merged.sort();
    merged
}

fn merge_from_packages(existing: &[&str], incoming: &[&str]) -> Vec<String> {
    let combined: Vec<&str> = existing
        .iter()
        .copied()
        .chain(incoming.iter().copied())
        .collect();
    canonical_package_import_order(combined.as_slice())
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

/// Two commands are mergeable iff they share name, kind, and argspec source,
/// both come from managed packages, and neither side is a physics-denylisted
/// command (Pr, det, exp — these intentionally override base definitions).
fn should_merge_command(existing: &CommandMeta, incoming: &CommandMeta) -> bool {
    existing.name == incoming.name
        && existing.kind == incoming.kind
        && existing.argspec.source == incoming.argspec.source
        && from_packages_are_managed(existing.from_packages)
        && from_packages_are_managed(incoming.from_packages)
        && !(is_physics_denylisted_command(existing.name)
            && (existing.from_packages.contains(&"physics")
                || incoming.from_packages.contains(&"physics")))
}

fn should_merge_environment(existing: &EnvMeta, incoming: &EnvMeta) -> bool {
    existing.name == incoming.name
        && existing.argspec.source == incoming.argspec.source
        && existing.body_mode == incoming.body_mode
        && from_packages_are_managed(existing.from_packages)
        && from_packages_are_managed(incoming.from_packages)
}

/// Produces a merged command: allowed_mode and tags are unioned,
/// from_packages collects both sources in canonical order.
fn merge_command_meta(existing: &CommandMeta, incoming: &CommandMeta) -> CommandMeta {
    debug_assert!(should_merge_command(existing, incoming));
    debug_assert_eq!(existing.argspec.args, incoming.argspec.args);

    make_command_meta(
        existing.name.to_string(),
        existing.kind,
        existing.allowed_mode.union(incoming.allowed_mode),
        existing.argspec.args.to_vec(),
        merge_tags(existing.tags, incoming.tags),
        existing.argspec.source.to_string(),
        merge_from_packages(existing.from_packages, incoming.from_packages),
    )
}

fn merge_environment_meta(existing: &EnvMeta, incoming: &EnvMeta) -> EnvMeta {
    debug_assert!(should_merge_environment(existing, incoming));
    debug_assert_eq!(existing.argspec.args, incoming.argspec.args);

    make_env_meta(
        existing.name.to_string(),
        existing.allowed_mode.union(incoming.allowed_mode),
        existing.argspec.args.to_vec(),
        existing.body_mode,
        merge_tags(existing.tags, incoming.tags),
        existing.argspec.source.to_string(),
        merge_from_packages(existing.from_packages, incoming.from_packages),
    )
}

pub(crate) fn lookup_command_node_name(node: &Node) -> Option<&str> {
    match node {
        Node::Command { name, .. } | Node::Infix { name, .. } | Node::Declarative { name, .. } => {
            Some(name.as_str())
        }
        _ => None,
    }
}

pub(crate) fn lookup_environment_node_name(node: &Node) -> Option<&str> {
    match node {
        Node::Environment { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn import_package_names(
    kb: &mut KnowledgeBase,
    requested: &[&str],
) -> Result<(), PackageLoadError> {
    for &name in requested {
        let pkg = texform_specs::builtin::lookup_package(name).ok_or_else(|| {
            PackageLoadError::UnknownPackage {
                name: name.to_string(),
            }
        })?;
        kb.import_builtin_package(pkg);
    }
    Ok(())
}

fn import_package_names_for_mode(
    kb: &mut KnowledgeBase,
    requested: &[&str],
    target_mode: ContentMode,
) -> Result<(), PackageLoadError> {
    for &name in requested {
        let pkg = texform_specs::builtin::lookup_package(name).ok_or_else(|| {
            PackageLoadError::UnknownPackage {
                name: name.to_string(),
            }
        })?;
        kb.import_builtin_package_for_mode(pkg, target_mode);
    }
    Ok(())
}

fn new_with_core() -> KnowledgeBase {
    let mut kb = KnowledgeBase::new();
    kb.import_builtin_package(&texform_specs::core_knowledge::CORE_PACKAGE);
    kb
}

fn new_with_core_for_mode(target_mode: ContentMode) -> KnowledgeBase {
    let mut kb = KnowledgeBase::new();
    kb.import_builtin_package_for_mode(&texform_specs::core_knowledge::CORE_PACKAGE, target_mode);
    kb
}

/// Same as [`try_build_kb_from_packages`] but preserves the caller's exact
/// import order instead of canonicalizing it. Useful for tests that need to
/// verify order-dependent behavior.
#[allow(dead_code)]
pub(crate) fn try_build_kb_from_exact_packages(
    requested: &[&str],
) -> Result<KnowledgeBase, PackageLoadError> {
    let mut kb = new_with_core();
    import_package_names(&mut kb, requested)?;
    Ok(kb)
}

#[cfg(test)]
fn build_default_kb(packages: Option<&[&str]>) -> KnowledgeBase {
    match packages {
        Some(list) => KnowledgeBase::build_from_packages(list),
        None => {
            let package_names = texform_specs::builtin::all_package_names();
            KnowledgeBase::build_from_packages(package_names.as_slice())
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/knowledge.rs"]
mod tests;
