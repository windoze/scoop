use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use scoop_identity::{PersistentTypeAliasId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::{CanonicalTypeAliasInterfacesV1, TypeAliasInterfaceRecordV1, TypeAliasTargetV1};

const PERSISTENT_ID_HEX_BYTES: u64 = 64;
const CHAIN_SEPARATOR_BYTES: u64 = 4;

/// Supplies validated transitive alias records and edge-specific public
/// reachability proofs for the complete dependency closure.
pub trait TypeAliasClosureAuthority {
    fn external_type_alias(
        &self,
        alias: PersistentTypeAliasId,
    ) -> Option<&TypeAliasInterfaceRecordV1>;

    fn is_type_alias_edge_authorized(
        &self,
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    ) -> bool;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasExpansionV1 {
    alias: PersistentTypeAliasId,
    target: Arc<SignatureTypeKey>,
}

impl TypeAliasExpansionV1 {
    pub const fn alias(&self) -> PersistentTypeAliasId {
        self.alias
    }

    pub fn target(&self) -> &SignatureTypeKey {
        self.target.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTypeAliasExpansionsV1 {
    entries: Vec<TypeAliasExpansionV1>,
}

impl CanonicalTypeAliasExpansionsV1 {
    pub fn entries(&self) -> &[TypeAliasExpansionV1] {
        &self.entries
    }

    pub fn get(&self, alias: PersistentTypeAliasId) -> Option<&TypeAliasExpansionV1> {
        self.entries
            .binary_search_by_key(&alias, TypeAliasExpansionV1::alias)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl CanonicalTypeAliasInterfacesV1 {
    pub fn expand_alias_closure<A>(
        &self,
        authority: &A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalTypeAliasExpansionsV1, TypeAliasExpansionError>
    where
        A: TypeAliasClosureAuthority,
    {
        let count = u64::try_from(self.records().len())
            .map_err(|_| TypeAliasExpansionError::Resource(integer_out_of_range(path)))?;
        meter
            .check_table_entries(count, path)
            .map_err(TypeAliasExpansionError::Resource)?;
        let mut entries = Vec::new();
        meter
            .try_reserve_collection_slots(&mut entries, self.records().len(), path)
            .map_err(TypeAliasExpansionError::Resource)?;

        let mut expander = TypeAliasExpander::new(self, authority, meter, path);
        for record in self.records() {
            let target = expander.expand(record.alias())?;
            entries.push(TypeAliasExpansionV1 {
                alias: record.alias(),
                target,
            });
        }
        Ok(CanonicalTypeAliasExpansionsV1 { entries })
    }
}

struct TypeAliasExpander<'table, 'authority, 'meter, A> {
    local: &'table CanonicalTypeAliasInterfacesV1,
    authority: &'authority A,
    meter: &'meter mut BudgetMeter,
    path: &'meter WirePath,
    memo: HashMap<PersistentTypeAliasId, Arc<SignatureTypeKey>>,
    active_positions: HashMap<PersistentTypeAliasId, usize>,
    stack: Vec<PersistentTypeAliasId>,
}

impl<'table, 'authority, 'meter, A> TypeAliasExpander<'table, 'authority, 'meter, A>
where
    A: TypeAliasClosureAuthority,
{
    fn new(
        local: &'table CanonicalTypeAliasInterfacesV1,
        authority: &'authority A,
        meter: &'meter mut BudgetMeter,
        path: &'meter WirePath,
    ) -> Self {
        Self {
            local,
            authority,
            meter,
            path,
            memo: HashMap::new(),
            active_positions: HashMap::new(),
            stack: Vec::new(),
        }
    }

    fn expand(
        &mut self,
        root: PersistentTypeAliasId,
    ) -> Result<Arc<SignatureTypeKey>, TypeAliasExpansionError> {
        let mut alias = root;
        let mut depth = 1_u64;
        let target = loop {
            self.meter
                .check_semantic_depth(depth, self.path)
                .map_err(TypeAliasExpansionError::Resource)?;
            if let Some(target) = self.memo.get(&alias) {
                break Arc::clone(target);
            }

            let target = self
                .local
                .get(alias)
                .or_else(|| self.authority.external_type_alias(alias))
                .map(TypeAliasInterfaceRecordV1::target)
                .cloned()
                .ok_or(TypeAliasExpansionError::MissingInterface { alias })?;
            self.meter
                .charge_nodes(1, self.path)
                .map_err(TypeAliasExpansionError::Resource)?;
            self.meter
                .charge_work(1, self.path)
                .map_err(TypeAliasExpansionError::Resource)?;
            self.push_active(alias)?;

            match target {
                TypeAliasTargetV1::Signature(target) => break Arc::new(target),
                TypeAliasTargetV1::Alias(next) => {
                    self.meter
                        .charge_edges(1, self.path)
                        .map_err(TypeAliasExpansionError::Resource)?;
                    self.meter
                        .charge_work(1, self.path)
                        .map_err(TypeAliasExpansionError::Resource)?;
                    if !self.authority.is_type_alias_edge_authorized(alias, next) {
                        return Err(TypeAliasExpansionError::UnauthorizedTarget {
                            source: alias,
                            target: next,
                        });
                    }
                    if let Some(start) = self.active_positions.get(&next).copied() {
                        return Err(self.cycle_error(start, next)?);
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        TypeAliasExpansionError::Resource(integer_out_of_range(self.path))
                    })?;
                    alias = next;
                }
            }
        };

        while let Some(alias) = self.stack.pop() {
            self.active_positions.remove(&alias);
            self.meter
                .try_reserve_map_slots(&mut self.memo, 1, self.path)
                .map_err(TypeAliasExpansionError::Resource)?;
            self.memo.insert(alias, Arc::clone(&target));
        }
        Ok(target)
    }

    fn push_active(&mut self, alias: PersistentTypeAliasId) -> Result<(), TypeAliasExpansionError> {
        self.meter
            .try_reserve_collection_slots(&mut self.stack, 1, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;
        self.meter
            .try_reserve_map_slots(&mut self.active_positions, 1, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;
        let position = self.stack.len();
        self.stack.push(alias);
        self.active_positions.insert(alias, position);
        Ok(())
    }

    fn cycle_error(
        &mut self,
        start: usize,
        repeated: PersistentTypeAliasId,
    ) -> Result<TypeAliasExpansionError, TypeAliasExpansionError> {
        let chain_count = self
            .stack
            .len()
            .checked_sub(start)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| TypeAliasExpansionError::Resource(integer_out_of_range(self.path)))?;
        let diagnostic_bytes = cycle_diagnostic_bytes(chain_count, self.path)?;
        self.meter
            .check_semantic_leaf(diagnostic_bytes, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;

        let mut chain = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut chain, chain_count, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;
        chain.extend_from_slice(&self.stack[start..]);
        chain.push(repeated);
        Ok(TypeAliasExpansionError::Cycle { chain })
    }
}

fn cycle_diagnostic_bytes(
    id_count: usize,
    path: &WirePath,
) -> Result<u64, TypeAliasExpansionError> {
    let id_count = u64::try_from(id_count)
        .map_err(|_| TypeAliasExpansionError::Resource(integer_out_of_range(path)))?;
    let id_bytes = id_count
        .checked_mul(PERSISTENT_ID_HEX_BYTES)
        .ok_or_else(|| TypeAliasExpansionError::Resource(integer_out_of_range(path)))?;
    let separator_bytes = id_count
        .saturating_sub(1)
        .checked_mul(CHAIN_SEPARATOR_BYTES)
        .ok_or_else(|| TypeAliasExpansionError::Resource(integer_out_of_range(path)))?;
    id_bytes
        .checked_add(separator_bytes)
        .ok_or_else(|| TypeAliasExpansionError::Resource(integer_out_of_range(path)))
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeAliasExpansionError {
    MissingInterface {
        alias: PersistentTypeAliasId,
    },
    UnauthorizedTarget {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    Cycle {
        chain: Vec<PersistentTypeAliasId>,
    },
    Resource(WireError),
}

impl fmt::Display for TypeAliasExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInterface { alias } => {
                write!(formatter, "missing type-alias interface {alias}")
            }
            Self::UnauthorizedTarget { source, target } => write!(
                formatter,
                "type alias {source} is not authorized to reference type alias {target}"
            ),
            Self::Cycle { chain } => {
                formatter.write_str("type-alias cycle: ")?;
                for (index, alias) in chain.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(" -> ")?;
                    }
                    alias.fmt(formatter)?;
                }
                Ok(())
            }
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypeAliasExpansionError {}

#[cfg(test)]
mod tests;
