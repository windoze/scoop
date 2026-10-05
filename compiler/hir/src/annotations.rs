//! Resolved source annotations retained without runtime materialization.

use crate::{
    CanonicalConstValueV1, DeclaredVisibility, DefinitionOrigin, EnumVariantFieldRef,
    EnumVariantRef, NominalOwner, PropertyId, StructFieldRef, TypeId,
};
use scoop_identity::{CborIdentityRecord, PersistentAnnotationId, SourceDeclarationKey};

#[derive(Clone, Debug)]
pub struct SourceAnnotationParameter {
    pub name: String,
    pub value_type: TypeId,
    pub default: Option<CanonicalConstValueV1>,
}

#[derive(Clone, Debug)]
pub struct SourceAnnotationDeclaration {
    pub identity: CborIdentityRecord<PersistentAnnotationId, SourceDeclarationKey>,
    pub parameters: Vec<SourceAnnotationParameter>,
    pub visibility: DeclaredVisibility,
    pub definition_origin: DefinitionOrigin,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SourceAnnotationTarget {
    Nominal(NominalOwner),
    Variant(EnumVariantRef),
    Field(StructFieldRef),
    VariantField(EnumVariantFieldRef),
    Property(PropertyId),
}

#[derive(Clone, Debug)]
pub struct SourceAnnotationApplication {
    pub annotation: PersistentAnnotationId,
    pub arguments: Vec<CanonicalConstValueV1>,
    pub definition_origin: DefinitionOrigin,
}

#[derive(Clone, Debug)]
pub struct SourceAnnotatedTarget {
    pub target: SourceAnnotationTarget,
    pub annotations: Vec<SourceAnnotationApplication>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceAnnotations {
    pub declarations: Vec<SourceAnnotationDeclaration>,
    pub targets: Vec<SourceAnnotatedTarget>,
}
