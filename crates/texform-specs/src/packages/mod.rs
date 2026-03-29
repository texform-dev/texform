use crate::specs::{PackageSpecs, load_package_specs_from_str};

macro_rules! package_loader {
    ($fn_name:ident, $file:literal, $name:literal) => {
        fn $fn_name() -> PackageSpecs {
            load_package_specs_from_str(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../resources/specs/",
                    $file
                )),
                $name,
            )
        }
    };
}

package_loader!(load_ams, "ams.yaml", "ams");
package_loader!(load_base, "base.yaml", "base");
package_loader!(load_bboldx, "bboldx.yaml", "bboldx");
package_loader!(load_boldsymbol, "boldsymbol.yaml", "boldsymbol");
package_loader!(load_physics, "physics.yaml", "physics");
package_loader!(load_textmacros, "textmacros.yaml", "textmacros");

pub struct Package {
    pub name: &'static str,
    pub load: fn() -> PackageSpecs,
}

pub const ALL_PACKAGES: &[Package] = &[
    Package {
        name: "ams",
        load: load_ams,
    },
    Package {
        name: "base",
        load: load_base,
    },
    Package {
        name: "bboldx",
        load: load_bboldx,
    },
    Package {
        name: "boldsymbol",
        load: load_boldsymbol,
    },
    Package {
        name: "physics",
        load: load_physics,
    },
    Package {
        name: "textmacros",
        load: load_textmacros,
    },
];

pub fn runtime_default_packages() -> &'static [&'static str] {
    &[]
}

pub fn get(name: &str) -> Option<&'static Package> {
    ALL_PACKAGES.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_resource_specs_are_registered() {
        let names: Vec<&str> = ALL_PACKAGES.iter().map(|pkg| pkg.name).collect();
        assert_eq!(
            names,
            vec![
                "ams",
                "base",
                "bboldx",
                "boldsymbol",
                "physics",
                "textmacros"
            ]
        );
    }

    #[test]
    fn registered_packages_load_embedded_yaml() {
        for package in ALL_PACKAGES {
            let specs = (package.load)();
            let is_empty = specs.characters.is_empty()
                && specs.commands.is_empty()
                && specs.environments.is_empty()
                && specs.delimiter_controls.is_empty();
            assert!(!is_empty, "package {} should not be empty", package.name);
        }
    }
}
