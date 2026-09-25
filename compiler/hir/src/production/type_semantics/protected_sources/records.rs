use super::*;

type Projection = (
    CanonicalProtectedDeclarationInterfacesV1,
    BTreeSet<CallableTemplateOrigin>,
);
pub(super) fn project(
    output: &ExportHirOutput,
    required: &CanonicalProtectedDeclarationRefsV1,
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
                resources::insert(&mut callables, id)?;
                if !matches!(id, CallableTemplateOrigin::Accessor(_)) {
                    require_protocol(&mut protocols, id)?;
                }
            }
            ProtectedDeclarationRefV1::Constructor(id) => {
                resources::insert(&mut constructors, *id)?;
                require_protocol(&mut protocols, CallableTemplateOrigin::Constructor(*id))?;
            }
            ProtectedDeclarationRefV1::Property(id) => resources::insert(&mut properties, *id)?,
            ProtectedDeclarationRefV1::NestedNominal(id) => resources::insert(&mut nested, *id)?,
        }
    }
    let mut records = Vec::new();
    let export = output.module();
    for source in super::super::nominal_callables::project(export, callables)? {
        let record = ProtectedCallableInterfaceV1::try_from(source).map_err(invalid)?;

        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Callable(Box::new(record)),
        )?;
    }
    for source in super::super::nominal_constructors::project(export, constructors)? {
        let record = ProtectedConstructorInterfaceV1::try_from(source).map_err(invalid)?;

        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Constructor(Box::new(record)),
        )?;
    }

    let properties =
        CanonicalPersistentIdsV1::try_new(properties.into_iter().collect()).map_err(invalid)?;
    for source in
        super::super::inheritance::source_properties::project_nominal(export, &properties)?
    {
        let record = ProtectedPropertyInterfaceV1::try_from(source).map_err(invalid)?;

        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::Property(Box::new(record)),
        )?;
    }
    for owner in nested {
        let (source, owners) = project_record(output, owner)?;
        for id in owners {
            require_protocol(&mut protocols, id)?;
        }
        let record = ProtectedNestedNominalInterfaceV1::try_from(source).map_err(invalid)?;

        resources::push(
            &mut records,
            ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(record)),
        )?;
    }

    Ok((
        CanonicalProtectedDeclarationInterfacesV1::try_new(records).map_err(invalid)?,
        protocols,
    ))
}

fn require_protocol(
    owners: &mut BTreeSet<CallableTemplateOrigin>,
    owner: CallableTemplateOrigin,
) -> Result<(), Error> {
    if !owners.contains(&owner) {
        // Overlapping nested source roots share declaration-side obligations.
        resources::insert(owners, owner)?;
    }
    Ok(())
}
