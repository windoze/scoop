use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use scoop_identity::{PersistentTypeAliasId, SignatureTypeKey};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::{CanonicalTypeAliasInterfacesV1, TypeAliasInterfaceRecordV1, TypeAliasTargetV1};

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
    pub fn expand_alias_closure(
        &self,
        dependencies: &[&CanonicalTypeAliasExpansionsV1],

        path: &WirePath,
    ) -> Result<CanonicalTypeAliasExpansionsV1, TypeAliasExpansionError> {
        let mut entries = Vec::new();
        scoop_wire::allocation::try_reserve(&mut entries, self.records().len(), path)
            .map_err(TypeAliasExpansionError::Resource)?;

        let mut expander = TypeAliasExpander::new(self, dependencies, path);
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

struct TypeAliasExpander<'table, 'dependencies, 'path> {
    local: &'table CanonicalTypeAliasInterfacesV1,
    dependencies: &'dependencies [&'dependencies CanonicalTypeAliasExpansionsV1],

    path: &'path WirePath,
    memo: HashMap<PersistentTypeAliasId, Arc<SignatureTypeKey>>,
    active_positions: HashMap<PersistentTypeAliasId, usize>,
    stack: Vec<PersistentTypeAliasId>,
}

impl<'table, 'dependencies, 'path> TypeAliasExpander<'table, 'dependencies, 'path> {
    fn new(
        local: &'table CanonicalTypeAliasInterfacesV1,
        dependencies: &'dependencies [&'dependencies CanonicalTypeAliasExpansionsV1],

        path: &'path WirePath,
    ) -> Self {
        Self {
            local,
            dependencies,

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
        let target = loop {
            if let Some(target) = self.memo.get(&alias) {
                break Arc::clone(target);
            }

            if let Some(expansion) = self.dependencies.iter().find_map(|table| table.get(alias)) {
                break Arc::clone(&expansion.target);
            }
            let target = self
                .local
                .get(alias)
                .map(TypeAliasInterfaceRecordV1::target)
                .cloned()
                .ok_or(TypeAliasExpansionError::MissingInterface { alias })?;

            self.push_active(alias)?;

            match target {
                TypeAliasTargetV1::Signature(target) => break Arc::new(target),
                TypeAliasTargetV1::Alias(next) => {
                    if let Some(start) = self.active_positions.get(&next).copied() {
                        return Err(self.cycle_error(start, next)?);
                    }
                    alias = next;
                }
            }
        };

        while let Some(alias) = self.stack.pop() {
            self.active_positions.remove(&alias);
            scoop_wire::allocation::try_reserve_map(&mut self.memo, 1, self.path)
                .map_err(TypeAliasExpansionError::Resource)?;
            self.memo.insert(alias, Arc::clone(&target));
        }
        Ok(target)
    }

    fn push_active(&mut self, alias: PersistentTypeAliasId) -> Result<(), TypeAliasExpansionError> {
        scoop_wire::allocation::try_reserve(&mut self.stack, 1, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;
        scoop_wire::allocation::try_reserve_map(&mut self.active_positions, 1, self.path)
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

        let mut chain = Vec::new();
        scoop_wire::allocation::try_reserve(&mut chain, chain_count, self.path)
            .map_err(TypeAliasExpansionError::Resource)?;
        chain.extend_from_slice(&self.stack[start..]);
        chain.push(repeated);
        Ok(TypeAliasExpansionError::Cycle { chain })
    }
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeAliasExpansionError {
    MissingInterface { alias: PersistentTypeAliasId },
    Cycle { chain: Vec<PersistentTypeAliasId> },
    Resource(WireError),
}

impl fmt::Display for TypeAliasExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInterface { alias } => {
                write!(formatter, "missing type-alias interface {alias}")
            }
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
