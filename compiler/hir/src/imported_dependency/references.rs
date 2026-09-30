use crate::{CallableReferenceTarget, Capture, DefinitionOrigin, FunctionTypeId, TypeId};

/// A provider-defined invoke with captures evaluated at this creation site.
#[derive(Debug, Clone)]
pub struct ImportedCallableReference {
    pub definition: crate::concrete::GeneratedCallableRecord,
    pub parent: scoop_identity::CallableTemplateOwner,
    pub owner_type_arguments: Vec<TypeId>,
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub origin: DefinitionOrigin,
}
