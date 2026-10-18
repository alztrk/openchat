//! Bounded Serde values for data crossing the renderer-to-native boundary.

use std::marker::PhantomData;

use serde::{
    de::{self, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use zeroize::Zeroize;

/// A sequence whose deserializer rejects elements beyond the supplied bound.
#[derive(Debug)]
pub struct BoundedVec<T, const MAX_ITEMS: usize>(Vec<T>);

impl<T, const MAX_ITEMS: usize> BoundedVec<T, MAX_ITEMS> {
    /// Returns the accepted sequence after its size has been checked during deserialization.
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<const MAX_BYTES: usize, const MAX_ITEMS: usize> BoundedVec<BoundedText<MAX_BYTES>, MAX_ITEMS> {
    /// Returns accepted text values after checking their individual byte and list limits.
    pub fn into_strings(self) -> Vec<String> {
        self.0.into_iter().map(BoundedText::into_inner).collect()
    }
}

impl<'de, T, const MAX_ITEMS: usize> Deserialize<'de> for BoundedVec<T, MAX_ITEMS>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedVecVisitor<T, const MAX_ITEMS: usize>(PhantomData<T>);

        impl<'de, T, const MAX_ITEMS: usize> Visitor<'de> for BoundedVecVisitor<T, MAX_ITEMS>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAX_ITEMS>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "a sequence containing at most {MAX_ITEMS} items")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let size_hint = sequence.size_hint();
                if size_hint.is_some_and(|size_hint| size_hint > MAX_ITEMS) {
                    return Err(de::Error::custom("sequence exceeds its item limit"));
                }

                let mut values = Vec::with_capacity(size_hint.unwrap_or(0).min(MAX_ITEMS));
                loop {
                    if values.len() == MAX_ITEMS {
                        if sequence.next_element::<IgnoredAny>()?.is_some() {
                            return Err(de::Error::custom("sequence exceeds its item limit"));
                        }
                        return Ok(BoundedVec(values));
                    }
                    match sequence.next_element()? {
                        Some(value) => values.push(value),
                        None => return Ok(BoundedVec(values)),
                    }
                }
            }
        }

        deserializer.deserialize_seq(BoundedVecVisitor::<T, MAX_ITEMS>(PhantomData))
    }
}

/// Text that is rejected during deserialization when it exceeds a UTF-8 byte bound.
#[derive(Debug)]
pub struct BoundedText<const MAX_BYTES: usize>(String);

impl<const MAX_BYTES: usize> BoundedText<MAX_BYTES> {
    /// Returns the text after its byte length has been checked during deserialization.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl<'de, const MAX_BYTES: usize> Deserialize<'de> for BoundedText<MAX_BYTES> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedTextVisitor<const MAX_BYTES: usize>;

        impl<const MAX_BYTES: usize> Visitor<'_> for BoundedTextVisitor<MAX_BYTES> {
            type Value = BoundedText<MAX_BYTES>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "a string no longer than {MAX_BYTES} UTF-8 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAX_BYTES {
                    return Err(E::custom("string exceeds its byte limit"));
                }
                Ok(BoundedText(value.to_owned()))
            }

            fn visit_string<E>(self, mut value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAX_BYTES {
                    value.zeroize();
                    return Err(E::custom("string exceeds its byte limit"));
                }
                Ok(BoundedText(value))
            }
        }

        deserializer.deserialize_string(BoundedTextVisitor::<MAX_BYTES>)
    }
}

/// Deserializes one text field while enforcing a caller-selected UTF-8 byte limit.
pub(crate) fn deserialize_bounded_text<'de, D, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedText::<MAX_BYTES>::deserialize(deserializer).map(BoundedText::into_inner)
}

/// Deserializes a bounded sequence of text values with a shared per-item byte limit.
pub(crate) fn deserialize_bounded_texts<'de, D, const MAX_ITEMS: usize, const MAX_BYTES: usize>(
    deserializer: D,
) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedVec::<BoundedText<MAX_BYTES>, MAX_ITEMS>::deserialize(deserializer)
        .map(BoundedVec::into_strings)
}
