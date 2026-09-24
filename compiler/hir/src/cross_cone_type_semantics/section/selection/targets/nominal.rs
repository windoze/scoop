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
        CheckedTypeSelectionDefinitionV1<'a>,
        CheckedExactTypeFactV1<'a>,
    ),
    Error<E>,
> {
    meter.charge_work(3, path)?;
    let key = foundation.exact_type_key(exact).map_err(Error::Source)?;
    let ExactTypeKey::Nominal(owner) = key else {
        return Err(Error::RequiresOdr(exact));
    };
    let facts = provider
        .facts
        .get_checked(exact)
        .ok_or(Error::MissingTarget(request))?;
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        meter.charge_work(1, path)?;
        let declaration = builtin.identity_record();
        if *owner != declaration.id() {
            continue;
        }
        if declaration.key().origin() != request.provider()
            || provider.provider != request.provider()
        {
            return Err(Error::DeclarationOwner);
        }
        if !matches!(
            request.usage(),
            SelectedTypeUseV1::Signature { .. }
                | SelectedTypeUseV1::Representation { .. }
                | SelectedTypeUseV1::TypeTest { .. }
                | SelectedTypeUseV1::ShapeSupport { .. }
        ) {
            return Err(Error::MissingTarget(request));
        }
        let (kind, gc) = match builtin {
            CoreBuiltinNominal::Unit => (
                ExactTypeKindV1::Value {
                    zst: ZstStatus::ZeroSized,
                },
                ExactTypeGcV1::GcFree,
            ),
            CoreBuiltinNominal::Any => (
                ExactTypeKindV1::Reference,
                ExactTypeGcV1::ContainsManagedReferences,
            ),
        };
        if facts.record().kind() != kind || facts.record().gc() != gc {
            return Err(Error::BuiltinFacts(builtin));
        }
        return Ok((
            CheckedTypeSelectionDefinitionV1::LanguageBuiltin(builtin),
            facts,
        ));
    }
    let nominal = provider
        .representations
        .get(*owner)
        .ok_or(Error::MissingTarget(request))?;
    let inheritance = provider
        .inheritance
        .table()
        .get(exact)
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
    Ok((
        CheckedTypeSelectionDefinitionV1::SourceNominal {
            representation: nominal,
            inheritance,
        },
        facts,
    ))
}
