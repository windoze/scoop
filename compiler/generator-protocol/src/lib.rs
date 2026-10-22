//! Explicit JSON projections for the synchronous source generator protocol.
//! Semantic data and all binding decisions remain in the shared frontend.

mod reference;
pub use reference::{Reference, ReferenceFormatError};
mod declarations;
pub use declarations::{CallableDeclaration, FieldId, MethodId, NominalId};
mod json;
pub use json::QueryJson;
mod annotations;
pub use annotations::Annotations;
mod signature;
pub use signature::pending_method_signature;
mod body;
pub use body::{BodyJson, applied_default};

#[cfg(test)]
mod tests;
