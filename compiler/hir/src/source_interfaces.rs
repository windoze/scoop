//! Export-side source-call protocol for defaults, named arguments and varargs.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileMetadata {
    pub provider: IntrinsicProviderId,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContext {
    pub function_name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

impl From<DefinitionOrigin> for EvaluationOrigin {
    fn from(origin: DefinitionOrigin) -> Self {
        Self {
            provider: origin.provider,
            file: origin.file,
            span: origin.span,
            context: origin.context,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcreteExpressionOrigin {
    pub definition: DefinitionOrigin,
    pub evaluation: EvaluationOrigin,
}

/// Export HIR contains both declaration-bound expression bodies and default
/// instances embedded in ordinary bodies. A template node has definition
/// provenance only; the concrete product closes the first branch by using its
/// own definition as the evaluation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionOrigin {
    Definition(DefinitionOrigin),
    Instantiated(ConcreteExpressionOrigin),
}

impl ExpressionOrigin {
    pub const fn definition(self) -> DefinitionOrigin {
        match self {
            Self::Definition(definition)
            | Self::Instantiated(ConcreteExpressionOrigin { definition, .. }) => definition,
        }
    }

    pub fn concrete(self) -> ConcreteExpressionOrigin {
        match self {
            Self::Definition(definition) => ConcreteExpressionOrigin {
                definition,
                evaluation: definition.into(),
            },
            Self::Instantiated(origin) => origin,
        }
    }

    pub const fn instantiate(self, evaluation: EvaluationOrigin) -> Self {
        Self::Instantiated(ConcreteExpressionOrigin {
            definition: self.definition(),
            evaluation,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExportValueParameter {
    pub name: String,
    pub calling: ExportParameterCalling,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterCalling {
    Required {
        value_type: TypeId,
    },
    Default {
        value_type: TypeId,
        source: ExportDefaultSourceId,
    },
    Vararg {
        parameter_type: ExportVarargParameterTypeId,
        omission: ExportVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportVarargOmission {
    EmptyArray,
    Default(ExportDefaultSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportVarargParameterType {
    pub element_type: TypeId,
    pub array_type: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterOwner {
    Function(FunctionId),
    StructConstructor(StructConstructorId),
    ClassConstructor(ClassConstructorId),
    VariantConstructor { enumeration: EnumId, variant: u32 },
}

#[derive(Debug, Clone)]
pub struct ExportParameterInterface {
    pub owner: ExportParameterOwner,
    pub parameters: Vec<ExportValueParameter>,
}

/// A default is a declaration-bound typed body. Parameter references are
/// locals in this template arena and are related to declaration positions by
/// `value_parameters`; callers never resolve their source names again.
#[derive(Debug, Clone)]
pub struct ExportDefaultExpr {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
    pub result_type: TypeId,
    /// The exact declaration identities referenced by `Type::Param` nodes in
    /// the template. An inherited source relates these to its own static view
    /// through `ExportDefaultSource::type_arguments`.
    pub type_parameters: Vec<TypeParamId>,
    pub receiver: Option<ExportDefaultReceiver>,
    pub value_parameters: Vec<ExportDefaultValueParameter>,
    /// Direct declaration-bound dependencies of the typed template. Each
    /// category has its own identity domain and every entry carries the
    /// access-domain proof produced before this export entity is committed.
    pub references: ExportDefaultReferences,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Default)]
pub struct ExportDefaultReferences {
    pub callables: Vec<ExportDefaultCallableRef>,
    pub constructors: Vec<ExportDefaultConstructorRef>,
    pub types: Vec<ExportDefaultTypeRef>,
    pub globals: Vec<ExportDefaultGlobalRef>,
    pub singleton_values: Vec<ExportDefaultSingletonValueRef>,
    pub fields: Vec<ExportDefaultFieldRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultAccessWitness {
    pub owner: ExportParameterOwner,
    pub call_domain: CallDomain,
    pub target_domain: AccessDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDomain {
    pub direct: EffectiveLookupDomain,
    pub slot: Option<SlotContractDomain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultCallableRef {
    pub target: ExportDefaultCallableTarget,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultCallableTarget {
    Callable(Callable),
    Bound(BoundCallableRefId),
    DerivedEquality(DerivedEqualityApplicationId),
    LocalFunction(LocalFunctionId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    FunctionAddress(FunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultConstructorRef {
    pub target: ExportDefaultConstructorTarget,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultConstructorTarget {
    Struct(StructConstructorApplicationId),
    Class(ClassConstructorApplicationId),
    Variant(AppliedEnumVariantRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultTypeRef {
    pub target: TypeId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultGlobalRef {
    pub target: GlobalId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSingletonValueRef {
    pub target: SingletonValueId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultFieldRef {
    pub target: FieldRef,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

/// A typed inheritance/application edge for one default source. The argument
/// at each position corresponds to the template parameter at the same
/// position and is expressed in the consuming declaration's type scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSource {
    pub expression: ExportDefaultExprId,
    pub type_arguments: Vec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultReceiver {
    pub local: LocalId,
    pub ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultValueParameter {
    pub position: u32,
    pub local: LocalId,
}
