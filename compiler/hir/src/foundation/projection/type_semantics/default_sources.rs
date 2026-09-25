use super::*;
use crate::{ExportDefinitionSourceV1, NominalDefaultSourceProductionV1};

pub(super) fn collect(
    output: &crate::DependencyHirOutput,
    sources: &mut Vec<ExportDefinitionSourceV1>,
) -> Result<(), HirFoundationBuildError> {
    use HirFoundationBuildError as Error;
    let defaults = NominalDefaultSourceProductionV1::from_dependency_hir(output)
        .map_err(|error| Error::DefaultSourceProduction(Box::new(error)))?;
    defaults
        .templates()
        .visit_definition_sources(
            &mut |_, source, _, path| {
                scoop_wire::allocation::try_reserve(sources, 1, path)?;
                sources.push(source.clone());
                Ok(())
            },
            &WirePath::root(),
        )
        .map_err(Error::DefaultSourceResource)
}
