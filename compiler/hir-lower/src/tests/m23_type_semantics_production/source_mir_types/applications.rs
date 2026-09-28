use super::*;
use scoop_mir::{MirTypeBridgeAuthority, MirTypeBridgeTypeIndexV1, MirTypeBridgeTypeLookupV1};

#[test]
fn actual_generic_type_exports_preserve_payloads_fields_and_original_owners() {
    for name in ["generic-values", "generic-combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-source-mir-types");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        with_production(&source, |output, input, hir, graph, sources| {
            let types = scoop_mir_lower::lower_type_exports(
                output.output().local.module(),
                hir,
                input,
                graph,
            )
            .unwrap();
            let restored: scoop_mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(&types);
            assert_eq!(restored.validate(graph, input.foundation()).unwrap(), types);
            let applications = CanonicalParamFreeMirTypeExportsV1::try_new(
                types
                    .records()
                    .iter()
                    .filter(|record| {
                        matches!(record.origin(), MirTypeOriginV1::NominalApplication(_))
                    })
                    .cloned()
                    .collect(),
            )
            .unwrap();
            assert!(applications.records().len() >= 3);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&applications, &applications]).unwrap();
            assert_eq!(index.record_count(), applications.records().len());
            for record in applications.records() {
                assert_eq!(index.get(record.exact()), Some(record));
                let actual = input
                    .module()
                    .meta
                    .source_exact_types
                    .get_by_identity(record.exact())
                    .unwrap();
                let scoop_identity::ExactTypeKey::NominalApplication { origin, .. } =
                    actual.identity_record().key()
                else {
                    panic!("application exact key")
                };
                assert_eq!(
                    record.origin(),
                    &MirTypeOriginV1::NominalApplication(*origin)
                );
                assert!(actual.nominal_specialization().is_some());
            }
            let record = applications
                .records()
                .iter()
                .find(|record| {
                    matches!(
                        record.representation(),
                        scoop_mir::MirTypeRepresentationV1::Struct { .. }
                    )
                })
                .unwrap();
            let mut conflicting = record.representation().clone();
            let scoop_mir::MirTypeRepresentationV1::Struct {
                interior_mutable, ..
            } = &mut conflicting
            else {
                unreachable!("selected a struct representation")
            };
            *interior_mutable = !*interior_mutable;
            let conflicting = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
                scoop_mir::ParamFreeMirTypeExportV1::try_new(
                    MirTypeBridgeAuthority {
                        identities: graph,
                        foundation: input.foundation(),
                    },
                    record.exact(),
                    record.origin().clone(),
                    record.facts(),
                    conflicting,
                    record.base_and_interfaces().clone(),
                )
                .unwrap(),
            ])
            .unwrap();
            assert!(matches!(
                MirTypeBridgeTypeIndexV1::try_new(&[&applications, &conflicting]),
                Err(scoop_mir::MirTypeBridgeLookupError::DuplicateType { exact })
                    if exact == record.exact()
            ));
            let record = applications
                .records()
                .iter()
                .find(|record| !record.representation().fields().is_empty())
                .unwrap();
            let foreign = sources
                .records()
                .iter()
                .flat_map(|record| record.representation().fields())
                .next()
                .unwrap()
                .field;
            let mut representation = record.representation().clone();
            let fields = match &mut representation {
                scoop_mir::MirTypeRepresentationV1::Struct { fields, .. } => fields,
                scoop_mir::MirTypeRepresentationV1::Class {
                    declared_fields, ..
                } => declared_fields,
                _ => panic!("declared fields"),
            };
            fields[0].field = foreign;
            assert!(matches!(scoop_mir::ParamFreeMirTypeExportV1::try_new(
                MirTypeBridgeAuthority { identities: graph, foundation: input.foundation() },
                record.exact(), record.origin().clone(), record.facts(), representation,
                record.base_and_interfaces().clone(),
            ), Err(scoop_mir::MirTypeBridgeError::FieldOwner { field }) if field == foreign));
        });
    }
}
