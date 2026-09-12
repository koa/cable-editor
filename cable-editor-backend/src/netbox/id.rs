use super::schema;
use cynic::impl_scalar;
use serde::{Deserialize, Serialize, Serializer, de};
use std::{
    fmt::{Display, Formatter},
    marker::PhantomData,
};

#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq, Hash, Default)]
pub struct NumberId(u32);

impl From<u32> for NumberId {
    fn from(value: u32) -> Self {
        NumberId(value)
    }
}
impl<'de> Deserialize<'de> for NumberId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(IdVisitor(PhantomData::<NumberId>::default()))
    }
}
impl Serialize for NumberId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}
impl From<NumberId> for u32 {
    fn from(value: NumberId) -> Self {
        value.0
    }
}
impl Display for NumberId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl_scalar!(NumberId, schema::ID);
pub struct IdVisitor<V: From<u32>>(pub PhantomData<V>);

impl<'de, V: From<u32>> de::Visitor<'de> for IdVisitor<V> {
    type Value = V;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a number formatted a string")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(V::from(v.parse().map_err(|e| E::custom(format!("{e:?}")))?))
    }
}
