use super::*;

pub type TypeId = Idx<Type>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type LambdaId = Idx<Lambda>;
pub type AnonymousFunctionId = Idx<AnonymousFunction>;
pub type LocalFunctionId = Idx<LocalFunction>;
pub type CallableReferenceId = Idx<CallableReference>;
pub type FunctionCoercionId = Idx<FunctionCoercion>;
pub type ForeignCallbackRegistrationId = Idx<ForeignCallbackRegistration>;
pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type InitializationUnitId = Idx<InitializationUnit>;
pub type InitializationFailureRootId = Idx<InitializationFailureRoot>;
pub type ObjectId = Idx<ObjectDecl>;
pub type ObjectTypeId = Idx<ObjectType>;
pub type CompanionRelationId = Idx<CompanionRelation>;
pub type SingletonValueId = Idx<SingletonValue>;
pub type SingletonPublishedRootId = Idx<SingletonPublishedRoot>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type ClassConstructorId = Idx<ClassConstructor>;
pub type StructConstructorId = Idx<StructConstructor>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstructorParamId(u32);

impl ConstructorParamId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceFamilyId(u32);

impl InterfaceFamilyId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(u32);

impl BindingId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

/// Local-concrete identity of one materialized structured loop occurrence.
/// This is deliberately distinct from the Export HIR identity family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopId(u32);

impl LoopId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariantId(u32);

impl VariantId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

/// Local-concrete identity of one class virtual-dispatch family. It is mapped
/// explicitly from the export-side family during HIR concretization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualMethodId(u32);

impl VirtualMethodId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceMethodSlot(u32);

impl InterfaceMethodSlot {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

/// A fully resolved type entity.  `gc_free` is mandatory by construction;
/// there is no unknown or deferred state in local-concrete HIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub kind: TypeKind,
    pub gc_free: bool,
}

/// A concrete type shape.  There is deliberately no `Param` variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Unit,
    Integer(IntegerKind),
    Boolean,
    String,
    Struct(StructId),
    Class(ClassId),
    Interface(InterfaceId),
    Any,
    Tuple(Vec<TypeId>),
    Function(FunctionTypeId),
    Ptr(TypeId),
    FunPtr(FunctionTypeId),
    Enum(EnumId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionType {
    pub canonical_type: TypeId,
    pub is_suspend: bool,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}
