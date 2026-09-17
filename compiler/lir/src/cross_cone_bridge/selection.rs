use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};

/// Request-local index into one validated ordinary-dependency LIR selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedDependencyLirCallableId(u32);

/// Complete canonical ordinary-dependency callable authority for one LIR
/// lowering request.
///
/// Construction consumes a locally validated bridge. Closure validation is
/// responsible for proving every selected record against its terminal
/// provider before this set reaches lowering.
pub struct SelectedDependencyLirSet {
    consumer: ConeIdentity,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedDependencyLirCallableId>,
    callables: Vec<SelectedDependencyLirCallableV1>,
}

impl SelectedDependencyLirSet {
    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<Self, SelectedDependencyLirSetBuildError> {
        let mut by_declaration = BTreeMap::new();
        let mut callables = Vec::with_capacity(bridge.selected().len());
        for selected in bridge.selected() {
            let index = u32::try_from(callables.len()).map_err(|_| {
                SelectedDependencyLirSetBuildError::TooManyCallables {
                    count: bridge.selected().len(),
                }
            })?;
            let id = SelectedDependencyLirCallableId(index);
            by_declaration.insert((selected.provider(), selected.bridge().declaration()), id);
            callables.push(selected.clone());
        }
        Ok(Self {
            consumer: bridge.artifact(),
            by_declaration,
            callables,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn callable(
        &self,
        id: SelectedDependencyLirCallableId,
    ) -> Option<&SelectedDependencyLirCallableV1> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedDependencyLirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    pub fn callables(&self) -> &[SelectedDependencyLirCallableV1] {
        &self.callables
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedDependencyLirSetBuildError {
    TooManyCallables { count: usize },
}

impl fmt::Display for SelectedDependencyLirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCallables { count } => write!(
                formatter,
                "dependency LIR selection contains {count} callables, exceeding the u32 id domain"
            ),
        }
    }
}

impl std::error::Error for SelectedDependencyLirSetBuildError {}
