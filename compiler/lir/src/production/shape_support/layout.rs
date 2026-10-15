use super::*;
use crate::{ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1};

impl StrongShapeDefinitionV1<PersistentLayoutId> {
    pub(crate) fn from_layout_definition(
        layout: PersistentLayoutId,
        physical: StrongShapeDefinitionRefV1,
    ) -> Option<Self> {
        (physical.subject() == ExternalStrongShapeSubjectV1::Layout(layout)).then_some(Self {
            semantic_id: layout,
            definition_plan: physical.definition(),
            symbol: physical.symbol(),
        })
    }
}

impl<I: PersistentId> DecodedStrongShapeDefinitionV1<I> {
    pub(crate) fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongShapeDefinitionV1<I>, crate::LinkDataError>
    where
        ValidatedIdentityGraph:
            scoop_identity::PersistentIdResolver<I, Error = scoop_identity::IdentityReferenceError>,
    {
        use crate::link_data::link_error;
        use scoop_identity::PersistentIdResolver;
        Ok(StrongShapeDefinitionV1::from_artifact(
            identities.resolve(self.semantic_id).map_err(link_error)?,
            PersistentIdResolver::<ObjectDefinitionPlanId>::resolve(
                identities,
                self.definition_plan,
            )
            .map_err(link_error)?,
            self.symbol.resolve(identities).map_err(link_error)?,
        ))
    }

    pub(crate) fn matches_definition(
        self,
        expected: StrongShapeDefinitionV1<I>,
    ) -> Result<bool, WireError> {
        let path = scoop_wire::WirePath::root();

        if self.semantic_id.verify(expected.semantic_id()).is_err()
            || self
                .definition_plan
                .verify(expected.definition_plan())
                .is_err()
        {
            return Ok(false);
        }
        // The frozen symbol-request product has no standalone refinement API.
        // Compare its complete canonical fields, never merely their digest.
        let actual = scoop_wire::encode_canonical_temporary(&self.symbol, &path)?;
        let expected = scoop_wire::encode_canonical_temporary(&expected.symbol(), &path)?;
        Ok(actual == expected)
    }
}

impl<I: PersistentId> DecodedStrongShapeRegistrationV1<I> {
    pub(crate) fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongShapeRegistrationV1<I>, crate::LinkDataError>
    where
        ValidatedIdentityGraph:
            scoop_identity::PersistentIdResolver<I, Error = scoop_identity::IdentityReferenceError>,
    {
        let definition = DecodedStrongShapeDefinitionV1 {
            semantic_id: self.semantic_id,
            definition_plan: self.definition_plan,
            symbol: self.symbol,
        }
        .read(identities)?;
        Ok(StrongShapeRegistrationV1::from_artifact(
            definition.semantic_id(),
            definition.definition_plan(),
            definition.symbol(),
        ))
    }

    pub(crate) fn matches_registration(
        self,
        expected: StrongShapeRegistrationV1<I>,
    ) -> Result<bool, WireError> {
        let path = scoop_wire::WirePath::root();

        if self.semantic_id.verify(expected.semantic_id()).is_err()
            || self
                .definition_plan
                .verify(expected.definition_plan())
                .is_err()
        {
            return Ok(false);
        }
        let actual = scoop_wire::encode_canonical_temporary(&self.symbol, &path)?;
        let expected = scoop_wire::encode_canonical_temporary(&expected.symbol(), &path)?;
        Ok(actual == expected)
    }
}
