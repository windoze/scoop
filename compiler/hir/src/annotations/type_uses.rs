use crate::{
    AnnotationParameterType, CanonicalAnnotationValueV1, SourceAnnotationTarget, TypeId,
    TypeParamId,
};
use scoop_identity::{PersistentAnnotationId, SourceIdentity, SourceSpan};

mod paths;
mod slots;
#[cfg(test)]
mod tests;
pub use paths::{TypeUseStep, substitute_type_annotations, type_annotation_path_exists};
pub use slots::AnnotationTypeSlot;

/// Identity of a source occurrence, independent of the interned semantic type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnnotationTypeUseId {
    pub source: SourceIdentity,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAnnotation {
    pub annotation: PersistentAnnotationId,
    pub arguments: Vec<CanonicalAnnotationValueV1>,
    pub source: SourceIdentity,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAnnotationNode {
    pub path: Vec<TypeUseStep>,
    pub annotations: Vec<TypeAnnotation>,
}

#[derive(Clone, Debug)]
pub struct SourceAnnotatedTypeUse {
    pub identity: AnnotationTypeUseId,
    pub ty: AnnotationParameterType<TypeId>,
    pub parameters: Vec<TypeParamId>,
    pub nodes: Vec<TypeAnnotationNode>,
}

#[derive(Clone, Debug)]
pub struct SourceAnnotationTypeSite {
    pub target: SourceAnnotationTarget,
    pub slot: AnnotationTypeSlot,
    pub type_use: AnnotationTypeUseId,
}
