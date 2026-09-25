use super::*;
use crate::tests::m23_type_semantics_production::source_dispatch::with_hir_sources;
use hir::TypeDefinitionSourceSemanticAuthority;

const MEMBERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/members.scoop"
));
const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/nested.scoop"
));

#[test]
fn complete_type_section_publishes_protected_members_constructors_and_nested_sources() {
    for (source, name, expected) in [
        (
            MEMBERS,
            "members",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-protected-production/members.scoop.snap"
            )),
        ),
        (
            NESTED,
            "nested",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-protected-production/nested.scoop.snap"
            )),
        ),
    ] {
        with_hir_source(source, |output, core| {
            let produced =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let bytes = encode(&produced.section().index_for_wire().unwrap()).unwrap();
            let decoded: hir::DecodedCrossConeTypeSemanticsSectionV1 =
                decode_canonical(&bytes).unwrap();
            let section = decoded
                .resolve(&mut fixture.identities, &WirePath::root())
                .unwrap();
            assert_eq!(encode(&section.index_for_wire().unwrap()).unwrap(), bytes);
            let inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output).unwrap();
            for record in section.inheritance().records() {
                let source = inventory.get(record.owner()).unwrap();
                assert_eq!(record.protected_members(), source.protected_members());
                assert_eq!(
                    record
                        .constructors()
                        .records()
                        .iter()
                        .map(|c| c.declaration())
                        .collect::<Vec<_>>(),
                    source.constructors().values()
                );
            }
            let foundation = fixture.bind().unwrap();
            let core = core.foundation.import_core_inputs(&core.interface).unwrap();
            sources.with_bound(
                &foundation,
                core.protocols().fundamental_types(),
                |members, constructors| {
                    let mut authority = members
                        .bind_parameter_protocols(constructors, &sources.protocols)
                        .unwrap();
                    for protocol in section.protected_source_interfaces().records() {
                        authority.validate_source_protocol(protocol).unwrap();
                    }
                    authority
                        .validate_protected_declarations(
                            section.protected_declarations(),
                            section.protected_source_interfaces(),
                            section.representation_support(),
                        )
                        .unwrap();
                    let incomplete = hir::CanonicalProtectedDeclarationInterfacesV1::try_new(
                        section.protected_declarations().records()[1..].to_vec(),
                    )
                    .unwrap();
                    assert!(matches!(
                        authority.validate_protected_declarations(
                            &incomplete,
                            section.protected_source_interfaces(),
                            section.representation_support()
                        ),
                        Err(hir::ProtectedDeclarationBindingError::Inventory)
                    ));
                    for record in section.inheritance().records() {
                        for constructor in record.constructors().records() {
                            assert_eq!(
                                constructor.source(),
                                constructors
                                    .constructor_source(constructor.declaration())
                                    .unwrap()
                            );
                        }
                    }
                },
            );
            section
                .representation_support()
                .validate_source_semantics(produced.foundation(), &WirePath::root())
                .unwrap();
            section
                .definition_source_inputs()
                .validate_definition_sources(
                    section.definition_sources(),
                    &mut SourceOrigins {
                        foundation: &foundation,
                        parameters: &sources.protocols,
                    },
                    &WirePath::root(),
                )
                .unwrap();
            let incomplete = hir::CanonicalExportDefinitionSourcesV1::try_new(
                section.definition_sources().sources()[1..].to_vec(),
            )
            .unwrap();
            assert!(matches!(
                section
                    .definition_source_inputs()
                    .validate_definition_sources(
                        &incomplete,
                        &mut SourceOrigins {
                            foundation: &foundation,
                            parameters: &sources.protocols
                        },
                        &WirePath::root()
                    ),
                Err(hir::TypeDefinitionSourceClosureError::Missing { .. })
            ));
            let dump = outline(&foundation, section.protected_declarations());
            if std::env::var_os("SCOOP_UPDATE_PROTECTED_PRODUCTION_SNAPSHOTS").is_some() {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tests/fixtures/m23-type-protected-production/{name}.scoop.snap"
                ));
                std::fs::write(path, dump).unwrap();
            } else {
                assert_eq!(dump, expected);
            }
        });
    }
}

#[test]
fn protected_type_section_is_stable_across_unrelated_arena_allocation() {
    let produce = |files: &[(&str, &str)]| {
        with_hir_sources(files, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            encode(&production.section().index_for_wire().unwrap()).unwrap()
        })
    };
    let first = produce(&[("src/main.scoop", NESTED)]);
    let second = produce(&[
        ("src/a-unused.scoop", "private fun unrelated(): Int = 0"),
        ("src/main.scoop", NESTED),
    ]);
    assert!(
        first == second,
        "type section bytes changed with unrelated arena allocation"
    );
}

#[test]
fn protected_type_production_publishes_the_required_default_body() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-protected-production/default.scoop"
    ));
    with_hir_source(source, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let defaults = production.section().protected_defaults().records();
        assert_eq!(defaults.len(), 1);
        assert!(matches!(
            defaults[0].key().owner(),
            CallableTemplateOrigin::Constructor(_)
        ));
        assert_eq!(
            defaults[0].definition_root().declaration(),
            defaults[0].key().owner()
        );
        assert_eq!(
            defaults[0].body().value().result_type(),
            defaults[0].result()
        );
    });
}

struct SourceOrigins<'a, 'f> {
    foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
    parameters: &'a hir::CanonicalNominalSourceParameterProtocolsV1,
}
impl TypeDefinitionSourceSemanticAuthority<&'static str> for SourceOrigins<'_, '_> {
    fn validate_type_definition_source_use(
        &mut self,
        source_use: hir::TypeDefinitionSourceUseV1<'_>,
        source: &hir::ExportDefinitionSourceV1,

        _path: &WirePath,
    ) -> Result<(), &'static str> {
        let present = match source_use {
            hir::TypeDefinitionSourceUseV1::SourceParameter {
                source: protocol,
                parameter_index,
            } => self
                .parameters
                .get(protocol.owner())
                .and_then(|record| record.parameters().get(parameter_index))
                .is_some_and(|parameter| parameter.definition_origin() == source),
            _ => self.foundation.contains_definition_source(source),
        };
        if present {
            Ok(())
        } else {
            Err("source is absent from its restored HIR declaration or parameter protocol")
        }
    }
}
