use super::*;
use scoop_identity::SignatureTypeKey;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
    meter: &mut BudgetMeter,
) -> Result<CanonicalProtectedCallableSourceInterfacesV1, Error> {
    let sources = super::super::nominal_parameters::project(export, required, meter)?;
    let mut records = Vec::new();
    for source in sources {
        let (owner, parameters) = source.into_parts();
        let count = parameters.len();
        resources::canonical(count, meter)?;
        let mut candidate = Vec::new();
        meter
            .try_reserve_collection_slots(&mut candidate, count, &WirePath::root())
            .map_err(resource)?;
        for (position, parameter) in parameters.into_iter().enumerate() {
            let position = u32::try_from(position).map_err(invalid)?;
            let (shape, kind, origin) = parameter.into_parts();
            let (name, value_type) = shape.into_parts();
            meter
                .charge_work(
                    (name.as_str().len() as u64)
                        .saturating_mul(u64::from(count.max(1).ilog2()) + 1),
                    &WirePath::root(),
                )
                .map_err(resource)?;
            let calling = calling(owner, position, kind, &value_type, meter)?;
            candidate.push(ProtectedSourceParameterV1::new(
                name, value_type, calling, origin,
            ));
        }
        let parameters =
            CanonicalProtectedSourceParametersV1::try_new(candidate).map_err(invalid)?;
        resources::push(
            &mut records,
            ProtectedCallableSourceInterfaceV1::try_new(owner, parameters).map_err(invalid)?,
            meter,
        )?;
    }
    resources::canonical(records.len(), meter)?;
    CanonicalProtectedCallableSourceInterfacesV1::try_new(records).map_err(invalid)
}
fn calling(
    owner: CallableTemplateOrigin,
    position: u32,
    kind: ProtectedParameterCallingKindV1,
    value_type: &SignatureTypeKey,
    meter: &mut BudgetMeter,
) -> Result<ProtectedParameterCallingV1, Error> {
    use ProtectedParameterCallingKindV1 as Kind;
    use ProtectedParameterCallingV1 as Calling;
    let template = || ProtectedDefaultTemplateKeyV1::try_new(owner, position).map_err(invalid);
    match kind {
        Kind::Required => Ok(Calling::Required),
        Kind::Default => Ok(Calling::Default {
            template: template()?,
        }),
        Kind::VarargEmpty | Kind::VarargDefault => {
            let SignatureTypeKey::NominalApplication { arguments, .. } = value_type else {
                return Err(invalid("source vararg value has no Array application"));
            };
            let [element] = arguments.as_slice() else {
                return Err(invalid("source vararg Array must have one type argument"));
            };
            resources::signature_copy(element, meter, 1)?;
            let element_type = element.clone();
            if kind == Kind::VarargEmpty {
                Ok(Calling::VarargEmpty { element_type })
            } else {
                Ok(Calling::VarargDefault {
                    element_type,
                    template: template()?,
                })
            }
        }
    }
}
