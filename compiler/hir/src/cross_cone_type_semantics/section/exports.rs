use super::*;
use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WirePath};

mod errors;
mod fact_shapes;
mod inventory;
mod overlap;
pub use errors::*;

/// All seven local tables and their independent source inventories are joined.
/// This internal state cannot be published: selected closure still has to pass.
#[derive(Debug)]
pub(super) struct CheckedTypeSectionExportsV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) candidate: &'a CrossConeTypeSemanticsSectionV1,
    pub(super) public: CheckedTypeSectionPublicSupportV1<'a>,
    pub(super) source_roots: &'a [SourceNominalId],
    pub(super) graph: CheckedNominalInheritanceGraphV1<'a>,
    pub(super) facts: CheckedExactTypeFactsV1<'a>,
    pub(super) representations: CheckedNominalRepresentationSupportV1<'a>,
    pub(super) inheritance: CheckedNominalInheritanceInterfacesV1<'a>,
    pub(super) inheritance_records: std::collections::BTreeMap<
        scoop_identity::PersistentExactTypeId,
        &'a NominalInheritanceInterfaceV1,
    >,
    pub(super) protected: CheckedProtectedDeclarationSourcesV1<'a>,
    pub(super) sources: CheckedProtectedSourceInterfacesV1<'a>,
    pub(super) defaults: CheckedProtectedDefaultTemplatesV1<'a>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate<'a, F, S, D, E>(
    candidate: &'a CrossConeTypeSemanticsSectionV1,
    public: CheckedTypeSectionPublicSupportV1<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    foundation: &'a F,
    declarations: &mut S,
    defaults: &mut D,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<CheckedTypeSectionExportsV1<'a>, TypeSectionExportValidationError<E>>
where
    F: TypeSectionFoundationSemanticAuthority<E>,
    S: TypeSectionDeclarationSemanticAuthority<E>,
    D: TypeSectionDefaultSemanticAuthority<E>,
{
    use TypeSectionExportValidationError as Error;
    let provider = foundation.current_provider();
    if public.provider() != provider {
        return Err(Error::Provider);
    }
    inventory::dependencies(provider, dependencies, meter, path)?;
    let source_roots = foundation.local_source_roots().map_err(Error::Source)?;
    let edges = foundation
        .local_inheritance_edges()
        .map_err(Error::Source)?;
    inventory::sources(candidate, source_roots, edges, foundation, meter, path)?;
    let fact_dependencies = inventory::facts(candidate, dependencies, foundation, meter, path)?;
    let facts = candidate
        .exact_facts
        .validate_semantics_with_dependencies(foundation, &fact_dependencies, meter)
        .map_err(|e| Error::Facts(Box::new(e)))?;
    let representations = candidate
        .representation_support
        .validate_source_semantics(foundation, meter, &path.clone().field(2))
        .map_err(|e| Error::Representation(Box::new(e)))?;
    fact_shapes::validate(representations, foundation, meter, path)?;
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        edges.iter().chain(dependencies.iter().flat_map(|section| {
            section
                .exports
                .candidate
                .inheritance
                .records()
                .iter()
                .map(NominalInheritanceInterfaceV1::edges)
        })),
        source_roots.iter().copied().chain(
            dependencies
                .iter()
                .flat_map(|section| section.exports.source_roots.iter().copied()),
        ),
        foundation,
        meter,
    )
    .map_err(|e| Error::Graph(Box::new(e)))?;
    let protected = candidate
        .protected_declarations
        .validate_sources(&graph, representations.table(), declarations, meter)
        .map_err(|e| Error::Protected(Box::new(e)))?;
    let inheritance = candidate
        .inheritance
        .validate_interfaces(&graph, protected, declarations, meter)
        .map_err(|e| Error::Inheritance(Box::new(e)))?;
    let inheritance_records =
        inventory::inheritance_records(inheritance, dependencies, meter, path)?;
    let sources = candidate
        .protected_source_interfaces
        .validate_protocols(
            protected,
            inheritance,
            &graph,
            candidate.protected_defaults.keys(),
            declarations,
            defaults,
            meter,
        )
        .map_err(|e| Error::Sources(Box::new(e)))?;
    let checked_defaults = candidate
        .protected_defaults
        .validate_semantics(
            &sources,
            &graph,
            inheritance,
            declarations,
            defaults,
            meter,
            &path.clone().field(6),
        )
        .map_err(|e| Error::Defaults(Box::new(e)))?;
    candidate
        .definition_source_inputs()
        .validate_definition_sources(
            &candidate.definition_sources,
            defaults,
            meter,
            &path.clone().field(7),
        )
        .map_err(|e| Error::Origins(Box::new(e)))?;
    overlap::validate(
        candidate,
        public,
        &graph,
        &sources,
        &checked_defaults,
        foundation,
        meter,
        path,
    )?;
    Ok(CheckedTypeSectionExportsV1 {
        provider,
        candidate,
        public,
        source_roots,
        graph,
        facts,
        representations,
        inheritance,
        inheritance_records,
        protected,
        sources,
        defaults: checked_defaults,
    })
}
