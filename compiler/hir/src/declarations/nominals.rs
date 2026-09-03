use super::*;

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    /// Application to this declaration's own type parameters (or the empty
    /// application for a parameter-free declaration).
    pub self_application: StructApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub attributes: StructAttributes,
    pub representation: StructRepresentation,
    pub interfaces: Vec<TypeId>,
    /// Source-complete mapping from each implemented interface member to the
    /// concrete declaration that implements it. Generic owner/interface
    /// arguments remain in template form and are substituted together.
    pub interface_implementations: Vec<InterfaceImplementation>,
    /// Member declarations in source order. Consumers follow this typed
    /// relation and never recover ownership by scanning `Module::functions`.
    pub methods: Vec<FunctionId>,
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
    pub self_application: EnumApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub no_gc: bool,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
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
    /// Language-level operator identity validated at the declaration site.
    /// `None` is an ordinary method; downstream consumers never recover an
    /// operator role from the method name or signature.
    pub operator: Option<OperatorKind>,
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

/// Closed set of operator member contracts implemented by the current
/// language milestone. Unsupported source modifiers are rejected before HIR
/// output, so an unknown operator cannot enter the pipeline as a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    Equals,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    pub self_application: ClassApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub representation: ClassRepresentation,
    /// Base class and the resolved constructor argument expressions.
    pub base_class: Option<(TypeId, Vec<Expr>)>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

impl ClassDecl {
    pub fn semantic_constructor(&self) -> &[ConstructorField] {
        self.representation.semantic_constructor()
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
    Declared(Vec<ConstructorField>),
    Intrinsic(IntrinsicTypeDeclaration),
}

impl ClassRepresentation {
    /// Source-visible primary-constructor properties. Intrinsic classes have
    /// hidden construction entries and no source constructor by declaration.
    pub fn semantic_constructor(&self) -> &[ConstructorField] {
        match self {
            Self::Declared(constructor) => constructor,
            Self::Intrinsic(_) => &[],
        }
    }
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

/// A primary-constructor property is simultaneously a source parameter and
/// an object field. Keeping both identities and mutability together prevents
/// delegation lowering and field assignment from reconstructing either fact.
#[derive(Debug, Clone)]
pub struct ConstructorField {
    pub parameter: ConstructorParamId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Fields in declaration order; unit variants have none. Named and
    /// constructor-style fields carry their names (and defaults),
    /// positional fields have generated `_1`-style names.
    pub fields: Vec<Field>,
    /// Constructor-style default values (constant expressions in M4).
    pub defaults: Vec<Option<Expr>>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}
