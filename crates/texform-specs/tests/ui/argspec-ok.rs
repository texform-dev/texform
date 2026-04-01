use texform_specs::argspec;
use texform_argspec::{ArgForm, ContentMode, ValueKind};

fn main() {
    let specs = argspec!("s m:T");
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].form, ArgForm::Star);
    assert_eq!(specs[0].kind, ValueKind::Star);
    assert_eq!(
        specs[1].kind,
        ValueKind::Content {
            mode: ContentMode::Text,
        }
    );
}
