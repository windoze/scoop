//! Explicit JSON projections for the synchronous source generator protocol.
//! Semantic data and all binding decisions remain in the shared frontend.

mod reference;
pub use reference::{Reference, ReferenceFormatError};
mod type_reference;
pub use type_reference::TypeReference;
mod declarations;
pub use declarations::{CallableDeclaration, FieldId, MethodId, NominalId};
mod json;
pub use json::QueryJson;
mod annotations;
pub use annotations::{Annotations, annotation_declaration};
mod annotation_targets;
pub use annotation_targets::{AnnotationStorageRole, AnnotationTarget, PropertyDeclaration};
mod signature;
pub use signature::pending_method_signature;
mod body;
pub use body::{BodyJson, applied_default};
mod descriptions;
pub use descriptions::{member_description, type_description};
mod queries;
pub use queries::{
    CompilerQuery, DefaultReference, ExportNamespace, Query, TypeRelation, execute_query,
};

#[cfg(test)]
mod tests;
