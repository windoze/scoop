use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    CanonicalNominalSourceCallablesV1 as Callables,
    CanonicalNominalSourcePropertiesV1 as Properties, NominalMemberBindingError as Error,
};
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, PersistentPropertyId, SignatureTypeKey,
};

mod corruption;
mod inventories;
mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/members-binding.scoop"
));
const CALLABLES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/callables.scoop"
));
const PROPERTIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/properties.scoop"
));
const PROTECTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));
const ABSTRACT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/property-abstract-overrides.scoop"
));

#[test]
fn nominal_member_sources_bind_complete_methods_accessors_variants_and_constants_from_bytes() {
    for input in [SOURCE, CALLABLES, PROPERTIES, PROTECTED, ABSTRACT] {
        with_sources(input, |_, fixture, sources, core| {
            let foundation = fixture.bind().unwrap();
            let nominals = foundation
                .bind_nominal_sources(&sources.nominals, &mut meter())
                .unwrap();
            let bound = nominals
                .bind_member_sources(&sources.properties, &sources.callables, core, &mut meter())
                .unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.properties(), &sources.properties);
            assert_eq!(bound.callables(), &sources.callables);
            for record in sources.callables.records() {
                assert_eq!(bound.callable_source(record.declaration()).unwrap(), record);
                match record.declaration() {
                    CallableTemplateOrigin::VariantConstructor(variant) => {
                        assert!(bound.callable_key(record.declaration()).is_err());
                        assert_eq!(
                            hir::NominalSupportCallableSemanticAuthority::source_enum_variant_key(
                                &bound, variant
                            )
                            .unwrap(),
                            nominals.enum_variant_key(variant).unwrap()
                        );
                    }
                    _ => assert_eq!(
                        bound.callable_key(record.declaration()).unwrap().origin(),
                        bound.provider()
                    ),
                }
            }
            for record in sources.properties.records() {
                assert_eq!(bound.property_source(record.declaration()).unwrap(), record);
                assert_eq!(
                    PersistentPropertyId::from_source_declaration(
                        bound.property_key(record.declaration()).unwrap()
                    )
                    .unwrap(),
                    record.declaration()
                );
                let proof = bound.property_proof(record.declaration()).unwrap();
                match record.payload() {
                    hir::NominalSupportPropertyPayloadV1::Const { .. } => {
                        assert_eq!(proof, hir::NominalMemberPropertyProofV1::Const)
                    }
                    hir::NominalSupportPropertyPayloadV1::Runtime { .. } => {
                        let generic = record
                            .declaration_access()
                            .lexical_owners()
                            .iter()
                            .any(|owner| matches!(owner, hir::SourceNominalId::GenericTemplate(_)));
                        let expected = if generic {
                            hir::NominalSupportPropertyAccessProofV1::GenericSourceMetadata
                        } else {
                            hir::NominalSupportPropertyAccessProofV1::ParamFreeDomains
                        };
                        assert_eq!(proof, hir::NominalMemberPropertyProofV1::Runtime(expected));
                    }
                }
            }
        });
    }
}

#[test]
fn nominal_member_binding_respects_shared_resources_before_publishing_any_sources() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        let nominals = foundation
            .bind_nominal_sources(&sources.nominals, &mut meter())
            .unwrap();
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                nominals
                    .bind_member_sources(
                        &sources.properties,
                        &sources.callables,
                        core,
                        &mut BudgetMeter::new(limits)
                    )
                    .is_err(),
                "{limits:?}"
            );
        }
    });
}
