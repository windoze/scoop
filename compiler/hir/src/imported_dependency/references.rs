use crate::{
    CallableTarget, Capture, DefinitionOrigin, Expr, FunctionTypeId,
    ImportedGenericCallableApplicationId, MethodCallee, TypeId,
};

#[derive(Debug, Clone)]
pub enum ImportedCallableReferenceTarget {
    Named(CallableTarget),
    Local(ImportedGenericCallableApplicationId),
    BoundMember {
        receiver: Box<Expr>,
        callee: MethodCallee,
    },
    BoundExtension {
        receiver: Box<Expr>,
        callee: CallableTarget,
    },
    BoundIntrinsic {
        receiver: Box<Expr>,
        declaration: scoop_identity::PersistentFunctionId,
        intrinsic: crate::PrimitiveMemberIntrinsic,
    },
}

impl ImportedCallableReferenceTarget {
    pub fn callee(&self, bounds: &crate::Arena<crate::BoundCallableRef>) -> Option<CallableTarget> {
        match self {
            Self::Named(callee) | Self::BoundExtension { callee, .. } => Some(*callee),
            Self::BoundMember { callee, .. } => callee.declared_callable(bounds),
            Self::Local(application) => Some(CallableTarget::Application(*application)),
            Self::BoundIntrinsic { .. } => None,
        }
    }

    pub fn receiver(&self) -> Option<&Expr> {
        match self {
            Self::BoundMember { receiver, .. }
            | Self::BoundExtension { receiver, .. }
            | Self::BoundIntrinsic { receiver, .. } => Some(receiver),
            Self::Named(_) | Self::Local(_) => None,
        }
    }

    pub fn receiver_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::BoundMember { receiver, .. }
            | Self::BoundExtension { receiver, .. }
            | Self::BoundIntrinsic { receiver, .. } => Some(receiver),
            Self::Named(_) | Self::Local(_) => None,
        }
    }
}

/// A provider-defined invoke with captures evaluated at this creation site.
#[derive(Debug, Clone)]
pub struct ImportedCallableReference {
    pub definition: crate::concrete::GeneratedCallableRecord,
    pub parent: scoop_identity::CallableTemplateOwner,
    pub owner_type_arguments: Vec<TypeId>,
    pub target: ImportedCallableReferenceTarget,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub origin: DefinitionOrigin,
}
