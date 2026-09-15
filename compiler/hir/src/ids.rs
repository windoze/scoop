use super::*;

pub type TypeId = Idx<Type>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type LambdaId = Idx<Lambda>;
pub type AnonymousFunctionId = Idx<AnonymousFunction>;
pub type LocalFunctionId = Idx<LocalFunction>;
pub type CallableReferenceId = Idx<CallableReference>;
pub type ImportedCoreCallableUseId = Idx<ImportedCoreCallableUse>;
pub type ImportedCoreTypeUseId = Idx<ImportedCoreTypeUse>;
pub type ImportedCoreValueUseId = Idx<ImportedCoreValueUse>;
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
pub type PropertyId = Idx<Property>;
pub type ExtensionPropertyId = Idx<ExtensionProperty>;
pub type PropertyGetterId = Idx<PropertyGetter>;
pub type PropertySetterId = Idx<PropertySetter>;
pub type DelegateStorageId = Idx<DelegateStorage>;
pub type GenericFunctionId = Idx<GenericFunction>;
pub type ResolvedGenericFunctionId = Idx<ResolvedGenericFunction>;
pub type MethodApplicationId = Idx<MethodApplication>;
pub type GenericMethodId = Idx<GenericMethod>;
pub type GenericMethodApplicationId = Idx<GenericMethodApplication>;
pub type DerivedEqualityApplicationId = Idx<DerivedEqualityApplication>;
pub type StructId = Idx<StructDecl>;
pub type EnumId = Idx<EnumDecl>;
pub type ClassId = Idx<ClassDecl>;
pub type ClassFieldId = Idx<ClassField>;
pub type ClassConstructorId = Idx<ClassConstructor>;
pub type StructConstructorId = Idx<StructConstructor>;
pub type InterfaceId = Idx<InterfaceDecl>;
pub type StructApplicationId = Idx<StructApplication>;
pub type EnumApplicationId = Idx<EnumApplication>;
pub type ClassApplicationId = Idx<ClassApplication>;
pub type ClassConstructorApplicationId = Idx<ClassConstructorApplication>;
pub type StructConstructorApplicationId = Idx<StructConstructorApplication>;
pub type InterfaceApplicationId = Idx<InterfaceApplication>;
pub type InterfaceMethodId = Idx<InterfaceMethod>;
pub type BoundCallableRefId = Idx<BoundCallableRef>;
pub type LocalId = Idx<Local>;
pub type ExportDefaultExprId = Idx<ExportDefaultExpr>;
pub type ExportDefaultSourceId = Idx<ExportDefaultSource>;
pub type ExportVarargParameterTypeId = Idx<ExportVarargParameterType>;
pub type SourceContextId = Idx<SourceContext>;
pub type ExportTypeAliasId = Idx<TypeAliasDecl>;

/// Export-side identity of one class virtual-dispatch family. Every override
/// in the family carries the same id; overloads always receive distinct ids.
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

/// A structurally non-empty sequence. Generic method applications use this
/// instead of a plain `Vec` because an empty method-argument group would mean
/// a different entity kind (an ordinary method application).
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

/// Identity of one primary-constructor parameter. Constructor delegation
/// expressions use this domain directly; these parameters do not belong to a
/// function body's `LocalId` arena.
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

/// Cone-wide identity of a lexical value binding. Unlike `LocalId`, which is
/// only meaningful inside one function body's local arena, this identity is
/// stable across nested callable bodies and can therefore name a capture
/// without falling back to a source name.
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

/// Cone-wide identity of one structured loop occurrence. Loop identities are
/// allocated monotonically, while lexical target stacks are reset at every
/// callable boundary so a jump cannot bind across callables.
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

/// Cone-wide semantic identity of a type parameter together with the exact
/// substitution slot assigned by its declaring HIR scope. Identity, rather
/// than a name or a position in a merged vector, decides equality. The slot
/// is source-produced replacement data and is never used as identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeParamId {
    identity: u32,
    substitution_slot: u32,
}

/// Driver-assigned identity of the source provider that defines a compiler
/// intrinsic. Source text cannot construct this identity; it is carried on the
/// validated intrinsic entity so later stages never reconstruct provenance
/// from a path, package name, or declaration position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntrinsicProviderId(u32);

impl IntrinsicProviderId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

impl TypeParamId {
    /// Construct a self-contained identity for handcrafted IR. Production
    /// HIR uses `with_substitution_slot` with a Cone-wide unique identity.
    pub const fn from_raw(raw: u32) -> Self {
        Self {
            identity: raw,
            substitution_slot: raw,
        }
    }

    pub const fn with_substitution_slot(identity: u32, substitution_slot: u32) -> Self {
        Self {
            identity,
            substitution_slot,
        }
    }

    pub const fn identity_raw(self) -> u32 {
        self.identity
    }

    /// The complete substitution environment slot emitted by HIR. Kept as
    /// `into_raw` for the existing IR API; it is deliberately not identity.
    pub const fn into_raw(self) -> u32 {
        self.substitution_slot
    }
}
