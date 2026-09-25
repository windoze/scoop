use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SignatureTypeKey};
use scoop_wire::WirePath;

use super::InheritanceSlotContractSemanticError as Error;
use crate::NominalInheritanceSemanticAuthority;

pub(super) fn validate_exact_identity<A: NominalInheritanceSemanticAuthority<E>, E>(
    exact: PersistentExactTypeId,
    authority: &A,
) -> Result<(), Error<E>> {
    let key = authority.exact_type_key(exact).map_err(Error::Foundation)?;

    if PersistentExactTypeId::from_key(key).ok() != Some(exact) {
        return Err(Error::Signature);
    }
    Ok(())
}

pub(super) fn match_parameters<A: NominalInheritanceSemanticAuthority<E>, E>(
    source: &[SignatureTypeKey],
    exact: &[PersistentExactTypeId],
    authority: &A,
) -> Result<(), Error<E>> {
    if source.len() != exact.len() {
        return Err(Error::Signature);
    }
    let path = WirePath::root();
    let mut pending = Vec::new();
    scoop_wire::allocation::try_reserve(&mut pending, source.len(), &path)
        .map_err(Error::Resource)?;
    pending.extend(
        source
            .iter()
            .zip(exact)
            .map(|(source, exact)| (source, *exact)),
    );
    while let Some((source, exact)) = pending.pop() {
        validate_exact_identity(exact, authority)?;
        let key = authority.exact_type_key(exact).map_err(Error::Foundation)?;
        match (source, key) {
            (SignatureTypeKey::Nominal(left), ExactTypeKey::Nominal(right)) if left == right => {}
            (
                SignatureTypeKey::NominalApplication {
                    origin: left,
                    arguments: source,
                },
                ExactTypeKey::NominalApplication {
                    origin: right,
                    arguments: exact,
                },
            ) if left == right => {
                push(&mut pending, source.as_slice(), exact.as_slice())?;
            }
            (SignatureTypeKey::Tuple(source), ExactTypeKey::Tuple(exact)) => {
                push(&mut pending, source.as_slice(), exact.as_slice())?
            }
            (SignatureTypeKey::RawPointer(source), ExactTypeKey::RawPointer(exact)) => {
                push_one(&mut pending, source, *exact)?
            }
            (
                SignatureTypeKey::Function {
                    effect: left,
                    parameters: source,
                    result: source_result,
                },
                ExactTypeKey::Function {
                    effect: right,
                    parameters: exact,
                    result: exact_result,
                },
            ) if left == right => {
                push(&mut pending, source, exact)?;
                push_one(&mut pending, source_result, *exact_result)?;
            }
            (
                SignatureTypeKey::NativeFunctionPointer {
                    calling_convention: left,
                    parameters: source,
                    result: source_result,
                },
                ExactTypeKey::NativeFunctionPointer {
                    calling_convention: right,
                    parameters: exact,
                    result: exact_result,
                },
            ) if left == right => {
                push(&mut pending, source, exact)?;
                push_one(&mut pending, source_result, *exact_result)?;
            }
            _ => return Err(Error::Signature),
        }
    }
    Ok(())
}

impl crate::NominalRepresentationSupportV1 {
    /// Uses the same exact/source type relation as callable contracts. The
    /// source sequence is retained in declaration order by representation data.
    pub(in crate::cross_cone_type_semantics) fn validate_exact_field_types<
        A: NominalInheritanceSemanticAuthority<E>,
        E,
    >(
        source: &[SignatureTypeKey],
        exact: &[PersistentExactTypeId],
        authority: &A,
    ) -> Result<(), Error<E>> {
        match_parameters(source, exact, authority)
    }
}

fn push<'s, E>(
    pending: &mut Vec<(&'s SignatureTypeKey, PersistentExactTypeId)>,
    source: &'s [SignatureTypeKey],
    exact: &[PersistentExactTypeId],
) -> Result<(), Error<E>> {
    if source.len() != exact.len() {
        return Err(Error::Signature);
    }
    scoop_wire::allocation::try_reserve(pending, source.len(), &WirePath::root())
        .map_err(Error::Resource)?;

    pending.extend(
        source
            .iter()
            .zip(exact)
            .map(|(source, exact)| (source, *exact)),
    );
    Ok(())
}
fn push_one<'s, E>(
    pending: &mut Vec<(&'s SignatureTypeKey, PersistentExactTypeId)>,
    source: &'s SignatureTypeKey,
    exact: PersistentExactTypeId,
) -> Result<(), Error<E>> {
    scoop_wire::allocation::try_reserve(pending, 1, &WirePath::root()).map_err(Error::Resource)?;

    pending.push((source, exact));
    Ok(())
}
