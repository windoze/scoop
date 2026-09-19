use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    InheritanceSlotContractSemanticAuthority, InheritanceSlotContractSemanticError as Error,
};

pub(super) fn validate_exact_identity<A: InheritanceSlotContractSemanticAuthority<E>, E>(
    exact: PersistentExactTypeId,
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<(), Error<E>> {
    let path = WirePath::root();
    meter.charge_work(1, &path).map_err(Error::Resource)?;
    let key = authority.exact_type_key(exact).map_err(Error::Foundation)?;
    let size = scoop_wire::encoded_length(key).map_err(Error::Encoding)?;
    meter.charge_sha256(size, &path).map_err(Error::Resource)?;
    if PersistentExactTypeId::from_key(key).ok() != Some(exact) {
        return Err(Error::Signature);
    }
    Ok(())
}

pub(super) fn match_parameters<A: InheritanceSlotContractSemanticAuthority<E>, E>(
    source: &[SignatureTypeKey],
    exact: &[PersistentExactTypeId],
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<(), Error<E>> {
    if source.len() != exact.len() {
        return Err(Error::Signature);
    }
    let path = WirePath::root();
    let mut pending = Vec::new();
    meter
        .try_reserve_collection_slots(&mut pending, source.len(), &path)
        .map_err(Error::Resource)?;
    pending.extend(
        source
            .iter()
            .zip(exact)
            .map(|(source, exact)| (source, *exact, 1)),
    );
    while let Some((source, exact, depth)) = pending.pop() {
        meter
            .check_semantic_depth(depth, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        validate_exact_identity(exact, authority, meter)?;
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
                push(
                    &mut pending,
                    source.as_slice(),
                    exact.as_slice(),
                    depth + 1,
                    meter,
                )?;
            }
            (SignatureTypeKey::Tuple(source), ExactTypeKey::Tuple(exact)) => push(
                &mut pending,
                source.as_slice(),
                exact.as_slice(),
                depth + 1,
                meter,
            )?,
            (SignatureTypeKey::RawPointer(source), ExactTypeKey::RawPointer(exact)) => {
                push_one(&mut pending, source, *exact, depth + 1, meter)?
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
                push(&mut pending, source, exact, depth + 1, meter)?;
                push_one(&mut pending, source_result, *exact_result, depth + 1, meter)?;
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
                push(&mut pending, source, exact, depth + 1, meter)?;
                push_one(&mut pending, source_result, *exact_result, depth + 1, meter)?;
            }
            _ => return Err(Error::Signature),
        }
    }
    Ok(())
}

fn push<'s, E>(
    pending: &mut Vec<(&'s SignatureTypeKey, PersistentExactTypeId, u64)>,
    source: &'s [SignatureTypeKey],
    exact: &[PersistentExactTypeId],
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), Error<E>> {
    if source.len() != exact.len() {
        return Err(Error::Signature);
    }
    meter
        .try_reserve_collection_slots(pending, source.len(), &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_edges(source.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    pending.extend(
        source
            .iter()
            .zip(exact)
            .map(|(source, exact)| (source, *exact, depth)),
    );
    Ok(())
}
fn push_one<'s, E>(
    pending: &mut Vec<(&'s SignatureTypeKey, PersistentExactTypeId, u64)>,
    source: &'s SignatureTypeKey,
    exact: PersistentExactTypeId,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), Error<E>> {
    meter
        .try_reserve_collection_slots(pending, 1, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_edges(1, &WirePath::root())
        .map_err(Error::Resource)?;
    pending.push((source, exact, depth));
    Ok(())
}
