use super::*;
use scoop_identity::PersistentSourceContextId;

#[derive(Debug)]
pub enum TypeFoundationBindingError {
    Resource(WireError),
    Identity(String),
    CanonicalKeyMismatch,
    MissingExact(PersistentExactTypeId),
    MissingFactShape(PersistentExactTypeId),
    MissingObject(PersistentTypeId),
    MissingNominal(SourceNominalId),
    MissingGenerated(PersistentTypeId),
    MissingAccessor(PersistentPropertyAccessorId),
    ForeignNominal(SourceNominalId),
    ForeignOrigin,
    MissingSourceContext(PersistentSourceContextId),
    SourceContextMismatch(PersistentSourceContextId),
    MissingSourceRecord,
    MissingSourcePoint(u64),
    MissingDefinitionSource,
    DeclarationOrigin(SourceNominalId),
    Access {
        owner: SourceNominalId,
        reason: String,
    },
    ObjectBacking(PersistentTypeId),
    RepresentationKind(PersistentTypeId),
}

impl From<WireError> for TypeFoundationBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for TypeFoundationBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => write!(f, "foundation source identity: {error}"),
            Self::CanonicalKeyMismatch => {
                f.write_str("source key differs from the validated identity graph")
            }
            Self::MissingExact(id) => write!(
                f,
                "exact source key {id} is absent from the owning foundation"
            ),
            Self::MissingFactShape(id) => write!(
                f,
                "fact shape {id} is absent from the bound source transcript"
            ),
            Self::MissingObject(id) => {
                write!(f, "object {id} is absent from the bound source transcript")
            }
            Self::MissingNominal(id) => write!(
                f,
                "nominal source key {id:?} is absent from the owning foundation"
            ),
            Self::MissingGenerated(id) => write!(
                f,
                "generated source key {id} is absent from the owning foundation"
            ),
            Self::MissingAccessor(id) => write!(
                f,
                "accessor source key {id} is absent from the owning foundation"
            ),
            Self::ForeignNominal(id) => {
                write!(f, "source nominal {id:?} belongs to a different provider")
            }
            Self::ForeignOrigin => f.write_str("source origin belongs to a different provider"),
            Self::MissingSourceContext(id) => write!(
                f,
                "source context {id} is absent from the owning foundation"
            ),
            Self::SourceContextMismatch(id) => {
                write!(f, "source context {id} names a different source file")
            }
            Self::MissingSourceRecord => {
                f.write_str("source file is absent from the owning foundation")
            }
            Self::MissingSourcePoint(offset) => write!(
                f,
                "source byte offset {offset} is absent from the owning foundation"
            ),
            Self::MissingDefinitionSource => {
                f.write_str("definition source is absent from the source transcript")
            }
            Self::DeclarationOrigin(id) => write!(
                f,
                "source nominal {id:?} does not match its foundation declaration origin"
            ),
            Self::Access { owner, reason } => {
                write!(f, "invalid source access for {owner:?}: {reason}")
            }
            Self::ObjectBacking(owner) => {
                write!(f, "invalid source object backing relation for {owner}")
            }
            Self::RepresentationKind(owner) => {
                write!(f, "source representation kind differs from nominal {owner}")
            }
        }
    }
}
impl std::error::Error for TypeFoundationBindingError {}
