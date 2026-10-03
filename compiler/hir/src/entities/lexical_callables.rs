use super::*;

/// Typed root of one stable lexical definition traversal. Paths are complete
/// relative to this root, so nested callables keep the same root while
/// appending their own segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalDefinitionRoot {
    Function(FunctionId),
    ClassConstructor(ClassConstructorId),
    StructConstructor(StructConstructorId),
    VariantConstructor(EnumVariantRef),
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub definition: LexicalFunctionDefinition,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function_type: FunctionTypeId,
    /// Type parameters inherited from the enclosing generic callable. The
    /// generated invoke body is instantiated with this complete prefix.
    pub owner_type_param_count: usize,
    pub body_type_arguments: CallableBodyTypeArguments,
    /// Structurally present even for no-capture lambdas; later M11 capture
    /// analysis fills this list rather than changing the entity shape.
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnonymousFunction {
    pub definition: LexicalFunctionDefinition,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function_type: FunctionTypeId,
    pub owner_type_param_count: usize,
    pub body_type_arguments: CallableBodyTypeArguments,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableBodyTypeArguments {
    /// Substitute the type arguments of the ordinary enclosing body.
    Lexical,
    /// A decoded or expanded body keeps the original lexical parameters in
    /// its current binder frame, even when the expression belongs to a
    /// different caller body.
    Explicit(Vec<TypeId>),
}

/// A block-local named function. Its definition locates the lifted body;
/// direct calls pass `captures` as hidden parameters, while taking `::name`
/// materializes a closure over the same body.
#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub definition: LexicalFunctionDefinition,
    /// Stable lexical declaration path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    /// Source signature in the lifted declaration's own binder frame.
    pub declaration_function_type: FunctionTypeId,
    /// Signature at this occurrence, after any default expansion.
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    /// Inherited binder prefix of the lifted declaration. Actual uses carry
    /// their complete arguments on the selected callable target.
    pub owner_type_param_count: usize,
    pub origin: DefinitionOrigin,
    pub span: Span,
}

/// Storage location of a lexical body. Its source identity and binder
/// frame remain attached to that definition, independently of occurrences.
#[derive(Debug, Clone, Copy)]
pub enum LexicalFunctionDefinition {
    Source {
        function: FunctionId,
        root: LexicalDefinitionRoot,
    },
    Template(ImportedGenericCallableTemplateId),
}

impl LexicalFunctionDefinition {
    pub fn source(self) -> Option<(FunctionId, LexicalDefinitionRoot)> {
        match self {
            LexicalFunctionDefinition::Source { function, root } => Some((function, root)),
            LexicalFunctionDefinition::Template(_) => None,
        }
    }

    pub fn source_function(self) -> FunctionId {
        self.source()
            .expect("source lookup retains its current declaration")
            .0
    }
}

impl LocalFunction {
    pub fn source(&self) -> Option<(FunctionId, LexicalDefinitionRoot)> {
        self.definition.source()
    }

    pub fn source_function(&self) -> FunctionId {
        self.definition.source_function()
    }
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub definition_root: CallableReferenceRoot,
    /// Stable definition-site path of the generated invoke wrapper.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    /// Complete definition-owner arguments in the current expansion frame.
    /// Unused owner binders still determine the invoke materialization.
    pub owner_type_arguments: Vec<TypeId>,
    pub captures: Vec<Capture>,
    /// Definition site of the source callable-reference expression. A bound
    /// receiver local value uses this origin rather than the receiver
    /// expression's evaluation provenance.
    pub origin: DefinitionOrigin,
    pub span: Span,
}

/// Two storage locations for the same lexical parent. A source root resolves
/// through current declaration identities; a persisted parent already carries
/// its original typed identity. Neither changes the reference's semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableReferenceRoot {
    Source(LexicalDefinitionRoot),
    Persistent(scoop_identity::LexicalCallableParent),
}

#[derive(Debug, Clone)]
pub enum CallableReferenceTarget {
    Named(CallableTarget),
    Local {
        definition_path: scoop_identity::StructuralDefinitionPath,
        callee: CallableTarget,
    },
    /// A member reference whose receiver expression is evaluated when the
    /// closure is created. The receiver's static type remains attached to the
    /// expression so MIR can preserve direct / virtual / interface dispatch.
    BoundMember {
        receiver: Box<Expr>,
        callee: MethodCallee,
    },
    /// A bound extension reference. Unlike a member reference its invoke
    /// wrapper always direct-calls the extension body, prepending the saved
    /// receiver to the ordinary source arguments.
    BoundExtension {
        receiver: Box<Expr>,
        callee: CallableTarget,
    },
    BoundIntrinsic {
        receiver: Box<Expr>,
        declaration: scoop_identity::PersistentFunctionId,
        intrinsic: PrimitiveMemberIntrinsic,
    },
}

impl CallableReferenceTarget {
    pub fn callee(&self, bounds: &Arena<BoundCallableRef>) -> Option<CallableTarget> {
        match self {
            Self::Named(callee)
            | Self::Local { callee, .. }
            | Self::BoundExtension { callee, .. } => Some(*callee),
            Self::BoundMember { callee, .. } => callee.declared_callable(bounds),
            Self::BoundIntrinsic { .. } => None,
        }
    }

    pub fn receiver(&self) -> Option<&Expr> {
        match self {
            Self::BoundMember { receiver, .. }
            | Self::BoundExtension { receiver, .. }
            | Self::BoundIntrinsic { receiver, .. } => Some(receiver),
            Self::Named(_) | Self::Local { .. } => None,
        }
    }

    pub fn receiver_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::BoundMember { receiver, .. }
            | Self::BoundExtension { receiver, .. }
            | Self::BoundIntrinsic { receiver, .. } => Some(receiver),
            Self::Named(_) | Self::Local { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub first_use_span: Span,
    /// Expression evaluated in the immediately enclosing callable when the
    /// closure object is created. It is either a local read or a transitive
    /// capture read, and therefore preserves by-value creation-time semantics.
    pub source: Expr,
}

#[derive(Debug, Clone)]
pub struct FunctionCoercion {
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}
