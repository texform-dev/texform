pub use crate::builtin::ALL_PACKAGES;
pub use crate::builtin::BuiltinPackage as Package;

pub fn all_package_names() -> Vec<&'static str> {
    crate::builtin::all_package_names()
}

pub fn get(name: &str) -> Option<&'static Package> {
    crate::builtin::lookup_package(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_resource_specs_are_registered() {
        let names: Vec<&str> = ALL_PACKAGES.iter().map(|pkg| pkg.name).collect();
        assert_eq!(all_package_names(), names);
        assert_eq!(
            names,
            vec![
                "ams",
                "base",
                "bboldx",
                "boldsymbol",
                "braket",
                "physics",
                "textmacros"
            ]
        );
    }

    #[test]
    fn registered_packages_expose_builtin_records() {
        for package in ALL_PACKAGES {
            let is_empty = package.characters.is_empty()
                && package.delimiters.is_empty()
                && package.commands.is_empty()
                && package.environments.is_empty();
            assert!(!is_empty, "package {} should not be empty", package.name);
        }
    }
}
