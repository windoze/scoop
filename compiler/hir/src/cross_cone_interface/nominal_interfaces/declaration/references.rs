//! Typed member and nested nominal references in the shared declaration.

mod decode;
mod records;
pub use decode::DecodedNestedSourceMemberRefV1;
pub use records::{
    CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1, NestedSourceMemberRefV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalDeclarationReferenceError {
    Duplicate,
    NonCanonicalOrder,
    Encoding(scoop_wire::cbor::EncodeError),
}
impl std::fmt::Display for NominalDeclarationReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate => f.write_str("duplicate nominal declaration reference"),
            Self::NonCanonicalOrder => {
                f.write_str("nominal declaration references are not in canonical key order")
            }
            Self::Encoding(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for NominalDeclarationReferenceError {}
