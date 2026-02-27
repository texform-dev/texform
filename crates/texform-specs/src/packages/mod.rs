use crate::specs::PackageSpecs;

pub mod base;
pub mod dev;

pub struct Package {
    pub name: &'static str,
    pub load: fn() -> PackageSpecs,
}

pub const ALL_PACKAGES: &[Package] = &[
    Package {
        name: "base",
        load: base::load,
    },
    Package {
        name: "dev",
        load: dev::load,
    },
];

pub const RUNTIME_DEFAULT_PACKAGES: &[&str] = &["base"];
pub const TEST_DEFAULT_PACKAGES: &[&str] = &["base", "dev"];

pub fn get(name: &str) -> Option<&'static Package> {
    ALL_PACKAGES.iter().find(|p| p.name == name)
}
