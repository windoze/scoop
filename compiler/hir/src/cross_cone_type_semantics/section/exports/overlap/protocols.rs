use super::*;

pub(super) fn validate<E>(
    source: &CheckedProtectedSourceInterfaceV1<'_>,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,

    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    let old = public.source_interfaces().get(source.owner());
    if !effective_public(source.declaration_access(), graph)? {
        return require(old.is_none());
    }
    let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
    require(equal(source.protocol().record(), old, path)?)
}

pub(super) fn equal(
    new: &ProtectedCallableSourceInterfaceV1,
    old: &CallableSourceInterfaceV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    if new.owner() != old.owner() || new.parameters().len_u32() != old.parameters().len_u32() {
        return Ok(false);
    }
    for (index, (new, old)) in new
        .parameters()
        .parameters()
        .iter()
        .zip(old.parameters().parameters())
        .enumerate()
    {
        let at = path.clone().index(index as u64);

        if new.name() != old.name()
            || new.definition_origin() != old.definition_origin()
            || !signature(new.value_type(), old.value_type(), &at)?
            || !calling(new.calling(), old.calling(), &at)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

impl ProtectedCallableSourceInterfaceV1 {
    pub(in crate::cross_cone_type_semantics) fn matches_shared_interface(
        &self,
        shared: &CallableSourceInterfaceV1,

        path: &WirePath,
    ) -> Result<bool, WireError> {
        equal(self, shared, path)
    }
}

fn calling(
    new: &ProtectedParameterCallingV1,
    old: &CallableParameterCallingV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    let kinds_match = matches!(
        (new, old),
        (
            ProtectedParameterCallingV1::Required,
            CallableParameterCallingV1::Required
        ) | (
            ProtectedParameterCallingV1::Default { .. },
            CallableParameterCallingV1::Default { .. }
        ) | (
            ProtectedParameterCallingV1::VarargEmpty { .. },
            CallableParameterCallingV1::VarargEmpty { .. }
        ) | (
            ProtectedParameterCallingV1::VarargDefault { .. },
            CallableParameterCallingV1::VarargDefault { .. }
        )
    );
    if !kinds_match || !optional_signature(new.element_type(), old.element_type(), path)? {
        return Ok(false);
    }
    Ok(match (new.template(), old.template()) {
        (None, None) => true,
        (Some(new), Some(old)) => {
            new.owner() == old.owner() && new.parameter_position() == old.parameter_position()
        }
        _ => false,
    })
}
