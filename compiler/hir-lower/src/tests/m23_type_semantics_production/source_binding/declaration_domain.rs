use super::super::source_dispatch::with_hir_source;
use super::nominal_parameters::support::Sources;
use super::*;
use hir::{ProtectedDeclarationSemanticAuthority, TypeSectionDeclarationSemanticAuthority};
use hir::{TypeDeclarationSourceAuthorityV1 as Domain, TypeDeclarationSourceBindingError as Error};

mod rejection;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/declaration-domain.scoop"
));
fn with_domain(
    source: &str,
    run: impl FnOnce(
        &hir::OrdinaryHirOutput<'_>,
        &mut Fixture,
        &Sources,
        &Domain,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let independent = Sources::from_output(output, &mut fixture);
        let source = Domain::from_ordinary_hir(output, &mut meter()).unwrap();
        assert_eq!(
            Domain::from_ordinary_hir(output, &mut meter()).unwrap(),
            source
        );
        let bytes = encode(&source).unwrap();
        let decoded: hir::DecodedTypeDeclarationSourceAuthorityV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let restored = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(restored, source);
        let core = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(
            output,
            &mut fixture,
            &independent,
            &restored,
            core.protocols().fundamental_types(),
        );
    });
}

#[test]
fn declaration_domain_roundtrips_and_replays_the_complete_artifact_source_chain() {
    for input in [
        SOURCE,
        "public class Plain {}",
        "fun unrelated(): Int = 1",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/protected-declarations.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/nested-binding.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/selections.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
        )),
    ] {
        with_domain(input, |output, fixture, independent, domain, core| {
            let e = domain.entries();
            assert_eq!(e.nominals, independent.members.nominals);
            assert_eq!(e.callables, independent.members.callables);
            assert_eq!(e.properties, independent.members.properties);
            assert_eq!(e.constructors, independent.constructors);
            let produced = hir::ProtectedDeclarationSourceProductionV1::from_export_hir(
                &output.output().export,
                &mut meter(),
            )
            .unwrap();
            let (table, protocols) =
                super::protected_declarations::support::restore(fixture, &produced);
            let foundation = fixture.bind().unwrap();
            domain
                .with_bound_sources(
                    &foundation,
                    &independent.protocols,
                    core,
                    &mut meter(),
                    |authority, meter| {
                        assert_eq!(authority.provider(), fixture.source.entries().provider);
                        assert_eq!(
                            authority.required_protected_declarations().unwrap(),
                            &e.required_protected
                        );
                        let entries = fixture.source.entries();
                        let graph =
                            hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
                                entries.local_inheritance_edges.records().iter(),
                                entries.source_roots.values().iter().copied(),
                                &foundation,
                                meter,
                            )
                            .unwrap();
                        assert!(std::ptr::eq(
                            authority
                                .validate_protected_sources(
                                    &table,
                                    &protocols,
                                    &entries.representations,
                                    &graph,
                                    meter
                                )
                                .unwrap()
                                .table(),
                            &table
                        ));
                    },
                )
                .unwrap();
            if input == SOURCE {
                let mut names: Vec<_> = e
                    .nominals
                    .records()
                    .iter()
                    .map(
                        |r| match foundation.nominal_key(r.owner()).unwrap().name() {
                            scoop_identity::DeclarationName::Named(name) => name.as_str(),
                            _ => panic!("source nominal name"),
                        },
                    )
                    .collect();
                names.sort();
                let outline = format!(
                    "nominals={}\nprotected={}\nconstructors={}\nproperties={}\ncallables={}\ninheritance={}\ninterfaces={}\nselections={}\ndispatch_callables={}\n",
                    names.join(","),
                    e.required_protected.values().len(),
                    e.constructors.records().len(),
                    e.properties.records().len(),
                    e.callables.records().len(),
                    e.inheritance.records().len(),
                    e.interfaces.records().len(),
                    e.selections.records().len(),
                    e.dispatch_callables.records().len()
                );
                assert_eq!(
                    outline,
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-nominals/declaration-domain.snap"
                    ))
                );
            }
        });
    }
}
