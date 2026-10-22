use crate::{QueryJson, Reference, json::object};
use scoop_hir::{SemanticAnnotationTarget, SemanticAnnotations, SemanticQueries};
use serde::{Serialize, Serializer, ser::SerializeSeq};

mod declarations;
pub use declarations::annotation_declaration;

/// Annotation applications from one already selected semantic target.
pub struct Annotations<'a> {
    pub queries: &'a dyn SemanticQueries,
    pub target: SemanticAnnotationTarget,
}

impl Serialize for Annotations<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.queries.describe_annotations(self.target) {
            SemanticAnnotations::Current(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&Application {
                        annotation: value.annotation,
                        arguments: &value.arguments,
                        origin: QueryJson(&self.queries.source_location(value.definition_origin)),
                    })?;
                }
                sequence.end()
            }
            SemanticAnnotations::Imported(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    let origin = value.definition_origin.origin();
                    sequence.serialize_element(&Application {
                        annotation: value.annotation,
                        arguments: &value.arguments,
                        origin: crate::json::Location {
                            source: origin.source(),
                            span: origin.span(),
                        },
                    })?;
                }
                sequence.end()
            }
        }
    }
}

struct Application<'a, O> {
    annotation: scoop_identity::PersistentAnnotationId,
    arguments: &'a [scoop_hir::CanonicalAnnotationValueV1],
    origin: O,
}

impl<O: Serialize> Serialize for Application<'_, O> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "annotation": Reference::from(self.annotation),
            "arguments": QueryJson(self.arguments), "origin": &self.origin,
        })
    }
}
