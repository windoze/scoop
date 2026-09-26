use super::*;
use scoop_identity::{ConeIdentity, PersistentExactTypeId};

/// Ownership of a recursive fact is independent of the transported candidate.
/// Structural types can belong to local support; nominal kind is not ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeSectionDependencyFactV1 {
    pub provider: ConeIdentity,
    pub exact: PersistentExactTypeId,
}

/// Immutable, target-compatible source/foundation closure. Every inventory is
/// projected from all actual source/support roots, never from the eight tables.
/// `fact_shape` must be the exact projection of the same real source shapes
/// supplied by `representation_source`; layout/native witnesses are excluded.
pub trait TypeSectionFoundationSemanticAuthority<E>:
    NominalInheritanceSemanticAuthority<E>
    + ExactTypeFactsSemanticAuthority<E>
    + NominalRepresentationSemanticAuthority<E>
{
    fn local_exact_facts(&self) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, E>;
    /// Strictly increasing exact IDs, excluding all locally owned facts.
    fn dependency_facts(&self) -> Result<&[TypeSectionDependencyFactV1], E>;
    /// Strict source-ID order. Includes generic metadata roots without granting
    /// any concrete representation, inheritance, or materialization capability.
    fn local_source_roots(&self) -> Result<&[SourceNominalId], E>;
    /// Source inheritance edges in exact-owner order, independently projected
    /// before reading the candidate inheritance interfaces.
    fn local_inheritance_edges(&self) -> Result<&[NominalInheritanceEdgesV1], E>;
    /// Canonical role proof shared with the actual property declaration. A
    /// selected Getter/Setter tag cannot choose an accessor's source role.
    fn selected_accessor_key(
        &self,
        accessor: scoop_identity::PersistentPropertyAccessorId,
    ) -> Result<&scoop_identity::PropertyAccessorKey, E>;
}

/// Declaration replay is an explicit section obligation. Artifact adapters
/// override this entry to check complete source payloads before graph replay.
pub trait TypeSectionDeclarationSemanticAuthority<E>:
    ProtectedDeclarationSemanticAuthority<E> + NominalInheritanceInterfaceSemanticAuthority<E>
{
    fn validate_protected_sources<'c>(
        &mut self,
        table: &'c CanonicalProtectedDeclarationInterfacesV1,
        _protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
        representations: &'c CanonicalNominalRepresentationSupportV1,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
    ) -> Result<CheckedProtectedDeclarationSourcesV1<'c>, ProtectedDeclarationSemanticError<E>>
    where
        Self: Sized,
    {
        table.validate_sources(graph, representations, self)
    }
}
