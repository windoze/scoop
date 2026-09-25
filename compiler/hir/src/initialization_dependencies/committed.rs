use scoop_identity::CallableTemplateOrigin;

use super::*;

impl crate::DependencyHirOutput {
    /// Replays actual accessor occurrences, including expanded defaults, from
    /// sealed executable HIR. A selected but unused accessor adds no edge.
    pub fn materialized_property_initialization_uses(
        &self,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Vec<HirPropertyInitializationUseV1>, Error> {
        let local = self.output().local.module();
        let units = local_unit_ids(
            local
                .initialization_units
                .iter()
                .map(|(_, unit)| unit.identity.id()),
        )?;
        let mut uses = Vec::new();
        for occurrence in self
            .committed_dependency_call_occurrences()
            .map_err(|error| Error::Calls(Box::new(error)))?
        {
            let callable = occurrence.callable();
            let Some(dependency_unit) = callable.initialization_unit() else {
                continue;
            };
            let declaration = callable.interface().declaration();
            let CallableTemplateOrigin::Accessor(accessor) = declaration else {
                return Err(Error::NonAccessorUnit(declaration));
            };
            let Some(local_unit) =
                initializer_root(occurrence.position().root, identities, &units)?
            else {
                continue;
            };
            push(
                &mut uses,
                HirPropertyInitializationUseV1 {
                    local_unit,
                    provider: callable.provider(),
                    dependency_unit,
                    accessor,
                },
                local.cone,
            )?;
        }
        canonicalize(uses)
    }
}
