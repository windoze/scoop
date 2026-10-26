use super::*;

#[derive(Debug, Default)]
pub struct DirectBuildOptions {
    pub optimization: scoop_lir::OptimizationMode,
    pub purpose: CompilationPurpose,
    pub target: Option<String>,
    pub sysroot: Option<PathBuf>,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub generators: Vec<std::ffi::OsString>,
}

#[derive(Debug, Default)]
pub struct DirectCheckOptions {
    pub purpose: CompilationPurpose,
    pub target: Option<String>,
    pub sysroot: Option<PathBuf>,
    pub generators: Vec<std::ffi::OsString>,
}

pub fn normalize_direct_check_request(
    operand: impl Into<PathBuf>,
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
    diagnostics: DiagnosticOutputPolicy,
    options: DirectCheckOptions,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    normalize_direct(
        operand,
        direct,
        support,
        SingleConeOutput::Check,
        diagnostics,
        StageDumpPolicy::None,
        DirectBuildOptions {
            purpose: options.purpose,
            target: options.target,
            sysroot: options.sysroot,
            generators: options.generators,
            ..Default::default()
        },
    )
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
    let output =
        SlibOutputDestination::new(output).map_err(BuildRequestNormalizationError::Request)?;
    normalize_direct(
        operand,
        direct,
        support,
        output.into(),
        diagnostics,
        emit,
        options,
    )
}

fn normalize_direct(
    operand: impl Into<PathBuf>,
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
    output: SingleConeOutput,
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
    let triple = match options.target.as_deref() {
        Some(triple) => triple,
        None => {
            scoop_toolchain::host_target_triple().map_err(BuildRequestNormalizationError::Target)?
        }
    };
    let target = if matches!(output, SingleConeOutput::Check) {
        target::SelectedTarget::Frontend(
            scoop_toolchain::select_target(triple)
                .map_err(BuildRequestNormalizationError::Target)?,
        )
    } else {
        let target = ResolvedTargetProfile::resolve_with(triple, &options.c_toolchain)
            .map_err(BuildRequestNormalizationError::Target)?;
        scoop_codegen::ValidatedBackendProfile::from_selection(
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile().contract(),
        )
        .map_err(BuildRequestNormalizationError::Backend)?;
        target::SelectedTarget::Production(Box::new(target))
    };
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
    SingleConeBuildRequest::new_selected(
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
