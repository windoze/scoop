use super::*;

impl SingleConeBuildRequest {
    pub(super) fn observe_ast(self) -> Result<SingleConeCompilation, SingleConeProductionError> {
        let current =
            load_current_input(self.current).map_err(SingleConeProductionError::Preflight)?;
        let sources = match current {
            LoadedCurrentConeInput::Manifest { manifest } => {
                parse_manifest_current(&manifest, self.target.id(), self.purpose)
            }
            LoadedCurrentConeInput::SingleFile { source } => parse_single_file_current(&source),
        }
        .map_err(SingleConeProductionError::Sources)?;
        let dumps = ast_dumps(&sources, self.emit);
        let warnings =
            CurrentConeDiagnosticSet::try_new(Vec::new(), &sources).map_err(|error| {
                SingleConeProductionError::Production(Box::new(
                    CurrentConeProductionError::before_hir(CurrentConeProductionFailure::Warnings(
                        error,
                    )),
                ))
            })?;
        Ok(SingleConeCompilation::Observed(StageObservation {
            warnings,
            dumps,
        }))
    }
}

pub(super) fn ast_dumps(
    sources: &CurrentConeParsedSources,
    emit: StageDumpPolicy,
) -> Vec<EmittedStageDump> {
    capture_stage_dump(emit, StageDumpKind::Ast, || {
        sources
            .sources()
            .sources()
            .iter()
            .map(|source| scoop_ast::dump(source.ast()))
            .collect()
    })
}
