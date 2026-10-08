use super::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Persistent template plus the exact substitution context in which this
    /// body exists. Declaration, application, and generated-template ids are
    /// distinct kinds and cannot be reconstructed from names or arena ids.
    pub materialization: CallableMaterialization,
    pub is_suspend: bool,
    pub modifiers: CallableModifiers,
    pub params: Vec<Param>,
    /// Hidden parameters of a named local body, independent of its use sites.
    pub capture_parameters: Vec<LocalCaptureParameter>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    /// Source-level receiver shape. The physical receiver remains the first
    /// entry in `params`, while this enum preserves whether that parameter is
    /// an extension receiver or a nominal member receiver.
    pub receiver: FunctionReceiver,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalCaptureParameter {
    pub binding: BindingId,
    pub local: LocalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionReceiver {
    None,
    Extension(TypeId),
    Method(Method),
}

impl FunctionReceiver {
    pub const fn value_type(self) -> Option<TypeId> {
        match self {
            Self::None => None,
            Self::Extension(receiver) => Some(receiver),
            Self::Method(method) => Some(method.owner),
        }
    }

    pub const fn method(self) -> Option<Method> {
        match self {
            Self::Method(method) => Some(method),
            Self::None | Self::Extension(_) => None,
        }
    }

    pub fn method_mut(&mut self) -> Option<&mut Method> {
        match self {
            Self::Method(method) => Some(method),
            Self::None | Self::Extension(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodOwner {
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
    Object(ObjectTypeId),
    /// A method whose owner has no local nominal arena entry, including
    /// structural derived methods and imported pointer members.
    /// Its complete concrete type retains the actual receiver identity.
    TypeOwned(TypeId),
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
    /// Target-independent source contract retained as the typed owner of the
    /// target-specific native contract produced by LIR lowering.
    pub source_contract: SourceNativeExternalContractRecord,
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub safety: Safety,
    pub params: Vec<TypeId>,
    pub result: scoop_identity::ExternResult<TypeId>,
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
    Abstract { locals: Arena<Local> },
    InitializationEnsure,
    Intrinsic(IntrinsicFunction),
    Extern(ExternFunctionId),
}

impl FunctionKind {
    pub fn locals(&self) -> Option<&Arena<Local>> {
        match self {
            Self::User(body) => Some(&body.locals),
            Self::Abstract { locals } => Some(locals),
            Self::InitializationEnsure | Self::Intrinsic(_) | Self::Extern(_) => None,
        }
    }
}

pub type HirIntegerOperation = IntegerOperation;
pub type HirIntegerConversion = IntegerConversion;

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
    pub definition: LocalValueDefinitionSite,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}
