//! Object envelope, relocation, digest-site, and stackmap verification.

use super::*;

/// Planned bytes paired with the generated-C production/envelope proof that
/// authorizes the remaining built-in object verifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeEnvelopeVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) proof: VerifiedCBridgeProductionEnvelopeSetV1,
}

impl CBridgeEnvelopeVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn proof(&self) -> &VerifiedCBridgeProductionEnvelopeSetV1 {
        &self.proof
    }

    pub fn verify_strong_relocations(
        self,
    ) -> Result<StrongRelocationVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            proof: c_bridge_proof,
        } = self;
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            production.target(),
            production.production.canonical_definitions(),
            &production.member_plan,
        )
        .map_err(BuiltinObjectProductionError::StrongSymbolPlan)?;
        let proof = {
            let scoop_lir_candidates = production.scoop_lir_candidates();
            let c_bridge_candidates = production.c_bridge_candidates();
            verify_builtin_object_strong_relocations_v1(
                &production.member_plan,
                &symbol_plan,
                &scoop_lir_candidates,
                c_bridge_proof,
                &c_bridge_candidates,
            )
            .map_err(BuiltinObjectProductionError::StrongRelocations)?
        };
        Ok(StrongRelocationVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            proof,
        })
    }
}

/// Unified proof that all provisional built-in members satisfy their exact
/// strong symbol, atom-range, and relocation plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRelocationVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) proof: VerifiedBuiltinObjectStrongRelocationSetV1,
}

impl StrongRelocationVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn proof(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.proof
    }

    pub fn verify_digest_patch_sites(
        self,
    ) -> Result<DigestPatchVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            proof: strong_relocations,
        } = self;
        let proof = {
            let candidates = production.scoop_lir_candidates();
            verify_scoop_lir_digest_patch_sites_v1(
                strong_relocations,
                &production.foundation,
                production.production.digest_finalization_plan().clone(),
                &candidates,
                &production.digest_patches,
            )
            .map_err(BuiltinObjectProductionError::DigestPatchSites)?
        };
        Ok(DigestPatchVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            proof,
        })
    }
}

/// Proof that every planned digest intent owns one in-atom,
/// relocation-free, provisionally zero Scoop object slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestPatchVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) proof: VerifiedScoopLirDigestPatchSiteSetV1,
}

impl DigestPatchVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn proof(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.proof
    }

    pub fn verify_stackmaps(
        self,
    ) -> Result<StackmapVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            proof: digest_patch_sites,
        } = self;
        let stackmaps = {
            let candidates = production.scoop_lir_candidates();
            let semantic_plan = production
                .production
                .registration_production()
                .safepoint_semantics();
            verify_scoop_lir_stackmaps_v1(
                digest_patch_sites.builtins().clone(),
                semantic_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::Stackmaps)?
        };
        Ok(StackmapVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            digest_patch_sites,
            stackmaps,
        })
    }
}

/// Complete normalized stackmap proof paired with the digest-site proof from
/// the same provisional Scoop object bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackmapVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) digest_patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    pub(in crate::object_production) stackmaps: VerifiedScoopLirStackmapSetV1,
}

impl StackmapVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn digest_patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.digest_patch_sites
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        &self.stackmaps
    }

    pub fn verify_registration_objects(
        self,
    ) -> Result<RegistrationObjectVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            digest_patch_sites,
            stackmaps,
        } = self;
        let registration_production = production.production.registration_production();
        let safepoint_plan = registration_production.safepoints().clone();
        let callable_plan = registration_production.callables().clone();
        let type_plan = registration_production.types().clone();
        let immortal_object_plan = registration_production.immortal_objects().clone();
        let static_storage_plan = registration_production.static_storages().clone();
        let initialization_plan = registration_production.initialization_units().clone();

        let (
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        ) = {
            let candidates = production.scoop_lir_candidates();
            let safepoint_registrations = verify_strong_safepoint_registrations_v1(
                stackmaps,
                digest_patch_sites.clone(),
                safepoint_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::SafepointRegistrations)?;
            let callable_registrations = verify_strong_callable_registrations_v1(
                digest_patch_sites.clone(),
                callable_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::CallableRegistrations)?;
            let type_registrations = verify_strong_type_registrations_v1(
                digest_patch_sites.clone(),
                type_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::TypeRegistrations)?;
            let immortal_object_registrations = verify_strong_immortal_object_registrations_v1(
                digest_patch_sites.clone(),
                immortal_object_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::ImmortalObjectRegistrations)?;
            let static_storage_registrations = verify_strong_static_storage_registrations_v1(
                digest_patch_sites.clone(),
                static_storage_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::StaticStorageRegistrations)?;
            let initialization_registrations = verify_strong_initialization_registrations_v1(
                digest_patch_sites,
                initialization_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::InitializationRegistrations)?;
            (
                safepoint_registrations,
                callable_registrations,
                type_registrations,
                immortal_object_registrations,
                static_storage_registrations,
                initialization_registrations,
            )
        };

        Ok(RegistrationObjectVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        })
    }
}
