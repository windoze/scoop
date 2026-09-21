use super::super::source_default_access_declarations::{function, required};
use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    CanonicalDefaultSourceAccessDeclarationsV1 as Table, DefaultSourceAccessBindingError as Error,
    DefaultSourceAccessDeclarationV1 as Record,
};
use scoop_identity::{DefinitionOriginSubject as Subject, SourceDeclarationKey};
mod corruption;
mod foundation;
mod inventory;
mod keys;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access-binding.scoop"
));
const OUTSIDE_ROOTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access-declarations.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access-declaration-combinations.scoop"
));

fn with_sources(
    source: &str,
    run: impl FnOnce(&hir::OrdinaryHirOutput<'_>, &Fixture, &BTreeSet<Subject>, &Table),
) {
    with_hir_source(source, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let required = required(output.output().export.module());
        let table =
            Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        let bytes = encode(&table).unwrap();
        let decoded: hir::DecodedCanonicalDefaultSourceAccessDeclarationsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let table = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(encode(&table).unwrap(), bytes);
        run(output, &fixture, &required, &table);
    });
}
fn nominal_subject(owner: hir::SourceNominalId) -> Subject {
    match owner {
        hir::SourceNominalId::Concrete(id) => Subject::Type(id),
        hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
    }
}
fn replacing(table: &Table, record: Record) -> Table {
    Table::try_new(
        table
            .records()
            .iter()
            .map(|r| {
                if r.subject() == record.subject() {
                    record.clone()
                } else {
                    r.clone()
                }
            })
            .collect(),
        &mut meter(),
    )
    .unwrap()
}
fn access_record(
    record: &Record,
    visibility: hir::DeclaredVisibilityV1,
    owners: Vec<hir::SourceNominalId>,
    origin: hir::ExportDefinitionSourceV1,
) -> Record {
    Record::try_new(
        record.subject(),
        hir::DeclarationAccessSourceV1::try_new(visibility, owners, origin).unwrap(),
    )
    .unwrap()
}

#[test]
fn default_access_binding_restores_all_declaration_roles_and_outside_root_sources() {
    for source in [SOURCE, OUTSIDE_ROOTS, COMBINATIONS] {
        with_sources(source, |_, fixture, required, table| {
            let foundation = fixture.bind().unwrap();
            let bound = foundation
                .bind_default_access_declarations(table, required, &mut meter())
                .unwrap();
            assert_eq!(bound.provider(), fixture.source.entries().provider);
            assert!(std::ptr::eq(bound.table(), table));
            for record in table.records() {
                assert_eq!(
                    bound.declaration(record.subject(), &mut meter()).unwrap(),
                    record
                );
                let key = bound.source_key(record.subject(), &mut meter()).unwrap();
                assert_eq!(key.origin(), bound.provider());
                let expected = expected_key(&fixture.identities, record.subject());
                assert_eq!(key, expected.as_ref());
            }
            if source == OUTSIDE_ROOTS {
                assert!(fixture.source.entries().sources.records().is_empty());
                assert!(
                    table
                        .records()
                        .iter()
                        .any(|r| matches!(r.subject(), Subject::Type(_)))
                );
            }
            let empty = Table::try_new(vec![], &mut meter()).unwrap();
            assert!(
                foundation
                    .bind_default_access_declarations(&empty, &BTreeSet::new(), &mut meter())
                    .unwrap()
                    .table()
                    .records()
                    .is_empty()
            );
        });
    }
}
fn expected_key(
    ids: &ValidatedIdentityGraph,
    subject: Subject,
) -> std::sync::Arc<SourceDeclarationKey> {
    use scoop_identity::*;
    match subject {
        Subject::Type(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::GenericType(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::Function(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::GenericFunction(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::Constructor(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::Property(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::ExtensionProperty(id) => ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
        Subject::PropertyAccessor(id) => {
            let accessor = ids.canonical_key::<_, PropertyAccessorKey>(id).unwrap();
            match accessor.owner() {
                PropertyOwner::Property(id) => {
                    ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap()
                }
                PropertyOwner::ExtensionProperty(id) => {
                    ids.canonical_key::<_, SourceDeclarationKey>(id).unwrap()
                }
            }
        }
        other => panic!("unexpected source subject {other:?}"),
    }
}
