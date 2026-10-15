use super::*;
use crate::{LinkDataError, LinkValueRepresentation, link_data::link_error};
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedExactLayoutExportV1 {
    pub(crate) fn link_value(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Option<(PersistentExactTypeId, LinkValueRepresentation)>, LinkDataError> {
        if self.semantic.role != RepresentationRole::ManagedValue {
            return Ok(None);
        }
        let RawBody::Value { representation, .. } = &self.semantic.body else {
            return Ok(None);
        };
        let kind = match representation {
            RawValue::Scalar(kind) => LinkValueRepresentation::Scalar(*kind),
            RawValue::QualifiedPointer(_) => LinkValueRepresentation::Pointer,
            _ => return Ok(None),
        };
        let exact = identities
            .resolve(self.semantic.exact)
            .map_err(link_error)?;
        Ok(Some((exact, kind)))
    }
}
