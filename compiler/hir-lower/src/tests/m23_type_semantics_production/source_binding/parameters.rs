use super::super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalInheritanceSourceParameterProtocolsV1 as Table,
    InheritanceParameterBindingError as Error, InheritanceSourceParameterProtocolV1 as Record,
    ProtectedSourceProtocolSemanticAuthority as _,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

mod contracts;
mod foundation;
mod inventory;
mod replay;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/parameter-protocols.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));

#[derive(Clone)]
struct Sources {
    properties: super::properties::Sources,
    constructors: hir::CanonicalInheritanceSourceConstructorsV1,
    callables: hir::CanonicalInheritanceSourceProtectedCallablesV1,
    protocols: Table,
}
impl Sources {
    fn from_output(output: &hir::OrdinaryHirOutput<'_>, fixture: &mut Fixture) -> Self {
        let properties = super::properties::Sources::from_output(output, fixture);
        macro_rules! restore {
            ($table:ty, $decoded:ty) => {{
                let value = <$table>::from_ordinary_hir(output, &mut meter()).unwrap();
                let bytes = encode(&value).unwrap();
                let decoded: $decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                let restored = decoded
                    .resolve(&mut fixture.identities, &mut meter())
                    .unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        Self {
            properties,
            constructors: restore!(
                hir::CanonicalInheritanceSourceConstructorsV1,
                hir::DecodedCanonicalInheritanceSourceConstructorsV1
            ),
            callables: restore!(
                hir::CanonicalInheritanceSourceProtectedCallablesV1,
                hir::DecodedCanonicalInheritanceSourceProtectedCallablesV1
            ),
            protocols: restore!(
                Table,
                hir::DecodedCanonicalInheritanceSourceParameterProtocolsV1
            ),
        }
    }
    fn bind<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
        core: &hir::ImportedCoreFundamentalTypeProtocol,
        meter: &mut BudgetMeter,
    ) -> Result<hir::BoundInheritanceParameterProtocolsV1<'a>, Error> {
        let protected = self.protected(foundation, core);
        let constructors = foundation
            .bind_inheritance_constructor_sources(
                &self.properties.dispatch.inventory,
                &self.constructors,
                &mut super::meter(),
            )
            .unwrap();
        protected.bind_parameter_protocols(&constructors, &self.protocols, core, meter)
    }
    fn protected<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
        core: &hir::ImportedCoreFundamentalTypeProtocol,
    ) -> hir::BoundInheritanceProtectedCallableSourcesV1<'a, 'f> {
        self.properties
            .bind(foundation, &mut meter())
            .unwrap()
            .bind_protected_callable_sources(&self.callables, core, &mut meter())
            .unwrap()
    }
    fn replace(&mut self, record: Record) {
        let owner = record.owner();
        let mut records = self.protocols.records().to_vec();
        *records.iter_mut().find(|r| r.owner() == owner).unwrap() = record;
        self.protocols = Table::try_new(records, &mut meter()).unwrap();
    }
}

#[test]
fn restored_parameter_protocols_bind_to_constructor_and_protected_sources() {
    for source in [SOURCE, COMBINED] {
        with_source(source, |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let inputs = core
                .foundation
                .import_core_inputs(&core.interface, &[])
                .unwrap();
            let protocol = inputs.protocols().fundamental_types();
            let mut bound = sources.bind(&foundation, protocol, &mut meter()).unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.table(), &sources.protocols);
            assert_eq!(
                bound.canonical_array_type().unwrap(),
                protocol.array().persistent()
            );
            for record in sources.protocols.records() {
                assert_eq!(bound.protocol(record.owner()).unwrap(), record);
                for (position, parameter) in record.parameters().iter().enumerate() {
                    let position = position as u32;
                    assert_eq!(
                        bound
                            .source_parameter_shape(record.owner(), position)
                            .unwrap(),
                        parameter.shape()
                    );
                    assert_eq!(
                        bound
                            .source_parameter_calling_kind(record.owner(), position)
                            .unwrap(),
                        parameter.calling_kind()
                    );
                    bound
                        .validate_source_parameter_origin(
                            record.owner(),
                            position,
                            parameter.definition_origin(),
                        )
                        .unwrap();
                }
                assert!(matches!(
                    bound.parameter(record.owner(), record.parameters().len() as u32),
                    Err(Error::Position { .. })
                ));
            }
        });
    }
}

#[test]
fn parameter_binding_uses_shared_budgets_before_publishing_protocols() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
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
                semantic_recursion: 4,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                sources.bind(
                    &foundation,
                    inputs.protocols().fundamental_types(),
                    &mut BudgetMeter::new(limits)
                ),
                Err(Error::Resource(_))
            ));
        }
    });
}
