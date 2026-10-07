//! Direct access to the registration records in a complete production section.

use super::*;

impl<D, C, I> ConeProductionSection<D, C, I> {
    /// Project the final code's roots and private GC/EH definitions into production.
    pub fn finalize_codegen(
        &mut self,
        foundation: &ConeLirFoundation,
        counts: &std::collections::BTreeMap<crate::SafepointId, u32>,
        removed_body_atoms: &std::collections::BTreeSet<crate::ObjectDefinitionAtomId>,
    ) -> Result<ConeLirFoundation, crate::StrongSafepointRegistrationPlanBuildError> {
        use std::collections::BTreeSet;
        let removed = self
            .registration_production
            .safepoints()
            .registrations()
            .iter()
            .filter(|plan| !counts.contains_key(&plan.safepoint()))
            .copied()
            .collect::<Vec<_>>();
        let sites = removed
            .iter()
            .map(|plan| plan.site())
            .collect::<BTreeSet<_>>();
        let definitions = removed
            .iter()
            .map(|plan| plan.definition_plan())
            .collect::<BTreeSet<_>>();
        let nodes = removed
            .iter()
            .map(|plan| plan.normalized_stackmap_fingerprint_node())
            .collect();
        let mut atoms = removed_body_atoms.clone();
        for definition in &definitions {
            atoms.extend(
                self.canonical_definitions
                    .plan(*definition)
                    .expect("registration definitions are in the production surface")
                    .atom_boundaries()
                    .iter()
                    .map(|boundary| boundary.atom()),
            );
        }
        self.registration_production
            .set_emitted_root_counts(counts)?;
        let retained = self
            .registration_production
            .safepoints()
            .registrations()
            .iter()
            .map(|plan| plan.site())
            .collect();
        self.image_plan.retain_safepoints(&retained);
        self.digest_finalization_plan
            .remove_safepoint_records(&nodes);
        self.canonical_definitions
            .remove_codegen_records(&definitions, &atoms);
        self.object_definition_plans
            .remove_codegen_records(&definitions, &atoms);
        Ok(foundation.without_codegen_records(&sites, &definitions, &atoms))
    }
}

impl ConeProductionSectionV2 {
    pub fn provider(&self) -> ConeIdentity {
        self.type_registrations().producer()
    }

    pub fn registration_identities(&self) -> &crate::RegistrationIdentitySurfaceV1 {
        self.registration_production().identities()
    }

    pub fn type_registrations(&self) -> &crate::StrongTypeRegistrationPlanSetV2 {
        self.registration_production().types()
    }

    pub fn callable_registrations(&self) -> &crate::StrongCallableRegistrationPlanSetV1 {
        self.registration_production().callables()
    }

    pub fn safepoint_registrations(&self) -> &crate::StrongSafepointRegistrationPlanSetV1 {
        self.registration_production().safepoints()
    }

    pub fn safepoint_semantics(&self) -> crate::StrongSafepointSemanticPlanSetV1 {
        self.registration_production().safepoint_semantics()
    }

    pub fn immortal_registrations(&self) -> &crate::StrongImmortalObjectRegistrationPlanSetV1 {
        self.registration_production().immortal_objects()
    }

    pub fn static_storage_registrations(&self) -> &crate::StrongStaticStorageRegistrationPlanSetV1 {
        self.registration_production().static_storages()
    }

    pub fn initialization_registrations(
        &self,
    ) -> &crate::StrongInitializationUnitRegistrationPlanSetV2 {
        self.registration_production().initialization_units()
    }
}
