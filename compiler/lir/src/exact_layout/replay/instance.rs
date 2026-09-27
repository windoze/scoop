use scoop_identity::{GeneratedNominalKey, PersistentTypeId};

use super::*;
use crate::{ArrayElementStorageV1, TypeInstanceShapeV1};

mod class;
pub use class::ClassLayoutBaseV1;

fn finish_instance(
    identity: ExactLayoutIdentityV1,
    shape: TypeInstanceShapeV1,
    representation: InstanceRepresentation,
    role: ScanRole,
    foundation: &ConeLirFoundation,
) -> Result<ExactInstanceLayoutV1, ExactLayoutReplayError> {
    let scan = scan_binding(&identity, role, foundation)?;
    Ok(ExactInstanceLayoutV1 {
        identity,
        shape,
        representation: InstanceRepresentationV1(representation),
        scan,
    })
}

impl ExactInstanceLayoutV1 {
    /// Replays either a value's own descriptor or its generated box helper.
    /// The containing section joins the representation to the source facts.
    pub fn boxed_payload(
        identity: ExactLayoutIdentityV1,
        payload: &ExactValueLayoutV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        require_roles(&payload.identity, &[RepresentationRole::ManagedValue])?;
        if matches!(
            payload.representation.kind(),
            ExactRepresentationKindV1::QualifiedPointer(crate::NichePointerKind::Managed)
        ) {
            return Err(ExactLayoutReplayError::BoxPayloadKind);
        }
        let expected = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
            payload: payload.identity.exact(),
        })
        .map_err(ExactLayoutReplayError::GeneratedNominal)?;
        if identity.exact() != payload.identity.exact()
            && identity.exact_key() != &ExactTypeKey::Nominal(expected)
        {
            return Err(ExactLayoutReplayError::BoxPayloadIdentity);
        }
        if payload.identity.target() != identity.target() {
            return Err(ExactLayoutReplayError::DependencyTarget);
        }

        let shape =
            TypeInstanceShapeV1::boxed_value(identity.target(), payload.value.storage().clone())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::BoxedPayload(payload.value.clone()),
            ScanRole::ManagedObject,
            foundation,
        )
    }

    /// Replays array geometry. The complete section must join the application
    /// origin to the trusted-core Array or MutableArray binding.
    pub fn inline_array(
        identity: ExactLayoutIdentityV1,
        element: &ExactValueLayoutV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        let ExactTypeKey::NominalApplication { arguments, .. } = identity.exact_key() else {
            return Err(ExactLayoutReplayError::ArrayElementIdentity);
        };
        if arguments.as_slice() != [element.identity.exact()] {
            return Err(ExactLayoutReplayError::ArrayElementIdentity);
        }
        if element.identity.target() != identity.target() {
            return Err(ExactLayoutReplayError::DependencyTarget);
        }

        let storage = ArrayElementStorageV1::from_value(element.value.storage());
        let shape = TypeInstanceShapeV1::inline_array(identity.target(), storage.clone())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::InlineArray {
                element: element.value.clone(),
                storage,
            },
            ScanRole::ArrayElement,
            foundation,
        )
    }

    /// Replays byte storage. The complete section must join the exact identity
    /// to the trusted-core String binding.
    pub fn inline_bytes(
        identity: ExactLayoutIdentityV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        if is_unit(identity.exact_key()) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        if !matches!(identity.exact_key(), ExactTypeKey::Nominal(_)) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        let shape = TypeInstanceShapeV1::inline_bytes(identity.target())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::InlineBytes,
            ScanRole::ManagedObject,
            foundation,
        )
    }

    pub fn abstract_reference(
        identity: ExactLayoutIdentityV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        if is_unit(identity.exact_key()) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        if !matches!(
            identity.exact_key(),
            ExactTypeKey::Nominal(_)
                | ExactTypeKey::NominalApplication { .. }
                | ExactTypeKey::Function { .. }
        ) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        finish_instance(
            identity,
            TypeInstanceShapeV1::abstract_ref(),
            InstanceRepresentation::AbstractReference,
            ScanRole::ManagedObject,
            foundation,
        )
    }
}
