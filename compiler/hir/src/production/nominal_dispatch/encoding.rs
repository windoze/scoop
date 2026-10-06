use super::*;

pub(in crate::production) fn project(
    export: &ExportHir,
    owner: NominalOwner,
) -> Result<Option<NominalElementEncodingV1>, Error> {
    let (encoding, parameters) = match owner {
        NominalOwner::Class(id) => (
            &export.classes[id].element_encoding,
            &export.classes[id].type_params,
        ),
        NominalOwner::Enum(id) => (
            &export.enums[id].element_encoding,
            &export.enums[id].type_params,
        ),
        _ => return Ok(None),
    };
    let Some(encoding) = encoding else {
        return Ok(None);
    };
    let mut projection = Projection::new(export);
    projection.binders = super::super::signatures::HirInterfaceSignatureProjector::new(export)
        .binder_frame(parameters, 0)
        .map_err(invalid)?;
    let host = projection.declaration_type(owner);
    let mut selections = Selections::new();
    projection.implementation_selections(
        std::slice::from_ref(&encoding.implementation),
        false,
        host,
        &mut selections,
    )?;
    let records = selections
        .into_iter()
        .map(|((role, slot), (selection, receiver))| {
            NominalDispatchSelectionV1::new(role, receiver, slot, selection)
        })
        .collect();
    Ok(Some(NominalElementEncodingV1::new(
        projection.type_key(encoding.element)?,
        projection.type_key(encoding.implementation.interface)?,
        CanonicalNominalDispatchSelectionsV1::try_new(records).map_err(invalid)?,
    )))
}
