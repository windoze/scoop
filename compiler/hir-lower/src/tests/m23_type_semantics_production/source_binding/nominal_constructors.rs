use super::super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalNominalSourceConstructorsV1 as Table, NominalConstructorBindingError as Error,
    NominalSupportConstructorInterfaceV1 as Record,
};
use scoop_identity::{
    CallableTemplateOrigin, DeclarationName, PersistentConstructorId, SignatureTypeKey,
};

mod contracts;
mod inventories;
mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/constructors.scoop"
));
const PARAMETERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
));

#[test]
fn complete_constructor_sources_bind_from_bytes_with_generic_and_restricted_owners() {
    for input in [SOURCE, PARAMETERS] {
        with_source(input, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
            let bound = nominals
                .bind_constructor_sources(&sources.constructors)
                .unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert_eq!(bound.table(), &sources.constructors);
            for record in sources.constructors.records() {
                assert_eq!(
                    bound.constructor_source(record.declaration()).unwrap(),
                    record
                );
                assert_eq!(
                    PersistentConstructorId::from_source_declaration(
                        bound.constructor_key(record.declaration()).unwrap()
                    )
                    .unwrap(),
                    record.declaration()
                );
                assert!(
                    nominals
                        .nominal_source(record.payload().owner())
                        .unwrap()
                        .constructors()
                        .values()
                        .contains(&record.declaration())
                );
            }
        });
    }
}
