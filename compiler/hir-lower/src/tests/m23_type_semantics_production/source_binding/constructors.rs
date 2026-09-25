use super::super::source_dispatch::with_source;
use super::*;
use hir::InheritanceConstructorBindingError as Error;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId, SignatureTypeKey};

mod contracts;
mod inventories;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/constructors.scoop"
));
const PROTECTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-construction.scoop"
));

#[derive(Clone)]
struct Sources {
    inventory: hir::CanonicalSourceInheritanceInventoriesV1,
    constructors: hir::CanonicalInheritanceSourceConstructorsV1,
}

impl Sources {
    fn from_output(output: &hir::DependencyHirOutput, fixture: &mut Fixture) -> Self {
        macro_rules! restore {
            ($canonical:ty, $decoded:ty) => {{
                let source = <$canonical>::from_dependency_hir(output).unwrap();
                let bytes = encode(&source).unwrap();
                let decoded: $decoded = decode_canonical(&bytes).unwrap();
                let restored = decoded.resolve(&mut fixture.identities).unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        Self {
            inventory: restore!(
                hir::CanonicalSourceInheritanceInventoriesV1,
                hir::DecodedCanonicalSourceInheritanceInventoriesV1
            ),
            constructors: restore!(
                hir::CanonicalInheritanceSourceConstructorsV1,
                hir::DecodedCanonicalInheritanceSourceConstructorsV1
            ),
        }
    }

    fn bind<'a, 'f>(
        &'a self,
        foundation: &'a hir::BoundTypeFoundationSourcesV1<'f>,
    ) -> Result<hir::BoundInheritanceConstructorSourcesV1<'a, 'f>, Error> {
        foundation.bind_inheritance_constructor_sources(&self.inventory, &self.constructors)
    }

    fn replace(&mut self, record: hir::NominalSupportConstructorInterfaceV1) {
        let declaration = record.declaration();
        let mut records = self.constructors.records().to_vec();
        *records
            .iter_mut()
            .find(|value| value.declaration() == declaration)
            .unwrap() = record;
        self.constructors =
            hir::CanonicalInheritanceSourceConstructorsV1::try_new(records).unwrap();
    }
}

#[test]
fn byte_restored_constructor_sources_bind_to_their_owned_foundation() {
    for source in [SOURCE, PROTECTED] {
        with_source(source, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let bound = sources.bind(&foundation).unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.table(), &sources.constructors);
            for owner in sources.inventory.records() {
                assert_eq!(
                    bound.required_for(owner.owner()).unwrap(),
                    owner.constructors()
                );
                for declaration in owner.constructors().values() {
                    assert_eq!(
                        PersistentConstructorId::from_source_declaration(
                            bound.constructor_key(*declaration).unwrap()
                        )
                        .unwrap(),
                        *declaration
                    );
                    assert_eq!(
                        bound.constructor_source(*declaration).unwrap(),
                        sources.constructors.get(*declaration).unwrap()
                    );
                }
            }
        });
    }
}

#[test]
fn constructor_keys_must_be_owned_even_when_the_identity_graph_contains_them() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_constructors(vec![]).unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities)
            .unwrap();
        assert!(matches!(
            sources.bind(&foundation),
            Err(Error::MissingKey(_))
        ));
    });
}
