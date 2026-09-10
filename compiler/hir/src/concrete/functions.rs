use super::*;

#[derive(Debug, Clone)]
pub struct Function {
    /// Completed request-local native-emission identity propagated from
    /// export HIR without reconstructing it from a display name.
    pub link_stem: CallableLinkStem,
    pub name: String,
    /// Persistent template plus the exact substitution context in which this
    /// body exists. Declaration, application, and generated-template ids are
    /// distinct kinds and cannot be reconstructed from names or arena ids.
    pub materialization: CallableMaterialization,
    /// Temporary native-symbol inputs retained until the persistent mangler
    /// replaces the current local emitter. This carries no semantic origin.
    pub emission: FunctionEmission,
    pub is_suspend: bool,
    pub modifiers: CallableModifiers,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    pub method: Option<Method>,
    pub span: Span,
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
pub enum FunctionEmission {
    Direct,
    Materialized {
        arguments: NonEmptyVec<TypeId>,
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

/// Local-concrete proof that a non-generic function has `@NoGC` effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoGcCallableRef(FunctionId);

impl NoGcCallableRef {
    pub fn map_from_export(
        source: crate::NoGcCallableRef,
        map: impl FnOnce(crate::FunctionId) -> FunctionId,
    ) -> Self {
        Self(map(source.function()))
    }

    pub const fn function(self) -> FunctionId {
        self.0
    }
}

/// Local-concrete proof that a non-generic function has managed effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedCallableRef(FunctionId);

impl ManagedCallableRef {
    pub fn map_from_export(
        source: crate::ManagedCallableRef,
        map: impl FnOnce(crate::FunctionId) -> FunctionId,
    ) -> Self {
        Self(map(source.function()))
    }

    pub const fn function(self) -> FunctionId {
        self.0
    }
}

pub type HirIntegerOperation = IntegerOperation<NoGcCallableRef, ManagedCallableRef>;
pub type HirIntegerConversion = IntegerConversion<NoGcCallableRef>;

#[derive(Debug, Clone)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Local {
    pub binding: BindingId,
    /// Template-local semantic selector retained so the module's total
    /// local-value relation can validate and reproduce the persistent key.
    pub selector: scoop_identity::LocalValueSelector,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}
