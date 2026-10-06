use super::*;

/// One compiler-managed exactly-once initialization action. Its persistent
/// identity is derived after concretization; `display_name` is diagnostic
/// decoration only. Dependencies contain only direct source initializer reads
/// validated by HIR lowering.
#[derive(Debug, Clone)]
pub struct InitializationUnit {
    pub display_name: String,
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationDependency>,
    pub span: Span,
}

/// Determines whether the runtime invokes an initialization unit during
/// image startup or leaves it pristine until a generated access gate runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationSchedule {
    EagerStartup,
    LazyAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    GenericDelegatedExtension {
        property: PropertyId,
        template: GenericDelegateTemplateId,
    },
    EagerTopLevel {
        property: PropertyId,
        storage: GlobalId,
    },
    LazySingleton {
        value: SingletonValueId,
        published_root: SingletonPublishedRootId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializationDependency {
    pub unit: InitializationUnitId,
    pub type_arguments: Vec<TypeId>,
    pub span: Span,
}

/// Typed identity of the managed Throwable root used to memoize one unit's
/// failure. Its physical global is allocated only after concretization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializationFailureRoot {
    pub unit: InitializationUnitId,
}
