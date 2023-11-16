use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use std::mem::MaybeUninit;

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

/// A super simple fixed-allocation vector.
pub struct FixedVec<T, const N: usize> {
    length: u32,
    array: [MaybeUninit<T>; N],
}

impl<T: Copy, const N: usize> FixedVec<T, N> {
    pub fn new() -> Self {
        Self {
            length: 0,
            array: [MaybeUninit::uninit(); N],
        }
    }

    pub fn get(&self, index: u32) -> Option<&T> {
        self.as_slice().get(index as usize)
    }

    pub fn push(&mut self, item: T) -> Option<T> {
        if self.length < N as u32 {
            self.array[self.length as usize] = MaybeUninit::new(item);
            self.length += 1;
            None
        } else {
            Some(item)
        }
    }

    pub fn as_slice(&self) -> &[T] {
        // CAST-SAFETY: MaybeUninit<T> is sized & aligned the same as T
        let ptr = self.array.as_ptr() as *const T;
        let len = self.length;

        // SAFETY: The properties of self.length (increment-on-push) guarantees
        //         that all indices before self.length contain valid items
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::FixedVec;

    #[test]
    fn fixed_vec() {
        let mut vec = FixedVec::<u32, 2>::new();
        assert!(vec.push(10).is_none());
        assert!(vec.push(20).is_none());
        assert!(!vec.push(30).is_none());

        assert!(vec.get(0).is_some());
        assert!(vec.get(1).is_some());
        assert!(!vec.get(2).is_some());
        assert!(!vec.get(12345678).is_some());

        assert_eq!(vec.as_slice().len(), 2);

        let mut vec = FixedVec::<u32, 2>::new();
        assert_eq!(vec.as_slice().len(), 0);
        assert!(vec.push(10).is_none());
        assert_eq!(vec.as_slice().len(), 1);
        assert!(vec.push(20).is_none());
        assert_eq!(vec.as_slice().len(), 2);
        assert!(!vec.push(30).is_none());
        assert_eq!(vec.as_slice().len(), 2);
    }
}
