use super::*;
use scoop_identity::SignatureTypeKey;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
) -> Result<CanonicalProtectedCallableSourceInterfacesV1, Error> {
    let sources = super::super::nominal_parameters::project(export, required)?;
    let mut records = Vec::new();
    for source in sources {
        let (owner, parameters) = source.into_parts();
        let count = parameters.len();

        let mut candidate = Vec::new();
        scoop_wire::allocation::try_reserve(&mut candidate, count, &WirePath::root())
            .map_err(resource)?;
        for (position, parameter) in parameters.into_iter().enumerate() {
            let position = u32::try_from(position).map_err(invalid)?;
            let (shape, kind, origin) = parameter.into_parts();
            let (name, value_type) = shape.into_parts();

            let calling = calling(owner, position, kind, &value_type)?;
            candidate.push(ProtectedSourceParameterV1::new(
                name, value_type, calling, origin,
            ));
        }
        let parameters =
            CanonicalProtectedSourceParametersV1::try_new(candidate).map_err(invalid)?;
        resources::push(
            &mut records,
            ProtectedCallableSourceInterfaceV1::try_new(owner, parameters).map_err(invalid)?,
        )?;
    }

    CanonicalProtectedCallableSourceInterfacesV1::try_new(records).map_err(invalid)
}
fn calling(
    owner: CallableTemplateOrigin,
    position: u32,
    kind: ProtectedParameterCallingKindV1,
    value_type: &SignatureTypeKey,
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
