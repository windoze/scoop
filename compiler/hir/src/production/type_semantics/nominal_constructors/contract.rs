use super::*;
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, DuplicateSignatureKey, SignatureTypeKey,
};

pub(super) fn project(
    export: &ExportHir,
    source: Constructor<'_>,
    meter: &mut BudgetMeter,
) -> Result<NominalSupportConstructorInterfaceV1, Error> {
    if source.key.origin() != export.cone || source.owner.declaration().origin() != export.cone {
        return Err(invalid(
            "nominal constructor source belongs to another Cone",
        ));
    }
    let owner = match source.owner {
        HirSourceNominalIdentity::Concrete(record) => SourceNominalId::Concrete(record.id()),
        HirSourceNominalIdentity::Generic(record) => SourceNominalId::GenericTemplate(record.id()),
    };
    let DuplicateSignatureKey::Constructor {
        parameters: expected,
    } = source.key.duplicate_signature()
    else {
        return Err(invalid(
            "constructor source identity has another declaration kind",
        ));
    };
    let path = WirePath::root();
    resources::binders(export, source.binders, source.binders.len(), meter)?;
    let projector = HirInterfaceSignatureProjector::new(export);
    let binders = projector.binder_frame(source.binders, 0).map_err(invalid)?;
    let parameters = super::super::source_parameter_shapes::project(
        export,
        &projector,
        source.parameters,
        &binders,
        expected,
        meter,
    )?;
    resources::ty(export, source.result, binders.len(), 3, meter)?;
    let result = projector
        .map_type(source.result, &binders)
        .map_err(invalid)?;
    meter
        .charge_work(binders.len() as u64 + 1, &path)
        .map_err(resource)?;
    let valid_result = match (owner, &result) {
        (SourceNominalId::Concrete(owner), SignatureTypeKey::Nominal(actual)) => owner == *actual,
        (SourceNominalId::GenericTemplate(owner), SignatureTypeKey::NominalApplication { origin, arguments }) => {
            owner == *origin && arguments.as_slice().len() == binders.len()
                && arguments.as_slice().iter().enumerate().all(|(index, argument)| {
                    matches!(argument, SignatureTypeKey::Binder { depth: 0, index: actual } if *actual as usize == index)
                })
        }
        _ => false,
    };
    if !valid_result {
        return Err(invalid(
            "constructor result differs from its source nominal owner",
        ));
    }
    let owners = source.key.owners().owners().len() as u64;
    meter
        .check_semantic_depth(owners + 1, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(owners, &path)
        .map_err(resource)?;
    meter.charge_work(owners, &path).map_err(resource)?;
    work(meter, export.export_definition_origins.records().len())?;
    let subject = DefinitionOriginSubject::Constructor(source.declaration);
    let origin = export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingDefinitionOrigin(subject))?;
    resources::name(origin.origin().source().logical_path().as_str(), meter)?;
    let access = super::super::nominals::declaration_access_for_subject(
        export,
        source.key,
        subject,
        source.visibility.into(),
    )?;
    let payload = NominalSourceCallablePayloadV1::try_new(
        CallableTemplateOrigin::Constructor(source.declaration),
        owner,
        CanonicalBinderListV1::try_new(Vec::new()).map_err(invalid)?,
        parameters,
        result,
        callable_interfaces::source_constructor_effects(source.safety, source.gc_effect)
            .map_err(invalid)?,
        CallableModalityV1::Final,
        CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(invalid)?,
    )
    .map_err(invalid)?;
    NominalSupportConstructorInterfaceV1::try_new(source.declaration, access, payload)
        .map_err(invalid)
}
