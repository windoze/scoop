use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeMirBridgeSectionV1, SelectedDependencyMirCallableV1};
mod callable;
pub use callable::{CallableRole, SelectedExternalMirCallable};

/// Request-local index into one validated external selection.
///
/// This id names a selected bridge record, not a MIR arena entry. The MIR
/// call target uses the separate `ExternalCallableUseId` domain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedExternalMirCallableId(u32);

/// A selected callable reference branded by the exact request-local set that
/// projected it from a validated cross-Cone MIR bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SelectedExternalMirCallableRef {
    selection: ExternalMirSelectionId,
    callable: SelectedExternalMirCallableId,
}

impl SelectedExternalMirCallableRef {
    pub const fn callable(self) -> SelectedExternalMirCallableId {
        self.callable
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ExternalMirSelectionId(u64);

fn next_external_mir_selection() -> ExternalMirSelectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let selection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the external MIR selection id space is exhausted");
    ExternalMirSelectionId(selection)
}

/// Complete canonical external callable selection for one MIR
/// lowering request.
///
/// Construction consumes only the locally validated bridge surface. Closure
/// validation remains responsible for proving every record against its
/// terminal provider before this set reaches a compiler request.
pub struct SelectedExternalMirSet {
    consumer: ConeIdentity,
    selection: ExternalMirSelectionId,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedExternalMirCallableId>,
    callables: Vec<SelectedExternalMirCallable>,
}

impl SelectedExternalMirSet {
    /// Constructs an empty selection for a current-Cone request.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            selection: next_external_mir_selection(),
            by_declaration: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeMirBridgeSectionV1,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        Self::try_from_callables(bridge.artifact(), bridge.selected().to_vec())
    }

    /// Seals a producer-side selection after its records have been projected
    /// from an already validated dependency closure.
    ///
    /// This entry point deliberately accepts typed bridge records rather than
    /// HIR ids or symbol names. The closure owner remains responsible for
    /// proving each record against its terminal provider before calling it.
    #[doc(hidden)]
    pub fn try_from_callables(
        consumer: ConeIdentity,
        selected: Vec<SelectedDependencyMirCallableV1>,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        Self::try_from_selections(
            consumer,
            selected
                .into_iter()
                .map(SelectedExternalMirCallable::dependency)
                .collect(),
        )
    }

    fn try_from_selections(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedExternalMirCallable>,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        selected.sort_unstable_by_key(|callable| (callable.provider(), callable.declaration()));
        if let Some(callable) = selected
            .iter()
            .find(|callable| callable.provider() == consumer)
        {
            return Err(SelectedExternalMirSetBuildError::SelectedCurrentProvider {
                provider: callable.provider(),
            });
        }
        if let Some(pair) = selected.windows(2).find(|pair| {
            (pair[0].provider(), pair[0].declaration())
                == (pair[1].provider(), pair[1].declaration())
        }) {
            return Err(SelectedExternalMirSetBuildError::DuplicateCallable {
                provider: pair[0].provider(),
                declaration: pair[0].declaration(),
            });
        }
        let mut by_declaration = BTreeMap::new();
        let mut callables = Vec::with_capacity(selected.len());
        for selected in selected {
            let index = u32::try_from(callables.len()).map_err(|_| {
                SelectedExternalMirSetBuildError::TooManyCallables {
                    count: callables.len().saturating_add(1),
                }
            })?;
            let id = SelectedExternalMirCallableId(index);
            by_declaration.insert((selected.provider(), selected.declaration()), id);
            callables.push(selected);
        }
        Ok(Self {
            consumer,
            selection: next_external_mir_selection(),
            by_declaration,
            callables,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn callable(
        &self,
        id: SelectedExternalMirCallableId,
    ) -> Option<&SelectedExternalMirCallable> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_ref(
        &self,
        id: SelectedExternalMirCallableId,
    ) -> Option<SelectedExternalMirCallableRef> {
        self.callable(id).map(|_| SelectedExternalMirCallableRef {
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
        id: SelectedExternalMirCallableId,
        gc_effect: crate::GcEffect,
    ) -> Option<crate::ExternalCallableUse> {
        self.callable_ref(id)
            .map(|reference| crate::ExternalCallableUse::new(reference, gc_effect))
    }

    pub fn resolve_callable(
        &self,
        reference: SelectedExternalMirCallableRef,
    ) -> Option<&SelectedExternalMirCallable> {
        (reference.selection == self.selection)
            .then(|| self.callable(reference.callable))
            .flatten()
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedExternalMirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    /// Projection into the existing ordinary-callable metadata partition.
    pub fn dependency_callables(&self) -> impl Iterator<Item = &SelectedDependencyMirCallableV1> {
        self.callables
            .iter()
            .filter(|callable| callable.role() == CallableRole::Ordinary)
            .map(SelectedExternalMirCallable::record)
    }

    pub fn initialization_cycle(&self) -> Option<SelectedExternalMirCallableId> {
        self.callables
            .iter()
            .position(|callable| callable.role() == CallableRole::InitializationCycle)
            .map(|index| SelectedExternalMirCallableId(index as u32))
    }

    /// Adds the service before any MIR uses are allocated and seals one complete set.
    pub fn with_initialization_cycle(
        self,
        record: SelectedDependencyMirCallableV1,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        if self.initialization_cycle().is_some() {
            return Err(SelectedExternalMirSetBuildError::DuplicateInitializationCycle);
        }
        if record.signature().receiver().is_present()
            || record.provider() != ConeIdentity::CORE
            || !matches!(
                record.declaration(),
                DependencyCallableDeclarationId::Function(_)
            )
        {
            return Err(SelectedExternalMirSetBuildError::InvalidInitializationCycle);
        }
        let mut callables = self.callables;
        callables.push(SelectedExternalMirCallable::initialization_cycle(record));
        Self::try_from_selections(self.consumer, callables)
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedExternalMirSetBuildError {
    DuplicateInitializationCycle,
    InvalidInitializationCycle,
    TooManyCallables {
        count: usize,
    },
    SelectedCurrentProvider {
        provider: ConeIdentity,
    },
    DuplicateCallable {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for SelectedExternalMirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInitializationCycle => {
                formatter.write_str("MIR selection contains duplicate initialization services")
            }
            Self::InvalidInitializationCycle => formatter.write_str(
                "MIR initialization service must name a core function without a receiver",
            ),
            Self::TooManyCallables { count } => write!(
                formatter,
                "external MIR selection contains {count} callables, exceeding the u32 id domain"
            ),
            Self::SelectedCurrentProvider { provider } => write!(
                formatter,
                "external MIR selection names current Cone {provider} as an external provider"
            ),
            Self::DuplicateCallable {
                provider,
                declaration,
            } => write!(
                formatter,
                "external MIR selection contains duplicate callable {provider}:{declaration:?}"
            ),
        }
    }
}

impl std::error::Error for SelectedExternalMirSetBuildError {}

#[cfg(test)]
mod tests;
