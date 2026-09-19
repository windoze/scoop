use super::*;

pub(super) fn resolve<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    provider: &Exports<'a>,
    exact: PersistentExactTypeId,
    request: SelectedExternalTypeUseV1,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<
    (
        &'a NominalRepresentationSupportV1,
        &'a NominalInheritanceInterfaceV1,
        CheckedExactTypeFactV1<'a>,
    ),
    Error<E>,
> {
    meter.charge_work(3, path)?;
    let key = foundation.exact_type_key(exact).map_err(Error::Source)?;
    let ExactTypeKey::Nominal(owner) = key else {
        return Err(Error::RequiresOdr(exact));
    };
    let nominal = provider
        .representations
        .get(*owner)
        .ok_or(Error::MissingTarget(request))?;
    let inheritance = provider
        .inheritance
        .table()
        .get(exact)
        .ok_or(Error::MissingTarget(request))?;
    let facts = provider
        .facts
        .get_checked(exact)
        .ok_or(Error::MissingTarget(request))?;
    let source = provider
        .graph
        .source(SourceNominalId::Concrete(*owner))
        .ok_or(Error::DeclarationOwner)?;
    if source.key.origin() != request.provider()
        || source.access.definition_origin().origin().source().cone() != request.provider()
    {
        return Err(Error::DeclarationOwner);
    }
    Ok((nominal, inheritance, facts))
}
