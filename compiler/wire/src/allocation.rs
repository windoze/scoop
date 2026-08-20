//! Fallible collection growth for lengths read from metadata.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::{WireError, WireErrorKind, WirePath};

fn allocation_error(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::Allocation, path.clone(), None)
}

pub fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    path: &WirePath,
) -> Result<(), WireError> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_error(path))
}

pub fn try_reserve_count<T>(
    values: &mut Vec<T>,
    additional: u64,
    path: &WirePath,
) -> Result<(), WireError> {
    let additional = usize::try_from(additional)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    try_reserve(values, additional, path)
}

pub fn try_reserve_map<K: Eq + Hash, V>(
    values: &mut HashMap<K, V>,
    additional: usize,
    path: &WirePath,
) -> Result<(), WireError> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_error(path))
}

pub fn try_reserve_set<T: Eq + Hash>(
    values: &mut HashSet<T>,
    additional: usize,
    path: &WirePath,
) -> Result<(), WireError> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_error(path))
}

pub fn try_copy_str(value: &str, path: &WirePath) -> Result<String, WireError> {
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| allocation_error(path))?;
    copy.push_str(value);
    Ok(copy)
}
