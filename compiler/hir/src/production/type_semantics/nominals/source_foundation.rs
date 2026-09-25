use super::*;

mod sources;

pub(super) struct Projection<'a> {
    pub concrete: Vec<ConcreteNominal<'a>>,
    pub root_exacts: BTreeSet<PersistentExactTypeId>,
    pub public: CanonicalNominalInterfacesV1,
    pub foundation: CrossConeTypeSemanticsFoundationV1,
}

impl CrossConeTypeSemanticsFoundationV1 {
    /// Projects source evidence before candidate dispatch, protected-callable,
    /// or default contracts are built. Serialization and binding have their
    /// own validation step; this sealed-HIR product is not a proof.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        Self::from_hir(output.output())
    }

    /// Projects declaration metadata from the same sealed HIR pair for every Cone.
    /// Source projection references existing exact types; it does not materialize them.
    pub fn from_hir(output: &Output) -> Result<Self, Error> {
        project(output).map(|projection| projection.foundation)
    }
}

pub(super) fn project<'a>(output: &'a Output) -> Result<Projection<'a>, Error> {
    let export = output.export.module();
    let local = output.local.module();
    let public =
        CanonicalNominalInterfacesV1::from_export_hir(export).map_err(|error| match error {
            crate::NominalInterfaceBuildError::Resource(error) => {
                inheritance::source_errors::resource(error)
            }
            other => Error::PublicInterface(other.to_string()),
        })?;
    let required = CanonicalSourceNominalIdsV1::from_export_hir(&output.export)?;
    let sources = sources::project(export, &required)?;
    let mut roots = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut roots,
        required.values().len(),
        &scoop_wire::WirePath::root(),
    )
    .map_err(inheritance::source_errors::resource)?;
    roots.extend_from_slice(required.values());
    // Source roots remain complete. Only the closed param-free subset supplies
    // representation and exact inheritance; no exact pair is synthesized.
    let concrete = source_inventory::from_pair(output, &required)?;
    let root_exacts = concrete
        .iter()
        .map(|nominal| nominal.exact)
        .collect::<BTreeSet<_>>();
    let fact_requirements = representation::fact_requirements(export, &concrete)?;
    let mut edges = concrete
        .iter()
        .map(|nominal| inheritance::project_source_edges(export, nominal))
        .collect::<Result<Vec<_>, _>>()?;
    edges.sort_unstable_by_key(NominalInheritanceEdgesV1::owner);
    let (local_exact_facts, dependency_facts, fact_shapes) =
        facts::source(export, local, &root_exacts, &fact_requirements)?;
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
