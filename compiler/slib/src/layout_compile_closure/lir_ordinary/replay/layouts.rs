use super::*;
use scoop_identity::{CanonicalScoopAbiFunctionSignature, PersistentExactTypeId};

pub(super) fn check(
    declaration: DependencyCallableDeclarationId,
    signature: &CanonicalScoopAbiFunctionSignature,
    target: lir::LirTargetProfile,
    layouts: &Layouts<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let exact = signature.signature();
    meter.charge_work(signature.arguments().len() as u64 + 1, &WirePath::root())?;
    for (exact, expected) in exact
        .receiver()
        .into_option()
        .into_iter()
        .chain(exact.parameters().iter().copied())
        .zip(signature.arguments())
    {
        if let Some(value) = value(declaration, layouts, exact, meter)? {
            let actual = value.scoop_abi_argument(target).map_err(|source| {
                abi_error(declaration, lir::ExactCallableAbiError::Abi(source))
            })?;
            if &actual != expected {
                return Err(Error::LayoutSignature { declaration, exact });
            }
        }
    }
    if let Some(value) = value(declaration, layouts, exact.result(), meter)? {
        let actual = value
            .scoop_abi_return(target)
            .map_err(|source| abi_error(declaration, lir::ExactCallableAbiError::Abi(source)))?;
        if actual != signature.result() {
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
    meter: &mut BudgetMeter,
) -> Result<Option<&'a lir::ExactValueLayoutV1>, Error> {
    let record = layouts
        .value_if_present(exact, meter)
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
