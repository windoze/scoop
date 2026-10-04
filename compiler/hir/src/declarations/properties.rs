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
    pub overrides: Vec<PropertyReference>,
    pub ty: TypeId,
    pub capability: PropertyCapability,
    pub representation: PropertyRepresentation,
    pub span: Span,
}

/// The actual property declaration overridden by a local property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyReference {
    Local(PropertyId),
    Imported {
        owner: TypeId,
        declaration: scoop_identity::PersistentPropertyId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyOwner {
    TopLevel,
    Extension(ExtensionPropertyId),
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
    Object(ObjectId),
}

/// The receiver template and type-parameter namespace of one top-level
/// extension property. Accessors are separate generic function templates;
/// their positional type arguments are fixed by this logical declaration.
#[derive(Debug, Clone)]
pub struct ExtensionProperty {
    pub property: PropertyId,
    pub receiver_ty: TypeId,
    pub type_params: Vec<TypeParamDecl>,
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

/// Mutually exclusive implementation representation. Every physical storage
/// category has its own typed identity; consumers never recover a delegate
/// field or global from an accessor name.
#[derive(Debug, Clone)]
pub enum PropertyRepresentation {
    Stored(StoredProperty),
    AccessorOnly,
    Delegated { storage: DelegateStorageId },
    GenericDelegated { template: GenericDelegateTemplateId },
    Const { value: ConstPropertyValue },
    NativeStorage { storage: GlobalId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstPropertyValue {
    Integer(HirIntegerConstant),
    Boolean(bool),
    Char(char),
    String(String),
}

#[derive(Debug, Clone)]
pub struct DelegateStorage {
    pub property: PropertyId,
    pub ty: TypeId,
    pub location: DelegateStorageLocation,
}

/// A receiver-parameterized delegate has no source-level physical global.
#[derive(Debug, Clone)]
pub struct GenericDelegateTemplate {
    pub property: PropertyId,
    pub ty: TypeId,
    pub initialization: InitializationUnitId,
}

/// Complete symbolic arguments at one use of a delegate template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericDelegateReference {
    pub template: GenericDelegateTemplateSource,
    pub arguments: NonEmptyVec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenericDelegateTemplateSource {
    Defined(GenericDelegateTemplateId),
    Imported(ImportedGenericDelegateTemplateId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegateStorageLocation {
    ClassField(ClassFieldId),
    ManagedGlobal(GlobalId),
}

#[derive(Debug, Clone)]
pub struct StoredProperty {
    pub backing: PropertyBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyBacking {
    TopLevelGlobal {
        storage: GlobalId,
        initialization: TopLevelInitialization,
    },
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
pub enum TopLevelInitialization {
    Image,
    Runtime(InitializationUnitId),
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
    Constant,
    Body(FunctionId),
    AbstractSlot(FunctionId),
}
