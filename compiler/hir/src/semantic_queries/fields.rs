//! Storage is distinct from logical property access and callable capability.

use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticTypeUse {
    pub declaration: TypeId,
    pub applied: TypeId,
}

#[derive(Clone, Debug)]
pub struct SemanticFieldDescription {
    pub identity: StaticFieldIdentity,
    pub name: String,
    pub value_type: SemanticTypeUse,
    pub storage: NominalFieldStorage,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Debug)]
pub struct SemanticVariantDescription {
    pub identity: scoop_identity::PersistentEnumVariantId,
    pub name: String,
    pub style: VariantStyle,
    pub fields: Vec<SemanticFieldDescription>,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Debug)]
pub struct SemanticNominalFields {
    pub declaration: SourceNominalId,
    pub fields: Vec<SemanticFieldDescription>,
    pub variants: Vec<SemanticVariantDescription>,
}
