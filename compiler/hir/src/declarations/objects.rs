use super::*;

/// One source `object` declaration. Its declaration, type, value and
/// published managed root deliberately belong to four distinct identity
/// domains; the backing class is only the physical object representation.
#[derive(Debug, Clone)]
pub struct ObjectDecl {
    /// Native-emission identity of the object declaration itself. Its
    /// compiler-owned backing class carries a distinct nominal role/stem.
    pub link_stem: NominalLinkStem,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub object_type: ObjectTypeId,
    pub singleton_value: SingletonValueId,
    pub kind: ObjectKind,
    pub backing_class: ClassId,
    pub span: Span,
}

/// Whether an object is an ordinary declaration or the singleton attached to
/// a nominal host through a separately typed companion relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Standalone,
    Companion(CompanionRelationId),
}

/// Source spelling of a companion. `Named` companions are addressable through
/// both this explicit name and the fixed `Companion` alias; `Default` has only
/// the alias. The host is a declaration template, so generic applications do
/// not replicate this relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionRelation {
    pub host: NominalOwner,
    pub object: ObjectId,
    pub name: CompanionName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionName {
    Default,
    Named(String),
}

/// Nominal type identity of an object declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectType {
    pub declaration: ObjectId,
    pub representation: ClassApplicationId,
    pub canonical_type: TypeId,
}

/// The unique source value denoted by an object name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingletonValue {
    pub declaration: ObjectId,
    pub object_type: ObjectTypeId,
    pub published_root: SingletonPublishedRootId,
    pub initialization: InitializationUnitId,
}

/// Typed identity of the moving-GC-aware root through which a completed
/// singleton becomes visible. Its physical global is allocated after
/// concretization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingletonPublishedRoot {
    pub value: SingletonValueId,
    pub ty: TypeId,
    pub link_name: String,
}
