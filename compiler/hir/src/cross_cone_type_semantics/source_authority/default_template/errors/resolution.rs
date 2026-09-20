use crate::*;
use scoop_identity::SourceOriginResolutionError;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceTemplateResolutionError<E> {
    Resource(WireError),
    Key(ProtectedSourceResolutionError<E>),
    DefinitionRoot(E),
    Locals(TemplateLocalTableValidationError<E>),
    Body(ExportDefaultBodyResolutionError<E, TemplateLocalLookupError>),
    Result(E),
    TypeParameters(BinderUseListValidationError<E>),
    Receiver(TemplateReceiverResolutionError<E, TemplateLocalLookupError>),
    ValueParameters(TemplateValueParameterListValidationError<TemplateLocalLookupError>),
    References(DefaultSourceReferencesResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(DefaultSourceTemplateBuildError),
}
impl<E: fmt::Display> fmt::Display for DefaultSourceTemplateResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Key(error) => write!(f, "invalid source default key: {error}"),
            Self::DefinitionRoot(error) => write!(f, "invalid source default root: {error}"),
            Self::Locals(error) => write!(f, "invalid source default locals: {error}"),
            Self::Body(error) => write!(f, "invalid source default body: {error}"),
            Self::Result(error) => write!(f, "invalid source default result: {error}"),
            Self::TypeParameters(error) => {
                write!(f, "invalid source default binder uses: {error}")
            }
            Self::Receiver(error) => write!(f, "invalid source default receiver: {error}"),
            Self::ValueParameters(error) => {
                write!(f, "invalid source default parameters: {error}")
            }
            Self::References(error) => write!(f, "invalid source default references: {error}"),
            Self::DefinitionOrigin(error) => write!(f, "invalid source default origin: {error}"),
            Self::Record(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DefaultSourceTemplateResolutionError<E> {}
