use super::*;
use crate::coff_object::{CoffObject, error};

pub(super) fn materialize(
    bytes: &mut Vec<u8>,
    surface: &ObjectSymbolSurfaceV1,
    definitions: &[scoop_lir::ObjectDefinitionPlanId],
) -> Result<(), CodegenError> {
    let mut output = CoffObject::read(bytes)?;
    // Callable registrations are emitted in the same member as their body.
    // Their distinct logical definitions must follow that body's native
    // selection, including when duplicate ODR members are discarded.
    let callable = definitions
        .iter()
        .filter_map(|id| surface.plan(*id))
        .find(|plan| {
            plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableRegistration
        })
        .map(|registration| {
            let body = surface
                .plans()
                .iter()
                .find(|plan| {
                    plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                        && plan.owner() == registration.owner()
                })
                .ok_or_else(|| error("callable registration has no body definition"))?;
            let (section, _) = output.extent(body.primary_symbol().symbol().as_str())?;
            Ok((section, body.primary_symbol().linkage()))
        })
        .transpose()?;
    for definition in definitions {
        let plan = surface
            .plan(*definition)
            .ok_or_else(|| error(format!("missing definition {definition}")))?;
        let (primary, _) = output.extent(plan.primary_symbol().symbol().as_str())?;
        let mut sections = plan
            .atom_boundaries()
            .iter()
            .map(|&boundary| output.existing_pair(boundary))
            .collect::<Result<Vec<_>, _>>()?;
        sections.push(primary);
        let (primary, linkage) = callable.unwrap_or((primary, plan.primary_symbol().linkage()));
        output.associate(primary, &sections, linkage)?;
    }
    *bytes = output.finish()?;
    Ok(())
}
