use texform_specs::argspec;
use texform_argspec::{ArgForm, ContentMode, ValueKind};

fn main() {
    let parsed = argspec!("s m:T");
    assert_eq!(parsed.source, "s m:T");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].form, ArgForm::Star);
    assert_eq!(parsed[0].kind, ValueKind::Star);
    assert_eq!(
        parsed[1].kind,
        ValueKind::Content {
            mode: ContentMode::Text,
        }
    );
}
