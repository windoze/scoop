use super::BodyProjection;
use crate::{DefaultExpressionKindV1, DefaultExpressionV1, DefaultFieldRefV1};
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

impl BodyProjection<'_, '_> {
    pub(super) fn initializing_receiver(
        &self,
        owner: SignatureTypeKey,
        origin: crate::DefinitionOrigin,
    ) -> Result<DefaultExpressionV1, super::super::DefaultBodyProjectionError> {
        let receiver_type = self.locals.initializing_receiver()?;
        let origin = self.origin(origin)?;
        let evaluation = scoop_identity::EvaluationOrigin::at_definition(origin.origin());
        let receiver = DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(LocalValueSelector::This),
            receiver_type.clone(),
            origin.clone(),
            evaluation.clone(),
        )
        .map_err(super::super::DefaultBodyProjectionError::Expression)?;
        if receiver_type == &owner {
            return Ok(receiver);
        }
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::ReferenceUpcast(Box::new(receiver)),
            owner,
            origin,
            evaluation,
        )
        .map_err(super::super::DefaultBodyProjectionError::Expression)
    }

    pub(super) fn initializing_class_field(
        &self,
        field: crate::InitializingClassFieldRef,
        origin: crate::DefinitionOrigin,
    ) -> Result<(DefaultExpressionV1, DefaultFieldRefV1), super::super::DefaultBodyProjectionError>
    {
        let owner = self.type_key(field.owner)?;
        let reference = DefaultFieldRefV1::Class {
            declaration: field.field,
            owner_type: owner.clone(),
        };
        Ok((self.initializing_receiver(owner, origin)?, reference))
    }
}
