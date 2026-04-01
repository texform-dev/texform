use crate::builtin::BuiltinPackage;
use crate::specs::{AllowedMode, ArgForm, ArgSpec, BuiltinCommandRecord, CommandKind, ValueKind};

pub const CORE_PACKAGE_NAME: &str = "core";

pub static LINEBREAK_ARGS: &[ArgSpec] = &[
    ArgSpec::with_form(false, true, ValueKind::Star, ArgForm::Star),
    ArgSpec::with_form(false, true, ValueKind::Dimension, ArgForm::Standard),
];

pub static LINEBREAK: BuiltinCommandRecord = BuiltinCommandRecord {
    name: "\\",
    kind: CommandKind::Prefix,
    allowed_mode: AllowedMode::Both,
    args: LINEBREAK_ARGS,
    tags: &[],
    spec_string: "!s !o:L",
};

pub static COMMANDS: &[&BuiltinCommandRecord] = &[&LINEBREAK];
pub static ENVIRONMENTS: &[&crate::specs::BuiltinEnvironmentRecord] = &[];
pub static CHARACTERS: &[&crate::specs::BuiltinCharacterRecord] = &[];
pub static DELIMITER_CONTROLS: &[&str] = &[];

pub static CORE_PACKAGE: BuiltinPackage = BuiltinPackage {
    name: CORE_PACKAGE_NAME,
    commands: COMMANDS,
    environments: ENVIRONMENTS,
    characters: CHARACTERS,
    delimiter_controls: DELIMITER_CONTROLS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_specs_include_linebreak_command() {
        assert_eq!(CORE_PACKAGE.commands.len(), 1);

        let linebreak = CORE_PACKAGE.commands[0];
        assert_eq!(linebreak.name, "\\");
        assert_eq!(linebreak.kind, CommandKind::Prefix);
        assert_eq!(linebreak.allowed_mode, AllowedMode::Both);
        assert_eq!(linebreak.spec_string, "!s !o:L");
        assert_eq!(linebreak.args.len(), 2);
    }
}
