use super::*;

pub(in crate::production) fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
pub(in crate::production) fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
