use super::*;

/// Complete member-aware defined and undefined symbol closure for the exact
/// built-in object bytes whose registration leaves were fingerprinted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkSymbolVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(in crate::object_production) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(in crate::object_production) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(in crate::object_production) callable_registrations:
        VerifiedStrongCallableRegistrationSetV1,
    pub(in crate::object_production) type_registrations: VerifiedStrongTypeRegistrationSetV1,
    pub(in crate::object_production) immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
}

impl LinkSymbolVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
    }

    pub const fn type_registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.type_registrations
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }

    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<RegistrationDependencyFingerprintedProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registrations,
            type_registrations,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        } = self;
        let (callables, types, immortal_objects, static_storages, initializations) = {
            let candidates = production.scoop_lir_candidates();
            let stackmaps = safepoints.registrations().stackmaps().clone();
            let callable_bodies = compute_strong_callable_body_object_fingerprints_v1(
                callable_registrations,
                stackmaps,
                undefined_symbols.clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::CallableBodyFingerprints)?;
            let callables = compute_strong_callable_fingerprints_v1(
                callable_bodies,
                production.production().canonical_callable_definitions(),
            )
            .map_err(BuiltinObjectProductionError::CallableFingerprints)?;

            let types = compute_strong_type_fingerprints_v1(
                type_registrations,
                production.production().canonical_shape_definitions(),
                undefined_symbols.clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::TypeFingerprints)?;

            let immortal_object_definitions =
                compute_strong_immortal_object_definition_fingerprints_v1(
                    immortal_object_registration_objects,
                    undefined_symbols.clone(),
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::ImmortalObjectDefinitionFingerprints)?;
            let immortal_objects = compute_strong_immortal_object_fingerprints_v1(
                immortal_object_definitions,
                production.production().canonical_shape_definitions(),
            )
            .map_err(BuiltinObjectProductionError::ImmortalObjectFingerprints)?;

            let static_storage_definitions =
                compute_strong_static_storage_definition_fingerprints_v1(
                    static_storage_registration_objects,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::StaticStorageDefinitionFingerprints)?;
            let static_storage_shapes =
                compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
                    .map_err(BuiltinObjectProductionError::StaticStorageShapeFingerprints)?;
            let static_storages =
                compute_strong_static_storage_fingerprints_v1(static_storage_shapes)
                    .map_err(BuiltinObjectProductionError::StaticStorageFingerprints)?;

            let initialization_definitions =
                compute_strong_initialization_definition_fingerprints_v1(
                    initialization_registration_objects,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::InitializationDefinitionFingerprints)?;
            let initializations = compute_strong_initialization_fingerprints_v1(
                initialization_definitions,
                callables.body_objects(),
            )
            .map_err(BuiltinObjectProductionError::InitializationFingerprints)?;

            (
                callables,
                types,
                immortal_objects,
                static_storages,
                initializations,
            )
        };

        Ok(RegistrationDependencyFingerprintedProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
        })
    }
}

/// Complete strong-registration fingerprints after every object and semantic
/// dependency leaf has been closed against the same object proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationDependencyFingerprintedProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(in crate::object_production) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(in crate::object_production) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(in crate::object_production) callables: VerifiedStrongCallableFingerprintSetV1,
    pub(in crate::object_production) types: VerifiedStrongTypeFingerprintSetV1,
    pub(in crate::object_production) immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    pub(in crate::object_production) static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    pub(in crate::object_production) initializations: VerifiedStrongInitializationFingerprintSetV1,
}

impl RegistrationDependencyFingerprintedProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeFingerprintSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &VerifiedStrongImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &VerifiedStrongStaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationFingerprintSetV1 {
        &self.initializations
    }

    pub fn finalize_strong_objects(
        self,
    ) -> Result<FinalizedStrongObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
        } = self;
        let final_objects = {
            let candidates = production.scoop_lir_candidates();
            let patch_sites = safepoints.registrations().patch_sites().clone();
            let image = verify_cone_image_v1(
                patch_sites.clone(),
                production.production.image_plan().clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::ConeImage)?;
            let entry = verify_entry_production_v1(
                patch_sites,
                production.production.entry_plan().clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::EntryProduction)?;
            let registrations = patch_strong_registration_fingerprints_v1(
                safepoints,
                callables,
                types,
                immortal_objects,
                static_storages,
                initializations,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::RegistrationPatch)?;
            let compatibility = scoop_slib::CompatibilityRecord::new(
                production.target_selection,
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            )
            .map_err(BuiltinObjectProductionError::Compatibility)?;
            let image_fingerprint =
                compute_runtime_image_fingerprint_v1(image, registrations, compatibility)
                    .map_err(BuiltinObjectProductionError::RuntimeImageFingerprint)?;
            let runtime_images = patch_runtime_image_fingerprint_v1(image_fingerprint)
                .map_err(BuiltinObjectProductionError::RuntimeImagePatch)?;
            patch_entry_production_v1(runtime_images, entry)
                .map_err(BuiltinObjectProductionError::EntryPatch)?
        };

        Ok(FinalizedStrongObjectProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            final_objects,
        })
    }
}
