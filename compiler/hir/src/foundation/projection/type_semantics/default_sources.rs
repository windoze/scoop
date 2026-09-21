use super::*;
use crate::{ExportDefinitionSourceV1, NominalDefaultSourceProductionV1};

pub(super) fn collect(
    output: &crate::OrdinaryHirOutput,
    sources: &mut Vec<ExportDefinitionSourceV1>,
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationBuildError> {
    use HirFoundationBuildError as Error;
    let defaults = NominalDefaultSourceProductionV1::from_ordinary_hir(output, meter)
        .map_err(|error| Error::DefaultSourceProduction(Box::new(error)))?;
    defaults
        .templates()
        .visit_definition_sources_metered(
            &mut |_, source, _, meter, path| {
                let bytes = source.origin().source().logical_path().as_str().len() as u64;
                meter.check_semantic_leaf(bytes, path)?;
                meter.check_table_entries(sources.len() as u64 + 1, path)?;
                meter.charge_work(bytes.saturating_add(1), path)?;
                meter.charge_owned_bytes(
                    bytes.saturating_add(std::mem::size_of::<ExportDefinitionSourceV1>() as u64),
                    path,
                )?;
                meter.try_reserve_collection_slots(sources, 1, path)?;
                sources.push(source.clone());
                Ok(())
            },
            meter,
            &WirePath::root(),
        )
        .map_err(Error::DefaultSourceResource)
}
