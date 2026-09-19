use super::*;
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    GeneratedEnumVariantRole,
};

impl MirTypeBridgeAuthority<'_> {
    pub(super) fn validate_generated(
        &self,
        nominal: PersistentTypeId,
        role: &GeneratedNominalKey,
        record: &ParamFreeMirTypeExportV1,
    ) -> Result<(), MirTypeBridgeError> {
        let key = self
            .identities
            .canonical_key::<_, GeneratedNominalKey>(nominal)?;
        if key.as_ref() != role {
            return Err(MirTypeBridgeError::GeneratedRoleMismatch { nominal });
        }
        // Object backing identities originate in HIR and are retained by MIR.
        // The three finite MIR shape helpers must also belong to this foundation.
        if !matches!(role, GeneratedNominalKey::ObjectBackingClass { .. })
            && self.foundation.as_canonical().generated_type_key(nominal) != Some(role)
        {
            return Err(MirTypeBridgeError::MissingGeneratedFoundation { nominal });
        }
        match (role, record.representation()) {
            (
                GeneratedNominalKey::ObjectBackingClass { object },
                MirTypeRepresentationV1::ObjectBacking { .. },
            ) => {
                if self.source_nominal(*object)?.declaration_kind() != SourceDeclarationKind::Object
                {
                    return Err(MirTypeBridgeError::GeneratedPayloadMismatch { nominal });
                }
            }
            (
                GeneratedNominalKey::BoxedValue { payload },
                MirTypeRepresentationV1::BoxedValue { payload: field },
            ) => {
                let key = FieldIdentityKey::box_payload(role)
                    .map_err(|_| MirTypeBridgeError::GeneratedPayloadMismatch { nominal })?;
                if field.value != *payload
                    || PersistentFieldId::from_key(&key).ok() != Some(field.field)
                {
                    return Err(MirTypeBridgeError::GeneratedPayloadMismatch { nominal });
                }
                if !matches!(
                    self.source_kind(*payload)?,
                    SourceDeclarationKind::Struct | SourceDeclarationKind::Enum
                ) {
                    return Err(MirTypeBridgeError::GeneratedPayloadMismatch { nominal });
                }
            }
            (
                GeneratedNominalKey::CoroutineStep { result },
                MirTypeRepresentationV1::CoroutineStep { variants },
            ) => {
                self.require_source_subject(*result)?;
                self.generated_variants(
                    nominal,
                    role,
                    variants,
                    [
                        GeneratedEnumVariantRole::CoroutineStepCompleted,
                        GeneratedEnumVariantRole::CoroutineStepSuspended,
                    ],
                    0,
                    *result,
                )?;
            }
            (
                GeneratedNominalKey::CoroutineSlot { value },
                MirTypeRepresentationV1::CoroutineSlot { variants },
            ) => {
                self.require_source_subject(*value)?;
                self.generated_variants(
                    nominal,
                    role,
                    variants,
                    [
                        GeneratedEnumVariantRole::CoroutineSlotEmpty,
                        GeneratedEnumVariantRole::CoroutineSlotValue,
                    ],
                    1,
                    *value,
                )?;
            }
            (
                GeneratedNominalKey::ClosureEnvironment { .. }
                | GeneratedNominalKey::CallableAdapterEnvironment { .. }
                | GeneratedNominalKey::CoroutineFrame { .. }
                | GeneratedNominalKey::ContinuationAdapterEnvironment { .. },
                _,
            ) => {
                return Err(MirTypeBridgeError::GeneratedExecutionShapeGate { nominal });
            }
            _ => {
                return Err(MirTypeBridgeError::OriginRepresentationMismatch {
                    exact: record.exact(),
                });
            }
        }
        Ok(())
    }

    fn require_source_subject(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeError> {
        let key = self.identities.canonical_key::<_, ExactTypeKey>(exact)?;
        let ExactTypeKey::Nominal(nominal) = key.as_ref() else {
            return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
        };
        self.source_nominal(*nominal)?;
        Ok(())
    }

    fn generated_variants(
        &self,
        nominal: PersistentTypeId,
        owner: &GeneratedNominalKey,
        variants: &[MirRepresentationVariantV1],
        roles: [GeneratedEnumVariantRole; 2],
        payload_index: usize,
        payload: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeError> {
        let fail = || MirTypeBridgeError::GeneratedPayloadMismatch { nominal };
        if variants.len() != 2 {
            return Err(fail());
        }
        for (index, (variant, role)) in variants.iter().zip(roles).enumerate() {
            let key = EnumVariantIdentityKey::generated(owner, role).map_err(|_| fail())?;
            if PersistentEnumVariantId::from_key(&key).ok() != Some(variant.variant) {
                return Err(fail());
            }
            if index == payload_index {
                let [field] = variant.fields.as_slice() else {
                    return Err(fail());
                };
                let field_key = EnumVariantFieldKey::new(
                    variant.variant,
                    EnumVariantFieldSelector::Positional {
                        declaration_index: 0,
                    },
                );
                if field.value != payload
                    || PersistentEnumVariantFieldId::from_key(&field_key).ok() != Some(field.field)
                {
                    return Err(fail());
                }
            } else if !variant.fields.is_empty() || variant.gc != MirGcKindV1::GcFree {
                return Err(fail());
            }
        }
        Ok(())
    }

    pub(super) fn validate_object_backing(
        &self,
        object: PersistentTypeId,
        exact: PersistentExactTypeId,
    ) -> Result<(), MirTypeBridgeError> {
        let key = self.identities.canonical_key::<_, ExactTypeKey>(exact)?;
        let nominal =
            PersistentTypeId::from_generated_key(&GeneratedNominalKey::ObjectBackingClass {
                object,
            })
            .map_err(|_| MirTypeBridgeError::ExactOriginMismatch { exact })?;
        if key.as_ref() != &ExactTypeKey::Nominal(nominal) {
            return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
        }
        Ok(())
    }
}
