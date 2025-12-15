use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Default)]
pub struct PackageSpecs {
    pub characters: Vec<String>,
    pub commands: Vec<Command>,
    pub environments: Vec<Environment>,
    pub delimiter_controls: Vec<String>,
    pub blacklist: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct PackageSpecsYamlSources {
    pub characters: Option<String>,
    pub commands: Option<String>,
    pub environments: Option<String>,
    pub delimiters: Option<String>,
    pub lists: Option<String>,
}

impl PackageSpecsYamlSources {
    pub fn parse(self, context: &str) -> PackageSpecs {
        let mut out = PackageSpecs::default();

        if let Some(text) = self.characters {
            let parsed: CharactersSource = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse characters specs ({context}): {e}"));
            out.characters = parsed.characters;
        }

        if let Some(text) = self.commands {
            let parsed: CommandsSource = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse commands specs ({context}): {e}"));
            out.commands = parsed.commands;
        }

        if let Some(text) = self.environments {
            let parsed: EnvironmentsSource = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse environments specs ({context}): {e}"));
            out.environments = parsed.environments;
        }

        if let Some(text) = self.delimiters {
            let parsed: DelimitersSource = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse delimiters specs ({context}): {e}"));
            out.delimiter_controls = parsed.delimiter_controls;
        }

        if let Some(text) = self.lists {
            let parsed: ListsSource = serde_yaml::from_str(&text)
                .unwrap_or_else(|e| panic!("failed to parse lists specs ({context}): {e}"));
            out.blacklist = parsed.blacklist;
        }

        out
    }
}

#[derive(Debug, Deserialize)]
pub struct Command {
    pub name: String,
    pub kind: CommandKind,
    #[serde(default)]
    pub has_star_variant: bool,
    #[serde(default)]
    pub args: Vec<ArgSpec>,
}

#[derive(Debug, Deserialize)]
pub struct Environment {
    pub name: String,
    #[serde(default)]
    pub has_star_variant: bool,
    #[serde(default)]
    pub args: Vec<ArgSpec>,
    pub body_mode: ContentMode,
}

#[derive(Debug, Deserialize)]
pub struct ArgSpec {
    pub kind: ArgumentKind,
    pub mode: ContentMode,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandKind {
    Prefix,
    Infix,
    Declarative,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgumentKind {
    Mandatory,
    Optional,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentMode {
    Math,
    Text,
}

#[derive(Debug, Default, Deserialize)]
struct CharactersSource {
    #[serde(default)]
    characters: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CommandsSource {
    #[serde(default)]
    commands: Vec<Command>,
}

#[derive(Debug, Default, Deserialize)]
struct EnvironmentsSource {
    #[serde(default)]
    environments: Vec<Environment>,
}

#[derive(Debug, Default, Deserialize)]
struct DelimitersSource {
    #[serde(default)]
    delimiter_controls: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ListsSource {
    #[serde(default)]
    blacklist: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_specs_files_partial() {
        let files = PackageSpecsYamlSources {
            characters: Some(
                r#"
characters:
  - alpha
  - beta
"#
                .to_string(),
            ),
            commands: Some(
                r#"
commands:
  - name: frac
    kind: prefix
    args:
      - kind: mandatory
        mode: math
"#
                .to_string(),
            ),
            ..Default::default()
        };

        let parsed = files.parse("test");
        assert_eq!(parsed.characters, vec!["alpha", "beta"]);
        assert_eq!(parsed.commands.len(), 1);
        assert_eq!(parsed.commands[0].name, "frac");
        assert!(!parsed.commands[0].has_star_variant);
        assert_eq!(parsed.commands[0].args.len(), 1);
        assert!(parsed.environments.is_empty());
        assert!(parsed.delimiter_controls.is_empty());
        assert!(parsed.blacklist.is_empty());
    }
}
