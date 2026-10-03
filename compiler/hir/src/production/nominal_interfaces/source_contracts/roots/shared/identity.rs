use super::*;

pub(super) struct Scope {
    pub provider: ConeIdentity,
    pub owner: PublicDeclarationOwnerV1,
}

pub(super) fn scope(key: &SourceDeclarationKey) -> Result<Scope, Error> {
    let owner = match key.owners().owners().last() {
        Some(DefinitionOwnerAtom::Type(id)) => {
            PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(*id))
        }
        Some(DefinitionOwnerAtom::GenericType(id)) => {
            PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(*id))
        }
        None if key.duplicate_signature().receiver_is_present() => {
            PublicDeclarationOwnerV1::Extension
        }
        None => PublicDeclarationOwnerV1::TopLevel,
        Some(_) => {
            return Err(invalid(
                "source declaration has a non-nominal lexical owner",
            ));
        }
    };
    Ok(Scope {
        provider: key.origin(),
        owner,
    })
}

pub(super) fn callable(
    export: &ExportHir,
    owner: ExportParameterOwner,
) -> Result<Option<(CallableTemplateOrigin, Scope)>, Error> {
    let (id, key) = match owner {
        ExportParameterOwner::Function(id) => {
            let Some(HirFunctionIdentity::Source(source)) = export.function_identities.get(id)
            else {
                return Ok(None);
            };
            if matches!(
                source.declaration().scope(),
                scoop_identity::DeclarationScope::LexicalScoped { .. }
            ) {
                return Ok(None);
            }
            let id = match source {
                HirSourceFunctionIdentity::Plain(record) => {
                    CallableTemplateOrigin::Function(record.id())
                }
                HirSourceFunctionIdentity::Generic(record) => {
                    CallableTemplateOrigin::GenericFunction(record.id())
                }
            };
            (id, source.declaration())
        }
        ExportParameterOwner::StructConstructor(id) => {
            let source = export
                .constructor_identities
                .get_struct(id)
                .ok_or_else(|| invalid("source constructor identity is absent"))?;
            (
                CallableTemplateOrigin::Constructor(source.id()),
                source.key(),
            )
        }
        ExportParameterOwner::ClassConstructor(id) => {
            let source = export
                .constructor_identities
                .get_class(id)
                .ok_or_else(|| invalid("class constructor identity is absent"))?;
            let Some(source) = source.source_record() else {
                return Ok(None);
            };
            (
                CallableTemplateOrigin::Constructor(source.id()),
                source.key(),
            )
        }
        ExportParameterOwner::VariantConstructor(variant) => {
            let source = export
                .nominal_identities
                .get_enum(variant.enumeration())
                .and_then(HirNominalIdentity::source)
                .ok_or_else(|| invalid("variant has no source enum identity"))?;
            let variant = export
                .enum_member_identities
                .get_variant(variant)
                .ok_or_else(|| invalid("variant constructor identity is absent"))?;
            return Ok(Some((
                CallableTemplateOrigin::VariantConstructor(variant.id()),
                Scope {
                    provider: source.declaration().origin(),
                    owner: PublicDeclarationOwnerV1::Nominal(source_nominal_id(source)),
                },
            )));
        }
    };
    Ok(Some((id, scope(key)?)))
}
