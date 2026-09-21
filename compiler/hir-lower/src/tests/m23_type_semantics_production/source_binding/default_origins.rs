use super::nominal_parameters::support::{Sources, with_sources};
use super::*;
use hir::{CanonicalDefaultSourceTemplatesV1 as Table, DefaultSourceOriginBindingError as Error};
use scoop_identity::CallableTemplateOrigin;

mod corruption;
mod dependencies;
mod resources;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/origin-binding.scoop"
));

pub(super) fn templates(output: &hir::OrdinaryHirOutput) -> Table {
    let production =
        hir::NominalDefaultSourceProductionV1::from_ordinary_hir(output, &mut meter()).unwrap();
    let bytes = encode(&production.templates().index_locals(&mut meter()).unwrap()).unwrap();
    let decoded: hir::DecodedCanonicalDefaultSourceTemplatesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded
        .resolve(&mut identity_closure(output), &mut meter())
        .unwrap();
    assert_eq!(
        encode(&restored.index_locals(&mut meter()).unwrap()).unwrap(),
        bytes
    );
    restored
}

pub(super) fn replace(table: &Table, template: hir::DefaultSourceTemplateV1) -> Table {
    Table::try_new(
        table
            .records()
            .iter()
            .map(|record| {
                if record.key() == template.key() {
                    template.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
        &mut meter(),
    )
    .unwrap()
}

fn rebuild(
    t: &hir::DefaultSourceTemplateV1,
    root: hir::PersistentLexicalRootV1,
    body: hir::ExportDefaultBodyV1,
    origin: hir::ExportDefinitionSourceV1,
) -> hir::DefaultSourceTemplateV1 {
    hir::DefaultSourceTemplateV1::try_new(
        t.key(),
        root,
        t.definition_path().clone(),
        t.locals().clone(),
        body,
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        t.references().clone(),
        origin,
        &mut meter(),
    )
    .unwrap()
}

#[test]
fn complete_default_locations_bind_from_artifact_bytes_for_all_source_roles() {
    for source in [
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters-binding.scoop"
        )),
    ] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                let bound = parameters
                    .bind_default_origins(&table, &[], &mut meter())
                    .unwrap();
                assert_eq!(bound.provider(), parameters.provider());
                assert!(std::ptr::eq(bound.parameters(), &parameters));
                assert!(std::ptr::eq(bound.templates(), &table));
                for predicate in [
                    (|owner| matches!(owner, CallableTemplateOrigin::Function(_)))
                        as fn(CallableTemplateOrigin) -> bool,
                    |owner| matches!(owner, CallableTemplateOrigin::GenericFunction(_)),
                    |owner| matches!(owner, CallableTemplateOrigin::Constructor(_)),
                    |owner| matches!(owner, CallableTemplateOrigin::VariantConstructor(_)),
                ] {
                    assert!(table.records().iter().any(|t| predicate(t.key().owner())));
                }
                if source == SOURCE {
                    assert!(
                        table
                            .records()
                            .iter()
                            .any(|t| t.key().owner() != t.definition_root().declaration())
                    );
                }
            });
        });
    }
}
