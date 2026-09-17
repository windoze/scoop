use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{
    CoreImportedCallableKind, ExactCallableSignature, PersistentFunctionId,
    StrongCallableDefinitionOwner,
};

use super::ImportedMirFoundation;

/// A public prelude callable or compiler-protocol service selected from the
/// trusted core HIR surface and proven against that artifact's MIR bridge.
///
/// Private borrows retain the exact imported foundation and production
/// surface that minted the value. The next stage can therefore reject a
/// selection from another artifact even when all persistent ids and payloads
/// are byte-identical.
#[derive(Clone)]
pub struct SelectedImportedMirCallable<'a> {
    pub(super) foundation: &'a ImportedMirFoundation,
    pub(super) production: &'a crate::CoreBootstrapBridgeSectionV1,
    pub(super) kind: CoreImportedCallableKind,
    pub(super) definition: PersistentFunctionId,
    pub(super) implementation: StrongCallableDefinitionOwner,
    pub(super) signature: ExactCallableSignature,
}

/// Request-local MIR id for one callable selected from trusted core.
/// It cannot be interchanged with its HIR or LIR counterpart.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreMirCallableId(u32);

/// One MIR callable reference branded by the selected-set world that proved
/// its core implementation bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCoreMirCallableRef {
    selection: ImportedCoreMirSelectionId,
    callable: ImportedCoreMirCallableId,
}

impl ImportedCoreMirCallableRef {
    pub const fn callable(self) -> ImportedCoreMirCallableId {
        self.callable
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ImportedCoreMirSelectionId(u64);

pub(super) fn next_imported_core_mir_selection() -> ImportedCoreMirSelectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let selection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the imported core MIR selection id space is exhausted");
    ImportedCoreMirSelectionId(selection)
}

/// The complete imported callable set for one ordinary MIR lowering.
///
/// Construction is bound to one imported foundation and production section;
/// every inserted selection must retain those exact proofs. The set cannot
/// be assembled from bare persistent identities or signatures.
pub struct SelectedImportedMirSet<'a> {
    foundation: &'a ImportedMirFoundation,
    production: &'a crate::CoreBootstrapBridgeSectionV1,
    selection: ImportedCoreMirSelectionId,
    by_kind: BTreeMap<CoreImportedCallableKind, ImportedCoreMirCallableId>,
    callables: Vec<SelectedImportedMirCallable<'a>>,
}

impl<'a> SelectedImportedMirSet<'a> {
    #[doc(hidden)]
    pub fn new(
        foundation: &'a ImportedMirFoundation,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
    ) -> Self {
        Self {
            foundation,
            production,
            selection: next_imported_core_mir_selection(),
            by_kind: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert(
        &mut self,
        selected: SelectedImportedMirCallable<'a>,
    ) -> Result<ImportedCoreMirCallableId, ImportedMirSelectionError> {
        if !selected.belongs_to(self.foundation, self.production) {
            return Err(ImportedMirSelectionError::ForeignSelection(selected.kind()));
        }
        if let Some(&id) = self.by_kind.get(&selected.kind()) {
            let retained = &self.callables[id.0 as usize];
            if retained.definition() != selected.definition()
                || retained.implementation() != selected.implementation()
                || retained.signature() != selected.signature()
            {
                return Err(ImportedMirSelectionError::ConflictingSelection(
                    selected.kind(),
                ));
            }
            return Ok(id);
        }
        let id = ImportedCoreMirCallableId(
            u32::try_from(self.callables.len())
                .expect("one MIR request cannot select more than u32::MAX core callables"),
        );
        self.by_kind.insert(selected.kind(), id);
        self.callables.push(selected);
        Ok(id)
    }

    pub fn callable(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<&SelectedImportedMirCallable<'a>> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_ref(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<ImportedCoreMirCallableRef> {
        self.callable(id).map(|_| ImportedCoreMirCallableRef {
            selection: self.selection,
            callable: id,
        })
    }

    pub fn callable_use(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<crate::ImportedCoreCallableUse> {
        self.callable_ref(id)
            .map(crate::ImportedCoreCallableUse::new)
    }

    pub fn resolve_callable(
        &self,
        reference: ImportedCoreMirCallableRef,
    ) -> Option<&SelectedImportedMirCallable<'a>> {
        (reference.selection == self.selection)
            .then(|| self.callable(reference.callable))
            .flatten()
    }

    pub fn callable_for_kind(
        &self,
        kind: CoreImportedCallableKind,
    ) -> Option<ImportedCoreMirCallableId> {
        self.by_kind.get(&kind).copied()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedMirFoundation,
        production: &crate::CoreBootstrapBridgeSectionV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.production, production)
    }

    #[doc(hidden)]
    pub fn callable_selections(&self) -> impl Iterator<Item = &SelectedImportedMirCallable<'a>> {
        self.callables.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedMirSelectionError {
    ForeignSelection(CoreImportedCallableKind),
    ConflictingSelection(CoreImportedCallableKind),
}

impl fmt::Display for ImportedMirSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSelection(kind) => write!(
                formatter,
                "imported MIR callable {kind:?} belongs to another core artifact projection"
            ),
            Self::ConflictingSelection(kind) => write!(
                formatter,
                "imported MIR callable {kind:?} has conflicting selected definitions"
            ),
        }
    }
}

impl std::error::Error for ImportedMirSelectionError {}

impl SelectedImportedMirCallable<'_> {
    pub const fn kind(&self) -> CoreImportedCallableKind {
        self.kind
    }

    pub const fn definition(&self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedMirFoundation,
        production: &crate::CoreBootstrapBridgeSectionV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.production, production)
    }
}

impl fmt::Debug for SelectedImportedMirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedMirCallable")
            .field("kind", &self.kind)
            .field("definition", &self.definition)
            .field("implementation", &self.implementation)
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}
