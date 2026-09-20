use super::*;
use crate::cross_cone_interface::SignatureNominalWalker;
use crate::production::{
    callable_interfaces::{source_constructor_effects, source_parameter_shapes},
    signatures::HirInterfaceSignatureProjector,
    type_semantics::nominals::declaration_access_for_subject,
};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, DuplicateSignatureKey};

pub(super) fn project(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
    source: Constructor<'_>,
    meter: &mut BudgetMeter,
) -> Result<NominalSupportConstructorInterfaceV1, Error> {
    if export.type_identities[source.result]
        .exact()
        .map(|identity| identity.id())
        != Some(nominal.exact)
    {
        return Err(Error::MissingConstructor(nominal.exact));
    }
    let DuplicateSignatureKey::Constructor {
        parameters: expected,
    } = source.key.duplicate_signature()
    else {
        return Err(invalid(
            "constructor source identity has another declaration kind",
        ));
    };
    let path = WirePath::root();
    meter.check_semantic_depth(3, &path).map_err(resource)?;
    meter
        .check_table_entries(expected.len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(expected.len() as u64, &path)
        .map_err(resource)?;
    for signature in expected {
        let mut walker = SignatureNominalWalker::new(signature, meter, &path).map_err(resource)?;
        while walker.next(meter, &path).map_err(resource)?.is_some() {
            meter.charge_work(1, &path).map_err(resource)?;
        }
    }
    // Both this accounting pass and the shared projector inspect the sealed
    // parameter interface, before allocating names or signature copies.
    meter
        .charge_work(
            (export.source_parameter_interfaces.len() as u64).saturating_mul(2),
            &path,
        )
        .map_err(resource)?;
    for interface in &export.source_parameter_interfaces {
        if interface.owner == source.parameters {
            for parameter in &interface.parameters {
                meter
                    .check_semantic_leaf(parameter.name.len() as u64, &path)
                    .map_err(resource)?;
                meter
                    .charge_owned_bytes(parameter.name.len() as u64, &path)
                    .map_err(resource)?;
                meter
                    .charge_work(parameter.name.len() as u64, &path)
                    .map_err(resource)?;
            }
        }
    }
    let projector = HirInterfaceSignatureProjector::new(export);
    let parameters = source_parameter_shapes(export, &projector, source.parameters, &[], expected)
        .map_err(invalid)?;
    let result = projector.map_type(source.result, &[]).map_err(invalid)?;
    let owners = source.key.owners().owners().len() as u64;
    meter
        .check_semantic_depth(owners.saturating_add(1), &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(owners, &path)
        .map_err(resource)?;
    meter.charge_work(owners, &path).map_err(resource)?;
    let access = declaration_access_for_subject(
        export,
        source.key,
        DefinitionOriginSubject::Constructor(source.declaration),
        source.visibility.into(),
    )?;
    let payload = NominalSourceCallablePayloadV1::try_new(
        CallableTemplateOrigin::Constructor(source.declaration),
        SourceNominalId::Concrete(nominal.owner),
        CanonicalBinderListV1::try_new(Vec::new()).map_err(invalid)?,
        parameters,
        result,
        source_constructor_effects(source.safety, source.gc_effect).map_err(invalid)?,
        CallableModalityV1::Final,
        CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(invalid)?,
    )
    .map_err(invalid)?;
    NominalSupportConstructorInterfaceV1::try_new(source.declaration, access, payload)
        .map_err(invalid)
}
