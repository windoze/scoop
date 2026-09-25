//! Shared parsed-request lowering, artifact production, and publication.

use super::*;
mod errors;
mod protocols;
pub use errors::{CurrentConeProductionError, CurrentConeProductionFailure};

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
        let hir = self.lower_hir(requested, protocols).map_err(|e| {
            CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Hir(e))
        })?;
        let warnings =
            CurrentConeDiagnosticSet::try_new(hir.hir.output().warnings.clone(), &self.sources)
                .map_err(|e| {
                    CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Warnings(
                        e,
                    ))
                })?;
        dump = dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Hir, || {
                scoop_hir::dump(&hir.hir.output().export)
            })
        });
        let artifact = (|| {
            let strong = protocols.lower_machine(hir, self.request, &mut dump)?;
            let producer =
                scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION")))
                    .map_err(CurrentConeProductionFailure::Producer)?;
            let owners = self
                .request
                .dependencies()
                .closure
                .dependency_symbol_owners()
                .cloned()
                .collect::<Vec<_>>();
            let artifact = strong
                .produce_artifact(
                    producer,
                    cone,
                    self.request.dependencies().direct_dependencies().to_vec(),
                    temporary_parent,
                    self.request.target(),
                    &owners,
                )
                .map_err(CurrentConeProductionFailure::Artifact)?;
            artifact
                .publish(
                    self.request.output().as_path(),
                    self.request.dependencies().dependency_first().to_vec(),
                )
                .map_err(CurrentConeProductionFailure::Publication)
        })();
        match artifact {
            Ok(artifact) => Ok(SingleConeProductionSuccess::new_cross_cone(
                artifact, warnings, dump,
            )),
            Err(cause) => Err(CurrentConeProductionError::after_hir(cause, warnings)),
        }
    }

    fn lower_hir(
        &self,
        requested: scoop_identity::RequestedConeKind,
        protocols: &ValidatedCompilerProtocols,
    ) -> Result<current_hir::CurrentConeHirArtifacts, CurrentConeHirStageError> {
        let world = self
            .request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .map_err(CurrentConeHirStageError::SemanticWorld)?;
        current_hir::CurrentConeHirArtifacts::lower(
            requested,
            &self.sources,
            protocols.hir_input(),
            &world,
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
