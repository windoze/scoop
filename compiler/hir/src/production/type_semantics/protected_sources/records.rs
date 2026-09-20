use super::*;

type Projection = (
    CanonicalProtectedDeclarationInterfacesV1,
    BTreeSet<CallableTemplateOrigin>,
);
pub(super) fn project(
    output: &ExportHirOutput,
    required: &CanonicalProtectedDeclarationRefsV1,
    meter: &mut BudgetMeter,
) -> Result<Projection, Error> {
    let mut callables = BTreeSet::new();
    let mut constructors = BTreeSet::<PersistentConstructorId>::new();
    let mut properties = BTreeSet::<PersistentPropertyId>::new();
    let mut nested = BTreeSet::new();
    let mut protocols = BTreeSet::new();
    for declaration in required.values() {
        match declaration {
            ProtectedDeclarationRefV1::Callable(id) => {
                let id = id.declaration();
                resources::insert(&mut callables, id, meter)?;
                if !matches!(id, CallableTemplateOrigin::Accessor(_)) {
                    require_protocol(&mut protocols, id, meter)?;
                }
            }
            ProtectedDeclarationRefV1::Constructor(id) => {
                resources::insert(&mut constructors, *id, meter)?;
                require_protocol(
                    &mut protocols,
                    CallableTemplateOrigin::Constructor(*id),
                    meter,
                )?;
            }
            ProtectedDeclarationRefV1::Property(id) => {
                resources::insert(&mut properties, *id, meter)?
            }
            ProtectedDeclarationRefV1::NestedNominal(id) => {
                resources::insert(&mut nested, *id, meter)?
            }
        }
    }
    let mut records = Vec::new();
    let export = output.module();
    for source in super::super::nominal_callables::project(export, callables, meter)? {
        let record = ProtectedCallableInterfaceV1::try_from(source).map_err(invalid)?;
        resources::boxed(&record, meter)?;
        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Callable(Box::new(record)),
            meter,
        )?;
    }
    for source in super::super::nominal_constructors::project(export, constructors, meter)? {
        let record = ProtectedConstructorInterfaceV1::try_from(source).map_err(invalid)?;
        resources::boxed(&record, meter)?;
        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Constructor(Box::new(record)),
            meter,
        )?;
    }
    resources::canonical(properties.len(), meter)?;
    let properties =
        CanonicalPersistentIdsV1::try_new(properties.into_iter().collect()).map_err(invalid)?;
    for source in
        super::super::inheritance::source_properties::project_nominal(export, &properties, meter)?
    {
        let record = ProtectedPropertyInterfaceV1::try_from(source).map_err(invalid)?;
        resources::boxed(&record, meter)?;
        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Property(Box::new(record)),
            meter,
        )?;
    }
    for owner in nested {
        let (source, owners) = project_record(output, owner, meter)?;
        for id in owners {
            require_protocol(&mut protocols, id, meter)?;
        }
        let record = ProtectedNestedNominalInterfaceV1::try_from(source).map_err(invalid)?;
        resources::boxed(&record, meter)?;
        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(record)),
            meter,
        )?;
    }
    resources::canonical(records.len(), meter)?;
    Ok((
        CanonicalProtectedDeclarationInterfacesV1::try_new(records).map_err(invalid)?,
        protocols,
    ))
}

fn require_protocol(
    owners: &mut BTreeSet<CallableTemplateOrigin>,
    owner: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    work(meter, owners.len())?;
    if !owners.contains(&owner) {
        // Overlapping nested source roots share declaration-side obligations.
        resources::insert(owners, owner, meter)?;
    }
    Ok(())
}
