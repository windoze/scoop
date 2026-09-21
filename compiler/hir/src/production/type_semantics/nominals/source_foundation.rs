use super::*;
use scoop_wire::{BudgetMeter, WirePath};

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
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::from_hir(output.output(), meter)
    }

    /// Projects declaration metadata from the same sealed HIR pair for every Cone.
    /// Source projection references existing exact types; it does not materialize them.
    pub fn from_hir(output: &Output, meter: &mut BudgetMeter) -> Result<Self, Error> {
        project(output, meter).map(|projection| projection.foundation)
    }
}

pub(super) fn project<'a>(
    output: &'a Output,
    meter: &mut BudgetMeter,
) -> Result<Projection<'a>, Error> {
    let path = WirePath::root();
    meter
        .check_semantic_depth(1, &path)
        .map_err(inheritance::source_resources::resource)?;
    meter
        .charge_nodes(1, &path)
        .map_err(inheritance::source_resources::resource)?;
    meter
        .charge_work(1, &path)
        .map_err(inheritance::source_resources::resource)?;
    let export = output.export.module();
    let local = output.local.module();
    let public = CanonicalNominalInterfacesV1::from_export_hir(export)
        .map_err(|error| Error::PublicInterface(error.to_string()))?;
    let required = CanonicalSourceNominalIdsV1::from_export_hir(&output.export, meter)?;
    let sources = sources::project(export, &required, meter)?;
    let mut roots = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut roots,
            required.values().len(),
            &scoop_wire::WirePath::root(),
        )
        .map_err(inheritance::source_resources::resource)?;
    roots.extend_from_slice(required.values());
    // Every param-free declaration in the source closure needs representation
    // support; generic templates remain source-only. Exact pairs are never made up.
    let concrete = source_inventory::from_pair(output, &required, meter)?;
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
