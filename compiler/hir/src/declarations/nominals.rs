use super::*;

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub access: NominalAccess,
    /// Application to this declaration's own type parameters (or the empty
    /// application for a parameter-free declaration).
    pub self_application: StructApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub attributes: StructAttributes,
    pub representation: StructRepresentation,
    pub constructors: Vec<StructConstructorId>,
    pub interfaces: Vec<TypeId>,
    /// Source-complete mapping from each implemented interface member to the
    /// concrete declaration that implements it. Generic owner/interface
    /// arguments remain in template form and are substituted together.
    pub interface_implementations: Vec<InterfaceImplementation>,
    /// Member declarations in source order. Consumers follow this typed
    /// relation and never recover ownership by scanning `Module::functions`.
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    /// Compiler-derived same-type equality declaration, when no explicit
    /// same-signature operator suppresses derivation. Applicability remains
    /// conditional on this application's field obligations.
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

impl StructDecl {
    pub fn semantic_fields(&self) -> &[Field] {
        self.representation.semantic_fields()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructApplication {
    pub template: StructId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: StructApplicationRepresentation,
}

#[derive(Debug, Clone)]
pub enum StructRepresentation {
    Declared(Vec<Field>),
    Intrinsic(IntrinsicTypeDeclaration),
}

impl StructRepresentation {
    /// Source-visible fields. Intrinsic types deliberately expose no source
    /// fields even though their compiler representation is not empty.
    pub fn semantic_fields(&self) -> &[Field] {
        match self {
            Self::Declared(fields) => fields,
            Self::Intrinsic(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}

/// Typed struct attributes. Raw annotation names and argument syntax never
/// cross the AST/HIR boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructAttributes {
    pub no_gc: bool,
    pub c_layout: Option<CLayout>,
    pub interior_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub access: NominalAccess,
    pub self_application: EnumApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub no_gc: bool,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumApplication {
    pub template: EnumId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// Effective dispatch modality of a member function (spec 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

/// Member-only function metadata. Keeping owner and modality together
/// makes it impossible for a method to reach downstream stages without
/// a dispatch modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
    /// Complete source-level dispatch identity. Overrides share a typed
    /// virtual family; interface declarations name their exact member.
    pub dispatch: MethodDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodDispatch {
    Direct,
    Virtual(VirtualMethodId),
    /// A final override is called directly through its own static type but
    /// still replaces the inherited virtual-family slot for base-typed calls.
    FinalOverride(VirtualMethodId),
    Interface(InterfaceMethodId),
}

/// Closed language-level identity assigned to a successfully validated
/// `operator` declaration. Consumers match this role directly and never
/// recover it from a source name, nominal owner or intrinsic symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    UnaryPlus,
    UnaryMinus,
    Not,
    Inc,
    Dec,
    Plus,
    Minus,
    Times,
    Div,
    Rem,
    RangeTo,
    RangeUntil,
    Contains,
    Get,
    Set,
    Invoke,
    PlusAssign,
    MinusAssign,
    TimesAssign,
    DivAssign,
    RemAssign,
    CompareTo,
    Equals,
    Component { index: std::num::NonZeroU32 },
    Iterator,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    pub access: NominalAccess,
    pub self_application: ClassApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub representation: ClassRepresentation,
    pub fields: Vec<ClassFieldId>,
    pub properties: Vec<PropertyId>,
    pub constructors: Vec<ClassConstructorId>,
    pub base_class: Option<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

/// Typed evaluation plan for one constructor edge. Source argument syntax and
/// missing/default state have already been eliminated.
#[derive(Debug, Clone)]
pub struct ConstructorArguments {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub args: Vec<Expr>,
}

impl ClassDecl {
    pub fn is_declared(&self) -> bool {
        matches!(self.representation, ClassRepresentation::Declared)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassApplication {
    pub template: ClassId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: ClassApplicationRepresentation,
}

#[derive(Debug, Clone)]
pub enum ClassRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeDeclaration),
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub owner: ClassId,
    pub property: PropertyId,
    pub ty: TypeId,
    pub source: ClassFieldSource,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassFieldSource {
    PrimaryParameter(ConstructorParamId),
    Body,
}

#[derive(Debug, Clone)]
pub struct ConstructorParameter {
    pub id: ConstructorParamId,
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct ClassConstructor {
    pub owner: ClassId,
    pub access: DeclarationAccess,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: ClassConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone)]
pub enum ClassConstructorKind {
    Primary {
        base: BaseInitialization,
        primary_stores: Vec<PrimaryFieldStore>,
        common_initialization: Vec<ClassInitializationStep>,
    },
    Secondary {
        delegation: ClassSecondaryDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct PrimaryFieldStore {
    pub field: ClassFieldId,
    pub parameter: ConstructorParamId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassInitializationStep {
    StoredProperty {
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    DelegatedProperty {
        storage: DelegateStorageId,
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    InitBlock {
        body: Body,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ConstructorExpression {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum BaseInitialization {
    Root,
    Super {
        target: ClassConstructorApplicationId,
        arguments: ConstructorArguments,
    },
}

#[derive(Debug, Clone)]
pub enum ClassSecondaryDelegation {
    This {
        target: ClassConstructorApplicationId,
        arguments: ConstructorArguments,
    },
    Terminal {
        base: BaseInitialization,
        common_initialization: Vec<ClassInitializationStep>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructorApplication {
    pub constructor: ClassConstructorId,
    pub owner: ClassApplicationId,
}

#[derive(Debug, Clone)]
pub struct StructConstructor {
    pub owner: StructId,
    pub access: DeclarationAccess,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: StructConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone)]
pub enum StructConstructorKind {
    Primary,
    Secondary {
        delegation: StructConstructorDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct StructConstructorDelegation {
    pub target: StructConstructorApplicationId,
    pub arguments: ConstructorArguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructConstructorApplication {
    pub constructor: StructConstructorId,
    pub owner: StructApplicationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntrinsicTypeDeclaration {
    pub kind: IntrinsicTypeKind,
    pub provider: IntrinsicProviderId,
}

/// Closed semantic identity of every compiler-represented nominal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntrinsicTypeKind {
    Int,
    UInt,
    Boolean,
    String,
    Array,
    MutableArray,
}

impl IntrinsicTypeKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Int => "core_int",
            Self::UInt => "core_uint",
            Self::Boolean => "core_boolean",
            Self::String => "core_string",
            Self::Array => "core_array",
            Self::MutableArray => "core_mutable_array",
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Int => "Int",
            Self::UInt => "UInt",
            Self::Boolean => "Boolean",
            Self::String => "String",
            Self::Array => "Array",
            Self::MutableArray => "MutableArray",
        }
    }

    pub const fn target(self) -> IntrinsicTypeTarget {
        match self {
            Self::Int | Self::UInt | Self::Boolean => IntrinsicTypeTarget::Struct,
            Self::String | Self::Array | Self::MutableArray => IntrinsicTypeTarget::Class,
        }
    }

    pub const fn parameters(self) -> IntrinsicTypeParameters {
        match self {
            Self::Int | Self::UInt | Self::Boolean | Self::String => IntrinsicTypeParameters::None,
            Self::Array | Self::MutableArray => IntrinsicTypeParameters::OneInvariantUnconstrained,
        }
    }

    pub fn application(self, arguments: &[TypeId]) -> IntrinsicTypeRepresentation {
        match (self, arguments) {
            (Self::Int, []) => IntrinsicTypeRepresentation::Int,
            (Self::UInt, []) => IntrinsicTypeRepresentation::UInt,
            (Self::Boolean, []) => IntrinsicTypeRepresentation::Boolean,
            (Self::String, []) => IntrinsicTypeRepresentation::String,
            (Self::Array, [element]) => IntrinsicTypeRepresentation::Array { element: *element },
            (Self::MutableArray, [element]) => {
                IntrinsicTypeRepresentation::MutableArray { element: *element }
            }
            _ => unreachable!("HIR validates the intrinsic declaration contract before use"),
        }
    }
}

/// Complete compiler representation of one nominal application. Generic
/// family variants contain their concrete element type directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Int,
    UInt,
    Boolean,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Fields in declaration order; unit variants have none. Named and
    /// constructor-style fields carry their names, positional fields have
    /// generated `_1`-style names.
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}
