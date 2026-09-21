use std::fmt;

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess,
    Unexpected, VariantAccess, Visitor,
};
use serde_path_to_error::Segment;

/// Path-aware serde error produced by [`read`].
pub type ReadError = serde_path_to_error::Error<serde_json::Error>;

/// Deserialize `value` so every derived struct accepts only a JSON object.
pub fn read<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, ReadError> {
    serde_path_to_error::deserialize(ObjectsOnly::new(value))
}

/// Convert a snake_case identifier to camelCase.
///
/// JavaScript bindings use this so error paths and `unknown field` tokens
/// match the camelCase keys callers wrote.
pub fn snake_to_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut uppercase_next = false;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Format a [`ReadError`] as `invalid {what}: {path}: {message}`.
///
/// `rename` is applied to path map keys and, for `unknown` / `missing` /
/// `duplicate field` messages, to backtick-quoted tokens. Other messages
/// are left unchanged so enum values such as `sup_first` stay snake_case.
pub fn format_read_error(error: &ReadError, what: &str, rename: impl Fn(&str) -> String) -> String {
    let path = format_path(error.path(), &rename);
    let message = format_message(&error.inner().to_string(), &rename);
    if path.is_empty() {
        format!("invalid {what}: {message}")
    } else {
        format!("invalid {what}: {path}: {message}")
    }
}

fn format_path(path: &serde_path_to_error::Path, rename: &impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    for segment in path.iter() {
        match segment {
            Segment::Map { key } => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(&rename(key));
            }
            Segment::Seq { index } => {
                out.push('[');
                out.push_str(&index.to_string());
                out.push(']');
            }
            Segment::Enum { variant } => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(variant);
            }
            Segment::Unknown => {}
        }
    }
    out
}

fn format_message(raw: &str, rename: &impl Fn(&str) -> String) -> String {
    let message = strip_location_suffix(raw);
    if message.starts_with("unknown field")
        || message.starts_with("missing field")
        || message.starts_with("duplicate field")
    {
        rewrite_backtick_tokens(message, rename)
    } else {
        message.to_owned()
    }
}

fn strip_location_suffix(message: &str) -> &str {
    let Some(at) = message.rfind(" at line ") else {
        return message;
    };
    let suffix = &message[at + " at line ".len()..];
    let Some((line, column)) = suffix.split_once(" column ") else {
        return message;
    };
    if !line.is_empty()
        && !column.is_empty()
        && line.bytes().all(|b| b.is_ascii_digit())
        && column.bytes().all(|b| b.is_ascii_digit())
    {
        &message[..at]
    } else {
        message
    }
}

fn rewrite_backtick_tokens(message: &str, rename: &impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(start) = rest.find('`') {
        out.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            let token = &rest[..end];
            out.push('`');
            out.push_str(&rename(token));
            out.push('`');
            rest = &rest[end + 1..];
        } else {
            out.push('`');
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

struct ObjectsOnly<D> {
    inner: D,
}

impl<D> ObjectsOnly<D> {
    fn new(inner: D) -> Self {
        Self { inner }
    }
}

macro_rules! forward_deserialize {
    ($($method:ident)*) => {
        $(
            fn $method<V>(self, visitor: V) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.inner.$method(WrapVisitor::forward(visitor))
            }
        )*
    };
}

impl<'de, D> Deserializer<'de> for ObjectsOnly<D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    forward_deserialize! {
        deserialize_any
        deserialize_bool
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
        deserialize_f32 deserialize_f64
        deserialize_char deserialize_str deserialize_string
        deserialize_bytes deserialize_byte_buf
        deserialize_option deserialize_unit
        deserialize_seq deserialize_map
        deserialize_identifier deserialize_ignored_any
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_unit_struct(name, WrapVisitor::forward(visitor))
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_newtype_struct(name, WrapVisitor::forward(visitor))
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_tuple(len, WrapVisitor::forward(visitor))
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_tuple_struct(name, len, WrapVisitor::forward(visitor))
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_enum(name, variants, WrapVisitor::forward(visitor))
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .deserialize_struct(name, fields, WrapVisitor::reject_seq(visitor))
    }

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

struct WrapVisitor<V> {
    inner: V,
    reject_seq: bool,
}

impl<V> WrapVisitor<V> {
    fn forward(inner: V) -> Self {
        Self {
            inner,
            reject_seq: false,
        }
    }

    fn reject_seq(inner: V) -> Self {
        Self {
            inner,
            reject_seq: true,
        }
    }
}

macro_rules! forward_visit {
    ($($method:ident($ty:ty))*) => {
        $(
            fn $method<E>(self, v: $ty) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.inner.$method(v)
            }
        )*
    };
}

impl<'de, V> Visitor<'de> for WrapVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    forward_visit! {
        visit_bool(bool)
        visit_i8(i8) visit_i16(i16) visit_i32(i32) visit_i64(i64) visit_i128(i128)
        visit_u8(u8) visit_u16(u16) visit_u32(u32) visit_u64(u64) visit_u128(u128)
        visit_f32(f32) visit_f64(f64)
        visit_char(char)
        visit_str(&str)
        visit_string(String)
        visit_bytes(&[u8])
        visit_byte_buf(Vec<u8>)
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.inner.visit_borrowed_str(v)
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.inner.visit_borrowed_bytes(v)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.inner.visit_none()
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.inner.visit_unit()
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.inner.visit_some(ObjectsOnly::new(deserializer))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.inner
            .visit_newtype_struct(ObjectsOnly::new(deserializer))
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if self.reject_seq {
            return Err(de::Error::invalid_type(Unexpected::Seq, &self));
        }
        self.inner.visit_seq(WrapSeqAccess { inner: seq })
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.inner.visit_map(WrapMapAccess { inner: map })
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        self.inner.visit_enum(WrapEnumAccess { inner: data })
    }
}

struct WrapSeed<S>(S);

impl<'de, S> DeserializeSeed<'de> for WrapSeed<S>
where
    S: DeserializeSeed<'de>,
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<S::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.deserialize(ObjectsOnly::new(deserializer))
    }
}

struct WrapMapAccess<A> {
    inner: A,
}

impl<'de, A> MapAccess<'de> for WrapMapAccess<A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.inner.next_key_seed(WrapSeed(seed))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.inner.next_value_seed(WrapSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct WrapSeqAccess<A> {
    inner: A,
}

impl<'de, A> SeqAccess<'de> for WrapSeqAccess<A>
where
    A: SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.inner.next_element_seed(WrapSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct WrapEnumAccess<A> {
    inner: A,
}

impl<'de, A> EnumAccess<'de> for WrapEnumAccess<A>
where
    A: EnumAccess<'de>,
{
    type Error = A::Error;
    type Variant = WrapVariantAccess<A::Variant>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let (value, variant) = self.inner.variant_seed(WrapSeed(seed))?;
        Ok((value, WrapVariantAccess { inner: variant }))
    }
}

struct WrapVariantAccess<A> {
    inner: A,
}

impl<'de, A> VariantAccess<'de> for WrapVariantAccess<A>
where
    A: VariantAccess<'de>,
{
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        self.inner.unit_variant()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.inner.newtype_variant_seed(WrapSeed(seed))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.inner.tuple_variant(len, WrapVisitor::forward(visitor))
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.inner
            .struct_variant(fields, WrapVisitor::reject_seq(visitor))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::bindings::input::{
        FlattenGroupsConfigInput, RewriteConfigInput, SerializeOptionsInput, TransformConfigInput,
    };

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(expecting = "an object")]
    struct Inner {
        name: String,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(expecting = "an object")]
    struct Holder {
        tags: Vec<String>,
        items: Vec<Inner>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ScriptOrder {
        SubFirst,
        SupFirst,
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(default, deny_unknown_fields, expecting = "an object")]
    struct ScriptOptions {
        order: Option<ScriptOrder>,
    }

    fn expect_sequence(result: Result<impl std::fmt::Debug, ReadError>) {
        let error = result.expect_err("sequence should be rejected");
        assert!(
            error
                .inner()
                .to_string()
                .contains("invalid type: sequence, expected an object"),
            "unexpected error: {}",
            error.inner()
        );
    }

    #[test]
    fn unknown_key_path_uses_snake_case_input_names() {
        let error = read::<TransformConfigInput>(json!({
            "flatten_groups": {
                "preserve_empty_gruop": true
            }
        }))
        .expect_err("typo should be rejected");
        assert_eq!(
            error.path().to_string(),
            "flatten_groups.preserve_empty_gruop"
        );
        assert!(error.inner().to_string().starts_with("unknown field"));
    }

    #[test]
    fn rewrite_array_is_rejected_as_non_object() {
        expect_sequence(read::<TransformConfigInput>(json!({ "rewrite": [] })));
        expect_sequence(read::<TransformConfigInput>(
            json!({ "rewrite": [false, 50] }),
        ));
    }

    #[test]
    fn top_level_array_is_rejected_as_non_object() {
        expect_sequence(read::<TransformConfigInput>(json!([])));
        expect_sequence(read::<TransformConfigInput>(json!([[true, 1]])));
    }

    #[test]
    fn vec_fields_accept_arrays_of_objects() {
        let holder = read::<Holder>(json!({
            "tags": ["a"],
            "items": [{ "name": "x" }]
        }))
        .expect("arrays of scalars and objects should deserialize");
        assert_eq!(
            holder,
            Holder {
                tags: vec!["a".into()],
                items: vec![Inner { name: "x".into() }],
            }
        );
    }

    #[test]
    fn vec_of_structs_rejects_nested_sequences() {
        expect_sequence(read::<Holder>(json!({
            "tags": ["a"],
            "items": [["command", "foo"]]
        })));
    }

    #[test]
    fn rewrite_null_is_equivalent_to_omitted() {
        let omitted = read::<TransformConfigInput>(json!({})).unwrap();
        let explicit_null = read::<TransformConfigInput>(json!({ "rewrite": null })).unwrap();
        assert_eq!(omitted, explicit_null);
        assert_eq!(explicit_null.rewrite, None);
    }

    #[test]
    fn rewrite_type_errors() {
        let string_bool = read::<TransformConfigInput>(json!({
            "rewrite": { "enabled": "yes" }
        }))
        .expect_err("string bool should fail");
        assert!(
            string_bool
                .inner()
                .to_string()
                .contains("invalid type: string"),
            "{}",
            string_bool.inner()
        );

        let bool_int = read::<TransformConfigInput>(json!({
            "rewrite": { "max_iterations": true }
        }))
        .expect_err("bool integer should fail");
        assert!(
            bool_int.inner().to_string().contains("invalid type"),
            "{}",
            bool_int.inner()
        );

        let float_int = read::<TransformConfigInput>(json!({
            "rewrite": { "max_iterations": 1.0 }
        }))
        .expect_err("float integer should fail");
        assert!(
            float_int.inner().to_string().contains("invalid type"),
            "{}",
            float_int.inner()
        );
    }

    #[test]
    fn format_read_error_identity_keeps_snake_case_unknown_field() {
        let error = read::<TransformConfigInput>(json!({
            "flatten_groups": {
                "preserve_empty_gruop": true
            }
        }))
        .unwrap_err();
        let message = format_read_error(&error, "transform config", |key| key.to_owned());
        assert!(
            message.starts_with(
                "invalid transform config: flatten_groups.preserve_empty_gruop: unknown field `preserve_empty_gruop`"
            ),
            "{message}"
        );
    }

    #[test]
    fn format_read_error_renames_unknown_field_tokens() {
        let error = read::<TransformConfigInput>(json!({
            "flatten_groups": {
                "preserve_empty_gruop": true
            }
        }))
        .unwrap_err();
        let identity = format_read_error(&error, "transform config", |key| key.to_owned());
        assert!(
            identity.contains("flatten_groups.preserve_empty_gruop"),
            "{identity}"
        );
        assert!(
            identity.contains("unknown field `preserve_empty_gruop`"),
            "{identity}"
        );

        let camel = format_read_error(&error, "transform config", snake_to_camel);
        assert!(
            camel.starts_with(
                "invalid transform config: flattenGroups.preserveEmptyGruop: unknown field `preserveEmptyGruop`"
            ),
            "{camel}"
        );
        assert!(
            camel.contains("preserveRenderedSpacing"),
            "expected-field list should be renamed too: {camel}"
        );
    }

    #[test]
    fn format_read_error_camelizes_transform_config_unknown_field() {
        let error = read::<TransformConfigInput>(json!({
            "flatten_groups": {
                "preserve_empty_gruop": true
            }
        }))
        .unwrap_err();
        let message = format_read_error(&error, "transform config", snake_to_camel);
        assert!(
            message.starts_with(
                "invalid transform config: flattenGroups.preserveEmptyGruop: unknown field `preserveEmptyGruop`, expected `enabled` or `preserveRenderedSpacing`"
            ),
            "{message}"
        );
        assert!(message.contains("preserveRenderedSpacing"), "{message}");
    }

    #[test]
    fn format_read_error_leaves_unknown_variant_message_unchanged() {
        let error = read::<ScriptOptions>(json!({
            "order": "sup_frist"
        }))
        .unwrap_err();
        let inner = error.inner().to_string();
        assert!(
            inner.contains("unknown variant `sup_frist`, expected `sub_first` or `sup_first`"),
            "{inner}"
        );
        let formatted = format_read_error(&error, "serialize options", snake_to_camel);
        assert!(
            formatted.contains("unknown variant `sup_frist`, expected `sub_first` or `sup_first`"),
            "{formatted}"
        );
    }

    #[test]
    fn flatten_groups_input_roundtrips_null_enabled() {
        let input = read::<FlattenGroupsConfigInput>(json!({
            "enabled": null,
            "preserve_rendered_spacing": null
        }))
        .unwrap();
        assert_eq!(input.enabled, None);
        assert_eq!(input.preserve_rendered_spacing, None);
        let rewrite = read::<RewriteConfigInput>(json!({ "enabled": null })).unwrap();
        assert_eq!(rewrite.enabled, None);
        let transform = read::<TransformConfigInput>(json!({ "flatten_groups": null })).unwrap();
        assert_eq!(transform.flatten_groups, None);
    }

    #[test]
    fn serialize_options_reject_legacy_nested_keys() {
        let error = read::<SerializeOptionsInput>(json!({
            "math": { "scripts": { "order": "sup_first" } }
        }))
        .expect_err("legacy nested keys should be rejected");
        let message = format_read_error(&error, "serialize options", |key| key.to_owned());
        assert!(message.contains("unknown field `math`"), "{message}");
        assert!(message.contains("script_order"), "{message}");
    }

    #[test]
    fn serialize_options_null_leaf_is_equivalent_to_omitted() {
        let omitted = read::<SerializeOptionsInput>(json!({})).unwrap();
        let explicit_null = read::<SerializeOptionsInput>(json!({ "script_order": null })).unwrap();
        assert_eq!(omitted, explicit_null);
        assert_eq!(explicit_null.script_order, None);
    }
}
