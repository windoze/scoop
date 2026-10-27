use super::*;
use scoop_protocol::{
    CurrentConeRequestV1, ScoopcBuildRequestV1, StageDumpPolicyV1, TrustedCoreRequestV1,
};

mod direct;
pub use direct::{
    DirectBuildOptions, DirectCheckOptions, normalize_direct_build_request,
    normalize_direct_check_request,
};

pub fn normalize_protocol_build_request(
    build: &ScoopcBuildRequestV1,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    let current = match build.current() {
        CurrentConeRequestV1::ExpandedCone { config } => CurrentConeInput::ExpandedManifest {
            manifest: Box::new(
                scoop_manifest::LoadedConeManifest::from_expanded(
                    config
                        .root
                        .to_path_buf()
                        .map_err(BuildRequestNormalizationError::HostPath)?,
                    config
                        .manifest_path
                        .to_path_buf()
                        .map_err(BuildRequestNormalizationError::HostPath)?,
                    config.manifest_text.clone(),
                    config.manifest.clone(),
                    config.metadata.clone(),
                )
                .map_err(BuildRequestNormalizationError::ManifestRoot)?,
            ),
        },
        CurrentConeRequestV1::SingleFile { source } => CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(
                source
                    .to_path_buf()
                    .map_err(BuildRequestNormalizationError::HostPath)?,
            )
            .map_err(BuildRequestNormalizationError::SingleFile)?,
        },
    };
    let dependencies = explicit_dependencies(
        protocol_paths(build.direct_slibs())?,
        protocol_paths(build.support_slibs())?,
    )?;
    let output = protocol_output(build.output())?;
    if matches!(output, SingleConeOutput::Check) && !build.native_inputs().is_empty() {
        return Err(BuildRequestNormalizationError::Request(
            SingleConeBuildRequestError::CheckRequiresNoMachineInputs,
        ));
    }
    let target = if output.artifact().is_none() {
        target::SelectedTarget::Frontend(
            scoop_toolchain::select_target(build.target().canonical_triple())
                .map_err(BuildRequestNormalizationError::Target)?,
        )
    } else {
        let target = ResolvedTargetProfile::resolve_request(build.target())
            .map_err(BuildRequestNormalizationError::Target)?;
        scoop_codegen::ValidatedBackendProfile::from_selection(
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile().contract(),
        )
        .map_err(BuildRequestNormalizationError::Backend)?;
        target::SelectedTarget::Production(Box::new(target))
    };
    let trusted_core = match build.trusted_core() {
        TrustedCoreRequestV1::ArtifactSlot { artifact } => TrustedCoreInput::Artifact(
            HostArtifactLocator::new(
                artifact
                    .to_path_buf()
                    .map_err(BuildRequestNormalizationError::HostPath)?,
            )
            .map_err(BuildRequestNormalizationError::Request)?,
        ),
        TrustedCoreRequestV1::Bootstrap => TrustedCoreInput::BootstrapSelf,
    };

    SingleConeBuildRequest::new_selected(
        current,
        dependencies,
        trusted_core,
        target,
        output,
        map_diagnostic_policy(build.diagnostics()),
        map_dump_policy(build.emit()),
    )
    .map_err(BuildRequestNormalizationError::Request)
    .and_then(|request| {
        request
            .with_optimization(build.optimization())
            .with_generators(
                scoop_toolchain::GeneratorPrograms::from_registrations(build.generators())
                    .map_err(BuildRequestNormalizationError::Generators)?,
            )
            .with_native_inputs(protocol_paths(build.native_inputs())?)
            .with_purpose(build.purpose())
            .map_err(BuildRequestNormalizationError::Request)
    })
}

fn protocol_output(
    output: &scoop_protocol::ScoopcOutputRequestV1,
) -> Result<SingleConeOutput, BuildRequestNormalizationError> {
    use scoop_protocol::ScoopcOutputRequestV1 as O;
    let artifact = |path: &scoop_protocol::HostPathCarrier| {
        SlibOutputDestination::new(
            path.to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?,
        )
        .map_err(BuildRequestNormalizationError::Request)
    };
    let documentation = |path: &scoop_protocol::HostPathCarrier| {
        DocumentationOutputDestination::new(
            path.to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?,
        )
        .map_err(BuildRequestNormalizationError::Request)
    };
    Ok(match output {
        O::Check => SingleConeOutput::Check,
        O::Artifact(path) => SingleConeOutput::Artifact(artifact(path)?),
        O::Documentation(path) => SingleConeOutput::Documentation(documentation(path)?),
        O::ArtifactWithDocumentation {
            artifact: slib,
            documentation: doc,
        } => SingleConeOutput::ArtifactWithDocumentation {
            artifact: artifact(slib)?,
            documentation: documentation(doc)?,
        },
    })
}

fn explicit_dependencies(
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
) -> Result<ExplicitDependencyInputs, BuildRequestNormalizationError> {
    let direct = direct
        .into_iter()
        .map(HostArtifactLocator::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(BuildRequestNormalizationError::Request)?;
    let support = support
        .into_iter()
        .map(HostArtifactLocator::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(BuildRequestNormalizationError::Request)?;
    Ok(ExplicitDependencyInputs::new(direct, support))
}

fn protocol_paths(
    paths: &[scoop_protocol::HostPathCarrier],
) -> Result<Vec<PathBuf>, BuildRequestNormalizationError> {
    paths
        .iter()
        .map(|path| {
            path.to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)
        })
        .collect()
}

fn map_diagnostic_policy(
    policy: scoop_protocol::DiagnosticOutputPolicyV1,
) -> DiagnosticOutputPolicy {
    match policy {
        scoop_protocol::DiagnosticOutputPolicyV1::Human => DiagnosticOutputPolicy::Human,
        scoop_protocol::DiagnosticOutputPolicyV1::Structured => DiagnosticOutputPolicy::Structured,
    }
}

fn map_dump_policy(policy: &StageDumpPolicyV1) -> StageDumpPolicy {
    match policy {
        StageDumpPolicyV1::None => StageDumpPolicy::None,
        StageDumpPolicyV1::Files { stages, .. } => StageDumpPolicy::Stages(*stages),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_protocol::StageDumpSet;
    #[test]
    fn policy_projection_is_total() {
        assert_eq!(
            map_diagnostic_policy(scoop_protocol::DiagnosticOutputPolicyV1::Structured),
            DiagnosticOutputPolicy::Structured
        );
        for (wire, expected) in [
            (StageDumpKind::Ast, StageDumpKind::Ast),
            (StageDumpKind::Hir, StageDumpKind::Hir),
            (StageDumpKind::Mir, StageDumpKind::Mir),
            (StageDumpKind::Optimizations, StageDumpKind::Optimizations),
            (StageDumpKind::Lir, StageDumpKind::Lir),
            (StageDumpKind::Symbols, StageDumpKind::Symbols),
        ] {
            assert_eq!(
                map_dump_policy(&StageDumpPolicyV1::Files {
                    stages: StageDumpSet::one(wire),
                    directory: scoop_protocol::HostPathCarrier::from_path(std::path::Path::new(
                        "dump"
                    ))
                    .unwrap()
                }),
                StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(expected))
            );
        }
    }
}
