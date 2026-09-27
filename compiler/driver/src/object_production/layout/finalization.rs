use super::*;

pub(crate) struct FinalizedLayoutObjects {
    pub(crate) target_selection: ValidatedLirTargetSelection,
    pub(crate) foundation: ConeLirFoundation,
    pub(crate) projection: slib::VerifiedSingleConeProductionCodeProjectionV2,
    pub(crate) members: Vec<SlibMember>,
}

impl PreparedLayoutObjects {
    pub(crate) fn finalize(
        self,
        undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
        cone: &ConeRecord,
        dependencies: &[DependencyRecord],
        source_count: usize,
    ) -> Result<FinalizedLayoutObjects, BuiltinObjectProductionError> {
        let objects = self.finalize_metadata(undefined)?;
        let members = final_members(
            &self.bindings.member_plan,
            &self.bindings.generated_c_bridge_members,
            objects.objects(),
        )?;
        let directory = members
            .iter()
            .map(|member| member.record().clone())
            .collect::<Vec<_>>();
        let objects = slib::verify_code_link_object_members_v2(objects, &directory)
            .map_err(BuiltinObjectProductionError::CodeLinkObjects)?;
        let projection = slib::verify_cross_cone_layout_production_code_projection_v1(
            cone,
            dependencies,
            source_count,
            self.production,
            objects,
        )
        .map_err(BuiltinObjectProductionError::ProductionCodeProjection)?;
        Ok(FinalizedLayoutObjects {
            target_selection: self.target_selection,
            foundation: self.foundation,
            projection,
            members,
        })
    }

    pub(crate) fn finalize_metadata(
        &self,
        undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    ) -> Result<slib::VerifiedEntryPatchSetV2, BuiltinObjectProductionError> {
        let candidates = self.candidates();
        let image = verify_cone_image_v1(
            self.patch_sites.clone(),
            self.production.image_plan().clone(),
            &candidates,
        )
        .map_err(BuiltinObjectProductionError::ConeImage)?;
        let entry = verify_entry_production_v1(
            self.patch_sites.clone(),
            self.production.entry_plan().clone(),
            &candidates,
        )
        .map_err(BuiltinObjectProductionError::EntryProduction)?;
        let registrations = registrations::finalize(self, undefined)?;
        let compatibility = slib::CompatibilityRecord::new(
            self.target_selection,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        )
        .map_err(BuiltinObjectProductionError::Compatibility)?;
        let image = slib::compute_runtime_image_fingerprint_v2(image, registrations, compatibility)
            .map_err(BuiltinObjectProductionError::RuntimeImageFingerprint)?;
        let image = slib::patch_runtime_image_fingerprint_v2(image)
            .map_err(BuiltinObjectProductionError::RuntimeImagePatch)?;
        slib::patch_entry_production_v2(image, entry)
            .map_err(BuiltinObjectProductionError::EntryPatch)
    }
}
