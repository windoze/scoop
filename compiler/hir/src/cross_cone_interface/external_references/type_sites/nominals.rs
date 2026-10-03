//! Structural exact-type leaves, shared by the writer and the bytes reader.

use scoop_identity::{ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId};
use scoop_wire::{WireError, WirePath};
use std::{borrow::Borrow, collections::BTreeSet};

pub fn collect_type_site_nominals<K: Borrow<ExactTypeKey>, E>(
    root: PersistentExactTypeId,
    mut lookup: impl FnMut(PersistentExactTypeId) -> Result<K, E>,
) -> Result<Vec<NominalDeclarationOwner>, HirTypeSiteExactError<E>> {
    let path = WirePath::root();
    let mut pending = Vec::new();
    let mut seen = BTreeSet::new();
    let mut nominals = Vec::new();
    push(&mut pending, root)?;
    while let Some(exact) = pending.pop() {
        if seen.contains(&exact) {
            continue;
        }

        seen.insert(exact);
        let key = lookup(exact).map_err(HirTypeSiteExactError::Identity)?;
        let owner = match key.borrow() {
            ExactTypeKey::Nominal(owner) => Some(NominalDeclarationOwner::Concrete(*owner)),
            ExactTypeKey::NominalApplication { origin, arguments } => {
                for child in arguments.as_slice() {
                    push(&mut pending, *child)?;
                }
                Some(NominalDeclarationOwner::GenericTemplate(*origin))
            }
            ExactTypeKey::Tuple(elements) => {
                for child in elements.as_slice() {
                    push(&mut pending, *child)?;
                }
                None
            }
            ExactTypeKey::RawPointer(child) => {
                push(&mut pending, *child)?;
                None
            }
            ExactTypeKey::Function {
                parameters, result, ..
            }
            | ExactTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for child in parameters.iter().chain(std::iter::once(result)) {
                    push(&mut pending, *child)?;
                }
                None
            }
        };
        if let Some(owner) = owner {
            scoop_wire::allocation::try_reserve(&mut nominals, 1, &path)?;
            nominals.push(owner);
        }
    }

    nominals.sort_unstable();
    nominals.dedup();
    Ok(nominals)
}

fn push<E>(
    pending: &mut Vec<PersistentExactTypeId>,
    exact: PersistentExactTypeId,
) -> Result<(), HirTypeSiteExactError<E>> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(pending, 1, &path)?;
    pending.push(exact);
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
