use super::*;

pub(super) fn validate(
    inputs: &HirFunctionIdentityInputs<'_>,
    function: FunctionId,
    identities: &[crate::HirDerivedEqualityFunctionIdentity],
    nominal_derived: bool,
) -> Result<(), HirFunctionIdentityError> {
    let applications = inputs
        .derived_equality_applications
        .iter()
        .filter(|(_, application)| application.function == function)
        .collect::<Vec<_>>();
    if applications.is_empty() && !nominal_derived {
        return Err(HirFunctionIdentityError::UnownedDerivedEqualityTemplate {
            function: raw_index(function),
        });
    }
    let expected = applications
        .into_iter()
        .filter_map(|(application, declaration)| {
            match inputs.type_identities.get(declaration.owner_ty) {
                Some(crate::HirTypeIdentity::Exact(owner)) => Some(Ok((application, owner.id()))),
                Some(crate::HirTypeIdentity::Open(_)) => None,
                None => Some(Err(
                    HirFunctionIdentityError::MissingDerivedEqualityOwnerType {
                        application: raw_index(application),
                    },
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if identities.len() != expected.len() {
        return Err(HirFunctionIdentityError::DerivedEqualityIdentity {
            function: raw_index(function),
        });
    }
    for (identity, (application, exact_owner)) in identities.iter().zip(expected) {
        if identity.application() != application
            || identity.record().key() != &(GeneratedCallableKey::DerivedEquality { exact_owner })
        {
            return Err(HirFunctionIdentityError::DerivedEqualityIdentity {
                function: raw_index(function),
            });
        }
    }
    Ok(())
}
