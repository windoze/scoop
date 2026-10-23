use crate::{
    ClassFieldId, ClassId, EditingCallee, FunctionId, InitializationUnitId, PropertyId,
    SemanticCallable, SemanticConstructor, SemanticSourceLocation, SourceContextConstructor,
    SourceDefaultKey,
};

/// The existing typed input checked for one source body region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingSourceBodyId {
    Function(FunctionId),
    Default(SourceDefaultKey),
    Construction(EditingConstructionBody),
}

/// A real source input in the original constructor initialization plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingConstructionBody {
    Delegation(SourceContextConstructor),
    Secondary(SourceContextConstructor),
    Property(PropertyId),
    InterfaceDelegate(ClassFieldId),
    InitBlock { owner: ClassId, member: u32 },
}

impl From<FunctionId> for EditingSourceBodyId {
    fn from(function: FunctionId) -> Self {
        Self::Function(function)
    }
}

impl EditingSourceBodyId {
    pub fn function(self) -> Option<FunctionId> {
        match self {
            Self::Function(function) => Some(function),
            Self::Default(_) | Self::Construction(_) => None,
        }
    }
}

/// One source region checked through its original frontend input.
#[derive(Clone, Debug)]
pub struct EditingSourceBody {
    pub id: EditingSourceBodyId,
    pub owner: EditingSourceBodyOwner,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Copy, Debug)]
pub enum EditingSourceBodyOwner {
    Callable(SemanticCallable),
    Initialization(InitializationUnitId),
    Default(EditingCallee),
    Constructor(SemanticConstructor),
}

impl EditingSourceBody {
    pub fn callee(&self) -> Option<EditingCallee> {
        match self.owner {
            EditingSourceBodyOwner::Callable(callable) => Some(callable.into()),
            EditingSourceBodyOwner::Default(callee) => Some(callee),
            EditingSourceBodyOwner::Constructor(constructor) => {
                Some(EditingCallee::Constructor(constructor))
            }
            EditingSourceBodyOwner::Initialization(_) => None,
        }
    }

    pub fn callable(&self) -> Option<SemanticCallable> {
        self.callee().and_then(EditingCallee::callable)
    }
}
