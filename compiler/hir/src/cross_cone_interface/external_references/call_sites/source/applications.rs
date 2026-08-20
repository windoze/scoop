use super::*;
use crate::HirDependencyCallInstantiationV1;
use scoop_identity::{
    CallableApplicationKey, CallableArguments, CallableInstantiationOwner, ExactTypeKey,
    ValidatedIdentityGraph,
};

pub(super) fn bindings(
    instantiation: HirDependencyCallInstantiationV1,
    source: &CallableDeclarationRecordV1,
    metadata: SharedTypeMetadataV1<'_>,
    identities: &ValidatedIdentityGraph,
) -> Result<Vec<Vec<PersistentExactTypeId>>, HirDependencyCallSignatureError> {
    use HirDependencyCallSignatureError as Error;
    let declaration = source.declaration();
    let own_arity = source.type_parameters().binders().len();
    let owner = source.owner().nominal_owner();
    let HirDependencyCallInstantiationV1::Application(id) = instantiation else {
        return if own_arity != 0 || matches!(owner, Some(SourceNominalId::GenericTemplate(_))) {
            Err(Error::GenericDeclaration(declaration))
        } else {
            Ok(Vec::new())
        };
    };
    let application = identities
        .canonical_key::<_, CallableApplicationKey>(id)
        .map_err(SharedTypeMetadataError::from)?;
    if application.origin() != declaration {
        return Err(Error::ApplicationOrigin {
            expected: declaration,
            actual: application.origin(),
        });
    }
    let owner_arguments = match (owner, application.instantiation_owner()) {
        (None, CallableInstantiationOwner::NoOwner) => Vec::new(),
        (Some(owner), CallableInstantiationOwner::ExactNominalOwner(exact)) => {
            let key = identities
                .canonical_key::<_, ExactTypeKey>(exact)
                .map_err(SharedTypeMetadataError::from)?;
            match (owner, key.as_ref()) {
                (SourceNominalId::Concrete(expected), ExactTypeKey::Nominal(actual))
                    if expected == *actual =>
                {
                    Vec::new()
                }
                (
                    SourceNominalId::GenericTemplate(expected),
                    ExactTypeKey::NominalApplication { origin, arguments },
                ) if expected == *origin => {
                    let declaration = metadata
                        .public
                        .nominal_interfaces()
                        .declaration(owner)
                        .ok_or(Error::ApplicationOwner(source.declaration()))?;
                    let expected = declaration.type_parameters().binders().len();
                    if arguments.as_slice().len() != expected {
                        return Err(Error::ApplicationArity {
                            expected,
                            actual: arguments.as_slice().len(),
                        });
                    }
                    arguments.as_slice().to_vec()
                }
                _ => return Err(Error::ApplicationOwner(declaration)),
            }
        }
        _ => return Err(Error::ApplicationOwner(declaration)),
    };
    let callable_arguments = match application.callable_arguments() {
        CallableArguments::NoCallableArguments => &[][..],
        CallableArguments::Arguments(arguments) => arguments.as_slice(),
    };
    if callable_arguments.len() != own_arity {
        return Err(Error::ApplicationArity {
            expected: own_arity,
            actual: callable_arguments.len(),
        });
    }
    if callable_arguments.is_empty() && owner_arguments.is_empty() {
        return Err(Error::RedundantApplication(declaration));
    }
    let mut scopes = Vec::new();
    if !callable_arguments.is_empty() {
        scopes.push(callable_arguments.to_vec());
    }
    if !owner_arguments.is_empty() {
        scopes.push(owner_arguments);
    }
    Ok(scopes)
}
