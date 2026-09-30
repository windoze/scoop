use super::*;

pub type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

/// Persistent identity of one concrete callable-reference invoke wrapper.
/// The definition path lives in the generated key so it cannot disagree with
/// the wrapper identity used by downstream stages.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CallableReferenceIdentity {
    callable: GeneratedCallableRecord,
    materialization: CallableMaterialization,
}

impl CallableReferenceIdentity {
    pub fn new(
        parent: LexicalCallableParent,
        path: StructuralDefinitionPath,
        context: CallableMaterializationContext,
    ) -> Result<Self, scoop_identity::GeneratedCallableIdentityError> {
        let callable =
            CborIdentityRecord::from_key(GeneratedCallableKey::CallableReferenceInvoke {
                parent,
                path,
            })?;
        let materialization =
            CallableMaterialization::new(CallableTemplateOwner::Generated(callable.id()), context);
        Ok(Self {
            callable,
            materialization,
        })
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub const fn materialization(&self) -> &CallableMaterialization {
        &self.materialization
    }

    pub fn definition_path(&self) -> &StructuralDefinitionPath {
        let GeneratedCallableKey::CallableReferenceInvoke { path, .. } = self.callable.key() else {
            unreachable!("a callable-reference identity always retains its invoke key")
        };
        path
    }
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnonymousFunction {
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub identity: CallableReferenceIdentity,
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub origin: DefinitionOrigin,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableReferenceTarget {
    Named(CallableTarget),
    Local {
        local_function: LocalFunctionId,
        callee: Callable,
    },
    BoundMember {
        receiver: Box<Expr>,
        callee: CallableTarget,
    },
    BoundExtension {
        receiver: Box<Expr>,
        callee: CallableTarget,
    },
    BoundIntrinsic {
        receiver: Box<Expr>,
        intrinsic: crate::PrimitiveMemberIntrinsic,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableTarget {
    Local(Callable),
    Imported(ImportedDependencyCallableUseId),
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackSignature {
    pub function: FunctionId,
    pub signature: FunctionTypeId,
}

impl CallableReferenceTarget {
    pub fn callee(&self) -> Option<CallableTarget> {
        match self {
            Self::Named(callee)
            | Self::BoundMember { callee, .. }
            | Self::BoundExtension { callee, .. } => Some(*callee),
            Self::Local { callee, .. } => Some(CallableTarget::Local(*callee)),
            Self::BoundIntrinsic { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub first_use_span: Span,
    pub source: Expr,
}

#[derive(Debug, Clone)]
pub struct FunctionCoercion {
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
}
