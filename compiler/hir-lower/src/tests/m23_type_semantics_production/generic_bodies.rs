use super::*;
use scoop_identity::{DefinitionOriginSubject, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-production/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-production/combined.scoop"
));

#[test]
fn generic_bodies_project_from_source_and_close_only_used_callable_templates() {
    with_hir_source(STANDALONE, |output, core| {
        let section = public_projection::public_interface_with_core(output, core);
        let table = section.generic_callable_bodies();
        let export = output.output().export.module();
        for name in ["identity", "composed", "helper"] {
            assert!(table.get(owner(export, name)).is_some(), "{name}");
        }
        for name in ["seed", "unused"] {
            assert!(table.get(owner(export, name)).is_none(), "{name}");
        }
        assert_eq!(table.records().len(), 3);
        let identity = table.get(owner(export, "identity")).unwrap();
        assert_eq!(
            identity.parameters(),
            &[scoop_identity::LocalValueSelector::Parameter {
                declaration_index: 0
            }]
        );
        assert_eq!(
            identity.result(),
            &SignatureTypeKey::Binder { depth: 0, index: 0 }
        );
        assert!(matches!(
            identity.statements()[0].kind(),
            hir::DefaultStatementKindV1::Return(_)
        ));
        roundtrip(output, core, &section);
        snapshot(output, "standalone");
    });
}

#[test]
fn generic_bodies_keep_owner_and_method_binders_and_lexical_capture_bodies() {
    with_hir_source(COMBINED, |output, core| {
        let section = public_projection::public_interface_with_core(output, core);
        let table = section.generic_callable_bodies();
        let export = output.output().export.module();
        let method = table.get(owner(export, "Box.choose")).unwrap();
        assert_eq!(
            method.type_parameters().arguments(),
            &[
                SignatureTypeKey::Binder { depth: 1, index: 0 },
                SignatureTypeKey::Binder { depth: 0, index: 0 },
            ]
        );
        assert!(table.get(owner(export, "Box.read")).is_some());
        assert!(table.get(owner(export, "echo")).is_some());
        assert!(table.get(owner(export, "captured")).is_some());
        assert!(table.records().iter().any(|body| matches!(
            body.owner(),
            hir::DefaultCallableDeclarationV1::Generated(_)
        )));
        let local = export.local_functions.iter().next().unwrap().1;
        let local_owner = function_owner(export, local.source_function());
        assert!(table.get(local_owner).is_some());
        roundtrip(output, core, &section);
        snapshot(output, "combined");
    });
}

#[test]
fn generic_body_projection_requires_its_real_definition_origin() {
    with_hir_source(STANDALONE, |output, _| {
        let mut export = output.output().export.module().clone();
        let hir::DefaultCallableDeclarationV1::GenericFunction(id) = owner(&export, "identity")
        else {
            panic!("identity is a generic declaration")
        };
        let subject = DefinitionOriginSubject::GenericFunction(id);
        export.export_definition_origins = hir::HirExportDefinitionOrigins::canonicalize(
            export
                .export_definition_origins
                .records()
                .iter()
                .filter(|record| record.subject() != subject)
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            hir::CanonicalExportGenericCallableBodiesV1::from_export_hir(&export),
            Err(hir::GenericTemplateProductionError::MissingOrigin(actual)) if actual == subject
        ));
    });
}

fn owner(export: &hir::ExportHir, name: &str) -> hir::DefaultCallableDeclarationV1 {
    let function = export
        .functions
        .iter()
        .find(|(_, declaration)| declaration.name == name)
        .unwrap_or_else(|| panic!("missing fixture function {name}"))
        .0;
    function_owner(export, function)
}

fn function_owner(
    export: &hir::ExportHir,
    function: hir::FunctionId,
) -> hir::DefaultCallableDeclarationV1 {
    match &export.function_identities[function] {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
            hir::DefaultCallableDeclarationV1::Function(record.id())
        }
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
            hir::DefaultCallableDeclarationV1::GenericFunction(record.id())
        }
        hir::HirFunctionIdentity::LexicalGenerated(record) => {
            hir::DefaultCallableDeclarationV1::Generated(record.id())
        }
        other => panic!("unexpected fixture callable identity {other:?}"),
    }
}

pub(super) fn roundtrip(
    output: &hir::DependencyHirOutput,
    core: &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    section: &hir::CrossConeHirInterfaceSectionV1,
) {
    let bytes = encode(&section.clone().index_for_wire().unwrap()).unwrap();
    let decoded: hir::DecodedCrossConeHirInterfaceSectionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut identities = source_inventory::identity_closure(output);
    let restored = decoded.resolve(&mut identities).unwrap();
    assert_eq!(&restored, section);
    restored
        .validate_definition_source_closure(&scoop_wire::WirePath::root())
        .unwrap();
    let export = output.output().export.module();
    let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
    let world = core.world(export.cone);
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        &foundation,
        &export.public_export_bindings,
        &world,
    );
    restored
        .validate_external_reference_closure(&mut authority, &scoop_wire::WirePath::root())
        .unwrap();
}

fn snapshot(output: &hir::DependencyHirOutput, name: &str) {
    let actual = hir::dump(&output.output().export);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-body-production")
        .join(format!("{name}.hir.snap"));
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}

#[test]
fn generic_bodies_retain_private_type_and_property_support_without_public_bindings() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-body-production/support.scoop"
    ));
    with_hir_source(source, |output, core| {
        let section = public_projection::public_interface_with_core(output, core);
        let export = output.output().export.module();
        assert!(
            section
                .generic_callable_bodies()
                .get(owner(export, "Support.pick"))
                .is_some()
        );
        assert_eq!(section.public_bindings().records().len(), 1);
        assert!(section.nominal_interfaces().records().is_empty());
        assert_eq!(section.nominal_interfaces().support_records().len(), 1);
        assert!(
            section
                .property_interfaces()
                .support_records()
                .iter()
                .any(|property| {
                    export.properties.iter().any(|(id, value)| {
                        value.name == "marker"
                            && export.property_identities[id].property_owner()
                                == property.declaration()
                    })
                })
        );
        roundtrip(output, core, &section);
        snapshot(output, "support");
    });
}

#[test]
fn generic_body_support_types_enter_shape_demands_before_concretization() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-body-production/concrete-support.scoop"
    ));
    with_hir_source(source, |output, core| {
        let section = public_projection::public_interface_with_core(output, core);
        let roots = output.output().local.materialization().roots();
        assert_eq!(roots.len(), 1);
        assert!(matches!(
            roots[0].declaration().name(),
            scoop_identity::DeclarationName::Named(name) if name.as_str() == "HiddenValue"
        ));
        assert_eq!(section.public_bindings().records().len(), 1);
        assert!(section.nominal_interfaces().records().is_empty());
        assert_eq!(section.nominal_interfaces().support_records().len(), 1);
        assert_eq!(section.generic_callable_bodies().records().len(), 1);
        let required =
            hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export).unwrap();
        assert_eq!(
            required.values(),
            &[hir::SourceNominalId::Concrete(roots[0].source())]
        );
        let source = produce_cross_cone_type_semantics(output, &section).unwrap();
        assert_eq!(source.representation_support().records().len(), 1);
        roundtrip(output, core, &section);
        snapshot(output, "concrete-support");
    });
}
