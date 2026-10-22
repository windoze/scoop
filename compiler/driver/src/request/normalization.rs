use super::*;
use scoop_protocol::{
    CurrentConeRequestV1, ScoopcBuildRequestV1, StageDumpPolicyV1, TrustedCoreRequestV1,
};

#[derive(Debug, Default)]
pub struct DirectBuildOptions {
    pub optimization: scoop_lir::OptimizationMode,
    pub purpose: CompilationPurpose,
    pub target: Option<String>,
    pub sysroot: Option<PathBuf>,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub generators: Vec<std::ffi::OsString>,
}

pub fn normalize_direct_build_request(
    operand: impl Into<PathBuf>,
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
    output: impl Into<PathBuf>,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
    options: DirectBuildOptions,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    let generators = if options.generators.is_empty() {
        scoop_toolchain::GeneratorPrograms::default()
    } else {
        let cwd = std::env::current_dir().map_err(|error| {
            BuildRequestNormalizationError::Generators(ToolchainError(format!(
                "cannot resolve generator paths against the current directory: {error}"
            )))
        })?;
        scoop_toolchain::GeneratorPrograms::from_cli(&options.generators, &cwd)
            .map_err(BuildRequestNormalizationError::Generators)?
    };
    let current = classify_current_cone_operand(operand)
        .map_err(BuildRequestNormalizationError::CurrentOperand)?;
    if options.purpose == CompilationPurpose::Test
        && matches!(current, CurrentConeInput::SingleFile { .. })
    {
        return Err(BuildRequestNormalizationError::Request(
            SingleConeBuildRequestError::TestRequiresManifest,
        ));
    }
    let dependencies = explicit_dependencies(direct, support)?;
    let output =
        SlibOutputDestination::new(output).map_err(BuildRequestNormalizationError::Request)?;
    let triple = match options.target.as_deref() {
        Some(triple) => triple,
        None => {
            scoop_toolchain::host_target_triple().map_err(BuildRequestNormalizationError::Target)?
        }
    };
    let target = ResolvedTargetProfile::resolve_with(triple, &options.c_toolchain)
        .map_err(BuildRequestNormalizationError::Target)?;
    scoop_codegen::ValidatedBackendProfile::from_selection(
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile().contract(),
    )
    .map_err(BuildRequestNormalizationError::Backend)?;
    let is_core = match &current {
        CurrentConeInput::Manifest { root } => {
            scoop_toolchain::load_cone_manifest(root)
                .map_err(BuildRequestNormalizationError::ManifestRoot)?
                .parsed()
                .semantic()
                .coordinate()
                == &scoop_identity::ConeCoordinate::reserved_core()
        }
        CurrentConeInput::SingleFile { .. } => false,
    };
    let sysroot = scoop_toolchain::configured_sysroot_root(options.sysroot)
        .map_err(BuildRequestNormalizationError::Target)?;
    let trusted_core = if is_core {
        TrustedCoreInput::BootstrapSelf
    } else {
        TrustedCoreInput::DependenciesOrDefault {
            sysroot: sysroot.clone(),
        }
    };
    SingleConeBuildRequest::new(
        current,
        dependencies,
        trusted_core,
        target,
        output,
        diagnostics,
        emit,
    )
    .and_then(|request| {
        request
            .with_native_sysroot(sysroot)
            .with_generators(generators)
            .with_optimization(options.optimization)
            .with_purpose(options.purpose)
    })
    .map_err(BuildRequestNormalizationError::Request)
}

pub fn normalize_protocol_build_request(
    build: &ScoopcBuildRequestV1,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    let current_path = match build.current() {
        CurrentConeRequestV1::ManifestRoot { root } => {
            let path = root
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            (true, path)
        }
        CurrentConeRequestV1::SingleFile { source } => {
            let path = source
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            (false, path)
        }
    };
    let dependencies = explicit_dependencies(
        protocol_paths(build.direct_slibs())?,
        protocol_paths(build.support_slibs())?,
    )?;
    let output = protocol_output(build.output())?;
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
    let (current, trusted_core) = match (current_path, build.trusted_core()) {
        ((true, path), TrustedCoreRequestV1::ArtifactSlot { artifact }) => {
            let root = ManifestRootLocator::from_path(path)
                .map_err(BuildRequestNormalizationError::ManifestRoot)?;
            let artifact = artifact
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            let input = HostArtifactLocator::new(&artifact)
                .map_err(BuildRequestNormalizationError::Request)?;
            (
                CurrentConeInput::Manifest { root },
                TrustedCoreInput::Artifact(input),
            )
        }
        ((false, path), TrustedCoreRequestV1::ArtifactSlot { artifact }) => {
            let source = SingleFileLocator::from_path(path)
                .map_err(BuildRequestNormalizationError::SingleFile)?;
            let artifact = artifact
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            let input = HostArtifactLocator::new(&artifact)
                .map_err(BuildRequestNormalizationError::Request)?;
            (
                CurrentConeInput::SingleFile { source },
                TrustedCoreInput::Artifact(input),
            )
        }
        ((true, path), TrustedCoreRequestV1::Bootstrap) => (
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::from_path(path)
                    .map_err(BuildRequestNormalizationError::ManifestRoot)?,
            },
            TrustedCoreInput::BootstrapSelf,
        ),
        _ => {
            return Err(BuildRequestNormalizationError::Request(
                SingleConeBuildRequestError::InvalidCurrentCoreCombination,
            ));
        }
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
            (StageDumpKind::Lir, StageDumpKind::Lir),
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
