use super::super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalInheritanceSourceProtectedCallablesV1 as Table,
    InheritanceProtectedCallableBindingError as Error, ProtectedCallableInterfaceV1 as Record,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

mod contracts;
mod inventory;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));
const DIRECT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-binding.scoop"
));

#[derive(Clone)]
struct Sources {
    properties: super::properties::Sources,
    callables: Table,
}
impl Sources {
    fn from_output(output: &hir::DependencyHirOutput, fixture: &mut Fixture) -> Self {
        let properties = super::properties::Sources::from_output(output, fixture);
        let source = Table::from_dependency_hir(output).unwrap();
        let bytes = encode(&source).unwrap();
        let decoded: hir::DecodedCanonicalInheritanceSourceProtectedCallablesV1 =
            decode_canonical(&bytes).unwrap();
        let callables = decoded.resolve(&mut fixture.identities).unwrap();
        assert_eq!(encode(&callables).unwrap(), bytes);
        Self {
            properties,
            callables,
        }
    }
    fn bind<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
    ) -> Result<hir::BoundInheritanceProtectedCallableSourcesV1<'a, 'f>, Error> {
        self.properties
            .bind(foundation)
            .unwrap()
            .bind_protected_callable_sources(&self.callables)
    }
    fn replace(&mut self, record: Record) {
        let declaration = record.declaration();
        let mut records = self.callables.records().to_vec();
        *records
            .iter_mut()
            .find(|r| r.declaration() == declaration)
            .unwrap() = record;
        self.callables = Table::try_new(records).unwrap();
    }
}

#[test]
fn restored_protected_sources_bind_methods_generics_accessors_and_overrides() {
    for source in [SOURCE, DIRECT] {
        with_source(source, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();

            let bound = sources.bind(&foundation).unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.table(), &sources.callables);
            assert_eq!(
                hir::ProtectedCallableSemanticAuthority::unit_type(&bound).unwrap(),
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id()
            );
            for record in sources.callables.records() {
                assert_eq!(bound.callable_source(record.declaration()).unwrap(), record);
                let key = bound.callable_key(record.declaration()).unwrap();
                assert_eq!(key.origin(), bound.provider());
                match record.declaration() {
                    CallableTemplateOrigin::Function(id) => assert_eq!(
                        scoop_identity::PersistentFunctionId::from_source_declaration(key).unwrap(),
                        id
                    ),
                    CallableTemplateOrigin::GenericFunction(id) => assert_eq!(
                        scoop_identity::PersistentGenericFunctionId::from_source_declaration(key)
                            .unwrap(),
                        id
                    ),
                    CallableTemplateOrigin::Accessor(id) => {
                        let accessor = foundation.accessor_key(id).unwrap();
                        assert_eq!(
                            accessor.owner(),
                            scoop_identity::PropertyOwner::Property(
                                scoop_identity::PersistentPropertyId::from_source_declaration(key)
                                    .unwrap()
                            )
                        );
                    }
                    _ => panic!("protected callable role"),
                }
            }
        });
    }
}
