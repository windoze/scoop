use super::*;

#[derive(Debug, Default)]
pub struct DirectBuildOptions {
    pub optimization: scoop_lir::OptimizationMode,
    pub debug_info: scoop_lir::DebugInfoLevel,
    pub export_ir: scoop_hir::IrTemplateExport,
    pub instrumentation: scoop_profile::Instrumentation,
    pub profile_use: Option<PathBuf>,
    pub profile_use_required: bool,
    pub target: Option<String>,
    pub sysroot: Option<PathBuf>,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub generators: Vec<std::ffi::OsString>,
}

#[derive(Debug, Default)]
pub struct DirectCheckOptions {
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
    let path = operand.into();
    if path.file_name().is_some_and(|name| name == "Cone.toml")
        || (!path.extension().is_some_and(|ext| ext == "scoop") && path.is_dir())
    {
        return Err(BuildRequestNormalizationError::ManifestRequiresScoop);
    }
    let current = CurrentConeInput::SingleFile {
        source: SingleFileLocator::from_path(path)
            .map_err(BuildRequestNormalizationError::SingleFile)?,
    };
    let dependencies = explicit_dependencies(direct, support)?;
    let triple = match options.target.as_deref() {
        Some(triple) => triple,
        None => {
            scoop_toolchain::host_target_triple().map_err(BuildRequestNormalizationError::Target)?
        }
    };
    let sysroot = scoop_toolchain::configured_sysroot_root(options.sysroot)
        .map_err(BuildRequestNormalizationError::Target)?;
    let target = if matches!(output, SingleConeOutput::Check) {
        target::SelectedTarget::Frontend(
            scoop_toolchain::select_target(triple)
                .map_err(BuildRequestNormalizationError::Target)?,
        )
    } else {
        let tools = scoop_toolchain::CToolchainOptions {
            sysroot: Some(sysroot.clone()),
            ..options.c_toolchain
        };
        let target = ResolvedTargetProfile::resolve_with(triple, &tools)
            .map_err(BuildRequestNormalizationError::Target)?;
        scoop_codegen::ValidatedBackendProfile::from_selection(
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile().contract(),
        )
        .map_err(BuildRequestNormalizationError::Backend)?;
        target::SelectedTarget::Production(Box::new(target))
    };
    let trusted_core = TrustedCoreInput::DependenciesOrDefault {
        sysroot: sysroot.clone(),
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
    .map(|request| {
        request
            .with_native_sysroot(sysroot)
            .with_generators(generators)
            .with_optimization(options.optimization)
            .with_debug_info(options.debug_info)
            .with_export_ir(options.export_ir)
            .with_instrumentation(options.instrumentation)
            .with_profile_use(options.profile_use, options.profile_use_required)
    })
    .map_err(BuildRequestNormalizationError::Request)
}
