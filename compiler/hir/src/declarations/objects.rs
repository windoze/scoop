use super::*;

/// One source `object` declaration. Its declaration, type, value and
/// published managed root deliberately belong to four distinct identity
/// domains; the backing class is only the physical object representation.
#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub object_type: ObjectTypeId,
    pub singleton_value: SingletonValueId,
    pub backing_class: ClassId,
    pub span: Span,
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
