use super::*;
use crate::{BackendScalarKind, NichePointerKind};

impl ExactValueLayoutV1 {
    /// Replays scalar geometry. The complete section must join `kind` and the
    /// exact identity to the same provider's checked intrinsic representation.
    pub fn scalar(
        identity: ExactLayoutIdentityV1,
        kind: ScalarRepresentationKindV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(
            &identity,
            &[RepresentationRole::ManagedValue, RepresentationRole::CValue],
        )?;
        nominal(identity.exact_key())?;
        let layout = match kind {
            ScalarRepresentationKindV1::Integer(kind) => identity.target().integer_layout(kind),
            ScalarRepresentationKindV1::Char => {
                identity.target().scalar_layout(BackendScalarKind::I32)
            }
            ScalarRepresentationKindV1::Boolean => {
                identity.target().scalar_layout(BackendScalarKind::I1)
            }
        };
        let storage = ValueStorageLayoutV1::inline(
            layout.size_bytes(),
            layout.alignment_bytes(),
            RefScan::None,
        )?;
        finish_value(
            identity,
            storage,
            ValueRepresentation::Scalar(kind),
            foundation,
        )
    }

    pub fn qualified_pointer(
        identity: ExactLayoutIdentityV1,
        kind: NichePointerKind,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        let layout = match kind {
            NichePointerKind::Managed => {
                require_roles(&identity, &[RepresentationRole::ManagedValue])?;
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
                identity.target().managed_pointer_layout()
            }
            NichePointerKind::Raw => {
                require_roles(
                    &identity,
                    &[RepresentationRole::ManagedValue, RepresentationRole::CValue],
                )?;
                if !matches!(identity.exact_key(), ExactTypeKey::RawPointer(_)) {
                    return Err(ExactLayoutReplayError::IdentityKind);
                }
                identity.target().data_pointer().layout()
            }
            NichePointerKind::Code => {
                require_roles(
                    &identity,
                    &[
                        RepresentationRole::ManagedValue,
                        RepresentationRole::CValue,
                        RepresentationRole::NativeFunctionPointer,
                    ],
                )?;
                if !matches!(
                    identity.exact_key(),
                    ExactTypeKey::NativeFunctionPointer { .. }
                ) {
                    return Err(ExactLayoutReplayError::IdentityKind);
                }
                identity.target().code_pointer().layout()
            }
        };
        let scan = match kind {
            NichePointerKind::Managed => RefScan::References(vec![0]),
            NichePointerKind::Raw | NichePointerKind::Code => RefScan::None,
        };
        let storage =
            ValueStorageLayoutV1::inline(layout.size_bytes(), layout.alignment_bytes(), scan)?;
        finish_value(
            identity,
            storage,
            ValueRepresentation::QualifiedPointer(kind),
            foundation,
        )
    }

    pub fn unit(
        identity: ExactLayoutIdentityV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedValue])?;
        if !is_unit(identity.exact_key()) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        finish_value(
            identity,
            ValueStorageLayoutV1::zero_sized(1)?,
            ValueRepresentation::IntrinsicValue(IntrinsicValueFamilyV1::Unit),
            foundation,
        )
    }
}
