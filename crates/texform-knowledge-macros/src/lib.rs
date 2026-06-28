use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{LitStr, parse_macro_input};
use texform_argspec::{ArgForm, ArgSpec, DelimiterToken, ValueKind, parse_arg_specs};

#[proc_macro]
pub fn argspec(input: TokenStream) -> TokenStream {
    let literal = parse_macro_input!(input as LitStr);
    let source = literal.value();
    match parse_arg_specs(source.as_str(), "argspec literal") {
        Ok(specs) => render_parsed_arg_spec(specs.as_slice(), source.as_str()).into(),
        Err(error) => syn::Error::new(
            literal.span(),
            format!(
                "invalid argspec literal at char {}: {}",
                error.char_index, error.message
            ),
        )
        .to_compile_error()
        .into(),
    }
}

fn render_parsed_arg_spec(specs: &[ArgSpec], source: &str) -> TokenStream2 {
    let rendered_specs = specs.iter().map(render_arg_spec);
    quote! {
        ::texform_knowledge::specs::ParsedArgSpec {
            args: &[#(#rendered_specs),*],
            source: #source,
        }
    }
}

fn render_arg_spec(spec: &ArgSpec) -> TokenStream2 {
    let required = spec.required;
    let no_leading_space = spec.no_leading_space;
    let nullable = spec.nullable;
    let kind = render_value_kind(spec.kind);
    let form = render_arg_form(&spec.form);

    quote! {
        ::texform_knowledge::specs::ArgSpec {
            required: #required,
            no_leading_space: #no_leading_space,
            nullable: #nullable,
            kind: #kind,
            form: #form,
        }
    }
}

fn render_value_kind(kind: ValueKind) -> TokenStream2 {
    match kind {
        ValueKind::Content { mode } => {
            let mode = render_content_mode(mode);
            quote! {
                ::texform_knowledge::specs::ValueKind::Content { mode: #mode }
            }
        }
        ValueKind::OperatorName => quote!(::texform_knowledge::specs::ValueKind::OperatorName),
        ValueKind::Delimiter => quote!(::texform_knowledge::specs::ValueKind::Delimiter),
        ValueKind::CSName => quote!(::texform_knowledge::specs::ValueKind::CSName),
        ValueKind::Dimension => quote!(::texform_knowledge::specs::ValueKind::Dimension),
        ValueKind::Integer => quote!(::texform_knowledge::specs::ValueKind::Integer),
        ValueKind::KeyVal => quote!(::texform_knowledge::specs::ValueKind::KeyVal),
        ValueKind::Column => quote!(::texform_knowledge::specs::ValueKind::Column),
        ValueKind::Star => quote!(::texform_knowledge::specs::ValueKind::Star),
    }
}

fn render_arg_form(form: &ArgForm) -> TokenStream2 {
    match form {
        ArgForm::Standard => quote!(::texform_knowledge::specs::ArgForm::Standard),
        ArgForm::Star => quote!(::texform_knowledge::specs::ArgForm::Star),
        ArgForm::Group => quote!(::texform_knowledge::specs::ArgForm::Group),
        ArgForm::Delimited { open, close } => {
            let open = render_delimiter_token(open);
            let close = render_delimiter_token(close);
            quote! {
                ::texform_knowledge::specs::ArgForm::Delimited {
                    open: #open,
                    close: #close,
                }
            }
        }
        ArgForm::Paired { pairs } => {
            let rendered_pairs = pairs.iter().map(|(open, close)| {
                let open = render_delimiter_token(open);
                let close = render_delimiter_token(close);
                quote! { (#open, #close) }
            });
            quote! {
                ::texform_knowledge::specs::ArgForm::Paired {
                    pairs: ::std::borrow::Cow::Borrowed(&[#(#rendered_pairs),*]),
                }
            }
        }
    }
}

fn render_delimiter_token(token: &DelimiterToken) -> TokenStream2 {
    match token {
        DelimiterToken::Char(ch) => quote!(::texform_knowledge::specs::DelimiterToken::Char(#ch)),
        DelimiterToken::ControlSeq(name) => {
            let name = name.as_ref();
            quote! {
                ::texform_knowledge::specs::DelimiterToken::ControlSeq(
                    ::std::borrow::Cow::Borrowed(#name)
                )
            }
        }
    }
}

fn render_content_mode(mode: texform_argspec::ContentMode) -> TokenStream2 {
    match mode {
        texform_argspec::ContentMode::Math => quote!(::texform_knowledge::specs::ContentMode::Math),
        texform_argspec::ContentMode::Text => quote!(::texform_knowledge::specs::ContentMode::Text),
    }
}
