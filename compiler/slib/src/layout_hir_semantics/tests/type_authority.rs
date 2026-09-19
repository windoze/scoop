use std::{cell::Cell, fmt};

use scoop_hir::*;
use scoop_identity::*;
use scoop_wire::{BudgetMeter, WirePath};

mod declarations;
mod defaults;
mod foundation;
mod nominal;

pub(super) use declarations::EmptyDeclarations;
pub(super) use defaults::EmptyDefaults;
pub(super) use foundation::EmptyFoundation;
pub(super) use nominal::NominalFixture;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    UnexpectedCall,
    MissingSourceRoot(SourceNominalId),
    WrongProvider {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
}

impl fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCall => formatter.write_str("unexpected empty-fixture authority call"),
            Self::MissingSourceRoot(root) => {
                write!(formatter, "missing independent source root {root:?}")
            }
            Self::WrongProvider { expected, actual } => {
                write!(
                    formatter,
                    "authority belongs to {actual}, expected {expected}"
                )
            }
        }
    }
}

impl std::error::Error for TestAuthorityError {}

#[derive(Clone, Copy)]
pub(super) struct CommittedUse {
    request: SelectedExternalTypeUseV1,
    origin: TypeSectionCommittedRootOriginV1,
}

#[derive(Default)]
pub(super) struct EmptyCommittedUses {
    roots: Vec<CommittedUse>,
    edges: Vec<CommittedUse>,
    validation_calls: Cell<usize>,
}

impl EmptyCommittedUses {
    pub(super) fn with_root(provider: ConeIdentity, exact: PersistentExactTypeId) -> Self {
        Self {
            roots: vec![CommittedUse {
                request: SelectedExternalTypeUseV1::new(
                    provider,
                    SelectedTypeUseV1::Representation { exact },
                ),
                origin: TypeSectionCommittedRootOriginV1::Source,
            }],
            ..Self::default()
        }
    }

    pub(super) fn with_invalid_recursive_origin(
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    ) -> Self {
        let missing_parent = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        Self {
            roots: Self::with_root(provider, exact).roots,
            edges: vec![CommittedUse {
                request: SelectedExternalTypeUseV1::new(
                    provider,
                    SelectedTypeUseV1::Representation { exact },
                ),
                origin: TypeSectionCommittedRootOriginV1::LocalSemanticSupport {
                    parent: missing_parent,
                },
            }],
            ..Self::default()
        }
    }

    pub(super) fn validation_calls(&self) -> usize {
        self.validation_calls.get()
    }
}

impl CommittedTypeUseSemanticAuthorityV1<TestAuthorityError> for EmptyCommittedUses {
    type Root = CommittedUse;
    type Edge = CommittedUse;

    fn committed_roots(&self) -> Result<&[Self::Root], TestAuthorityError> {
        Ok(&self.roots)
    }

    fn root_request(
        &self,
        root: &Self::Root,
    ) -> Result<SelectedExternalTypeUseV1, TestAuthorityError> {
        Ok(root.request)
    }

    fn root_origin(
        &self,
        root: &Self::Root,
    ) -> Result<TypeSectionCommittedRootOriginV1, TestAuthorityError> {
        Ok(root.origin)
    }

    fn validate_root(
        &self,
        _root: &Self::Root,
        _target: CheckedTypeSelectionTargetV1<'_>,
        _context: TypeSectionUseContextV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), TestAuthorityError> {
        self.validation_calls.set(self.validation_calls.get() + 1);
        Ok(())
    }

    fn semantic_edges(
        &self,
        _parent: CheckedTypeSelectionTargetV1<'_>,
    ) -> Result<&[Self::Edge], TestAuthorityError> {
        Ok(&self.edges)
    }

    fn edge_request(
        &self,
        edge: &Self::Edge,
    ) -> Result<SelectedExternalTypeUseV1, TestAuthorityError> {
        Ok(edge.request)
    }

    fn edge_origin(
        &self,
        edge: &Self::Edge,
    ) -> Result<TypeSectionCommittedRootOriginV1, TestAuthorityError> {
        Ok(edge.origin)
    }

    fn validate_edge(
        &self,
        _parent: CheckedTypeSelectionTargetV1<'_>,
        _edge: &Self::Edge,
        _target: CheckedTypeSelectionTargetV1<'_>,
        _context: TypeSectionUseContextV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), TestAuthorityError> {
        self.validation_calls.set(self.validation_calls.get() + 1);
        Ok(())
    }
}
