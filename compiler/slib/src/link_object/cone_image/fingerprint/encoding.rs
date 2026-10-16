//! Encode the six runtime tables from the newly produced registrations.

use super::*;

pub(super) struct RuntimeImageFingerprintInputV1<'proof, D, C, I> {
    pub(super) image: &'proof ConeImageDataV1,
    pub(super) registrations: &'proof VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    pub(super) runtime_abi: RuntimeAbiFingerprint,
    pub(super) target_profile: TargetProfileFingerprint,
}

impl<D, C, I> RuntimeImageFingerprintInputV1<'_, D, C, I>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    pub(super) fn records(&self) -> Result<crate::RuntimeImageRecords, RuntimeEncodeError> {
        let mut encoder = RuntimeEncoder::new();
        encoder.fixed(self.runtime_abi.as_array())?;
        encoder.fixed(self.target_profile.as_array())?;
        self.image.plan().cone().runtime_encode(&mut encoder)?;
        encoder.sequence_length(self.image.plan().dependencies().len())?;
        for dependency in self.image.plan().dependencies() {
            encoder.fixed(dependency.as_array())?;
        }
        let mut records = crate::RuntimeImageRecords::new(encoder.into_bytes());
        self.encode_static_storages(&mut records)?;
        self.encode_immortal_objects(&mut records)?;
        self.encode_initializations(&mut records)?;
        self.encode_types(&mut records)?;
        self.encode_safepoints(&mut records)?;
        self.encode_callables(&mut records)?;
        Ok(records)
    }

    fn encode_static_storages(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.static_storages();
        let plans = fingerprints.shapes().registrations().plan().registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            runtime_encode_strong_static_storage_record_v1(
                &mut encoder,
                plan,
                fingerprint.layout(),
                fingerprint.scan(),
            )?;
            output.push(0, plan.registration_definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }

    fn encode_immortal_objects(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.immortal_objects();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, _fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            runtime_encode_strong_immortal_object_record_v1(
                &mut encoder,
                plan.object(),
                plan.object_size(),
                plan.required_alignment(),
                plan.type_registration(),
                plan.definition_owner(),
            )?;
            output.push(1, plan.registration_definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }

    fn encode_initializations(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.initializations();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            let gateway = fingerprint
                .gateway_body()
                .zip(fingerprint.gateway_definition());
            runtime_encode_strong_initialization_record_v1(&mut encoder, plan, gateway)?;
            output.push(2, plan.registration_definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }

    fn encode_types(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.types();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            runtime_encode_type_record_v1(
                &mut encoder,
                plan,
                fingerprint.descriptor_definition().as_array(),
                fingerprint.layout().as_array(),
            )?;
            output.push(3, plan.definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }

    fn encode_safepoints(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.safepoints();
        let plans = fingerprints.registrations().plan().registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            runtime_encode_safepoint_record_v1(
                &mut encoder,
                *plan,
                fingerprint.stackmap().as_array(),
            )?;
            output.push(4, plan.definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }

    fn encode_callables(
        &self,
        output: &mut crate::RuntimeImageRecords,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.callables();
        let plans = fingerprints
            .body_objects()
            .registrations()
            .plan()
            .registrations();
        let records = plans.iter().zip(fingerprints.fingerprints());
        for (plan, fingerprint) in records {
            let mut encoder = RuntimeEncoder::new();
            runtime_encode_callable_record_v1(
                &mut encoder,
                *plan,
                fingerprint.body_definition().as_array(),
            )?;
            output.push(5, plan.definition_plan(), encoder.into_bytes());
        }
        Ok(())
    }
}
