//! Canonical image contents, optionally projected onto selected definitions.

use super::*;

pub(super) struct RuntimeImageFingerprintInputV1<'proof, D, C, I> {
    pub(super) image: &'proof VerifiedConeImageV1,
    pub(super) registrations: &'proof VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    pub(super) runtime_abi: RuntimeAbiFingerprint,
    pub(super) target_profile: TargetProfileFingerprint,
    pub(super) selected: Option<&'proof BTreeSet<scoop_identity::ObjectDefinitionPlanId>>,
}

impl<D, C, I> RuntimeEncode for RuntimeImageFingerprintInputV1<'_, D, C, I>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(self.runtime_abi.as_array())?;
        encoder.fixed(self.target_profile.as_array())?;
        self.image.plan().cone().runtime_encode(encoder)?;
        encoder.sequence_length(self.image.plan().dependencies().len())?;
        for dependency in self.image.plan().dependencies() {
            encoder.fixed(dependency.as_array())?;
        }
        self.encode_static_storages(encoder)?;
        self.encode_immortal_objects(encoder)?;
        self.encode_initializations(encoder)?;
        self.encode_types(encoder)?;
        self.encode_safepoints(encoder)?;
        self.encode_callables(encoder)
    }
}

impl<D, C, I> RuntimeImageFingerprintInputV1<'_, D, C, I>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    fn encode_static_storages(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.static_storages();
        let plans = fingerprints.shapes().registrations().plan().registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.registration_definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, fingerprint) in records {
            runtime_encode_strong_static_storage_record_v1(
                encoder,
                plan,
                fingerprint.layout(),
                fingerprint.scan(),
            )?;
        }
        Ok(())
    }

    fn encode_immortal_objects(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.immortal_objects();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.registration_definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, _fingerprint) in records {
            runtime_encode_strong_immortal_object_record_v1(
                encoder,
                plan.object(),
                plan.object_size(),
                plan.required_alignment(),
                plan.type_registration(),
                plan.definition_owner(),
            )?;
        }
        Ok(())
    }

    fn encode_initializations(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.initializations();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.registration_definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, fingerprint) in records {
            let gateway = fingerprint
                .gateway_body()
                .zip(fingerprint.gateway_definition());
            runtime_encode_strong_initialization_record_v1(encoder, plan, gateway)?;
        }
        Ok(())
    }

    fn encode_types(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.types();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, fingerprint) in records {
            runtime_encode_type_record_v1(
                encoder,
                plan,
                fingerprint.descriptor_definition().as_array(),
                fingerprint.layout().as_array(),
            )?;
        }
        Ok(())
    }

    fn encode_safepoints(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.safepoints();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, fingerprint) in records {
            runtime_encode_safepoint_record_v1(encoder, *plan, fingerprint.stackmap().as_array())?;
        }
        Ok(())
    }

    fn encode_callables(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.callables();
        let plans = fingerprints
            .body_objects()
            .registrations()
            .plan()
            .registrations();
        let records = plans
            .iter()
            .zip(fingerprints.fingerprints())
            .filter(|(plan, _)| {
                self.selected
                    .is_none_or(|selected| selected.contains(&plan.definition_plan()))
            });
        encoder.sequence_length(records.clone().count())?;
        for (plan, fingerprint) in records {
            runtime_encode_callable_record_v1(
                encoder,
                *plan,
                fingerprint.body_definition().as_array(),
            )?;
        }
        Ok(())
    }
}
