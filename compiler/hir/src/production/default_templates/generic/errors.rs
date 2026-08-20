use std::fmt;

use scoop_identity::{DefinitionOriginSubject, PersistentSourceContextId, SourceIdentity};

#[derive(Debug)]
pub enum GenericTemplateProductionError {
    ContextParameters(crate::SourceParameterProjectionError),
    Declarations(Box<crate::NominalInterfaceBuildError>),
    Entity(crate::DefaultEntityProjectionError),
    Signature(crate::HirInterfaceSignatureProjectionError),
    Locals(Box<crate::DefaultTemplateEnvelopeProjectionError>),
    Body(Box<crate::DefaultBodyProjectionError>),
    Effects(crate::CallableEffectProjectionError),
    MissingOrigin(DefinitionOriginSubject),
    MissingSource(SourceIdentity),
    MissingContext(PersistentSourceContextId),
    Span(DefinitionOriginSubject),
    MissingBinder(crate::TypeParamId),
    CaptureParameters(crate::FunctionId),
    TooManyCaptures(crate::FunctionId),
    Binders(crate::BinderUseListBuildError),
    Record(crate::GenericCallableBodyBuildError),
    Table(crate::GenericCallableBodyTableError),
    Initialization(crate::GenericInitializationBuildError),
    Delegate(crate::GenericDelegateTemplateBuildError),
    Wire(scoop_wire::WireError),
}

impl fmt::Display for GenericTemplateProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextParameters(error) => error.fmt(f),
            Self::Declarations(error) => error.fmt(f),
            Self::Entity(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Locals(error) => error.fmt(f),
            Self::Body(error) => error.fmt(f),
            Self::Effects(error) => error.fmt(f),
            Self::MissingOrigin(owner) => {
                write!(f, "generic body {owner:?} has no definition origin")
            }
            Self::MissingSource(source) => {
                write!(f, "generic body has no source file for {source:?}")
            }
            Self::MissingContext(context) => {
                write!(f, "generic body has no source context for {context:?}")
            }
            Self::Span(owner) => write!(
                f,
                "generic body {owner:?} has a source span outside the HIR range"
            ),
            Self::MissingBinder(parameter) => write!(
                f,
                "generic body condition references out-of-scope binder {parameter:?}"
            ),
            Self::CaptureParameters(function) => write!(
                f,
                "local function {function:?} lacks its capture ABI parameters"
            ),
            Self::TooManyCaptures(function) => {
                write!(f, "closure function {function:?} capture count exceeds u32")
            }
            Self::Binders(error) => error.fmt(f),
            Self::Record(error) => error.fmt(f),
            Self::Table(error) => error.fmt(f),
            Self::Initialization(error) => error.fmt(f),
            Self::Delegate(error) => error.fmt(f),
            Self::Wire(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for GenericTemplateProductionError {}
