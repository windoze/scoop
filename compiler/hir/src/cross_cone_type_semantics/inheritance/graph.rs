use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ExactTypeKey, GeneratedNominalKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<Self, InheritanceGraphError<E>>
    where
        A: NominalInheritanceSemanticAuthority<E>,
    {
        let mut graph = Self {
            nodes: Default::default(),
            sources: Default::default(),
            object_backings: Default::default(),
        };
        let path = WirePath::root();
        for record in records {
            meter
                .charge_nodes(1, &path)
                .map_err(InheritanceGraphError::Resource)?;
            meter
                .charge_collection_slots(1, &path)
                .map_err(InheritanceGraphError::Resource)?;
            meter
                .charge_work(1, &path)
                .map_err(InheritanceGraphError::Resource)?;
            let exact = record.owner();
            let key = authority
                .exact_type_key(exact)
                .map_err(InheritanceGraphError::Foundation)?;
            let ExactTypeKey::Nominal(id) = key else {
                return Err(InheritanceGraphError::NonParamFreeSource(exact));
            };
            if PersistentExactTypeId::from_key(key).ok() != Some(exact) {
                return Err(InheritanceGraphError::ExactIdentity(exact));
            }
            let source = SourceNominalId::Concrete(*id);
            graph.validate_source(source, authority, meter, 1)?;
            let kind = graph.sources[&source].key.declaration_kind();
            validate_kind(record, kind)?;
            if kind == SourceDeclarationKind::Object {
                graph.validate_object(exact, *id, authority, meter)?;
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
        graph.validate_edges(meter)?;
        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        for exact in graph.nodes.keys() {
            graph.visit(*exact, &mut active, &mut complete, meter, 1)?;
        }
        Ok(graph)
    }

    fn validate_edges<E>(&self, meter: &mut BudgetMeter) -> Result<(), InheritanceGraphError<E>> {
        let path = WirePath::root();
        for (owner, node) in &self.nodes {
            if let DirectClassBaseV1::ClassBase { exact } = node.edges.direct_base() {
                meter
                    .charge_edges(1, &path)
                    .map_err(InheritanceGraphError::Resource)?;
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
            meter
                .charge_edges(node.edges.direct_interfaces().len() as u64, &path)
                .map_err(InheritanceGraphError::Resource)?;
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
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<(), InheritanceGraphError<E>> {
        let path = WirePath::root();
        meter
            .charge_work(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        meter
            .check_semantic_depth(depth, &path)
            .map_err(InheritanceGraphError::Resource)?;
        if complete.contains(&exact) {
            return Ok(());
        }
        if active.contains(&exact) {
            return Err(InheritanceGraphError::Cycle(exact));
        }
        meter
            .charge_collection_slots(2, &path)
            .map_err(InheritanceGraphError::Resource)?;
        active.insert(exact);
        let edges = self.nodes[&exact].edges;
        if let DirectClassBaseV1::ClassBase { exact } = edges.direct_base() {
            self.visit(exact, active, complete, meter, depth + 1)?;
        }
        for interface in edges.direct_interfaces() {
            self.visit(*interface, active, complete, meter, depth + 1)?;
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
    NonParamFreeSource(PersistentExactTypeId),
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
            Self::NonParamFreeSource(exact) => write!(
                f,
                "inheritance owner {exact} is not a param-free source nominal"
            ),
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
