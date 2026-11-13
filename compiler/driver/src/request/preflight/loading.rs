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
            debug_info,
            export_ir,
            instrumentation,
            profile_use,
            profile_use_required,
            purpose,
            native_inputs,
            generators,
            jobs,
        } = self;
        if profile_use.is_some()
            && (optimization != scoop_lir::OptimizationMode::Release
                || instrumentation != scoop_profile::Instrumentation::Off)
        {
            return Err(SingleConePreflightError::Profile(
                "--profile-use requires the release profile without instrumentation".into(),
            ));
        }
        if profile_use_required && profile_use.is_none() {
            return Err(SingleConePreflightError::Profile(
                "--profile-use-required requires --profile-use".into(),
            ));
        }
        let current = load_current_input(current)?;
        if let TrustedCoreInput::Artifact(input) = &trusted_core {
            dependencies.direct.push(input.clone());
        }
        let mut loaded = LoadedExplicitDependencyInputs::load(
            dependencies.direct(),
            dependencies.support(),
            target.selection(),
        )
        .map_err(|source| SingleConePreflightError::ExplicitDependencyLoad(Box::new(source)))?;
        if let TrustedCoreInput::DependenciesOrDefault { sysroot } = trusted_core
            && !loaded
                .contains_explicit_core()
                .map_err(SingleConePreflightError::Dependencies)?
        {
            let slot =
                crate::trusted_core::resolve_trusted_core_slot_at(&sysroot, target.selection())
                    .map_err(|source| SingleConePreflightError::DefaultCoreSlot(Box::new(source)))?
                    .with_library_root(scoop_toolchain::system_library_root(
                        &sysroot,
                        target.selection().target(),
                        instrumentation,
                        profile_use.is_some(),
                    ));
            let input = crate::HostArtifactLocator::new(slot.artifact())
                .map_err(|source| SingleConePreflightError::Request(Box::new(source)))?;
            if let Some(output) = output.artifact() {
                super::super::output::validate_output_inputs(
                    [(
                        crate::OutputAliasRole::TrustedCoreArtifact,
                        input.as_path().to_owned(),
                    )],
                    output,
                )
                .map_err(|source| SingleConePreflightError::Request(Box::new(source)))?;
            }
            loaded
                .append_direct(&input, target.selection())
                .map_err(|source| {
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
            debug_info,
            export_ir,
            instrumentation,
            profile_use,
            profile_use_required,
            purpose,
            native_inputs,
            generators,
            jobs,
        })
    }
}
