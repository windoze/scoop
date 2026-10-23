//! Complete logical type descriptions, including open declaration binders.

use crate::*;

#[derive(Clone, Debug)]
pub enum SemanticTypeStructure {
    Unit,
    Any,
    Nominal(SemanticNominalDescription),
    Tuple(Vec<TypeId>),
    Function(FunctionType),
    Pointer(TypeId),
    NativeFunctionPointer(FunctionType),
    Parameter(TypeParamDecl),
    Companion {
        host: TypeParamDecl,
        bounds: CompanionBounds,
    },
}

#[derive(Clone, Debug)]
pub struct SemanticNominalDescription {
    pub declaration: SourceNominalId,
    pub kind: StaticNominalKind,
    pub arguments: Vec<TypeId>,
    pub parameters: Vec<TypeParamDecl>,
    /// The declared companion application, irrespective of source visibility.
    pub companion: Option<TypeId>,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}
