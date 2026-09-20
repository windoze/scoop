use super::*;
use scoop_identity::{AccessorRole, DefinitionOriginSubject, PropertyOwner};

use super::super::foundation::binding::sources::AccessAuthority;

pub(super) fn validate(
    bound: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceDispatchBindingError> {
    use InheritanceDispatchBindingError as Error;
    let source = bound.foundation;
    let entries = source.source().entries();
    let key_count = bound
        .functions
        .len()
        .saturating_add(bound.properties.len())
        .saturating_add(entries.accessor_keys.values().len());
    charge_queries(
        bound.callables.records().len().saturating_mul(2),
        key_count,
        meter,
    )?;
    for record in bound.callables.records() {
        let declaration = record.declaration();
        let (key, subject) = declaration_key(bound, declaration)?;
        let path = WirePath::root();
        NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
        if key.origin() != entries.provider {
            return Err(Error::ForeignDeclaration(declaration));
        }
        let access = record.declaration_access();
        let origin = access.definition_origin();
        let owners = access.lexical_owners().len() as u64;
        meter.check_semantic_depth(owners.saturating_add(1), &path)?;
        charge_queries(
            access.lexical_owners().len().saturating_mul(2),
            entries.sources.records().len(),
            meter,
        )?;
        meter.charge_work(
            owners
                .saturating_add(1)
                .saturating_pow(2)
                .saturating_mul(64),
            &path,
        )?;
        meter.charge_work(
            u64::from(
                source
                    .foundation
                    .as_canonical()
                    .counts()
                    .definition_origins
                    .max(1)
                    .ilog2(),
            ) + 1,
            &path,
        )?;
        if source
            .foundation
            .definition_origin(subject)
            .map(|record| record.origin())
            != Some(origin.origin())
        {
            return Err(Error::DefinitionOrigin(declaration));
        }
        let source_bytes = origin.origin().source().logical_path().as_str().len() as u64;
        meter.charge_work(
            source_bytes.saturating_mul(
                u64::from(entries.definition_sources.sources().len().max(1).ilog2()) + owners + 1,
            ),
            &path,
        )?;
        access
            .validate_for_declaration(key, &mut AccessAuthority(source))
            .map_err(|error| Error::Access {
                declaration,
                reason: error.to_string(),
            })?;
        let signature = record.signature().exact_signature();
        charge(signature.parameters().len().saturating_add(2), meter)?;
        charge_queries(
            signature.parameters().len().saturating_add(2),
            entries.exact_keys.values().len(),
            meter,
        )?;
        for exact in signature
            .receiver()
            .into_option()
            .into_iter()
            .chain(signature.parameters().iter().copied())
            .chain(std::iter::once(signature.result()))
        {
            source.exact_type_key(exact)?;
        }
    }
    Ok(())
}

fn declaration_key<'a>(
    bound: &'a BoundInheritanceDispatchSourcesV1<'_, '_>,
    declaration: InheritanceCallableDeclarationV1,
) -> Result<(&'a SourceDeclarationKey, DefinitionOriginSubject), InheritanceDispatchBindingError> {
    use InheritanceCallableDeclarationV1 as Declaration;
    match declaration {
        Declaration::Function(id) => Ok((
            bound.function_key(id)?,
            DefinitionOriginSubject::Function(id),
        )),
        Declaration::Getter(id) | Declaration::Setter(id) => {
            let key = bound.accessor_key(id)?;
            let role = match declaration {
                Declaration::Getter(_) => AccessorRole::Getter,
                Declaration::Setter(_) => AccessorRole::Setter,
                Declaration::Function(_) => unreachable!("function handled above"),
            };
            let PropertyOwner::Property(property) = key.owner() else {
                return Err(InheritanceDispatchBindingError::AccessorRole(declaration));
            };
            if key.role() != role {
                return Err(InheritanceDispatchBindingError::AccessorRole(declaration));
            }
            Ok((
                bound.property_key(property)?,
                DefinitionOriginSubject::PropertyAccessor(id),
            ))
        }
    }
}
