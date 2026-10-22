use super::*;

pub(crate) struct FinalizedLayoutObjects {
    pub(crate) foundation: ConeLirFoundation,
    pub(crate) projection: slib::VerifiedSingleConeProductionCodeProjectionV2,
    pub(crate) members: Vec<SlibMember>,
}

impl PreparedLayoutObjects {
    pub(crate) fn finalize(
        self,
        undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
        cone: &ConeRecord,
        native_objects: Vec<SlibMember>,
    ) -> Result<FinalizedLayoutObjects, BuiltinObjectProductionError> {
        let objects = self.finalize_metadata(undefined)?;
        let mut members = final_members(
            &self.bindings.member_plan,
            &self.bindings.generated_c_bridge_members,
            objects.objects(),
        )?;
        members.extend(native_objects);
        members.sort_unstable_by_key(|member| member.record().id());
        let directory = members
            .iter()
            .map(|member| member.record().clone())
            .collect::<Vec<_>>();
        let objects = slib::build_code_link_object_members_v2(objects, &directory)
            .map_err(BuiltinObjectProductionError::CodeLinkObjects)?;
        let projection = slib::verify_cross_cone_layout_production_code_projection_v1(
            cone,
            self.production,
            objects,
        )
        .map_err(BuiltinObjectProductionError::ProductionCodeProjection)?;
        Ok(FinalizedLayoutObjects {
            foundation: self.foundation,
            projection,
            members,
        })
    }

    pub(crate) fn finalize_metadata(
        &self,
        undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    ) -> Result<slib::FinalRuntimeObjectsV2, BuiltinObjectProductionError> {
        let image = bind_cone_image_v1(&self.builtins, self.production.image_plan().clone())
            .map_err(BuiltinObjectProductionError::ConeImage)?;
        let entry =
            bind_entry_production_v1(self.builtins.clone(), self.production.entry_plan().clone())
                .map_err(BuiltinObjectProductionError::EntryProduction)?;
        let registrations = registrations::finalize(self, undefined)?;
        let compatibility = slib::CompatibilityRecord::new(
            self.target_selection,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        )
        .map_err(BuiltinObjectProductionError::Compatibility)?;
        let image = slib::compute_runtime_image_fingerprint_v2(image, registrations, compatibility)
            .map_err(BuiltinObjectProductionError::RuntimeImageFingerprint)?;
        Ok(slib::FinalRuntimeObjectsV2::new(image, entry))
    }
}
