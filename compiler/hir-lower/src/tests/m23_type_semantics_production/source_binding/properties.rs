use super::super::source_dispatch::{INTERFACES, VIRTUAL, with_hir_source as with_source};
use super::*;
use hir::{InheritancePropertyBindingError as Error, NominalSupportPropertyInterfaceV1 as Record};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SignatureTypeKey};

mod abstract_overrides;
mod contracts;
mod inventory;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
));
const DIRECT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/property-direct.scoop"
));

const ABSTRACT_OVERRIDES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/property-abstract-overrides.scoop"
));

const OBJECT_OVERRIDES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/property-object-overrides.scoop"
));

#[derive(Clone)]
struct Sources {
    dispatch: super::dispatch_binding::Sources,
    properties: hir::CanonicalInheritanceSourcePropertiesV1,
}
impl Sources {
    fn from_output(output: &hir::OrdinaryHirOutput<'_>, fixture: &mut Fixture) -> Self {
        let dispatch = super::dispatch_binding::Sources::from_output(output, fixture);
        let source =
            hir::CanonicalInheritanceSourcePropertiesV1::from_ordinary_hir(output, &mut meter())
                .unwrap();
        let bytes = encode(&source).unwrap();
        let decoded: hir::DecodedCanonicalInheritanceSourcePropertiesV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let properties = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(encode(&properties).unwrap(), bytes);
        Self {
            dispatch,
            properties,
        }
    }
    fn bind<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
        meter: &mut BudgetMeter,
    ) -> Result<hir::BoundInheritancePropertySourcesV1<'a, 'f>, Error> {
        self.dispatch
            .bind(foundation, &mut super::meter())
            .unwrap()
            .bind_property_sources(&self.properties, meter)
    }
    fn replace(&mut self, record: Record) {
        let declaration = record.declaration();
        let mut records = self.properties.records().to_vec();
        *records
            .iter_mut()
            .find(|r| r.declaration() == declaration)
            .unwrap() = record;
        self.properties =
            hir::CanonicalInheritanceSourcePropertiesV1::try_new(records, &mut meter()).unwrap();
    }
}

fn payload(record: &Record) -> &hir::NominalSourcePropertyPayloadV1 {
    let hir::NominalSupportPropertyPayloadV1::Runtime { interface } = record.payload() else {
        panic!("runtime source property");
    };
    interface
}
fn replace_payload(record: &Record, payload: hir::NominalSourcePropertyPayloadV1) -> Record {
    Record::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::NominalSupportPropertyPayloadV1::Runtime { interface: payload },
    )
    .unwrap()
}

#[test]
fn byte_restored_property_sources_bind_to_owned_identity_and_accessor_roles() {
    for source in [
        SOURCE,
        DIRECT,
        VIRTUAL,
        INTERFACES,
        ABSTRACT_OVERRIDES,
        OBJECT_OVERRIDES,
    ] {
        with_source(source, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let bound = sources.bind(&foundation, &mut meter()).unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.table(), &sources.properties);
            for record in sources.properties.records() {
                let id = record.declaration();
                assert_eq!(
                    PersistentPropertyId::from_source_declaration(bound.property_key(id).unwrap())
                        .unwrap(),
                    id
                );
                assert_eq!(bound.property_source(id).unwrap(), record);
                let shape = bound.property_shape(id).unwrap();
                assert_eq!(shape.getter, payload(record).getter());
                assert_eq!(shape.representation, payload(record).representation());
            }
        });
    }
}

#[test]
fn property_keys_must_be_owned_even_when_the_shared_graph_resolves_them() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_properties(vec![]).unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
            .unwrap();
        assert!(matches!(
            sources.bind(&foundation, &mut meter()),
            Err(Error::MissingKey(_))
        ));
    });
}

#[test]
fn property_binding_rejects_resource_exhaustion_before_publishing_sources() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
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
        ] {
            assert!(matches!(
                sources.bind(&foundation, &mut BudgetMeter::new(limits)),
                Err(Error::Resource(_))
            ));
        }
    });
}
