//! Logical body JSON borrows the shared projection and its default application.

use crate::{CallableDeclaration, QueryJson, Reference, json::object};
use scoop_hir as hir;
use scoop_identity as identity;
use serde::{Serialize, Serializer, ser::Error};

mod calls;
mod control_flow;
mod effects;
mod expressions;
mod locals;
mod nested;
mod operations;
mod patterns;
mod references;
mod requirements;
mod roots;
mod statements;

pub struct BodyJson<'a, T: ?Sized> {
    node: &'a T,
    application: Option<&'a hir::SemanticDefaultApplication>,
    introduced_binders: u32,
}

impl<'a, T: ?Sized> BodyJson<'a, T> {
    pub const fn new(node: &'a T) -> Self {
        Self {
            node,
            application: None,
            introduced_binders: 0,
        }
    }

    fn child<'b, U: ?Sized>(&self, node: &'b U) -> BodyJson<'b, U>
    where
        'a: 'b,
    {
        BodyJson {
            node,
            application: self.application,
            introduced_binders: self.introduced_binders,
        }
    }
}

pub fn applied_default(
    application: &hir::SemanticDefaultApplication,
) -> BodyJson<'_, hir::ExportDefaultTemplateV1> {
    BodyJson {
        node: application.definition(),
        application: Some(application),
        introduced_binders: 0,
    }
}

impl<'a, T> Serialize for BodyJson<'a, [T]>
where
    BodyJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.node.len()))?;
        for value in self.node {
            sequence.serialize_element(&self.child(value))?;
        }
        sequence.end()
    }
}

impl<'a, T> Serialize for BodyJson<'a, Vec<T>>
where
    BodyJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.child(self.node.as_slice()).serialize(serializer)
    }
}

impl<'a, T: ?Sized> Serialize for BodyJson<'a, Box<T>>
where
    BodyJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.child(self.node.as_ref()).serialize(serializer)
    }
}

impl<'a, T> Serialize for BodyJson<'a, Option<T>>
where
    BodyJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            Some(value) => serializer.serialize_some(&self.child(value)),
            None => serializer.serialize_none(),
        }
    }
}

impl Serialize for BodyJson<'_, identity::SignatureTypeKey> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.application {
            None => QueryJson(self.node).serialize(serializer),
            Some(application) => {
                let value = application
                    .applied_type_in_scope(self.node, self.introduced_binders)
                    .map_err(S::Error::custom)?;
                QueryJson(&value).serialize(serializer)
            }
        }
    }
}

impl Serialize for BodyJson<'_, identity::OptionalSignatureType> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            identity::OptionalSignatureType::Absent => serializer.serialize_none(),
            identity::OptionalSignatureType::Present(value) => {
                serializer.serialize_some(&self.child(value))
            }
        }
    }
}
