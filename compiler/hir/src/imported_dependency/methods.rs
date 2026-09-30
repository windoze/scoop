use crate::{CallableTarget, DerivedEqualityApplicationId, FunctionTypeId, TypeId};
use scoop_identity::{CallableTemplateOrigin, PersistentDispatchSlotId};

/// A member selected in the provider's lexical scope.
#[derive(Debug, Clone)]
pub enum ImportedMethodCallee {
    Callable(CallableTarget),
    InterfaceBound(Box<ImportedInterfaceBoundCallable>),
    DerivedEquality(DerivedEqualityApplicationId),
}

/// The declared slot is retained until the actual receiver is concrete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedInterfaceBoundCallable {
    pub receiver_type: TypeId,
    pub interface: TypeId,
    pub member: CallableTemplateOrigin,
    pub slot: PersistentDispatchSlotId,
    pub declared: CallableTarget,
    pub signature: FunctionTypeId,
}

impl ImportedMethodCallee {
    pub fn declared_callable(&self) -> Option<CallableTarget> {
        match self {
            Self::Callable(callee) => Some(*callee),
            Self::InterfaceBound(bound) => Some(bound.declared),
            Self::DerivedEquality(_) => None,
        }
    }
}
