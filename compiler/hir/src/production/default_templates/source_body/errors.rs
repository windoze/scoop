use crate::{DefaultTemplateEnvelopeProjectionError, ExportParameterOwner};
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceBodyProductionError {
    Resource(scoop_wire::WireError),
    References(crate::DefaultSourceReferencesProductionError),
    MissingInterface(ExportParameterOwner),
    DuplicateInterface(ExportParameterOwner),
    MissingParameter {
        owner: ExportParameterOwner,
        position: u32,
    },
    NoDefault {
        owner: ExportParameterOwner,
        position: u32,
    },
    Scope(DefaultTemplateEnvelopeProjectionError),
    Body(DefaultTemplateEnvelopeProjectionError),
}
impl From<scoop_wire::WireError> for DefaultSourceBodyProductionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for DefaultSourceBodyProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::References(error) => error.fmt(f),
            Self::Scope(error) | Self::Body(error) => error.fmt(f),
            Self::MissingInterface(owner) => write!(
                f,
                "default source owner {owner:?} has no parameter interface"
            ),
            Self::DuplicateInterface(owner) => write!(
                f,
                "default source owner {owner:?} has duplicate parameter interfaces"
            ),
            Self::MissingParameter { owner, position } => write!(
                f,
                "default source owner {owner:?} has no parameter {position}"
            ),
            Self::NoDefault { owner, position } => write!(
                f,
                "default source owner {owner:?} parameter {position} has no default body"
            ),
        }
    }
}
impl std::error::Error for DefaultSourceBodyProductionError {}

impl DefaultSourceBodyProductionError {
    pub fn resource_error(&self) -> Option<&scoop_wire::WireError> {
        use crate::{DefaultBodyProjectionError as Body, DefaultEntityProjectionError as Entity};
        use DefaultTemplateEnvelopeProjectionError as Envelope;
        match self {
            Self::Resource(error)
            | Self::Scope(Envelope::Resource(error))
            | Self::Body(Envelope::Resource(error))
            | Self::Scope(Envelope::Provider(Entity::Resource(error)))
            | Self::Body(Envelope::Provider(Entity::Resource(error)))
            | Self::Body(Envelope::Body(Body::Resource(error)))
            | Self::Body(Envelope::Body(Body::Entity(Entity::Resource(error)))) => Some(error),
            Self::References(error) => error.resource_error(),
            _ => None,
        }
    }
}
