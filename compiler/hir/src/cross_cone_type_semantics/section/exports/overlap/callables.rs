use super::*;

pub(super) fn validate<E>(
    declaration: CallableTemplateOrigin,
    access: &DeclarationAccessSourceV1,
    source: &NominalSourceCallablePayloadV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    lookup(public.callable_interfaces().records().len(), meter, path)?;
    let old = public.callable_interfaces().get(declaration);
    let visible = effective_public(access, graph, meter, path)?;
    let restricted_setter = if let CallableTemplateOrigin::Accessor(id) = declaration {
        sequence(public.property_interfaces().records().len(), meter, path)?;
        public
            .property_interfaces()
            .records()
            .iter()
            .any(|property| {
                property.owner().nominal_owner() == Some(source.owner())
                    && property.capability().setter() == Some(id)
                    && property.capability().setter_access()
                        == Some(PropertySetterPublicAccessV1::Restricted)
            })
    } else {
        false
    };
    if !visible && !restricted_setter {
        return require(old.is_none());
    }
    if restricted_setter {
        require(public_owners(access, graph, meter, path)?)?;
    }
    let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
    require(
        old.owner().nominal_owner() == Some(source.owner())
            && old.receiver().is_none()
            && old.effects() == source.effects()
            && old.modality() == source.modality(),
    )?;
    require(binders(
        old.type_parameters(),
        source.type_parameters(),
        meter,
        path,
    )?)?;
    require(parameters(
        old.parameters(),
        source.parameters(),
        meter,
        path,
    )?)?;
    require(signature(old.result(), source.result(), meter, path)?)?;
    let public_slot = !source.slot_relations().is_empty();
    require((old.access() == PublicLookupAccessV1::PublicSlot) == public_slot)
}

fn parameters(
    left: &CanonicalSourceParameterShapesV1,
    right: &CanonicalSourceParameterShapesV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    sequence(left.parameters().len(), meter, path)?;
    sequence(right.parameters().len(), meter, path)?;
    if left.len_u32() != right.len_u32() {
        return Ok(false);
    }
    for (index, (left, right)) in left.parameters().iter().zip(right.parameters()).enumerate() {
        let at = path.clone().index(index as u64);
        text(left.name().as_str(), meter, &at)?;
        text(right.name().as_str(), meter, &at)?;
        if left.name() != right.name()
            || !signature(left.value_type(), right.value_type(), meter, &at)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
