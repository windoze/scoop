use super::*;

/// Final Scoop object bytes after every strong registration, runtime-image,
/// and entry digest write has been applied and revalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedStrongObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(in crate::object_production) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(in crate::object_production) final_objects: VerifiedEntryPatchSetV1,
}

impl FinalizedStrongObjectProductionV1 {
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

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV1 {
        &self.final_objects
    }

    pub fn fingerprint_code(
        self,
        cone: &ConeRecord,
        direct_dependencies: &[DependencyRecord],
        source_count: usize,
    ) -> Result<CodeFingerprintedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan: _,
            defined_symbols,
            undefined_symbols,
            final_objects,
        } = self;
        let members = final_link_object_members(&production, &final_objects)?;
        let directory = members
            .iter()
            .map(|member| member.record().clone())
            .collect::<Vec<_>>();
        let link_objects = verify_code_link_object_members_v1(final_objects, &directory)
            .map_err(BuiltinObjectProductionError::CodeLinkObjects)?;
        let code_projection = verify_single_cone_production_code_projection_v1(
            cone,
            direct_dependencies,
            source_count,
            production.production.clone(),
            link_objects,
        )
        .map_err(BuiltinObjectProductionError::ProductionCodeProjection)?;
        let native_requirements =
            scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
                production.target(),
                &production.foundation,
            )
            .map_err(BuiltinObjectProductionError::NativeRequirementSurface)?;
        let code = compute_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols,
            undefined_symbols,
        )
        .map_err(BuiltinObjectProductionError::CodeFingerprint)?;
        let production_manifest = SingleConeProductionManifestV1::from_verified_code(code);

        Ok(CodeFingerprintedObjectProductionV1 {
            target_selection: production.target_selection,
            lir_foundation: production.foundation,
            c_bridge_profile: production.c_bridge_profile,
            members,
            production_manifest,
        })
    }
}

pub(in crate::object_production) fn final_link_object_members(
    production: &PlannedBuiltinObjectProductionV1,
    final_objects: &VerifiedEntryPatchSetV1,
) -> Result<Vec<SlibMember>, BuiltinObjectProductionError> {
    final_members(
        &production.member_plan,
        &production.generated_c_bridge_members,
        final_objects.objects(),
    )
}

/// Final LinkObject members plus the unique Code/production-manifest proof.
/// Provisional object bytes and their earlier state-machine proofs are no
/// longer retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeFingerprintedObjectProductionV1 {
    pub(in crate::object_production) target_selection: ValidatedLirTargetSelection,
    pub(in crate::object_production) lir_foundation: ConeLirFoundation,
    pub(in crate::object_production) c_bridge_profile: CBridgeToolchainProfileV1,
    pub(in crate::object_production) members: Vec<SlibMember>,
    pub(in crate::object_production) production_manifest: SingleConeProductionManifestV1,
}

impl CodeFingerprintedObjectProductionV1 {
    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.lir_foundation
    }

    pub const fn c_bridge_profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.c_bridge_profile
    }

    pub fn members(&self) -> &[SlibMember] {
        &self.members
    }

    pub const fn production_manifest(&self) -> &SingleConeProductionManifestV1 {
        &self.production_manifest
    }
}
