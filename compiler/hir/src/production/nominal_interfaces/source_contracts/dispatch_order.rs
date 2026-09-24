use super::*;
use std::collections::BTreeSet;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    meter: &mut BudgetMeter,
) -> Result<NominalDispatchOrderV1, Error> {
    match local {
        LocalNominalId::Struct(_) | LocalNominalId::Enum(_) => {
            Ok(NominalDispatchOrderV1::NonVirtual)
        }
        LocalNominalId::Class(id) => class(export, id, meter),
        LocalNominalId::Object(id) => class(export, export.objects[id].backing_class, meter),
        LocalNominalId::Interface(id) => interface(export, id, meter),
    }
}

fn class(
    export: &ExportHir,
    id: ClassId,
    meter: &mut BudgetMeter,
) -> Result<NominalDispatchOrderV1, Error> {
    let mut slots = Vec::new();
    let mut seen = BTreeSet::new();
    for id in &export.classes[id].methods {
        work(meter, 1)?;
        let method = export.functions[*id]
            .method
            .ok_or_else(|| invalid("class method has no dispatch relation"))?;
        let family = match method.dispatch {
            MethodDispatch::Direct => continue,
            MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => family,
            MethodDispatch::Interface(_) => {
                return Err(invalid(
                    "class method carries interface declaration dispatch",
                ));
            }
        };
        work(meter, seen.len())?;
        meter
            .charge_collection_slots(1, &WirePath::root())
            .map_err(resource)?;
        if seen.insert(family) {
            let slot = export
                .dispatch_slot_identities
                .get_virtual(family)
                .ok_or_else(|| invalid("virtual family has no persistent slot identity"))?
                .id();
            push(&mut slots, slot, meter)?;
        }
    }
    Ok(NominalDispatchOrderV1::Class { slots })
}

fn interface(
    export: &ExportHir,
    id: InterfaceId,
    meter: &mut BudgetMeter,
) -> Result<NominalDispatchOrderV1, Error> {
    let declaration = &export.interfaces[id];
    let projector = HirInterfaceSignatureProjector::new(export);
    resources::binders(
        export,
        &declaration.type_params,
        declaration.type_params.len(),
        meter,
    )?;
    let binders = projector
        .binder_frame(&declaration.type_params, 0)
        .map_err(invalid)?;
    let mut parents = Vec::new();
    for parent in &declaration.parents {
        let ty = export.interface_applications[*parent].canonical_type;
        resources::ty(export, ty, binders.len(), 2, meter)?;
        push(
            &mut parents,
            projector.map_type(ty, &binders).map_err(invalid)?,
            meter,
        )?;
    }
    let mut members = Vec::new();
    for member in &declaration.methods {
        let slot = export
            .dispatch_slot_identities
            .get_interface(*member)
            .ok_or_else(|| invalid("interface member has no persistent slot identity"))?
            .id();
        let mut overrides = BTreeSet::new();
        for inherited in &export.interface_methods[*member].overrides {
            work(meter, overrides.len())?;
            meter
                .charge_collection_slots(1, &WirePath::root())
                .map_err(resource)?;
            let inherited = export
                .dispatch_slot_identities
                .get_interface(*inherited)
                .ok_or_else(|| {
                    invalid("overridden interface member has no persistent slot identity")
                })?
                .id();
            overrides.insert(inherited);
        }
        charge_canonical(overrides.len(), meter)?;
        let overrides =
            CanonicalPersistentIdsV1::try_new(overrides.into_iter().collect()).map_err(invalid)?;
        push(
            &mut members,
            InterfaceSourceMemberV1::new(slot, overrides),
            meter,
        )?;
    }
    Ok(NominalDispatchOrderV1::Interface { parents, members })
}
