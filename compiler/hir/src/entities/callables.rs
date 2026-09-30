use super::*;

pub trait FunctionCallee: Copy {
    fn function(self, module: &Module) -> FunctionId;
}

impl FunctionCallee for Callable {
    fn function(self, module: &Module) -> FunctionId {
        callable_function(module, self)
    }
}

/// A generic type parameter that must denote a recursively GC-free value
/// whenever it is used as the pointee of the compiler-represented `Ptr`
/// family. This condition is intentionally distinct from a callable's
/// `@NoGC` effect requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequiresGcFreePointee {
    pub type_param: TypeParamId,
}

/// A generic HIR function definition. Generic identity is deliberately
/// separate from the underlying function identity (AGENTS.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericFunction {
    pub function: FunctionId,
    /// Type parameters whose concrete arguments must be GC-free for this
    /// generic definition to satisfy its `@NoGC` contract. The requirement
    /// is inferred from the resolved signature/body and checked at every
    /// instantiation; unused/representation-erased parameters are omitted.
    pub no_gc_type_params: Vec<TypeParamId>,
    /// Pointee constraints inferred from this template's signature/body and
    /// transitively required generic calls.
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
}

/// A generic function with every call-site type argument resolved.
/// MIR consumes this entity to produce a separate monomorphized
/// function entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGenericFunction {
    pub generic: GenericFunctionId,
    pub type_args: Vec<TypeId>,
}

/// Exact source-side owner of a method call. Parameter-free owners still use
/// their canonical empty application, so every method application has one
/// uniform and complete representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodOwnerApplication {
    Class(ClassApplicationId),
    Struct(StructApplicationId),
    Enum(EnumApplicationId),
    Interface(InterfaceApplicationId),
    Object(ObjectTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodApplication {
    pub function: FunctionId,
    pub owner: MethodOwnerApplication,
}

/// Exact origin of one compiler-derived value-type equality method.
/// Local nominal declarations reuse their owner application. Structural and
/// imported types carry their actual owner type without a local declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivedEqualityOrigin {
    Nominal(MethodOwnerApplication),
    TypeOwned(TypeId),
}

/// One application of a compiler-derived value-type equality method. The
/// source method declaration supplies stable callable identity; every other
/// property needed to materialize the concrete method is mandatory here.
/// `body` is complete application-specific HIR, including exact nested
/// callees, so concretization only substitutes types and callable identities.
#[derive(Debug, Clone)]
pub struct DerivedEqualityApplication {
    pub function: FunctionId,
    pub origin: DerivedEqualityOrigin,
    pub owner_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub span: Span,
    pub body: Body,
}

/// A non-virtual generic method template. Its declaration identity is
/// separate from every other generic callable family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericMethod {
    pub function: FunctionId,
    pub no_gc_type_params: Vec<TypeParamId>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
}

/// Exact owner kinds accepted by non-interface generic methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenericMethodOwner {
    Class(ClassApplicationId),
    Struct(StructApplicationId),
    Enum(EnumApplicationId),
    Object(ObjectTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericMethodApplication {
    pub method: GenericMethodId,
    pub owner: GenericMethodOwner,
    pub method_arguments: NonEmptyVec<TypeId>,
}

/// The fully-resolved callable stored on HIR calls. A generic call
/// cannot be represented as a plain function plus an unrelated type
/// argument vector: it must reference a resolved generic entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
    Generic(ResolvedGenericFunctionId),
    Method(MethodApplicationId),
    GenericMethod(GenericMethodApplicationId),
}

/// A selected callable application or an ordinary external definition.
/// Storage handles do not replace the target's original declaration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableTarget {
    Local(Callable),
    Application(ImportedGenericCallableApplicationId),
    Dependency(ImportedDependencyCallableUseId),
}

impl From<Callable> for CallableTarget {
    fn from(callable: Callable) -> Self {
        Self::Local(callable)
    }
}

pub(crate) fn callable_function(module: &Module, callable: Callable) -> FunctionId {
    match callable {
        Callable::Function(function) => function,
        Callable::Generic(id) => {
            module.generic_functions[module.instantiations[id].generic].function
        }
        Callable::Method(id) => module.method_applications[id].function,
        Callable::GenericMethod(id) => {
            module.generic_methods[module.generic_method_applications[id].method].function
        }
    }
}
