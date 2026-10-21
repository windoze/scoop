use super::wire::*;
use crate::link_data::link_error;
use crate::*;
use scoop_identity::{ObjectDefinitionPlanOwner, ValidatedIdentityGraph};

impl DecodedStrongRegistrationProductionSurface {
    pub(crate) fn read_compile(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        layouts: &[&CanonicalExactLayoutExportsV1],
        dependencies: &[&CompileProduction],
    ) -> Result<
        (
            Vec<StrongStaticStorageSemanticPlanV1>,
            Vec<CompileInitializationUnit>,
        ),
        LinkDataError,
    > {
        let producer = foundation.producer();
        let mut definitions = Vec::new();
        for dependency in dependencies {
            for unit in dependency.initialization_units() {
                let definition = unit.definition();
                let plan = dependency
                    .canonical_definitions()
                    .plan(definition.registration().plan())
                    .ok_or_else(|| {
                        link_error(format!(
                            "missing initialization definition {}",
                            definition.unit()
                        ))
                    })?;
                if matches!(
                    plan.definition_owner(),
                    ObjectDefinitionPlanOwner::Strong { .. }
                ) {
                    definitions.push(definition);
                }
            }
        }
        let local_definitions = self
            .initialization_units
            .iter()
            .map(|unit| unit.read_definition(producer, identities))
            .collect::<Result<Vec<_>, _>>()?;
        definitions.extend_from_slice(&local_definitions);
        let definitions = StrongInitializationDefinitionCatalogV2::new(producer, &definitions)
            .map_err(link_error)?;
        let static_storages = self
            .static_storages
            .into_iter()
            .map(|storage| storage.read(foundation, identities, layouts))
            .collect::<Result<_, _>>()?;
        let initialization = self
            .initialization_units
            .into_iter()
            .zip(local_definitions)
            .map(|(unit, definition)| unit.read(definition, identities, &definitions))
            .collect::<Result<_, _>>()?;
        Ok((static_storages, initialization))
    }
}
