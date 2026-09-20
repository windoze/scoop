use super::*;

pub(super) struct Projection<'a> {
    pub concrete: Vec<ConcreteNominal<'a>>,
    pub root_exacts: BTreeSet<PersistentExactTypeId>,
    pub public: CanonicalNominalInterfacesV1,
    pub foundation: CrossConeTypeSemanticsFoundationV1,
}

impl CrossConeTypeSemanticsFoundationV1 {
    /// Projects source evidence before candidate dispatch, protected-callable,
    /// or default contracts are built. Serialization and binding have their
    /// own shared resource meter; this sealed-HIR product is not a proof.
    pub fn from_ordinary_hir(output: &OrdinaryHirOutput<'_>) -> Result<Self, Error> {
        project(output).map(|projection| projection.foundation)
    }
}

pub(super) fn project<'a>(output: &'a OrdinaryHirOutput<'_>) -> Result<Projection<'a>, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let public = CanonicalNominalInterfacesV1::from_export_hir(export)
        .map_err(|error| Error::PublicInterface(error.to_string()))?;
    let mut roots = Vec::new();
    let mut concrete = Vec::new();
    let mut sources = BTreeMap::new();
    for local_id in public_nominals(export) {
        let source = identity(export, local_id)?.source().ok_or_else(|| {
            let (kind, index) = location(local_id);
            Error::GeneratedPublicNominal { kind, index }
        })?;
        let id = source_id(source);
        roots.push(id);
        sources.insert(
            id,
            TypeSemanticsSourceEvidenceV1 {
                key: source.declaration().clone(),
                access: declaration_access(
                    export,
                    source.declaration(),
                    nominal_access(export, local_id).declared.into(),
                )?,
            },
        );
        if let Some(nominal) = source_inventory::concrete(export, local, local_id, source)? {
            concrete.push(nominal);
        }
    }
    roots.sort_unstable();
    if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::InvalidTable {
            table: "source-root",
            reason: "duplicate source nominal identity".into(),
        });
    }
    let root_exacts = concrete
        .iter()
        .map(|nominal| nominal.exact)
        .collect::<BTreeSet<_>>();
    let fact_requirements = representation::fact_requirements(export, &concrete)?;
    let (local_exact_facts, dependency_facts, fact_shapes) = facts::source(
        output.imported_core(),
        export,
        local,
        &root_exacts,
        &fact_requirements,
    )?;
    let mut edges = Vec::with_capacity(concrete.len());
    for nominal in &concrete {
        let edge = inheritance::project_edges(export, nominal)?;
        if let DirectClassBaseV1::ClassBase { exact } = edge.direct_base()
            && !root_exacts.contains(&exact)
        {
            return Err(Error::MissingLocalSupport(exact));
        }
        if let Some(exact) = edge
            .direct_interfaces()
            .iter()
            .find(|exact| !root_exacts.contains(exact))
        {
            return Err(Error::MissingLocalSupport(*exact));
        }
        edges.push(edge);
    }
    edges.sort_unstable_by_key(NominalInheritanceEdgesV1::owner);
    let representations =
        authority_projection::representation_evidence(export, local, &concrete, &sources, &public)?;
    let representation_owners =
        CanonicalPersistentIdsV1::try_new(concrete.iter().map(|nominal| nominal.owner).collect())
            .map_err(|error| Error::InvalidTable {
            table: "representation inventory",
            reason: error.to_string(),
        })?;
    let generated = authority_projection::generated_nominal_keys(export)?;
    let foundation = CrossConeTypeSemanticsFoundationV1::new(
        export.cone,
        authority_projection::exact_type_keys(local, &generated)?,
        sources,
        representations,
        generated,
        authority_projection::property_accessor_keys(export),
        authority_projection::definition_sources(export),
        roots,
        local_exact_facts,
        dependency_facts,
        edges,
        fact_shapes,
        representation_owners,
    );
    Ok(Projection {
        concrete,
        root_exacts,
        public,
        foundation,
    })
}
