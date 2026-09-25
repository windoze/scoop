use super::*;
use hir::{HirDependencyCallReasonV1, SelectedTypeUseV1};
use scoop_wire::{decode_canonical, encode};

mod parameters;
mod rejections;
mod relations;
mod support;
use support::*;

#[test]
fn runtime_constructor_calls_preserve_every_actual_cast_and_default_occurrence() {
    for exception in ["runtime-cast-exception", "runtime-cast-adapter-exception"] {
        for (case, minimum) in [("runtime-cast-standalone", 1), ("runtime-cast-combined", 3)] {
            with_fixture(case, exception, |output, core| {
                let interface = public_projection::public_interface_with_core(output, core);
                let module = output.output().local.module();
                assert!(
                    core.world(module.cone)
                        .has_materializable_nominal_source(
                            ConeIdentity::CORE,
                            core.interface
                                .compiler_protocols()
                                .class_cast_exception_type(),
                        )
                        .unwrap()
                );
                let mut positions = Vec::new();
                module
                    .visit_executable_expressions(|occurrence| {
                        if let hir::concrete::ExprKind::Cast {
                            check_ty,
                            optional: false,
                            ..
                        } = occurrence.expression.kind
                        {
                            positions.push((
                                occurrence.position,
                                module.exact_type_identities.get(check_ty).unwrap().id(),
                            ));
                        }
                        Ok::<_, std::convert::Infallible>(())
                    })
                    .unwrap();
                assert_eq!(positions.len(), minimum, "{case}");
                let actual = calls(&interface)
                    .map(|(reference, site)| {
                        let HirDependencyCallReasonV1::CastFailure { checked_type } = site.reason()
                        else {
                            unreachable!("the iterator selects runtime calls")
                        };
                        assert!(site.arguments().is_empty() && site.witness_indices().is_empty());
                        assert!(
                            reference.roles().contains(
                                hir::ExternalHirReferenceRoleV1::RuntimeOperationDependency
                            )
                        );
                        (site.position(), *checked_type)
                    })
                    .collect::<Vec<_>>();
                positions.sort_unstable();
                assert_eq!(actual, positions, "{case}");
                let mut graph = identities(output, core);
                let mut wire_interface = interface.clone();
                let decoded: hir::DecodedCrossConeHirInterfaceSectionV1 =
                    decode_canonical(&encode(&wire_interface.index_for_wire().unwrap()).unwrap())
                        .unwrap();
                let decoded = decoded.resolve(&mut graph).unwrap();
                assert_eq!(decoded, interface);
                let mut dump = String::new();
                with_metadata(output, &decoded, core, |metadata, dependency| {
                    let required = decoded
                        .external_references()
                        .materialized_type_dependencies(metadata.provider, metadata.identities)
                        .unwrap();
                    for (reference, site) in calls(&decoded) {
                        assert!(required.contains(&(reference.origin(), site.result())));
                        let (_, owner) = site
                            .runtime_constructor_source(
                                reference.target(),
                                dependency.provider,
                                dependency.identities,
                                dependency.public,
                            )
                            .unwrap();
                        site.validate_runtime_constructor_role(
                            reference.target(),
                            owner,
                            core.interface.compiler_protocols(),
                        )
                        .unwrap();
                        dump.push_str(&format!(
                            "call {:?}\nposition {:?}\nreason {:?}\nresult {}\norigin {:?}\n",
                            reference.target(),
                            site.position(),
                            site.reason(),
                            site.result(),
                            site.origin()
                        ));
                    }
                    let selected = metadata.materialized_type_uses(&[dependency]).unwrap();

                    for record in selected.records() {
                        dump.push_str(&format!(
                            "selected {} {:?}\n",
                            record.provider(),
                            record.usage()
                        ));
                    }
                });
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tests/fixtures/m23-executable-type-sites/{case}.{exception}.hir.snap"
                ));
                if std::env::var_os("SCOOP_UPDATE_RUNTIME_CAST_SNAPSHOTS").is_some() {
                    std::fs::write(&path, &dump).unwrap();
                }
                assert_eq!(dump, std::fs::read_to_string(path).unwrap());
            });
        }
    }
}

#[test]
fn runtime_cast_construction_cannot_bypass_generic_representation_dependencies() {
    with_hir_source(
        &runtime_fixture("runtime-cast-standalone"),
        |output, core| {
            let public = public_projection::public_interface_with_core(output, core);
            let expected = core
                .interface
                .compiler_protocols()
                .class_cast_exception_type();
            let world = core.world(output.output().local.module().cone);

            assert!(
                !world
                    .has_materializable_nominal_source(ConeIdentity::CORE, expected)
                    .unwrap()
            );

            with_metadata(output, &public, core, |metadata, dependency| {
                assert!(matches!(
                    metadata.materialized_type_uses(&[dependency]),
                    Err(hir::SharedTypeMetadataError::SourceOnlyNominal(owner)) if owner == expected
                ));
            });
        },
    );
}
