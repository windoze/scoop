use super::*;
use hir::DefaultSourceOriginSiteV1 as Site;
use scoop_wire::{WireError, WirePath};

mod foundation;
mod resources;

const ORIGINS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/origins.scoop"
));
fn label(site: Site<'_>) -> String {
    match site {
        Site::Root => "root".into(),
        Site::Local(_) => "local".into(),
        Site::Body(site) => format!("body:{site:?}"),
        Site::Callable(_) => "callable".into(),
        Site::Constructor(_) => "constructor".into(),
        Site::Type(_) => "type".into(),
        Site::Global(_) => "global".into(),
        Site::Singleton(_) => "singleton".into(),
        Site::Field(_) => "field".into(),
    }
}
fn occurrences(
    table: &Table,
) -> Vec<(
    hir::ProtectedDefaultTemplateKeyV1,
    String,
    hir::ExportDefinitionSourceV1,
    WirePath,
)> {
    let mut result = Vec::new();
    table
        .visit_definition_sources_metered(
            &mut |template, origin, site, _, path| {
                result.push((template.key(), label(site), origin.clone(), path.clone()));
                Ok::<_, WireError>(())
            },
            &mut meter(),
            &WirePath::root(),
        )
        .unwrap();
    result
}

#[test]
fn default_source_origins_preserve_typed_sites_paths_and_repeated_occurrences() {
    with_hir_source(ORIGINS, |output, _| {
        let production = Production::from_dependency_hir(output, &mut meter()).unwrap();
        let table = production.templates();
        let actual = occurrences(table);
        assert_eq!(actual, occurrences(&restored(output, table)));
        let export = output.output().export.module();
        let names: BTreeMap<_, _> = export
            .source_parameter_interfaces
            .iter()
            .map(|source| {
                (
                    declaration(export, source.owner),
                    owner_name(export, source.owner),
                )
            })
            .collect();
        let mut counts = BTreeMap::<String, BTreeMap<String, usize>>::new();
        for (key, site, _, _) in &actual {
            *counts
                .entry(names[&key.owner()].clone())
                .or_default()
                .entry(site.clone())
                .or_default() += 1;
        }
        let summary = counts
            .iter()
            .map(|(owner, sites)| {
                format!(
                    "{owner}: {}\n",
                    sites
                        .iter()
                        .map(|(site, count)| format!("{site}={count}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<String>();
        assert_eq!(
            summary,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/origins.snap"
            ))
        );
        for (index, template) in table.records().iter().enumerate() {
            let visits: Vec<_> = actual
                .iter()
                .filter(|(key, _, _, _)| *key == template.key())
                .collect();
            assert_eq!(visits[0].1, "root");
            assert_eq!(visits[0].3, WirePath::root().index(index as u64).field(12));
            let roots: BTreeSet<_> = visits.iter().map(|record| &record.2).collect();
            assert!(visits.len() >= roots.len());
            let source_locals = template
                .locals()
                .records()
                .iter()
                .filter(|local| {
                    matches!(
                        local.definition(),
                        hir::TemplateLocalDefinitionV1::Source(_)
                    )
                })
                .count();
            assert_eq!(
                visits.iter().filter(|record| record.1 == "local").count(),
                source_locals
            );
            for (field_index, (kind, count)) in [
                ("callable", template.references().callables().len()),
                ("constructor", template.references().constructors().len()),
                ("type", template.references().types().len()),
                ("global", template.references().globals().len()),
                ("singleton", template.references().singleton_values().len()),
                ("field", template.references().fields().len()),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(
                    visits.iter().filter(|record| record.1 == kind).count(),
                    count
                );
                for (reference_index, record) in
                    visits.iter().filter(|record| record.1 == kind).enumerate()
                {
                    assert_eq!(
                        record.3,
                        WirePath::root()
                            .index(index as u64)
                            .field(11)
                            .field(field_index as u32 + 1)
                            .index(reference_index as u64)
                            .field(2)
                    );
                }
            }
        }
        assert!(
            actual.len()
                > actual
                    .iter()
                    .map(|record| &record.2)
                    .collect::<BTreeSet<_>>()
                    .len()
        );
    });
}
