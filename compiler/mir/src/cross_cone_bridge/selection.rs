use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeMirBridgeSectionV1, SelectedDependencyMirCallableV1};

/// Request-local index into one validated ordinary-dependency selection.
///
/// This id names a selected bridge record, not a MIR arena entry. The MIR
/// call target uses the separate `ImportedDependencyMirCallableId` domain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedDependencyMirCallableId(u32);

/// A selected callable reference branded by the exact request-local set that
/// projected it from a validated cross-Cone MIR bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SelectedDependencyMirCallableRef {
    selection: DependencyMirSelectionId,
    callable: SelectedDependencyMirCallableId,
}

impl SelectedDependencyMirCallableRef {
    pub const fn callable(self) -> SelectedDependencyMirCallableId {
        self.callable
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DependencyMirSelectionId(u64);

fn next_dependency_mir_selection() -> DependencyMirSelectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let selection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the dependency MIR selection id space is exhausted");
    DependencyMirSelectionId(selection)
}

/// Complete canonical ordinary-dependency callable selection for one MIR
/// lowering request.
///
/// Construction consumes only the locally validated bridge surface. Closure
/// validation remains responsible for proving every record against its
/// terminal provider before this set reaches a compiler request.
pub struct SelectedDependencyMirSet {
    consumer: ConeIdentity,
    selection: DependencyMirSelectionId,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedDependencyMirCallableId>,
    callables: Vec<SelectedDependencyMirCallableV1>,
}

impl SelectedDependencyMirSet {
    /// Constructs the closed empty dependency selection used by an ordinary
    /// core-only request.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            selection: next_dependency_mir_selection(),
            by_declaration: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeMirBridgeSectionV1,
    ) -> Result<Self, SelectedDependencyMirSetBuildError> {
        let mut by_declaration = BTreeMap::new();
        let mut callables = Vec::with_capacity(bridge.selected().len());
        for selected in bridge.selected() {
            let index = u32::try_from(callables.len()).map_err(|_| {
                SelectedDependencyMirSetBuildError::TooManyCallables {
                    count: bridge.selected().len(),
                }
            })?;
            let id = SelectedDependencyMirCallableId(index);
            by_declaration.insert((selected.provider(), selected.declaration()), id);
            callables.push(selected.clone());
        }
        Ok(Self {
            consumer: bridge.artifact(),
            selection: next_dependency_mir_selection(),
            by_declaration,
            callables,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn callable(
        &self,
        id: SelectedDependencyMirCallableId,
    ) -> Option<&SelectedDependencyMirCallableV1> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_ref(
        &self,
        id: SelectedDependencyMirCallableId,
    ) -> Option<SelectedDependencyMirCallableRef> {
        self.callable(id).map(|_| SelectedDependencyMirCallableRef {
            selection: self.selection,
            callable: id,
        })
    }

    /// Mint one effect-refined MIR arena value from this exact selected set.
    /// The effect is supplied by the committed HIR callable use; the later
    /// LIR bridge projection verifies it against the provider's canonical ABI
    /// and root plan.
    pub fn callable_use(
        &self,
        id: SelectedDependencyMirCallableId,
        gc_effect: crate::GcEffect,
    ) -> Option<crate::ImportedDependencyMirCallableUse> {
        self.callable_ref(id)
            .map(|reference| crate::ImportedDependencyMirCallableUse::new(reference, gc_effect))
    }

    pub fn resolve_callable(
        &self,
        reference: SelectedDependencyMirCallableRef,
    ) -> Option<&SelectedDependencyMirCallableV1> {
        (reference.selection == self.selection)
            .then(|| self.callable(reference.callable))
            .flatten()
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedDependencyMirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    pub fn callables(&self) -> &[SelectedDependencyMirCallableV1] {
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
pub enum SelectedDependencyMirSetBuildError {
    TooManyCallables { count: usize },
}

impl fmt::Display for SelectedDependencyMirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCallables { count } => write!(
                formatter,
                "dependency MIR selection contains {count} callables, exceeding the u32 id domain"
            ),
        }
    }
}

impl std::error::Error for SelectedDependencyMirSetBuildError {}

#[cfg(test)]
mod tests;
