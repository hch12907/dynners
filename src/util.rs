use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;

/// This helper is intended to aid deserializing fields that can contain a
/// string or a string array. It will always deserialize a single string into
/// a `Vector` containing that string. String arrays are deserialized as-is.
/// 
/// For example,
/// ```
/// TOML ["a", "b"] ---> vec![Box("a"), Box("b")]` and
/// TOML "c" ---> vec![Box("c")]
/// ```
pub(super) fn one_or_more_string<'de, D>(deserializer: D) -> Result<Vec<Box<str>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OneOrMoreString;

    impl<'de> Visitor<'de> for OneOrMoreString {
        type Value = Vec<Box<str>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a string array")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(vec![value.to_string().into_boxed_str()])
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(OneOrMoreString)
}
