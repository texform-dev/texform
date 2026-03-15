use std::sync::OnceLock;

use crate::specs::PackageSpecs;

pub mod dev;
pub mod test;

pub struct Package {
    pub name: &'static str,
    pub load: fn() -> PackageSpecs,
}

pub const ALL_PACKAGES: &[Package] = &[
    Package {
        name: "test",
        load: test::load,
    },
    Package {
        name: "dev",
        load: dev::load,
    },
];

pub const DEFAULT_EXCLUDED_PACKAGES: &[&str] = &["test", "dev"];
pub const PARSE_WITH_ARGSPEC_DEFAULT_PACKAGES: &[&str] = &["test"];

fn all_package_names() -> &'static [&'static str] {
    static ALL_PACKAGE_NAMES: OnceLock<Box<[&'static str]>> = OnceLock::new();
    ALL_PACKAGE_NAMES.get_or_init(|| {
        ALL_PACKAGES
            .iter()
            .map(|package| package.name)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    })
}

pub fn runtime_default_packages() -> &'static [&'static str] {
    static RUNTIME_DEFAULT_PACKAGES: OnceLock<Box<[&'static str]>> = OnceLock::new();
    RUNTIME_DEFAULT_PACKAGES.get_or_init(|| {
        ALL_PACKAGES
            .iter()
            .map(|package| package.name)
            .filter(|name| !DEFAULT_EXCLUDED_PACKAGES.contains(name))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    })
}

pub fn test_default_packages() -> &'static [&'static str] {
    all_package_names()
}

pub fn get(name: &str) -> Option<&'static Package> {
    ALL_PACKAGES.iter().find(|p| p.name == name)
}
