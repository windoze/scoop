use scoop_identity::CallableTemplateOrigin;

use crate::{
    BinderUseListBuildError, CallableInterfaceBuildError, CallableSourceInterfaceProductionError,
    DefaultArrayAssemblyBuildError, DefaultBindingActionBuildError, DefaultBindingPlanBuildError,
    DefaultBindingProjectionBuildError, DefaultBindingShapeBuildError,
    DefaultCallableBodyTypeArgumentsBuildError, DefaultCallableRefBuildError,
    DefaultCallableReferenceBuildError, DefaultControlFlowBuildError, DefaultExpressionBuildError,
    DefaultForIterationPlanBuildError, DefaultLexicalCallableBuildError,
    DefaultLocalFunctionBuildError, DefaultPatternBuildError, DefaultStatementBuildError,
    ExportDefaultBodyBuildError, ExportDefaultReferenceKindV1, ExportDefaultReferenceSetBuildError,
    ExportDefaultTemplateBuildError, ExportDefaultTemplateKeyV1,
    ExportDefaultTemplateSetBuildError, HirDefinitionSourceProjectionError,
    HirInterfaceSignatureProjectionError, TemplateLocalRecordBuildError,
    TemplateLocalTableBuildError, TemplateReceiverBuildError, TemplateValueParameterBuildError,
    TemplateValueParameterListBuildError,
};

mod display;

#[derive(Debug)]
pub enum DefaultTemplateProductionError {
    CallableInterfaces(CallableInterfaceBuildError),
    SourceInterfaces(CallableSourceInterfaceProductionError),
    MissingSourceInterface(CallableTemplateOrigin),
    DuplicateSourceInterface(CallableTemplateOrigin),
    MissingCanonicalSourceInterface(CallableTemplateOrigin),
    SourceParameterArity {
        owner: CallableTemplateOrigin,
        hir: usize,
        canonical: usize,
    },
    TooManySourceParameters {
        owner: CallableTemplateOrigin,
    },
    Template {
        key: ExportDefaultTemplateKeyV1,
        source: Box<DefaultTemplateEnvelopeProjectionError>,
    },
    Table(ExportDefaultTemplateSetBuildError),
}

#[derive(Debug)]
pub enum DefaultTemplateEnvelopeProjectionError {
    Resource(scoop_wire::WireError),
    UnknownDefaultSource(u32),
    UnknownDefaultExpression(u32),
    Provider(DefaultEntityProjectionError),
    TypeParameterArity {
        expected: usize,
        actual: usize,
    },
    TypeParameterIdentity {
        position: usize,
    },
    BinderUse(BinderUseListBuildError),
    Signature(HirInterfaceSignatureProjectionError),
    DefinitionOrigin(HirDefinitionSourceProjectionError),
    DuplicateLocalBinding(u32),
    LocalRecord {
        local: u32,
        source: TemplateLocalRecordBuildError,
    },
    LocalTable(TemplateLocalTableBuildError),
    Receiver(TemplateReceiverBuildError),
    ValueParameter {
        index: usize,
        source: TemplateValueParameterBuildError,
    },
    ValueParameters(TemplateValueParameterListBuildError),
    Body(DefaultBodyProjectionError),
    References(DefaultReferenceProjectionError),
    Record(ExportDefaultTemplateBuildError),
}

#[derive(Debug)]
pub enum DefaultEntityProjectionError {
    Resource(scoop_wire::WireError),
    Unknown { kind: &'static str, index: u32 },
    MissingIdentity { kind: &'static str, index: u32 },
    UnsupportedFunctionIdentity { function: u32 },
    ExpectedSourceDeclaration { function: u32 },
    InvalidLocalFunctionBinders { local_function: u32 },
    ExpectedOrdinaryProperty { property: u32 },
    Type(HirInterfaceSignatureProjectionError),
    Callable(DefaultCallableRefBuildError),
    GeneratedIdentity(scoop_identity::GeneratedCallableIdentityError),
    ImportedCoreUnavailable(u32),
    ImportedCoreKind(u32),
    ImportedDependencyUnavailable(u32),
}

#[derive(Debug)]
pub enum DefaultBodyProjectionError {
    Resource(scoop_wire::WireError),
    Entity(DefaultEntityProjectionError),
    Signature(HirInterfaceSignatureProjectionError),
    DefinitionOrigin(HirDefinitionSourceProjectionError),
    UnknownLocal(u32),
    UnknownBinding(u32),
    InvalidCaptureSource,
    InvalidLiteralPattern,
    UnsupportedExpression(&'static str),
    UnsupportedAssignment(&'static str),
    LoopControlOutsideLoop(&'static str),
    NonInnermostLoop {
        control: &'static str,
        expected: u32,
        actual: u32,
    },
    TooManyOwnerTypeParameters,
    BodyTypeArguments(DefaultCallableBodyTypeArgumentsBuildError),
    Expression(DefaultExpressionBuildError),
    Statement(DefaultStatementBuildError),
    Pattern(DefaultPatternBuildError),
    ControlFlow(DefaultControlFlowBuildError),
    BindingShape(DefaultBindingShapeBuildError),
    BindingProjection(DefaultBindingProjectionBuildError),
    BindingAction(DefaultBindingActionBuildError),
    BindingPlan(DefaultBindingPlanBuildError),
    ForPlan(DefaultForIterationPlanBuildError),
    ArrayAssembly(DefaultArrayAssemblyBuildError),
    LocalFunction(DefaultLocalFunctionBuildError),
    LexicalCallable(DefaultLexicalCallableBuildError),
    CallableReference(DefaultCallableReferenceBuildError),
    Body(ExportDefaultBodyBuildError),
}

#[derive(Debug)]
pub enum DefaultReferenceProjectionError {
    Entity(DefaultEntityProjectionError),
    Signature(HirInterfaceSignatureProjectionError),
    DefinitionOrigin(HirDefinitionSourceProjectionError),
    RestrictedTarget {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
    },
    Owner {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    InvalidCallDomain {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
    },
    Set(ExportDefaultReferenceSetBuildError),
}

impl From<DefaultEntityProjectionError> for DefaultBodyProjectionError {
    fn from(source: DefaultEntityProjectionError) -> Self {
        Self::Entity(source)
    }
}

impl From<HirInterfaceSignatureProjectionError> for DefaultBodyProjectionError {
    fn from(source: HirInterfaceSignatureProjectionError) -> Self {
        Self::Signature(source)
    }
}

impl From<HirDefinitionSourceProjectionError> for DefaultBodyProjectionError {
    fn from(source: HirDefinitionSourceProjectionError) -> Self {
        Self::DefinitionOrigin(source)
    }
}

impl From<DefaultEntityProjectionError> for DefaultReferenceProjectionError {
    fn from(source: DefaultEntityProjectionError) -> Self {
        Self::Entity(source)
    }
}

impl From<scoop_wire::WireError> for DefaultTemplateEnvelopeProjectionError {
    fn from(source: scoop_wire::WireError) -> Self {
        Self::Resource(source)
    }
}

impl From<scoop_wire::WireError> for DefaultEntityProjectionError {
    fn from(source: scoop_wire::WireError) -> Self {
        Self::Resource(source)
    }
}

impl From<scoop_wire::WireError> for DefaultBodyProjectionError {
    fn from(source: scoop_wire::WireError) -> Self {
        Self::Resource(source)
    }
}
