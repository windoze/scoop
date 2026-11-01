//! Shared parsed-request lowering, artifact production, and publication.

use super::*;
mod documentation;
mod errors;
pub use documentation::DocumentationProductionError;
mod layout;
mod layout_references;
mod layout_selection;
mod native;
mod protocols;
pub use errors::{CurrentConeProductionError, CurrentConeProductionFailure};
pub use layout::LayoutProductionError;
pub(super) use layout_references::collect_mir_references;
pub(super) use layout_selection::select_lir_dependencies;
use std::ops::ControlFlow;

impl ParsedSingleConeBuildRequest<'_, '_> {
    pub fn build_and_publish(
        self,
        temporary_parent: &Path,
    ) -> Result<SingleConeProductionSuccess, CurrentConeProductionError> {
        if self.request.output().artifact().is_none() {
            return Err(CurrentConeProductionError::before_hir(
                CurrentConeProductionFailure::ArtifactOutputRequired,
            ));
        }
        match self.compile(Some(temporary_parent), None)? {
            SingleConeCompilation::Published(result) => Ok(*result),
            SingleConeCompilation::Observed(_)
            | SingleConeCompilation::Documented(_)
            | SingleConeCompilation::Checked(_) => {
                unreachable!("production has no observation stop")
            }
        }
    }

    pub(super) fn compile(
        self,
        temporary_parent: Option<&Path>,
        stop_after: Option<StageDumpKind>,
    ) -> Result<SingleConeCompilation, CurrentConeProductionError> {
        let protocols = self.request.protocols();
        let cone = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest } => {
                manifest_record(manifest, self.request.purpose())
            }
            ValidatedCurrentConeInput::SingleFile { .. } => scoop_slib::ConeRecord::new(
                ConeCoordinate::reserved_single_file(),
                scoop_slib::ConeKind::Executable,
                scoop_slib::ConeSourceForm::SingleFile,
            ),
        };
        let cone = cone
            .and_then(|cone| {
                if cone.kind() != scoop_slib::ConeKind::Executable
                    || self
                        .request
                        .target_selection()
                        .target()
                        .native_object_format()
                        != scoop_lir::NativeObjectFormat::Coff64
                {
                    return Ok(cone);
                }
                let subsystem = match (self.request.purpose(), self.request.current()) {
                    (
                        CompilationPurpose::Production,
                        ValidatedCurrentConeInput::Manifest { manifest },
                    ) => manifest.parsed().semantic().windows().subsystem(),
                    _ => scoop_identity::WindowsSubsystem::Console,
                };
                cone.with_windows_subsystem(subsystem)
            })
            .map_err(|e| {
                CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Cone(e))
            })?;
        let requested = match cone.kind() {
            scoop_slib::ConeKind::Library => scoop_identity::RequestedConeKind::Library,
            scoop_slib::ConeKind::Executable => scoop_identity::RequestedConeKind::Executable,
        };
        let emit = self.request.emit();
        let mut dump = observation::ast_dumps(&self.sources, emit);
        let world = self
            .request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .map_err(CurrentConeHirStageError::SemanticWorld)
            .map_err(|e| {
                CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Hir(e))
            })?;
        let mut hir = self.lower_hir(requested, protocols, &world).map_err(|e| {
            let warnings = CurrentConeDiagnosticSet::try_new(
                packages::single_file_warnings(&self.sources),
                &self.sources,
            )
            .expect("single-file warnings reference their parsed source");
            CurrentConeProductionError::after_hir(CurrentConeProductionFailure::Hir(e), warnings)
        })?;
        let used_generators = std::mem::take(&mut hir.used_generators);
        let compiler_inputs = std::mem::take(&mut hir.compiler_inputs);
        let mut source_warnings = packages::single_file_warnings(&self.sources);
        source_warnings.extend(hir.hir.output().warnings.clone());
        source_warnings.extend(hir.generator_warnings.clone());
        let warnings = CurrentConeDiagnosticSet::try_new_with_generated(
            source_warnings,
            &self.sources,
            &hir.generated_sources,
        )
        .map_err(|e| {
            CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Warnings(e))
        })?;
        if matches!(self.request.output(), crate::SingleConeOutput::Check) {
            return Ok(SingleConeCompilation::Checked(
                crate::SingleConeCheckSuccess {
                    warnings,
                    used_generators,
                    compiler_inputs,
                },
            ));
        }
        let mut hir = match current_hir::CurrentConeHirArtifacts::from_hir(hir, &world) {
            Ok(hir) => hir,
            Err(error) => {
                return Err(CurrentConeProductionError::after_hir(
                    CurrentConeProductionFailure::Hir(error),
                    warnings,
                ));
            }
        };
        if self.request.purpose() == scoop_protocol::CompilationPurpose::Test {
            hir.test_directory = Some(current_hir::test_directory::project(
                &hir.hir.output().export.test_containers,
                &self.sources,
            ));
        }
        dump.extend(capture_stage_dump(emit, StageDumpKind::Hir, || {
            format!(
                "== Export ==\n{}== Static Shapes ==\n{}== LocalConcrete ==\n{}== CrossCone ==\n{}",
                scoop_hir::dump(&hir.hir.output().export),
                scoop_hir::dump_static_shapes(hir.hir.output().export.module(), &world),
                scoop_hir::dump_local(&hir.hir.output().local),
                scoop_hir::dump_cross_cone(&hir.hir, &hir.cross_cone_section)
            )
        }));
        if stop_after == Some(StageDumpKind::Hir) {
            return Ok(SingleConeCompilation::Observed(StageObservation {
                warnings,
                dumps: dump,
            }));
        }
        let documentation = match documentation::PendingDocumentation::prepare(
            &self,
            &hir,
            cone.coordinate(),
            requested,
        ) {
            Ok(documentation) => documentation,
            Err(cause) => {
                return Err(CurrentConeProductionError::after_hir(
                    CurrentConeProductionFailure::Documentation(cause),
                    warnings,
                ));
            }
        };
        if self.request.output().artifact().is_none() {
            let documentation = match documentation
                .expect("documentation-only output has an input")
                .publish()
            {
                Ok(documentation) => documentation,
                Err(cause) => {
                    return Err(CurrentConeProductionError::after_hir(
                        CurrentConeProductionFailure::Documentation(cause),
                        warnings,
                    ));
                }
            };
            return Ok(SingleConeCompilation::Documented(
                crate::SingleConeDocumentationSuccess {
                    documentation,
                    warnings,
                    dumps: dump,
                    used_generators,
                    compiler_inputs,
                },
            ));
        }
        let artifact = (|| {
            let ControlFlow::Continue(artifact) = protocols::lower_machine(
                hir,
                self.request,
                cone,
                temporary_parent.expect("artifact output has a temporary workspace"),
                &mut dump,
                stop_after,
            )?
            else {
                return Ok(ControlFlow::Break(()));
            };
            artifact
                .publish(
                    self.request
                        .output()
                        .artifact()
                        .expect("machine production has an artifact destination")
                        .as_path(),
                )
                .map(ControlFlow::Continue)
                .map_err(CurrentConeProductionFailure::Publication)
        })();
        match artifact {
            Ok(ControlFlow::Break(())) => Ok(SingleConeCompilation::Observed(StageObservation {
                warnings,
                dumps: dump,
            })),
            Ok(ControlFlow::Continue(artifact)) => {
                let documentation = match documentation.map(|input| input.publish()).transpose() {
                    Ok(documentation) => documentation,
                    Err(cause) => {
                        return Err(CurrentConeProductionError::after_hir(
                            CurrentConeProductionFailure::Documentation(cause),
                            warnings,
                        ));
                    }
                };
                let mut success = SingleConeProductionSuccess::new_cross_cone(
                    artifact,
                    warnings,
                    dump,
                    used_generators,
                    compiler_inputs,
                );
                success.documentation = documentation;
                Ok(SingleConeCompilation::Published(Box::new(success)))
            }
            Err(cause) => Err(CurrentConeProductionError::after_hir(cause, warnings)),
        }
    }

    pub(super) fn lower_hir(
        &self,
        requested: scoop_identity::RequestedConeKind,
        protocols: &ValidatedCompilerProtocols,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<current_hir::CurrentConeHir, CurrentConeHirStageError> {
        let mut source_names = self
            .request
            .dependencies()
            .semantic()
            .identity_inputs()
            .map(|(coordinate, _)| {
                (
                    coordinate
                        .identity()
                        .expect("loaded dependency coordinates are valid"),
                    coordinate.to_string(),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let current_coordinate = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest } => {
                manifest.parsed().semantic().coordinate().to_string()
            }
            ValidatedCurrentConeInput::SingleFile { .. } => {
                ConeCoordinate::reserved_single_file().to_string()
            }
        };
        source_names.insert(self.sources.cone(), current_coordinate);
        let input_root = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest } => manifest.input_root(),
            ValidatedCurrentConeInput::SingleFile { source } => source
                .resolved_path()
                .parent()
                .expect("an input source has a parent"),
        };
        let inputs =
            std::sync::Arc::new(scoop_toolchain::CompileTimeInputs::default().with_sources(
                input_root.to_owned(),
                self.sources.iter().map(|source| {
                    (
                        source.text().identity().clone(),
                        source.diagnostic().display_locator().to_owned(),
                    )
                }),
            ));
        if self.request.purpose() == CompilationPurpose::Test {
            let ValidatedCurrentConeInput::Manifest { manifest } = self.request.current() else {
                unreachable!("test request normalization requires a manifest")
            };
            let test_dependencies = manifest
                .parsed()
                .tests()
                .dependency_iter()
                .map(|(_, dependency)| {
                    dependency
                        .coordinate()
                        .identity()
                        .expect("dependency validation resolved this exact coordinate")
                })
                .collect();
            return current_hir::CurrentConeHir::lower_tests(
                &self.sources,
                protocols.hir_input(),
                world,
                source_names,
                test_dependencies,
                self.request.generators(),
                inputs,
            );
        }
        current_hir::CurrentConeHir::lower_with_documentation(
            requested,
            &self.sources,
            protocols.hir_input(),
            world,
            source_names,
            self.request.output().documentation().is_some(),
            self.request.generators(),
            inputs,
        )
    }
}

fn manifest_record(
    manifest: &LoadedConeManifest,
    purpose: CompilationPurpose,
) -> Result<scoop_slib::ConeRecord, scoop_slib::ConeRecordError> {
    let semantic = manifest.parsed().semantic();
    let kind = match (purpose, semantic.requested_kind()) {
        (CompilationPurpose::Test, _) => scoop_slib::ConeKind::Executable,
        (_, scoop_identity::RequestedConeKind::Library) => scoop_slib::ConeKind::Library,
        (_, scoop_identity::RequestedConeKind::Executable) => scoop_slib::ConeKind::Executable,
    };
    scoop_slib::ConeRecord::new(
        semantic.coordinate().clone(),
        kind,
        scoop_slib::ConeSourceForm::Manifest,
    )
}
