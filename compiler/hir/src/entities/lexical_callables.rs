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
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
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
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
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
    /// A hygienically expanded default fixes the generated body's original
    /// lexical parameters even though the expression now belongs to a
    /// different caller body.
    Explicit(Vec<TypeId>),
}

/// A block-local named function. `function` is its lifted body; direct calls
/// pass `captures` as hidden parameters, while taking `::name` materializes a
/// closure over the same body.
#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical declaration path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    /// Type parameters inherited from enclosing generic callables form the
    /// prefix of the lifted function's combined type-parameter namespace.
    pub owner_type_param_count: usize,
    pub origin: DefinitionOrigin,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable definition-site path of the generated invoke wrapper.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    /// Type parameters of the callable containing this reference expression.
    /// A non-zero value requires a concrete closure per enclosing instance.
    pub owner_type_param_count: usize,
    pub captures: Vec<Capture>,
    /// Definition site of the source callable-reference expression. A bound
    /// receiver local value uses this origin rather than the receiver
    /// expression's evaluation provenance.
    pub origin: DefinitionOrigin,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableReferenceTarget {
    Named(Callable),
    Local {
        local_function: LocalFunctionId,
        callee: Callable,
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
        callee: Callable,
    },
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
