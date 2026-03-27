use crate::specs::{
    AllowedMode, ArgForm, ArgSpec, CommandKind, CommandSpec, PackageSpecs, ValueKind,
};

pub const CORE_PACKAGE_NAME: &str = "core";

pub fn specs() -> PackageSpecs {
    PackageSpecs {
        characters: vec![],
        commands: vec![linebreak_command()],
        environments: vec![],
        delimiter_controls: vec![],
    }
}

fn linebreak_command() -> CommandSpec {
    CommandSpec {
        name: "\\".to_string(),
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Both,
        args: vec![no_leading_space_star(), no_leading_space_dimension()],
        tags: vec![],
        spec_string: "!s !o:L".to_string(),
    }
}

fn no_leading_space_star() -> ArgSpec {
    ArgSpec::with_form(false, true, ValueKind::Star, ArgForm::Star)
}

fn no_leading_space_dimension() -> ArgSpec {
    ArgSpec::with_form(false, true, ValueKind::Dimension, ArgForm::Standard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_specs_include_linebreak_command() {
        let specs = specs();
        assert_eq!(specs.commands.len(), 1);

        let linebreak = &specs.commands[0];
        assert_eq!(linebreak.name, "\\");
        assert_eq!(linebreak.kind, CommandKind::Prefix);
        assert_eq!(linebreak.allowed_mode, AllowedMode::Both);
        assert_eq!(linebreak.spec_string, "!s !o:L");
        assert_eq!(linebreak.args.len(), 2);
    }
}
