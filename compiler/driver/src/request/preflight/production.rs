//! Shared parsed-request lowering, artifact production, and publication.

use super::*;
mod errors;
mod layout;
mod layout_references;
mod layout_selection;
mod native;
mod protocols;
pub use errors::{CurrentConeProductionError, CurrentConeProductionFailure};
pub use layout::LayoutProductionError;
pub(super) use layout_references::collect_mir_references;
pub(super) use layout_selection::select_lir_dependencies;

impl ParsedSingleConeBuildRequest<'_, '_> {
    pub fn build_and_publish(
        self,
        temporary_parent: &Path,
    ) -> Result<SingleConeProductionSuccess, CurrentConeProductionError> {
        let protocols = self.request.protocols();
        let cone = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest } => manifest_record(manifest),
            ValidatedCurrentConeInput::SingleFile { .. } => scoop_slib::ConeRecord::new(
                ConeCoordinate::reserved_single_file(),
                scoop_slib::ConeKind::Executable,
                scoop_slib::ConeSourceForm::SingleFile,
            ),
        };
        let cone = cone.map_err(|e| {
            CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Cone(e))
        })?;
        let requested = match cone.kind() {
            scoop_slib::ConeKind::Library => scoop_identity::RequestedConeKind::Library,
            scoop_slib::ConeKind::Executable => scoop_identity::RequestedConeKind::Executable,
        };
        let emit = self.request.emit();
        let mut dump = capture_stage_dump(emit, StageDumpKind::Ast, || {
            self.sources
                .sources()
                .sources()
                .iter()
                .map(|source| scoop_ast::dump(source.ast()))
                .collect()
        });
        let world = self
            .request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .map_err(CurrentConeHirStageError::SemanticWorld)
            .map_err(|e| {
                CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Hir(e))
            })?;
        let hir = self.lower_hir(requested, protocols, &world).map_err(|e| {
            CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Hir(e))
        })?;
        let warnings =
            CurrentConeDiagnosticSet::try_new(hir.hir.output().warnings.clone(), &self.sources)
                .map_err(|e| {
                    CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Warnings(
                        e,
                    ))
                })?;
        dump.extend(capture_stage_dump(emit, StageDumpKind::Hir, || {
            format!(
                "== Export ==\n{}== Static Shapes ==\n{}== LocalConcrete ==\n{}== CrossCone ==\n{}",
                scoop_hir::dump(&hir.hir.output().export),
                scoop_hir::dump_static_shapes(hir.hir.output().export.module(), &world),
                scoop_hir::dump_local(&hir.hir.output().local),
                scoop_hir::dump_cross_cone(&hir.hir, &hir.cross_cone_section)
            )
        }));
        let artifact = (|| {
            let artifact =
                protocols::lower_machine(hir, self.request, cone, temporary_parent, &mut dump)?;
            artifact
                .publish(self.request.output().as_path())
                .map_err(CurrentConeProductionFailure::Publication)
        })();
        match artifact {
            Ok(artifact) => Ok(SingleConeProductionSuccess::new_cross_cone(
                artifact, warnings, dump,
            )),
            Err(cause) => Err(CurrentConeProductionError::after_hir(cause, warnings)),
        }
    }

    pub(super) fn lower_hir(
        &self,
        requested: scoop_identity::RequestedConeKind,
        protocols: &ValidatedCompilerProtocols,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<current_hir::CurrentConeHirArtifacts, CurrentConeHirStageError> {
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
        current_hir::CurrentConeHirArtifacts::lower(
            requested,
            &self.sources,
            protocols.hir_input(),
            world,
            source_names,
        )
    }
}

fn manifest_record(
    manifest: &LoadedConeManifest,
) -> Result<scoop_slib::ConeRecord, scoop_slib::ConeRecordError> {
    let semantic = manifest.parsed().semantic();
    let kind = match semantic.requested_kind() {
        scoop_identity::RequestedConeKind::Library => scoop_slib::ConeKind::Library,
        scoop_identity::RequestedConeKind::Executable => scoop_slib::ConeKind::Executable,
    };
    scoop_slib::ConeRecord::new(
        semantic.coordinate().clone(),
        kind,
        scoop_slib::ConeSourceForm::Manifest,
    )
}
