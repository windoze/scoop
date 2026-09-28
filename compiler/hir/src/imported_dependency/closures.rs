use crate::{Capture, FunctionTypeId, ImportedGenericCallableApplicationId};
use scoop_identity::StructuralDefinitionPath;

/// A dependency closure creation keeps its provider body outside the source
/// function arena, and carries the capture values at this creation site.
#[derive(Debug, Clone)]
pub struct ImportedClosure {
    pub kind: ImportedClosureKind,
    pub application: ImportedGenericCallableApplicationId,
    pub definition_path: StructuralDefinitionPath,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
}

#[derive(Debug, Clone, Copy)]
pub enum ImportedClosureKind {
    Lambda,
    AnonymousFunction,
}
