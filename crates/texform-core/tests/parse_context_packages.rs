use texform_core::parse::{ContentMode, ParseContext};
use texform_knowledge::builtin::PackageName;

#[test]
fn package_context_loads_linebreak_from_base_and_textmacros() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let math_linebreak = ctx
        .lookup_command("\\", ContentMode::Math)
        .expect("expected math linebreak command from base knowledge");
    assert_eq!(math_linebreak.argspec.source, "!s !o:L");
    assert_eq!(
        math_linebreak.allowed_mode,
        texform_core::parse::AllowedMode::Math
    );
    assert_eq!(math_linebreak.from_packages, &["base"]);

    let text_linebreak = ctx
        .lookup_command("\\", ContentMode::Text)
        .expect("expected text linebreak command from textmacros knowledge");
    assert_eq!(text_linebreak.argspec.source, "!s !o:L");
    assert_eq!(
        text_linebreak.allowed_mode,
        texform_core::parse::AllowedMode::Text
    );
    assert_eq!(text_linebreak.from_packages, &["textmacros"]);
}

#[test]
fn explicit_context_exposes_enabled_packages_in_import_order() {
    let ctx = ParseContext::from_packages(&["physics", "base", "braket"]);
    // Core verifies the internal canonical merge order; the facade has the
    // public builder contract test.
    assert_eq!(
        ctx.enabled_packages(),
        &[PackageName::Base, PackageName::Braket, PackageName::Physics]
    );
    assert!(ctx.has_enabled_package(PackageName::Physics));
    assert!(!ctx.has_enabled_package(PackageName::Ams));
}

#[test]
fn empty_context_exposes_no_enabled_packages() {
    let ctx = ParseContext::empty();
    assert!(ctx.enabled_packages().is_empty());
    assert!(!ctx.has_enabled_package(PackageName::Base));
}

#[test]
fn explicit_all_packages_include_braket() {
    let package_names = texform_knowledge::builtin::all_package_names();
    let ctx = ParseContext::from_packages(package_names.as_slice());
    assert!(ctx.lookup_command("braket", ContentMode::Math).is_some());
}
