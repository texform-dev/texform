use crate::specs::{BuiltinCharacterRecord, BuiltinCommandRecord, BuiltinEnvironmentRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinPackage {
    pub name: &'static str,
    pub commands: &'static [&'static BuiltinCommandRecord],
    pub environments: &'static [&'static BuiltinEnvironmentRecord],
    pub characters: &'static [&'static BuiltinCharacterRecord],
    pub delimiter_controls: &'static [&'static str],
}

mod generated_prelude {
    pub use crate::specs::{
        AllowedMode, ArgForm, ArgSpec, BuiltinCharacterAttributes, BuiltinCharacterRecord,
        BuiltinCommandRecord, BuiltinEnvironmentRecord, CommandKind, ContentMode, DelimiterToken,
        ValueKind,
    };
}

include!("generated.rs");

pub fn lookup_package(name: &str) -> Option<&'static BuiltinPackage> {
    ALL_PACKAGES.iter().find(|pkg| pkg.name == name)
}

pub fn all_package_names() -> Vec<&'static str> {
    ALL_PACKAGES.iter().map(|pkg| pkg.name).collect()
}
