use super::*;

pub trait FunctionCallee: Copy {
    fn function(self, module: &Module) -> FunctionId;
}

impl FunctionCallee for Callable {
    fn function(self, module: &Module) -> FunctionId {
        callable_function(module, self)
    }
}

impl FunctionCallee for MethodCallee {
    fn function(self, module: &Module) -> FunctionId {
        method_callee_function(module, self)
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
/// Nominal declarations reuse their owner application; structural Unit/tuple
/// methods carry their owner type directly because they have no declaration
/// arena whose identity could stand in for that type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivedEqualityOrigin {
    Nominal(MethodOwnerApplication),
    Structural(TypeId),
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

/// Export-HIR use of one callable selected from the trusted core artifact.
/// The wrapper gives this arena its own id domain; LocalConcrete HIR defines
/// a distinct wrapper and therefore cannot reuse its indices accidentally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedCoreCallableUse {
    reference: ImportedCoreCallableRef,
}

impl ImportedCoreCallableUse {
    pub fn new(reference: ImportedCoreCallableRef) -> Self {
        Self { reference }
    }

    pub const fn reference(self) -> ImportedCoreCallableRef {
        self.reference
    }
}

/// Export-HIR use of one type selected from the trusted core artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedCoreTypeUse {
    reference: ImportedCoreTypeRef,
}

impl ImportedCoreTypeUse {
    pub fn new(reference: ImportedCoreTypeRef) -> Self {
        Self { reference }
    }

    pub const fn reference(self) -> ImportedCoreTypeRef {
        self.reference
    }
}

/// Export-HIR use of one value selected from the trusted core artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedCoreValueUse {
    reference: ImportedCoreValueRef,
}

impl ImportedCoreValueUse {
    pub fn new(reference: ImportedCoreValueRef) -> Self {
        Self { reference }
    }

    pub const fn reference(self) -> ImportedCoreValueRef {
        self.reference
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

pub(crate) fn method_callee_function(module: &Module, callee: MethodCallee) -> FunctionId {
    match callee {
        MethodCallee::Callable(callable) => callable_function(module, callable),
        MethodCallee::Bound(bound) => match module.bound_callable_refs[bound].source {
            BoundCallableSource::Class { callable, .. } => callable_function(module, callable),
            BoundCallableSource::Interface { member, .. } => {
                module.interface_methods[member].function
            }
        },
        MethodCallee::DerivedEquality(application) => {
            module.derived_equality_applications[application].function
        }
    }
}
