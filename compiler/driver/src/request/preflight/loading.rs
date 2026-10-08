use super::*;

impl SingleConeBuildRequest {
    pub(super) fn load_preflight_inner(
        self,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        let Self {
            current,
            mut dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
            optimization,
            native_inputs,
        } = self;
        let current = load_current_input(current)?;
        if let TrustedCoreInput::Artifact(input) = &trusted_core {
            dependencies.direct.push(input.clone());
        }
        let mut loaded =
            LoadedExplicitDependencyInputs::load(dependencies.direct(), dependencies.support())
                .map_err(|source| {
                    SingleConePreflightError::ExplicitDependencyLoad(Box::new(source))
                })?;
        if let TrustedCoreInput::DependenciesOrDefault { sysroot } = trusted_core
            && !loaded
                .contains_explicit_core(target.lir_target_selection())
                .map_err(SingleConePreflightError::Dependencies)?
        {
            let slot = crate::trusted_core::resolve_trusted_core_slot_at(
                &sysroot,
                target.lir_target_selection(),
            )
            .map_err(|source| SingleConePreflightError::DefaultCoreSlot(Box::new(source)))?;
            let input = crate::HostArtifactLocator::new(slot.artifact())
                .map_err(|source| SingleConePreflightError::Request(Box::new(source)))?;
            super::super::output::validate_output_inputs(
                [(
                    crate::OutputAliasRole::TrustedCoreArtifact,
                    input.as_path().to_owned(),
                )],
                &output,
            )
            .map_err(|source| SingleConePreflightError::Request(Box::new(source)))?;
            loaded.append_direct(&input).map_err(|source| {
                SingleConePreflightError::ExplicitDependencyLoad(Box::new(source))
            })?;
        }
        Ok(LoadedSingleConeBuildRequest {
            current,
            dependencies: loaded,
            target,
            output,
            diagnostics,
            emit,
            optimization,
            native_inputs,
        })
    }
}
