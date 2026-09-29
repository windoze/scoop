use crate::{
    Capture, DefinitionOrigin, Expr, FunctionTypeId, ImportedCallableTemplateParent,
    ImportedDependencyCallableUseId, ImportedGenericCallableApplicationId, ImportedMethodCallee,
    TypeId,
};

/// A resolved dependency target shared by direct calls and callable references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedCallableTarget {
    Application(ImportedGenericCallableApplicationId),
    Dependency(ImportedDependencyCallableUseId),
}

#[derive(Debug, Clone)]
pub enum ImportedCallableReferenceTarget {
    Named(ImportedCallableTarget),
    Local(ImportedGenericCallableApplicationId),
    BoundMember {
        receiver: Box<Expr>,
        callee: ImportedMethodCallee,
    },
    BoundExtension {
        receiver: Box<Expr>,
        callee: ImportedCallableTarget,
    },
}

impl ImportedCallableReferenceTarget {
    pub fn callee(&self) -> Option<ImportedCallableTarget> {
        match self {
            Self::Named(callee) | Self::BoundExtension { callee, .. } => Some(*callee),
            Self::BoundMember { callee, .. } => callee.declared_callable(),
            Self::Local(application) => Some(ImportedCallableTarget::Application(*application)),
        }
    }

    pub fn receiver(&self) -> Option<&Expr> {
        match self {
            Self::BoundMember { receiver, .. } | Self::BoundExtension { receiver, .. } => {
                Some(receiver)
            }
            Self::Named(_) | Self::Local(_) => None,
        }
    }

    pub fn receiver_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::BoundMember { receiver, .. } | Self::BoundExtension { receiver, .. } => {
                Some(receiver)
            }
            Self::Named(_) | Self::Local(_) => None,
        }
    }
}

/// A provider-defined invoke with captures evaluated at this creation site.
#[derive(Debug, Clone)]
pub struct ImportedCallableReference {
    pub definition: crate::concrete::GeneratedCallableRecord,
    pub parent: ImportedCallableTemplateParent,
    pub owner_type_arguments: Vec<TypeId>,
    pub target: ImportedCallableReferenceTarget,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub origin: DefinitionOrigin,
}
