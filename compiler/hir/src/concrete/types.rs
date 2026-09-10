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

/// Local-concrete provenance identities. They deliberately are not aliases
/// for export-side arena ids: the HIR concretizer is the only component that
/// maps a checked source declaration to one of these ids.
macro_rules! local_origin_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn into_raw(self) -> u32 {
                self.0
            }
        }
    };
}

local_origin_id!(GenericFunctionOriginId);
local_origin_id!(OwnerParameterizedMethodOriginId);
local_origin_id!(GenericMethodOriginId);

/// A structurally non-empty local-concrete sequence. It is intentionally a
/// distinct container from ExportHir's generic-application sequence so an
/// export application cannot be passed to MIR through a shared wrapper type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NonEmptyVec<T> {
    first: T,
    rest: Vec<T>,
}

impl<T> NonEmptyVec<T> {
    pub fn new(first: T, rest: Vec<T>) -> Self {
        Self { first, rest }
    }

    pub fn from_vec(mut values: Vec<T>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let rest = values.split_off(1);
        Some(Self {
            first: values.pop().expect("the non-empty prefix was checked"),
            rest,
        })
    }

    pub fn len(&self) -> usize {
        1 + self.rest.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }
}

impl<T: Copy> NonEmptyVec<T> {
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().copied().collect()
    }
}

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
