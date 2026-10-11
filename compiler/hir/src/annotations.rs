//! Resolved source annotations retained without runtime materialization.

use crate::{
    CanonicalAnnotationValueV1, DeclaredVisibility, DefinitionOrigin, EnumVariantFieldRef,
    EnumVariantRef, NominalOwner, PropertyId, StructFieldRef, TypeId,
};
use scoop_identity::{CborIdentityRecord, PersistentAnnotationId, SourceDeclarationKey};

mod targets;
pub use targets::{AnnotationStorageRole, SourceAnnotationCallable, SourceAnnotationReceiver};
mod rules;
pub use rules::{AnnotationRules, AnnotationTargetKind};

#[derive(Clone, Debug)]
pub struct SourceAnnotationParameter {
    pub name: String,
    pub value_type: crate::AnnotationParameterType<TypeId>,
    pub default: Option<CanonicalAnnotationValueV1>,
}

#[derive(Clone, Debug)]
pub struct SourceAnnotationDeclaration {
    pub identity: CborIdentityRecord<PersistentAnnotationId, SourceDeclarationKey>,
    pub parameters: Vec<SourceAnnotationParameter>,
    pub rules: AnnotationRules,
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
    Annotation(PersistentAnnotationId),
    Callable(SourceAnnotationCallable),
    Parameter {
        callable: SourceAnnotationCallable,
        index: u32,
    },
    Receiver(SourceAnnotationReceiver),
    TypeAlias(crate::ExportTypeAliasId),
    ClassField(crate::ClassFieldId),
    PropertyStorage {
        property: PropertyId,
        role: AnnotationStorageRole,
    },
}

#[derive(Clone, Debug)]
pub struct SourceAnnotationApplication {
    pub annotation: PersistentAnnotationId,
    pub arguments: Vec<CanonicalAnnotationValueV1>,
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
