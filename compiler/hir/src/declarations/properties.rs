use super::*;

/// One source-level logical property. Its callable capability and physical
/// representation are closed sums, so consumers never infer either from a
/// field name or a pair of optional accessor ids.
#[derive(Debug, Clone)]
pub struct Property {
    pub owner: PropertyOwner,
    pub name: String,
    pub access: DeclarationAccess,
    pub modifier: MethodModifier,
    pub is_override: bool,
    pub overrides: Vec<PropertyId>,
    pub override_access: Vec<PropertyOverrideAccessWitness>,
    pub ty: TypeId,
    pub capability: PropertyCapability,
    pub representation: PropertyRepresentation,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyOwner {
    TopLevel,
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyCapability {
    ReadOnly {
        getter: PropertyGetterId,
    },
    ReadWrite {
        getter: PropertyGetterId,
        setter: PropertySetterId,
    },
}

impl PropertyCapability {
    pub const fn getter(self) -> PropertyGetterId {
        match self {
            Self::ReadOnly { getter } | Self::ReadWrite { getter, .. } => getter,
        }
    }

    pub const fn setter(self) -> Option<PropertySetterId> {
        match self {
            Self::ReadOnly { .. } => None,
            Self::ReadWrite { setter, .. } => Some(setter),
        }
    }
}

/// Mutually exclusive implementation representation. Delegate and const
/// variants are introduced by their dedicated M21 gates; declarations using
/// those source forms are rejected before a Property is allocated until then.
#[derive(Debug, Clone)]
pub enum PropertyRepresentation {
    Stored(StoredProperty),
    AccessorOnly,
    NativeStorage { storage: GlobalId },
}

#[derive(Debug, Clone)]
pub struct StoredProperty {
    pub backing: PropertyBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyBacking {
    ClassField {
        field: ClassFieldId,
        initializer: ClassPropertyInitializer,
    },
    StructField {
        owner: StructId,
        index: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassPropertyInitializer {
    PrimaryParameter(ConstructorParamId),
    Expression,
    SyntheticNone,
}

#[derive(Debug, Clone)]
pub struct PropertyGetter {
    pub access: DeclarationAccess,
    pub implementation: PropertyAccessorImplementation,
    pub attributes: FunctionAttributes,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct PropertySetter {
    pub access: DeclarationAccess,
    pub implementation: PropertyAccessorImplementation,
    pub attributes: FunctionAttributes,
    pub parameter_name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyAccessorImplementation {
    Storage,
    Body(FunctionId),
    AbstractSlot(FunctionId),
}
