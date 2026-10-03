use super::*;
use source_dispatch::with_hir_source;

mod positions;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-initialization/standalone.scoop"
));

#[test]
fn generic_initializations_preserve_source_execution_and_roundtrip_shared_nodes() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-initialization");
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
        with_hir_source(&source, |output, core| {
            let section = public_projection::public_interface_with_core(output, core);
            let table = section.generic_initializations();
            let export = output.output().export.module();
            assert_eq!(
                table.records().len(),
                if case == "standalone" { 2 } else { 5 }
            );
            let named = |name: &str| {
                let (id, _) = export
                    .classes
                    .iter()
                    .find(|(_, declaration)| declaration.name == name)
                    .unwrap();
                let identity = export.nominal_identities[id].generic_type_id().unwrap();
                table.get(identity).unwrap()
            };
            if case == "standalone" {
                let box_ = named("Box");
                assert_eq!(box_.constructors().len(), 3);
                assert!(matches!(
                    box_.common(),
                    [
                        hir::ExportCommonInitializationStepV1::Field { .. },
                        hir::ExportCommonInitializationStepV1::Field { .. },
                        hir::ExportCommonInitializationStepV1::Body(_),
                    ]
                ));
                let primary = box_
                    .constructors()
                    .iter()
                    .find_map(|constructor| match constructor.kind() {
                        hir::ExportConstructorInitializationKindV1::ClassPrimary {
                            primary_stores,
                            ..
                        } => Some(primary_stores),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(primary.len(), 1);
                assert_eq!(
                    primary[0].parameter,
                    scoop_identity::LocalValueSelector::Parameter {
                        declaration_index: 1
                    }
                );
                let structure = table
                    .records()
                    .iter()
                    .find(|record| {
                        record.constructors().iter().any(|constructor| {
                            matches!(
                                constructor.kind(),
                                hir::ExportConstructorInitializationKindV1::StructPrimary
                            )
                        })
                    })
                    .unwrap();
                assert_eq!(structure.constructors().len(), 2);
                assert!(structure.common().is_empty());
                let (unused, _) = export
                    .classes
                    .iter()
                    .find(|(_, declaration)| declaration.name == "Unused")
                    .unwrap();
                assert!(
                    table
                        .get(export.nominal_identities[unused].generic_type_id().unwrap())
                        .is_none()
                );
            } else {
                let alternate = named("Alternate");
                assert_eq!(alternate.common().len(), 3);
                assert_eq!(alternate.constructors().iter().filter(|constructor| matches!(constructor.kind(), hir::ExportConstructorInitializationKindV1::ClassSecondaryTerminal { .. })).count(), 2);
                assert!(matches!(
                    named("Holder").common(),
                    [
                        hir::ExportCommonInitializationStepV1::Field { .. },
                        hir::ExportCommonInitializationStepV1::Body(_),
                    ]
                ));
                assert_eq!(named("Cell").constructors().len(), 1);
            }
            let helper = export
                .functions
                .iter()
                .find(|(_, function)| function.name == "keep")
                .unwrap()
                .0;
            let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(identity)) =
                &export.function_identities[helper]
            else {
                panic!("the constructor helper is generic")
            };
            assert!(
                section
                    .generic_callable_bodies()
                    .get(hir::DefaultCallableDeclarationV1::GenericFunction(
                        identity.id()
                    ))
                    .is_some()
            );
            generic_bodies::roundtrip(output, core, &section);
            let identities = source_inventory::identity_closure(output);
            let groups = output
                .output()
                .local
                .exact_type_identities
                .nominal_specialization_records();
            assert!(!groups.is_empty());
            for group in groups {
                assert_eq!(
                    identities
                        .canonical_key::<_, scoop_identity::SpecializationKey>(group.id())
                        .unwrap()
                        .as_ref(),
                    group.key(),
                );
            }
        });
    }
}

#[test]
fn generic_initialization_records_reject_incomplete_or_inconsistent_shapes() {
    use hir::{
        CanonicalExportGenericInitializationsV1 as Table,
        ExportConstructorInitializationKindV1 as Kind, ExportConstructorInitializationV1 as Ctor,
        ExportGenericNominalInitializationV1 as Nominal, GenericInitializationBuildError as Error,
    };

    with_hir_source(STANDALONE, |output, core| {
        let section = public_projection::public_interface_with_core(output, core);
        let records = section.generic_initializations().records();
        let class = records
            .iter()
            .find(|record| !record.common().is_empty())
            .unwrap();
        let structure = records
            .iter()
            .find(|record| record.common().is_empty())
            .unwrap();
        let primary = class
            .constructors()
            .iter()
            .find(|constructor| matches!(constructor.kind(), Kind::ClassPrimary { .. }))
            .unwrap();
        let rebuild = |kind| {
            Ctor::try_new(
                primary.declaration().clone(),
                primary.inputs().clone(),
                primary.effects(),
                primary.predicates().clone(),
                primary.definition_origin().clone(),
                kind,
            )
        };

        assert_eq!(
            Nominal::try_new(class.owner(), Vec::new(), Vec::new()),
            Err(Error::MissingConstructors)
        );
        assert_eq!(
            Nominal::try_new(
                class.owner(),
                Vec::new(),
                vec![primary.clone(), primary.clone()]
            ),
            Err(Error::ConstructorOrder)
        );
        assert_eq!(
            Nominal::try_new(structure.owner(), Vec::new(), class.constructors().to_vec()),
            Err(Error::ConstructorOwner)
        );
        assert_eq!(
            Table::try_new(vec![class.clone(), class.clone()]),
            Err(Error::NominalOrder)
        );
        assert_eq!(
            Nominal::try_new(
                structure.owner(),
                class.common().to_vec(),
                structure.constructors().to_vec()
            ),
            Err(Error::StructCommonInitialization)
        );
        assert_eq!(rebuild(Kind::StructPrimary), Err(Error::ConstructorKind));

        let Kind::ClassPrimary {
            base,
            primary_stores,
        } = primary.kind()
        else {
            unreachable!("the primary constructor was selected above")
        };
        let mut invalid_stores = primary_stores.clone();
        invalid_stores[0].parameter = scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 999,
        };
        assert_eq!(
            rebuild(Kind::ClassPrimary {
                base: base.clone(),
                primary_stores: invalid_stores
            }),
            Err(Error::PrimaryStore)
        );

        let hir::ExportCommonInitializationStepV1::Field { field, value } = &class.common()[0]
        else {
            panic!("the first common step initializes a field")
        };
        assert_eq!(
            rebuild(Kind::ClassSecondaryTerminal {
                base: None,
                body: value.clone()
            }),
            Err(Error::StatementResults)
        );
        let empty_value =
            hir::ExportTemplateFragmentV1::new(value.locals().clone(), Vec::new(), Vec::new());
        assert_eq!(
            Nominal::try_new(
                class.owner(),
                vec![hir::ExportCommonInitializationStepV1::Field {
                    field: field.clone(),
                    value: empty_value
                }],
                class.constructors().to_vec()
            ),
            Err(Error::FieldInitializer)
        );
    });
}
