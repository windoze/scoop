//! Structural exact-type leaves, shared by the writer and the bytes reader.

use scoop_identity::{ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::{borrow::Borrow, collections::BTreeSet};

pub fn collect_type_site_nominals<K: Borrow<ExactTypeKey>, E>(
    root: PersistentExactTypeId,
    mut lookup: impl FnMut(PersistentExactTypeId) -> Result<K, E>,
    meter: &mut BudgetMeter,
) -> Result<Vec<NominalDeclarationOwner>, HirTypeSiteExactError<E>> {
    let path = WirePath::root();
    let mut pending = Vec::new();
    let mut seen = BTreeSet::new();
    let mut nominals = Vec::new();
    push(&mut pending, root, 1, meter)?;
    while let Some((exact, depth)) = pending.pop() {
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_work(1 + u64::from(seen.len().max(1).ilog2()), &path)?;
        if seen.contains(&exact) {
            continue;
        }
        meter.check_table_entries(seen.len() as u64 + 1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_collection_slots(1, &path)?;
        meter.charge_owned_bytes(
            (std::mem::size_of::<PersistentExactTypeId>() + 32) as u64,
            &path,
        )?;
        seen.insert(exact);
        let key = lookup(exact).map_err(HirTypeSiteExactError::Identity)?;
        let owner = match key.borrow() {
            ExactTypeKey::Nominal(owner) => Some(NominalDeclarationOwner::Concrete(*owner)),
            ExactTypeKey::NominalApplication { origin, arguments } => {
                for child in arguments.as_slice() {
                    push(&mut pending, *child, depth.saturating_add(1), meter)?;
                }
                Some(NominalDeclarationOwner::GenericTemplate(*origin))
            }
            ExactTypeKey::Tuple(elements) => {
                for child in elements.as_slice() {
                    push(&mut pending, *child, depth.saturating_add(1), meter)?;
                }
                None
            }
            ExactTypeKey::RawPointer(child) => {
                push(&mut pending, *child, depth.saturating_add(1), meter)?;
                None
            }
            ExactTypeKey::Function {
                parameters, result, ..
            }
            | ExactTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for child in parameters.iter().chain(std::iter::once(result)) {
                    push(&mut pending, *child, depth.saturating_add(1), meter)?;
                }
                None
            }
        };
        if let Some(owner) = owner {
            meter
                .charge_owned_bytes(std::mem::size_of::<NominalDeclarationOwner>() as u64, &path)?;
            meter.try_reserve_collection_slots(&mut nominals, 1, &path)?;
            nominals.push(owner);
        }
    }
    let count = nominals.len() as u64;
    meter.charge_work(
        count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
        &path,
    )?;
    nominals.sort_unstable();
    nominals.dedup();
    Ok(nominals)
}

fn push<E>(
    pending: &mut Vec<(PersistentExactTypeId, u64)>,
    exact: PersistentExactTypeId,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), HirTypeSiteExactError<E>> {
    let path = WirePath::root();
    meter.charge_edges(1, &path)?;
    meter.charge_owned_bytes(
        std::mem::size_of::<(PersistentExactTypeId, u64)>() as u64,
        &path,
    )?;
    meter.try_reserve_collection_slots(pending, 1, &path)?;
    pending.push((exact, depth));
    Ok(())
}

#[derive(Debug)]
pub enum HirTypeSiteExactError<E> {
    Identity(E),
    Resource(WireError),
}
impl<E> From<WireError> for HirTypeSiteExactError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for HirTypeSiteExactError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for HirTypeSiteExactError<E> {}
