use super::{QueryJson, sources::Location};
use crate::Reference;
use scoop_hir::{AnnotationElementTypeV1, AnnotationElementV1, CanonicalAnnotationValueV1};
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, CanonicalAnnotationValueV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            CanonicalAnnotationValueV1::Scalar(value) => {
                object!(serializer, { "kind": "scalar", "value": QueryJson(value) })
            }
            CanonicalAnnotationValueV1::IntegralEnum {
                value_type,
                variant,
                value,
            } => object!(serializer, {
                "kind": "integralEnum", "valueType": QueryJson(value_type),
                "variant": Reference::from(*variant), "value": QueryJson(value),
            }),
            CanonicalAnnotationValueV1::Annotation {
                annotation,
                arguments,
            } => object!(serializer, {
                "kind": "annotation", "annotation": Reference::from(*annotation),
                "arguments": QueryJson(arguments.as_slice()),
            }),
            CanonicalAnnotationValueV1::Array {
                element_type,
                elements,
            } => object!(serializer, {
                "kind": "array", "elementType": QueryJson(element_type),
                "elements": QueryJson(elements.as_slice()),
            }),
        }
    }
}

impl Serialize for QueryJson<'_, AnnotationElementTypeV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            AnnotationElementTypeV1::Scalar(kind) => {
                object!(serializer, { "kind": "scalar", "scalar": QueryJson(kind) })
            }
            AnnotationElementTypeV1::IntegralEnum(ty) => {
                object!(serializer, { "kind": "integralEnum", "valueType": QueryJson(ty) })
            }
            AnnotationElementTypeV1::Annotation(id) => {
                object!(serializer, { "kind": "annotation", "annotation": Reference::from(*id) })
            }
        }
    }
}

impl Serialize for QueryJson<'_, AnnotationElementV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "value": QueryJson(&self.0.value),
            "origin": Location { source: &self.0.source, span: self.0.span },
        })
    }
}
