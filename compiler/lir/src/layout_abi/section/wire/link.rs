use super::*;
use crate::{LinkDataError, LinkValueRepresentation};
use scoop_identity::{PersistentExactTypeId, ValidatedIdentityGraph};
use std::collections::BTreeMap;

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn read_link_values(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<BTreeMap<PersistentExactTypeId, LinkValueRepresentation>, LinkDataError> {
        self.layouts.read_link_values(identities)
    }
}
