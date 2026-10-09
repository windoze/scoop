use super::*;
use scoop_identity::{CanonicalScoopAbiFunctionSignature, PersistentExactTypeId};

pub(super) fn check(
    declaration: DependencyCallableDeclarationId,
    signature: &CanonicalScoopAbiFunctionSignature,
    _target: lir::LirTargetProfile,
    layouts: &Layouts<'_>,
) -> Result<(), Error> {
    let exact = signature.signature();

    for (exact, expected) in exact
        .receiver()
        .into_option()
        .into_iter()
        .chain(exact.parameters().iter().copied())
        .zip(signature.arguments())
    {
        if let Some(value) = value(declaration, layouts, exact)? {
            let actual = value.canonical_storage();
            if actual != expected.storage() {
                return Err(Error::LayoutSignature { declaration, exact });
            }
        }
    }
    if let Some(value) = value(declaration, layouts, exact.result())? {
        let is_unit = matches!(
            value.representation().kind(),
            lir::ExactRepresentationKindV1::IntrinsicValue(lir::IntrinsicValueFamilyV1::Unit)
        );
        let matches = if is_unit {
            signature.result() == scoop_identity::ScoopAbiReturn::UnitVoid
        } else {
            signature.result().storage() == Some(value.canonical_storage())
        };
        if !matches {
            return Err(Error::LayoutSignature {
                declaration,
                exact: exact.result(),
            });
        }
    }
    Ok(())
}

fn value<'a>(
    declaration: DependencyCallableDeclarationId,
    layouts: &'a Layouts<'_>,
    exact: PersistentExactTypeId,
) -> Result<Option<&'a lir::ExactValueLayoutV1>, Error> {
    let record = layouts
        .value_if_present(exact)
        .map_err(|source| abi_error(declaration, source))?;
    match record.map(lir::ExactLayoutExportV1::kind) {
        Some(lir::ExactLayoutBodyKindV1::Value(value)) => Ok(Some(value)),
        Some(lir::ExactLayoutBodyKindV1::Instance(_)) => Err(abi_error(
            declaration,
            lir::ExactCallableAbiError::LayoutRole,
        )),
        None => Ok(None),
    }
}

fn abi_error(
    declaration: DependencyCallableDeclarationId,
    source: lir::ExactCallableAbiError,
) -> Error {
    Error::Abi {
        declaration,
        source: Box::new(source),
    }
}
