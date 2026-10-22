use super::QueryJson;
use crate::{AnnotationTarget, Reference};
use scoop_hir as hir;
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, hir::AnnotationTargetV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AnnotationTarget::from(*self.0).serialize(serializer)
    }
}

impl<'a, T> Serialize for QueryJson<'a, hir::AnnotationParameterType<T>>
where
    QueryJson<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::AnnotationParameterType::Value(ty) => {
                object!(serializer, {"kind": "value", "valueType": QueryJson(ty)})
            }
            hir::AnnotationParameterType::Annotation(id) => {
                object!(serializer, {"kind": "annotation", "annotation": Reference::from(*id)})
            }
            hir::AnnotationParameterType::AnnotationArray { array, element } => {
                object!(serializer, {
                    "kind": "annotationArray", "array": Reference::from(*array), "element": Reference::from(*element),
                })
            }
        }
    }
}
