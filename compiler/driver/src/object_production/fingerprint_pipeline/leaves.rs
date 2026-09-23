use super::*;

/// All six registration-object leaf families fingerprinted from one exact
/// provisional Scoop object set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectLeafFingerprintedProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(in crate::object_production) callable_registration_objects:
        VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) type_registration_objects:
        VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    pub(in crate::object_production) initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
}

impl RegistrationObjectLeafFingerprintedProductionV1 {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::object_production) const fn from_verified_leaves(
        production: PlannedBuiltinObjectProductionV1,
        symbol_plan: PlannedStrongObjectSymbolSetV1,
        safepoints: VerifiedStrongSafepointFingerprintSetV1,
        callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
        type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
        immortal_object_registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
        static_storage_registration_objects: VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
        initialization_registration_objects: VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    ) -> Self {
        Self {
            production,
            symbol_plan,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        }
    }

    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
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

    pub fn verify_link_symbol_requirements(
        self,
        dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    ) -> Result<LinkSymbolVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        } = self;
        let patch_sites = callable_registration_objects
            .registrations()
            .patch_sites()
            .clone();
        let strong_closure = patch_sites.builtins().strong_relocations().clone();
        let defined_symbols =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong_closure)
                .map_err(BuiltinObjectProductionError::DefinedSymbols)?;
        let bridge_plan = production.production.generated_bridge_plan().clone();
        let current_cone = verify_current_cone_undefined_requirements_v1(
            strong_closure.clone(),
            bridge_plan.clone(),
        )
        .map_err(BuiltinObjectProductionError::CurrentConeRequirements)?;
        let native_requirements =
            scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
                production.target(),
                &production.foundation,
            )
            .map_err(BuiltinObjectProductionError::NativeRequirementSurface)?;
        let dependencies = verify_dependency_strong_requirements_v1(
            production.target(),
            strong_closure,
            production.production.external_bridges().clone(),
            dependency_owners,
        )
        .map_err(BuiltinObjectProductionError::DependencyRequirements)?;
        let source =
            verify_source_external_requirements_v1(dependencies, native_requirements.clone())
                .map_err(BuiltinObjectProductionError::SourceExternalRequirements)?;
        let runtime_and_eh =
            verify_runtime_and_eh_requirements_v1(source, production.target_selection)
                .map_err(BuiltinObjectProductionError::RuntimeAndEhRequirements)?;
        let bridge_semantics = verify_generated_c_bridge_semantics_v1(
            patch_sites,
            bridge_plan,
            native_requirements,
            &production.c_bridge_profile,
        )
        .map_err(BuiltinObjectProductionError::GeneratedBridgeSemantics)?;
        let external =
            verify_c_bridge_target_support_requirements_v1(runtime_and_eh, bridge_semantics)
                .map_err(BuiltinObjectProductionError::CBridgeTargetSupportRequirements)?;
        let external = seal_builtin_object_external_requirements_v1(external)
            .map_err(BuiltinObjectProductionError::UnclassifiedExternalRequirement)?;
        let undefined_symbols = finalize_undefined_symbol_requirements_v1(current_cone, external)
            .map_err(BuiltinObjectProductionError::UndefinedSymbols)?;

        Ok(LinkSymbolVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        })
    }
}
