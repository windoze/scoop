use super::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Complete source/application category. MIR consumes this sum type
    /// directly and never infers genericity or method ownership from an
    /// argument vector, function name, or the optional `method` field.
    pub origin: FunctionOrigin,
    pub is_suspend: bool,
    pub modifiers: CallableModifiers,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionOrigin {
    Free(FreeFunctionOrigin),
    Method(MethodOrigin),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreeFunctionOrigin {
    Plain,
    Generic {
        origin: GenericFunctionOriginId,
        arguments: NonEmptyVec<TypeId>,
        symbol: InstanceSymbol,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodOrigin {
    pub owner: MethodOwner,
    pub specialization: MethodSpecialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodOwner {
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
    Object(ObjectTypeId),
    /// A compiler-derived method on a structural value type such as Unit or
    /// tuple. The exact concrete owner type is part of the identity.
    Structural(TypeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodSpecialization {
    Plain,
    OwnerParameterized {
        origin: OwnerParameterizedMethodOriginId,
        symbol: InstanceSymbol,
    },
    Generic {
        origin: GenericMethodOriginId,
        method_arguments: NonEmptyVec<TypeId>,
        symbol: InstanceSymbol,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceSymbol {
    Unique,
    Overloaded { discriminator: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
    pub dispatch: MethodDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodDispatch {
    Direct,
    Virtual(VirtualMethodId),
    /// Direct call at the declaring static type plus inherited vtable
    /// membership for base-typed calls.
    FinalOverride(VirtualMethodId),
    Interface {
        interface: InterfaceId,
        slot: InterfaceMethodSlot,
    },
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
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    Intrinsic(IntrinsicFunction),
    Extern(ExternFunctionId),
}

#[derive(Debug, Clone)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Local {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}
