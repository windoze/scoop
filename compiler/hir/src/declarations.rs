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
pub struct InterfaceDecl {
    pub name: String,
    pub self_application: InterfaceApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    /// Exact parent applications in declaration order.
    pub parents: Vec<InterfaceApplicationId>,
    /// Methods declared directly by this interface, in itable order after
    /// inherited methods. Inheritance traversal follows `parents` and these
    /// typed ids; consumers never reconstruct ownership from function names.
    pub methods: Vec<InterfaceMethodId>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceApplication {
    pub template: InterfaceId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
}

/// One interface method declaration. Its callable signature and effects live
/// on the directly referenced function; this relation is the authoritative
/// ownership edge and is never recovered by scanning `Module::functions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceMethod {
    pub owner: InterfaceId,
    pub function: FunctionId,
}

/// One complete nominal conformance generated by HIR inheritance checking.
/// `methods` contains an entry for every method in the interface inheritance
/// closure, not only methods declared directly on `interface`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceImplementation {
    pub interface: InterfaceApplicationId,
    pub methods: Vec<InterfaceMethodImplementation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethodImplementation {
    pub member: InterfaceMethodId,
    pub target: InterfaceImplementationTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceImplementationTarget {
    /// Exact ordinary method application selected by HIR conformance
    /// checking. Generic methods cannot implement interface slots.
    Method(MethodApplicationId),
    /// An abstract class may promise an interface while leaving a member for
    /// a concrete subclass. Calls through such a specialization use the
    /// interface application directly instead of guessing a class member.
    Subclass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParamDecl {
    pub id: TypeParamId,
    pub name: String,
    pub variance: Variance,
    /// Complete declaration-site constraint set. The sum type makes kind
    /// bounds and interface upper bounds mutually exclusive by construction.
    pub bounds: TypeParamBounds,
    pub span: Span,
}

impl TypeParamDecl {
    /// Representation kind implied by this parameter's constraints. Interface
    /// upper bounds are reference capabilities, but do not make the generic
    /// value itself a `ref`-kind parameter: value types may implement them and
    /// are boxed only at an actual interface crossing.
    pub fn kind(&self) -> TypeParamKind {
        match self.bounds {
            TypeParamBounds::Value { .. } => TypeParamKind::Value,
            TypeParamBounds::Ref { .. } => TypeParamKind::Ref,
            TypeParamBounds::Unconstrained | TypeParamBounds::Interfaces(_) => TypeParamKind::Any,
        }
    }

    pub fn interface_bounds(&self) -> &[InterfaceBound] {
        match &self.bounds {
            TypeParamBounds::Interfaces(bounds) => bounds,
            TypeParamBounds::Unconstrained
            | TypeParamBounds::Value { .. }
            | TypeParamBounds::Ref { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeParamBounds {
    Unconstrained,
    Value {
        span: Span,
    },
    Ref {
        span: Span,
    },
    /// Ordered, distinct, fully resolved interface applications.
    Interfaces(Vec<InterfaceBound>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceBound {
    /// Complete interface application.  The bound cannot name a declaration
    /// without its arguments or another nominal kind.
    pub application: InterfaceApplicationId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamKind {
    Any,
    Value,
    Ref,
}

/// Convenience signature record used by tests and by the fully concrete
/// interface representation. ExportHir interface ownership does not store
/// this record: it stores `InterfaceMethodId -> FunctionId` directly so the
/// declaration identity and callable cannot diverge.
#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    pub is_suspend: bool,
    pub attributes: FunctionAttributes,
    pub type_params: Vec<TypeParamDecl>,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
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

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

/// A typed, GC-free initializer accepted for local global storage.
#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPtr,
    NullFunPtr,
    Struct {
        application: StructApplicationId,
        fields: Vec<ConstantValue>,
    },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Complete declaration identity. Generic functions carry their distinct
    /// template id directly; consumers never recover it by scanning the
    /// `generic_functions` arena or by inspecting `type_params`.
    pub genericity: FunctionGenericity,
    /// Whether calls use the coroutine ABI rather than the ordinary ABI.
    pub is_suspend: bool,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    /// Member metadata; the receiver of a method is the first entry of
    /// `params` (named `this`). Top-level functions have `None`.
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionGenericity {
    Plain,
    Generic {
        definition: GenericFunctionId,
        parameters: Vec<TypeParamDecl>,
    },
    /// An ordinary method whose signature/body depends only on its nominal
    /// owner's parameters. Its concrete identity comes from a
    /// `MethodApplication`, not a generic-function application.
    OwnerParameterizedMethod {
        owner_parameters: Vec<TypeParamDecl>,
        no_gc_type_params: Vec<TypeParamId>,
    },
    /// A non-virtual method with its own parameters. The two groups are
    /// structurally separate; no downstream consumer receives a merged
    /// argument vector and an index at which it is expected to split it.
    GenericMethod {
        definition: GenericMethodId,
        owner_parameters: Vec<TypeParamDecl>,
        method_parameters: NonEmptyVec<TypeParamDecl>,
    },
}

impl Function {
    pub fn type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::Plain => 0,
            FunctionGenericity::Generic { parameters, .. } => parameters.len(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters.len() + method_parameters.len(),
        }
    }

    pub fn owner_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            }
            | FunctionGenericity::GenericMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::Plain | FunctionGenericity::Generic { .. } => 0,
        }
    }

    pub fn method_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::GenericMethod {
                method_parameters, ..
            } => method_parameters.len(),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => 0,
        }
    }

    pub fn type_param(&self, id: TypeParamId) -> &TypeParamDecl {
        match &self.genericity {
            FunctionGenericity::Plain => panic!("plain function has no type parameters"),
            FunctionGenericity::Generic { parameters, .. } => parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("generic function owns the referenced type parameter"),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("method owner owns the referenced type parameter"),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .find(|parameter| parameter.id == id)
                .expect("generic method owns the referenced type parameter"),
        }
    }

    pub fn type_params(&self) -> Vec<&TypeParamDecl> {
        match &self.genericity {
            FunctionGenericity::Plain => Vec::new(),
            FunctionGenericity::Generic { parameters, .. } => parameters.iter().collect(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.iter().collect(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .collect(),
        }
    }

    pub fn generic_definition(&self) -> Option<GenericFunctionId> {
        match &self.genericity {
            FunctionGenericity::Plain
            | FunctionGenericity::OwnerParameterizedMethod { .. }
            | FunctionGenericity::GenericMethod { .. } => None,
            FunctionGenericity::Generic { definition, .. } => Some(*definition),
        }
    }

    pub fn generic_method_definition(&self) -> Option<GenericMethodId> {
        match self.genericity {
            FunctionGenericity::GenericMethod { definition, .. } => Some(definition),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAttributes {
    pub safety: Safety,
    pub gc_effect: GcEffect,
    /// M12 currently supports only cdecl for native-addressable functions.
    pub calling_convention: CallingConvention,
}

impl Default for FunctionAttributes {
    fn default() -> Self {
        Self {
            safety: Safety::Safe,
            gc_effect: GcEffect::Managed,
            calling_convention: CallingConvention::Cdecl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
}

#[derive(Debug, Clone)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub safety: Safety,
    pub params: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeId,
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    /// Stable source identity for a conditional derived equality method. Its
    /// application-specific ordinary body lives on
    /// `DerivedEqualityApplication` and must be present before concretization
    /// requests this function.
    DerivedEquality,
    /// A validated compiler intrinsic. Raw annotation text does not cross the
    /// AST/HIR boundary: kind and defining provider are both typed and
    /// mandatory.
    Intrinsic(IntrinsicFunction),
    /// A bodyless native declaration. Complete ABI metadata lives in the
    /// independent extern arena and is referenced by a typed id.
    Extern(ExternFunctionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicFunction {
    pub kind: IntrinsicFunctionKind,
    pub provider: IntrinsicProviderId,
}
