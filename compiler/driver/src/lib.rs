//! Compiler driver library: pipeline orchestration entry points.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8 and
//! `docs/milestone4/DESIGN.md` section 1 for the sysroot model: the
//! compilation unit is every `src/*.scoop` of the sysroot's `scoop.core`
//! Cone followed by the user file, all compiled together.

use scoop_ast::{Diagnostic, DiagnosticSeverity};
use std::path::{Path, PathBuf};

mod inputs;
mod linking;
mod object_production;
mod request;
mod trusted_core;

pub use inputs::{load_inputs, render_diagnostics};
use linking::{LinkRequest, build_runtime, compile_c_bridge, link};
pub use object_production::{
    PlannedScoopLirObjectInputV1, PlannedScoopLirObjectProductionV1, ScoopLirObjectProductionError,
};
pub use request::{
    BuildRequestNormalizationError, CoreBootstrapHirInputError, CoreBootstrapHirStageError,
    CoreBootstrapLirStageError, CoreBootstrapMirStageError, CoreOnlyRequestValidationError,
    CurrentConeInput, CurrentConeOperandError, CurrentConeOperandErrorKind,
    CurrentConeSourceStageError, DiagnosticOutputPolicy, ExplicitDependencyInputs,
    HostArtifactLocator, LoadedCurrentConeInput, LoadedSingleConeBuildRequest,
    LoadedTrustedCoreInput, NonCoreDependencyInput, OrdinaryCoreOnlyHirInputError, OutputAliasRole,
    OutputIsolationErrorKind, ParsedCoreBootstrapBuildRequest, ParsedOrdinaryConeBuildRequest,
    ParsedSingleConeBuildRequest, SingleConeBuildRequest, SingleConeBuildRequestError,
    SingleConePreflightError, SlibOutputDestination, StageDumpKind, StageDumpPolicy,
    TrustedCoreBootstrapHirInput, TrustedCoreBootstrapHirOutput, TrustedCoreBootstrapLirOutput,
    TrustedCoreBootstrapMirOutput, TrustedCoreInput, ValidatedCoreOnlyBuildRequest,
    ValidatedCurrentConeInput, ValidatedExplicitDependencyInputSet, classify_current_cone_operand,
    normalize_direct_build_request, normalize_protocol_build_request,
};
pub use trusted_core::{
    CoreBootstrapAuthority, LoadedTrustedCoreArtifact, TrustedCoreArtifactAuthority,
    TrustedCoreArtifactAuthorityError, TrustedCoreArtifactInput, TrustedCoreArtifactInputError,
    TrustedCoreArtifactLoadError, TrustedCoreArtifactLoadOperation, TrustedCoreArtifactSlot,
    TrustedCoreArtifactValidationError, TrustedCoreArtifactView, TrustedCoreBootstrapInput,
    TrustedCoreCallableProjectionError, TrustedCoreSlot, TrustedCoreSlotError,
    TrustedCoreSlotErrorKind, TrustedCoreSlotIoOperation, TrustedCoreSourceSlot,
    ValidatedTrustedCoreArtifact, resolve_trusted_core_slot,
};

/// Text dumps of every pipeline stage, for golden-dump testing and
/// `scoopc build --emit`.
pub struct StageDumps {
    pub ast: String,
    pub hir: String,
    pub mir: String,
    pub lir: String,
}

/// Result of a successful [`compile_file`] run.
pub struct CompileSuccess {
    pub dumps: StageDumps,
    /// Path to the linked executable.
    pub binary: PathBuf,
    /// Non-fatal source diagnostics. Their presence does not change success.
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Default)]
pub struct CompileOptions {
    pub library_paths: Vec<PathBuf>,
    /// Internal compiler/test authority switch. It is deliberately absent
    /// from the stable CLI, environment, manifest, and source language.
    pub intrinsic_declaration_policy: scoop_hir_lower::IntrinsicDeclarationPolicy,
}

/// One input file of the compilation unit: a display name (for
/// diagnostics) plus the source text.
pub struct SourceFileInput {
    /// Display name used when rendering diagnostics. The compiler never derives a
    /// package or visibility key from it.
    pub name: String,
    pub source: String,
    pub identity: scoop_identity::SourceIdentity,
}

/// Compile one source file through the full pipeline: locate the sysroot,
/// load the `scoop.core` Cone sources, then parse → HIR → MIR → LIR →
/// object file → link with the C runtime.
///
/// All failures — malformed sysroot, unreadable file, stage diagnostics,
/// codegen or linker errors — are reported as diagnostics. Driver-level
/// failures lack a span; stage diagnostics keep their original spans and
/// carry the index of their input file (core files first, the user file
/// last). Render them via [`render_diagnostics`] with [`load_inputs`].
pub fn compile_file(path: &Path, out_dir: &Path) -> Result<CompileSuccess, Vec<Diagnostic>> {
    compile_file_with_options(path, out_dir, &CompileOptions::default())
}

pub fn compile_file_with_options(
    path: &Path,
    out_dir: &Path,
    options: &CompileOptions,
) -> Result<CompileSuccess, Vec<Diagnostic>> {
    let target_profile = scoop_codegen::ResolvedTargetProfile::resolve_host()
        .map_err(|error| vec![no_span(0, format!("target configuration failed: {error}"))])?;
    let inputs = load_inputs(path)?;
    let user_index = inputs.len() - 1;

    // M22 core and the current unit still enter one lowering request. The
    // current unit crosses the atomic, identity-typed source-set boundary even
    // though this driver currently supplies exactly one user source.
    let mut core_files = Vec::with_capacity(user_index);
    let mut diagnostics = Vec::new();
    for (index, input) in inputs[..user_index].iter().enumerate() {
        match scoop_parser::parse(&input.source) {
            Ok(file) => core_files.push(file),
            Err(mut parse_diagnostics) => {
                for diagnostic in &mut parse_diagnostics {
                    diagnostic.reattribute_single_source(index);
                }
                diagnostics.extend(parse_diagnostics);
            }
        }
    }
    let user_sources = scoop_parser::parse_all(scoop_ast::NonEmptyVec::new(
        scoop_parser::IdentifiedSourceInput::new(
            &inputs[user_index].identity,
            &inputs[user_index].source,
        ),
        Vec::new(),
    ));
    let user_sources = match user_sources {
        Ok(sources) => Some(sources),
        Err(errors) => {
            diagnostics.extend(errors.into_iter().map(|error| {
                let mut diagnostic = error.into_diagnostic();
                diagnostic.reattribute_single_source(user_index);
                diagnostic
            }));
            None
        }
    };
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let user_sources = user_sources.expect("an absent parsed source always produced diagnostics");
    let ast_dump = core_files
        .iter()
        .map(scoop_ast::dump)
        .chain(
            user_sources
                .sources()
                .iter()
                .map(|source| scoop_ast::dump(source.ast())),
        )
        .collect::<String>();

    // HIR lowering consumes the whole compilation unit at once; diagnostics
    // already carry the index into the core-then-user input order.
    let core_provider = scoop_hir::IntrinsicProviderId::from_raw(0);
    let user_provider = scoop_hir::IntrinsicProviderId::from_raw(1);
    let hir_input = scoop_hir_lower::LegacyCombinedSources::try_new(
        core_files
            .iter()
            .zip(&inputs[..user_index])
            .map(|(source, input)| scoop_hir_lower::ProviderSource {
                source,
                identity: input.identity.clone(),
                provider: core_provider,
                name: &input.name,
                source_text: &input.source,
            })
            .collect(),
        user_provider,
        user_sources,
        |identity| {
            assert_eq!(identity, &inputs[user_index].identity);
            scoop_hir_lower::CurrentSourceDetails {
                display_locator: &inputs[user_index].name,
                source_text: &inputs[user_index].source,
            }
        },
    )
    .map_err(|error| {
        vec![no_span(
            user_index,
            format!("invalid combined source input: {error}"),
        )]
    })?;
    let hir = scoop_hir_lower::lower_combined_sources(
        scoop_identity::RequestedConeKind::Executable,
        &hir_input,
        options.intrinsic_declaration_policy.clone(),
    )?;
    let hir_production = scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.export)
        .map_err(|error| {
            vec![no_span(
                user_index,
                format!("HIR production projection failed: {error}"),
            )]
        })?;
    let hir_dump = scoop_hir::dump(&hir.export);
    let warnings = hir.warnings;

    let mir = scoop_mir_lower::lower(&hir.local);
    mir.validate().map_err(|error| {
        vec![no_span(
            user_index,
            format!("MIR validation failed: {error}"),
        )]
    })?;
    let mir_dump = scoop_mir::dump(&mir);
    let mir_foundation = scoop_mir::OdrFreeMirFoundation::from_module(&mir).map_err(|error| {
        vec![no_span(
            user_index,
            format!("MIR strong-profile projection failed: {error}"),
        )]
    })?;
    let mir_production =
        scoop_mir_lower::lower_production_section(mir.cone, &hir_production, &mir_foundation)
            .map_err(|error| {
                vec![no_span(
                    user_index,
                    format!("MIR production projection failed: {error}"),
                )]
            })?;
    let entry_source =
        scoop_lir_lower::lower_entry_production_source(mir_production.entry_bridge());
    let strong_mir = scoop_mir::SingleConeStrongMirInput::try_new(
        mir,
        mir_foundation,
        mir_production,
        scoop_mir::CoreShapeSupportSourceInput::NotCore,
    )
    .map_err(|error| {
        vec![no_span(
            user_index,
            format!("MIR strong-profile sealing failed: {error}"),
        )]
    })?;

    let lir = scoop_lir_lower::lower(&strong_mir, target_profile.lir_target())
        .map_err(|error| vec![no_span(user_index, format!("LIR lowering failed: {error}"))])?;
    let lir_dump = scoop_lir::dump(lir.module());

    let build = (|| -> Result<(StageDumps, PathBuf), Vec<Diagnostic>> {
        std::fs::create_dir_all(out_dir).map_err(|e| {
            vec![no_span(
                user_index,
                format!("cannot create output directory {}: {e}", out_dir.display()),
            )]
        })?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("scoop-out");
        let binary = out_dir.join(stem);

        let scoop_objects = scoop_codegen::emit_object_set(
            &lir,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            entry_source,
            out_dir,
            target_profile.backend(),
        )
        .map_err(|e| vec![no_span(user_index, format!("codegen failed: {e}"))])?;
        let object_paths = scoop_objects
            .members()
            .iter()
            .map(|member| member.path().to_path_buf())
            .collect::<Vec<_>>();

        let bridge_object = match scoop_codegen::c_bridge_source(lir.module()).map_err(|e| {
            vec![no_span(
                user_index,
                format!("C bridge generation failed: {e}"),
            )]
        })? {
            Some(source) => {
                let source_path = out_dir.join(format!("{stem}.ffi.c"));
                let object_path = out_dir.join(format!("{stem}.ffi.o"));
                std::fs::write(&source_path, source).map_err(|error| {
                    vec![no_span(
                        user_index,
                        format!("cannot write C bridge {}: {error}", source_path.display()),
                    )]
                })?;
                compile_c_bridge(
                    &source_path,
                    &object_path,
                    target_profile.c_bridge_toolchain(),
                    user_index,
                )?;
                Some(object_path)
            }
            None => None,
        };

        let runtime_lib = build_runtime(user_index, target_profile.runtime_build())?;
        let mut libraries = Vec::new();
        for (_, extern_) in lir.module().extern_functions.iter() {
            if !extern_.library.is_empty() && !libraries.contains(&extern_.library) {
                libraries.push(extern_.library.clone());
            }
        }
        for (_, global) in lir.module().native_globals.iter() {
            if !global.library.is_empty() && !libraries.contains(&global.library) {
                libraries.push(global.library.clone());
            }
        }
        link(LinkRequest {
            objects: &object_paths,
            bridge_object: bridge_object.as_deref(),
            runtime_lib: &runtime_lib,
            libraries: &libraries,
            library_paths: &options.library_paths,
            binary: &binary,
            profile: target_profile.final_link(),
            file: user_index,
        })?;

        Ok((
            StageDumps {
                ast: ast_dump,
                hir: hir_dump,
                mir: mir_dump,
                lir: lir_dump,
            },
            binary,
        ))
    })();
    match build {
        Ok((dumps, binary)) => Ok(CompileSuccess {
            dumps,
            binary,
            warnings,
        }),
        Err(mut diagnostics) => {
            diagnostics.extend(warnings);
            Err(diagnostics)
        }
    }
}

/// A driver-level diagnostic without a source span, attributed to the
/// input file at index `file`.
fn no_span(file: usize, message: String) -> Diagnostic {
    Diagnostic::without_span(DiagnosticSeverity::Error, file, message)
}

/// Root of the Cargo workspace (the driver crate lives in `compiler/driver`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}
