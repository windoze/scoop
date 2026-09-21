use super::*;

#[derive(Debug)]
pub enum DefaultSourceDeclarationBindingError {
    Resource(WireError),
    Origins(Box<DefaultSourceOriginBindingError>),
    Nominal(Box<NominalSourceBindingError>),
    Member(Box<NominalMemberBindingError>),
    Constructor(Box<NominalConstructorBindingError>),
    Parameter(Box<NominalParameterBindingError>),
    Shape(DefaultTemplateProviderShapeBuildError),
    ProviderParameter(DefaultTemplateProviderParameterBuildError),
    ReceiverShape(DefaultNominalReceiverBuildError),
    Signature(Box<MeteredSignatureTypeSemanticError<NominalSourceBindingError>>),
    Receiver(MeteredTemplateReceiverSemanticValidationError),
    Prefix(MeteredTemplateValueParameterSemanticValidationError),
    Substitution(MeteredDefaultTemplateTypeSubstitutionError),
    Record {
        key: ProtectedDefaultTemplateKeyV1,
        error: Box<Self>,
    },
    MissingProvider(ConeIdentity),
    MissingTemplate(ProtectedDefaultTemplateKeyV1),
    Declaration(CallableTemplateOrigin),
    DefaultParameter {
        declaration: CallableTemplateOrigin,
        position: usize,
    },
    ParameterArity,
    DefinitionPath,
    MappingArity,
    DirectMapping {
        index: usize,
    },
    ParameterType {
        index: usize,
    },
    BindingFieldOwner(scoop_identity::PersistentFieldId),
    DataFlow(Box<ExportDefaultLocalDataFlowValidationError<Self>>),
    NestedIdentity {
        identity: DefaultNestedCallableIdentityV1,
        reason: DefaultSourceNestedIdentityFailureV1,
    },
    LocalScope(TemplateLocalScopeValidationError),
    LocalType {
        index: usize,
        error: Box<MeteredSignatureTypeSemanticError<NominalSourceBindingError>>,
    },
    BodyEnvelope(
        Box<DefaultBodyProviderEnvelopeSemanticValidationError<NominalSourceBindingError>>,
    ),
    ResultType,
    SuspendPermission,
}
macro_rules! boxed_from {
    ($($ty:ty => $variant:ident),+ $(,)?) => { $(impl From<$ty> for Error {
        fn from(error: $ty) -> Self { Self::$variant(Box::new(error)) }
    })+ };
}
boxed_from! {
    DefaultSourceOriginBindingError => Origins, NominalSourceBindingError => Nominal,
    NominalMemberBindingError => Member, NominalConstructorBindingError => Constructor,
    NominalParameterBindingError => Parameter,
    MeteredSignatureTypeSemanticError<NominalSourceBindingError> => Signature,
}
macro_rules! direct_from {
    ($($ty:ty => $variant:ident),+ $(,)?) => { $(impl From<$ty> for Error {
        fn from(error: $ty) -> Self { Self::$variant(error) }
    })+ };
}
direct_from! {
    DefaultTemplateProviderParameterBuildError => ProviderParameter,
    WireError => Resource, DefaultTemplateProviderShapeBuildError => Shape,
    DefaultNominalReceiverBuildError => ReceiverShape,
    MeteredTemplateReceiverSemanticValidationError => Receiver,
    MeteredTemplateValueParameterSemanticValidationError => Prefix,
    MeteredDefaultTemplateTypeSubstitutionError => Substitution,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Origins(e) => e.fmt(f),
            Self::Nominal(e) => e.fmt(f),
            Self::Member(e) => e.fmt(f),
            Self::Constructor(e) => e.fmt(f),
            Self::Parameter(e) => e.fmt(f),
            Self::Shape(e) => e.fmt(f),
            Self::ProviderParameter(e) => e.fmt(f),
            Self::ReceiverShape(e) => e.fmt(f),
            Self::Signature(e) => e.fmt(f),
            Self::Receiver(e) => e.fmt(f),
            Self::Prefix(e) => e.fmt(f),
            Self::Substitution(e) => e.fmt(f),
            Self::Record { key, error } => {
                write!(f, "default source {key:?} declaration contract: {error}")
            }
            Self::MissingProvider(cone) => {
                write!(f, "missing default declaration provider {cone:?}")
            }
            Self::MissingTemplate(key) => write!(f, "missing default declaration contract {key:?}"),
            Self::Declaration(id) => write!(f, "invalid default source declaration {id:?}"),
            Self::DefaultParameter {
                declaration,
                position,
            } => write!(
                f,
                "default provider {declaration:?} has no default at parameter {position}"
            ),
            Self::ParameterArity => {
                f.write_str("default source provider and owner parameter counts differ")
            }
            Self::DefinitionPath => {
                f.write_str("default source path does not identify its provider's default ordinal")
            }
            Self::MappingArity => {
                f.write_str("default source mapping differs from its provider binder count")
            }
            Self::DirectMapping { index } => write!(
                f,
                "direct default source mapping argument {index} is not identity"
            ),
            Self::ParameterType { index } => write!(
                f,
                "default source parameter {index} differs after provider substitution"
            ),
            Self::BindingFieldOwner(field) => write!(
                f,
                "default binding field {field:?} differs from its source struct owner"
            ),
            Self::DataFlow(error) => error.fmt(f),
            Self::NestedIdentity { identity, reason } => {
                write!(f, "default nested identity {identity:?}: {reason:?}")
            }
            Self::LocalScope(error) => error.fmt(f),
            Self::LocalType { index, error } => {
                write!(f, "default source local {index} type: {error}")
            }
            Self::BodyEnvelope(error) => error.fmt(f),
            Self::ResultType => {
                f.write_str("default source result differs from its original provider parameter")
            }
            Self::SuspendPermission => {
                f.write_str("default source suspend permission differs from its declarations")
            }
        }
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceNestedIdentityFailureV1 {
    MissingArtifactRecord,
    Kind,
    DefinitionPath,
    DefinitionSource,
    LexicalParent,
    DefinitionContext,
}
