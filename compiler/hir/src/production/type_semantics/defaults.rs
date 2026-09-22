//! Publish complete omission templates using the common source-body projection.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{invalid, resource, work};
use super::nested_sources::resources;
use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

mod references;
mod witness;

impl CanonicalProtectedDefaultTemplatesV1 {
    /// Projects the templates required by these source interfaces. The inheritance
    /// table supplies the resolved root-slot domains; semantic validation is separate.
    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
        protocols: &CanonicalProtectedCallableSourceInterfacesV1,
        inheritance: &CanonicalNominalInheritanceInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalProtectedDefaultTemplatesV1, Error> {
        let export = output.output().export.module();
        let owners = super::source_defaults::local_owners(
            export,
            protocols.records().len(),
            |owner| protocols.get(owner).is_some(),
            meter,
        )
        .map_err(invalid)?;
        let slots = witness::root_slots(inheritance, meter)?;
        let mut templates = Vec::new();
        for protocol in protocols.records() {
            work(meter, owners.len())?;
            let local = *owners
                .get(&protocol.owner())
                .ok_or_else(|| invalid("default protocol has no source declaration"))?;
            for parameter in protocol.parameters().parameters() {
                work(meter, 1)?;
                let Some(key) = parameter.calling().template() else {
                    continue;
                };
                let source = DefaultSourceBodyProductionV1::from_dependency_hir(
                    output,
                    local,
                    key.parameter_position(),
                    meter,
                )
                .map_err(invalid)?
                .into_source_template(meter)
                .map_err(invalid)?;
                if source.key() != key {
                    return Err(invalid(
                        "default body disagrees with its source parameter key",
                    ));
                }
                let publication = witness::Publication::new(export, local, &source, &slots, meter)?;
                let references = references::project(&source, &publication, meter)?;
                resources::push(
                    &mut templates,
                    source
                        .into_protected_template(references)
                        .map_err(invalid)?,
                    meter,
                )?;
            }
        }
        resources::canonical(templates.len(), meter)?;
        CanonicalProtectedDefaultTemplatesV1::try_new(templates).map_err(invalid)
    }
}
