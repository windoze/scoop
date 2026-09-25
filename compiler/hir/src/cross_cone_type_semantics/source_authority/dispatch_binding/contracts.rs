use super::*;
use scoop_identity::{AccessorRole, DefinitionOriginSubject, PropertyOwner};

use super::super::foundation::binding::sources::AccessAuthority;

pub(super) fn validate(
    bound: &BoundInheritanceDispatchSourcesV1<'_, '_>,
) -> Result<(), InheritanceDispatchBindingError> {
    use InheritanceDispatchBindingError as Error;
    let source = bound.foundation;
    let entries = source.source().entries();

    for record in bound.callables.records() {
        let declaration = record.declaration();
        let (key, subject) = declaration_key(bound, declaration)?;

        if key.origin() != entries.provider {
            return Err(Error::ForeignDeclaration(declaration));
        }
        let access = record.declaration_access();
        let origin = access.definition_origin();

        if source
            .foundation
            .definition_origin(subject)
            .map(|record| record.origin())
            != Some(origin.origin())
        {
            return Err(Error::DefinitionOrigin(declaration));
        }

        access
            .validate_for_declaration(key, &mut AccessAuthority(source))
            .map_err(|error| Error::Access {
                declaration,
                reason: error.to_string(),
            })?;
        let signature = record.signature().exact_signature();

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
