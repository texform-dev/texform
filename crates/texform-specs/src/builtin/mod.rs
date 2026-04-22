use crate::specs::{
    BuiltinCharacterRecord, BuiltinCommandRecord, BuiltinDelimiterRecord, BuiltinEnvironmentRecord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinPackage {
    pub name: &'static str,
    pub commands: &'static [&'static BuiltinCommandRecord],
    pub environments: &'static [&'static BuiltinEnvironmentRecord],
    pub characters: &'static [&'static BuiltinCharacterRecord],
    pub delimiters: &'static [&'static BuiltinDelimiterRecord],
}

mod generated_prelude {
    pub use crate::specs::{
        AllowedMode, BuiltinCharacterAttributes, BuiltinCharacterRecord, BuiltinCommandRecord,
        BuiltinDelimiterRecord, BuiltinEnvironmentRecord, CommandKind, ContentMode,
    };
}

include!("generated.rs");

pub fn lookup_package(name: &str) -> Option<&'static BuiltinPackage> {
    ALL_PACKAGES.iter().find(|pkg| pkg.name == name)
}

pub fn all_package_names() -> Vec<&'static str> {
    ALL_PACKAGES.iter().map(|pkg| pkg.name).collect()
}
