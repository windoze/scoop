use super::*;

/// One compiler-managed exactly-once initialization action. The stable key is
/// independent from arena order and link symbols; dependencies contain only
/// direct source initializer reads validated by HIR lowering.
#[derive(Debug, Clone)]
pub struct InitializationUnit {
    pub stable_key: String,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationDependency>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    EagerTopLevel {
        property: PropertyId,
        storage: GlobalId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializationDependency {
    pub unit: InitializationUnitId,
    pub span: Span,
}

/// Typed identity of the managed Throwable root used to memoize one unit's
/// failure. Its physical global is allocated only after concretization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializationFailureRoot {
    pub unit: InitializationUnitId,
}
