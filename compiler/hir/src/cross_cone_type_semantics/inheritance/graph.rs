use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ExactTypeKey, GeneratedNominalKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::WireError;

use super::{
    CheckedInheritanceNodeV1, CheckedNominalInheritanceGraphV1, DirectClassBaseV1,
    NominalInheritanceEdgesV1, NominalInheritanceModalityV1,
};
use crate::{
    DeclarationAccessSourceSemanticError, DeclarationAccessSourceV1, ExportDefinitionSourceV1,
    NominalRepresentationSupportV1, SourceNominalId,
};

/// Foundation facts from one explicit, target-compatible dependency closure.
/// Source origins must be checked against the artifact which owns the source;
/// no current-Cone fallback or string-name lookup is permitted.
pub trait NominalInheritanceSemanticAuthority<E> {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, E>;
    fn nominal_declaration_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, E>;
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, E>;
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, E>;
    fn validate_definition_source(&self, source: &ExportDefinitionSourceV1) -> Result<(), E>;
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, E>;
    fn generated_nominal_key(&self, nominal: PersistentTypeId) -> Result<&GeneratedNominalKey, E>;
}

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub fn validate<A, E>(
        records: impl IntoIterator<Item = &'a NominalInheritanceEdgesV1>,
        authority: &'a A,
    ) -> Result<Self, InheritanceGraphError<E>>
    where
        A: NominalInheritanceSemanticAuthority<E>,
    {
        Self::validate_with_source_roots(records, std::iter::empty(), authority)
    }

    /// Source roots preserve generic declaration metadata without creating an
    /// exact inheritance node. The enclosing section supplies the independently
    /// established definition-side source inventory.
    pub fn validate_with_source_roots<A, E>(
        records: impl IntoIterator<Item = &'a NominalInheritanceEdgesV1>,
        source_roots: impl IntoIterator<Item = SourceNominalId>,
        authority: &'a A,
    ) -> Result<Self, InheritanceGraphError<E>>
    where
        A: NominalInheritanceSemanticAuthority<E>,
    {
        let mut graph = Self {
            nodes: Default::default(),
            sources: Default::default(),
            object_backings: Default::default(),
        };

        for record in records {
            let exact = record.owner();
            let key = authority
                .exact_type_key(exact)
                .map_err(InheritanceGraphError::Foundation)?;
            let source = match key {
                ExactTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
                ExactTypeKey::NominalApplication { origin, .. } => {
                    SourceNominalId::GenericTemplate(*origin)
                }
                _ => return Err(InheritanceGraphError::NonNominalSource(exact)),
            };
            if PersistentExactTypeId::from_key(key).ok() != Some(exact) {
                return Err(InheritanceGraphError::ExactIdentity(exact));
            }
            graph.validate_source(source, authority)?;
            let kind = graph.sources[&source].key.declaration_kind();
            validate_kind(record, kind)?;
            if kind == SourceDeclarationKind::Object {
                let SourceNominalId::Concrete(id) = source else {
                    return Err(InheritanceGraphError::ObjectBacking(exact));
                };
                graph.validate_object(exact, id, authority)?;
            }
            if graph
                .nodes
                .insert(
                    exact,
                    CheckedInheritanceNodeV1 {
                        edges: record,
                        source,
                    },
                )
                .is_some()
            {
                return Err(InheritanceGraphError::DuplicateNode(exact));
            }
        }
        for source in source_roots {
            graph.validate_source(source, authority)?;
        }
        graph.validate_edges()?;
        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        for exact in graph.nodes.keys() {
            graph.visit(*exact, &mut active, &mut complete)?;
        }
        Ok(graph)
    }

    fn validate_edges<E>(&self) -> Result<(), InheritanceGraphError<E>> {
        for (owner, node) in &self.nodes {
            if let DirectClassBaseV1::ClassBase { exact } = node.edges.direct_base() {
                let base = self
                    .nodes
                    .get(&exact)
                    .ok_or(InheritanceGraphError::MissingNode(exact))?;
                if self.sources[&base.source].key.declaration_kind() != SourceDeclarationKind::Class
                {
                    return Err(InheritanceGraphError::ClassBaseKind {
                        owner: *owner,
                        base: exact,
                    });
                }
                if base.edges.modality() == NominalInheritanceModalityV1::Final {
                    return Err(InheritanceGraphError::FinalBase {
                        owner: *owner,
                        base: exact,
                    });
                }
            }

            for interface in node.edges.direct_interfaces() {
                let target = self
                    .nodes
                    .get(interface)
                    .ok_or(InheritanceGraphError::MissingNode(*interface))?;
                if target.edges.modality() != NominalInheritanceModalityV1::Interface {
                    return Err(InheritanceGraphError::InterfaceEdgeKind {
                        owner: *owner,
                        interface: *interface,
                    });
                }
            }
        }
        Ok(())
    }

    fn visit<E>(
        &self,
        exact: PersistentExactTypeId,
        active: &mut BTreeSet<PersistentExactTypeId>,
        complete: &mut BTreeSet<PersistentExactTypeId>,
    ) -> Result<(), InheritanceGraphError<E>> {
        if complete.contains(&exact) {
            return Ok(());
        }
        if active.contains(&exact) {
            return Err(InheritanceGraphError::Cycle(exact));
        }

        active.insert(exact);
        let edges = self.nodes[&exact].edges;
        if let DirectClassBaseV1::ClassBase { exact } = edges.direct_base() {
            self.visit(exact, active, complete)?;
        }
        for interface in edges.direct_interfaces() {
            self.visit(*interface, active, complete)?;
        }
        active.remove(&exact);
        complete.insert(exact);
        Ok(())
    }
}

fn validate_kind<E>(
    record: &NominalInheritanceEdgesV1,
    kind: SourceDeclarationKind,
) -> Result<(), InheritanceGraphError<E>> {
    let valid = match kind {
        SourceDeclarationKind::Class => {
            record.modality() != NominalInheritanceModalityV1::Interface
        }
        SourceDeclarationKind::Interface => {
            record.modality() == NominalInheritanceModalityV1::Interface
                && record.direct_base() == DirectClassBaseV1::NoClassBase
        }
        SourceDeclarationKind::Object => record.modality() == NominalInheritanceModalityV1::Final,
        SourceDeclarationKind::Struct
        | SourceDeclarationKind::Enum
        | SourceDeclarationKind::AnnotationClass => {
            record.modality() == NominalInheritanceModalityV1::Final
                && record.direct_base() == DirectClassBaseV1::NoClassBase
        }
        SourceDeclarationKind::Function
        | SourceDeclarationKind::Constructor
        | SourceDeclarationKind::Property
        | SourceDeclarationKind::ExtensionProperty
        | SourceDeclarationKind::TypeAlias => false,
    };
    if valid {
        Ok(())
    } else {
        Err(InheritanceGraphError::KindModality(record.owner()))
    }
}

#[derive(Debug)]
pub enum InheritanceGraphError<E> {
    DeclarationSource(DeclarationAccessSourceSemanticError<E>),
    Resource(WireError),
    Foundation(E),
    Source {
        owner: SourceNominalId,
        error: DeclarationAccessSourceSemanticError<E>,
    },
    SourceIdentity(SourceNominalId),
    SourceOrigin(SourceNominalId),
    ExactIdentity(PersistentExactTypeId),
    NonNominalSource(PersistentExactTypeId),
    DuplicateNode(PersistentExactTypeId),
    MissingNode(PersistentExactTypeId),
    KindModality(PersistentExactTypeId),
    ClassBaseKind {
        owner: PersistentExactTypeId,
        base: PersistentExactTypeId,
    },
    InterfaceEdgeKind {
        owner: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    FinalBase {
        owner: PersistentExactTypeId,
        base: PersistentExactTypeId,
    },
    Cycle(PersistentExactTypeId),
    ObjectBacking(PersistentExactTypeId),
}
impl<E: fmt::Display> fmt::Display for InheritanceGraphError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclarationSource(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => write!(f, "invalid inheritance foundation: {error}"),
            Self::Source { owner, error } => {
                write!(f, "invalid source access for {owner:?}: {error}")
            }
            Self::SourceIdentity(owner) => {
                write!(f, "source nominal key does not derive {owner:?}")
            }
            Self::SourceOrigin(owner) => write!(
                f,
                "source nominal origin differs from access origin for {owner:?}"
            ),
            Self::ExactIdentity(exact) => {
                write!(f, "exact key does not derive inheritance owner {exact}")
            }
            Self::NonNominalSource(exact) => {
                write!(f, "inheritance owner {exact} is not a nominal application")
            }
            Self::DuplicateNode(exact) => write!(f, "duplicate inheritance owner {exact}"),
            Self::MissingNode(exact) => write!(f, "missing inheritance closure node {exact}"),
            Self::KindModality(exact) => write!(
                f,
                "inheritance kind, modality, or class base disagree for {exact}"
            ),
            Self::ClassBaseKind { owner, base } => {
                write!(f, "class base {base} of {owner} is not a class")
            }
            Self::InterfaceEdgeKind { owner, interface } => write!(
                f,
                "interface edge {interface} of {owner} is not an interface"
            ),
            Self::FinalBase { owner, base } => {
                write!(f, "inheritance owner {owner} extends final class {base}")
            }
            Self::Cycle(exact) => write!(f, "inheritance cycle at {exact}"),
            Self::ObjectBacking(exact) => write!(
                f,
                "object {exact} representation and foundation backing-class relation disagree"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceGraphError<E> {}
