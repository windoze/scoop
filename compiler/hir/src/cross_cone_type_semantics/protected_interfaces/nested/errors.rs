use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NestedSourceBuildError {
    Kind,
    Modality,
    Binders,
    SupportKind,
    Owner,
    Access,
    Duplicate,
    NonCanonicalOrder,
    ReferenceClosure,
    Encoding(scoop_wire::cbor::EncodeError),
}
impl fmt::Display for NestedSourceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kind => {
                f.write_str("nested source shape or constructor partition has the wrong kind")
            }
            Self::Modality => f.write_str("nested nominal kind and modality disagree"),
            Self::Binders => {
                f.write_str("nested nominal binders disagree with its source identity")
            }
            Self::SupportKind => {
                f.write_str("nested source template cannot carry concrete support")
            }
            Self::Owner => f.write_str("nested source support has a different lexical owner"),
            Self::Access => f.write_str(
                "protected nested declaration requires protected access and a lexical owner",
            ),
            Self::Duplicate => f.write_str("duplicate nested source declaration reference"),
            Self::NonCanonicalOrder => {
                f.write_str("nested source references are not in canonical key order")
            }
            Self::ReferenceClosure => {
                f.write_str("nested source references and support records do not close exactly")
            }
            Self::Encoding(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for NestedSourceBuildError {}
