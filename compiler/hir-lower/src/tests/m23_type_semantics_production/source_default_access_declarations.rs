use super::source_dispatch::with_hir_source;
use super::source_inventory::identity_closure;
use super::*;
use hir::{
    CanonicalDefaultSourceAccessDeclarationsV1 as Table,
    DecodedCanonicalDefaultSourceAccessDeclarationsV1 as Decoded,
    DefaultSourceAccessDeclarationV1 as Record,
};
use scoop_identity::DefinitionOriginSubject as Subject;
use scoop_wire::{decode_canonical, encode};
mod determinism;
mod rejection;
mod wire;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access-declarations.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/access-declaration-combinations.scoop"
));
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn required(export: &hir::ExportHir) -> BTreeSet<Subject> {
    let constants: BTreeSet<_> = export
        .properties
        .iter()
        .filter_map(|(_, property)| {
            let getter = property.capability.getter();
            (export.property_getters[getter].implementation
                == hir::PropertyAccessorImplementation::Constant)
                .then(|| {
                    Subject::PropertyAccessor(export.property_accessor_identities[getter].id())
                })
        })
        .collect();
    export
        .export_definition_origins
        .records()
        .iter()
        .filter(|r| {
            r.origin().source().cone() == export.cone
                && r.subject().kind_tag() <= 8
                && !constants.contains(&r.subject())
        })
        .map(|r| r.subject())
        .collect()
}
fn table(output: &hir::OrdinaryHirOutput<'_>) -> Table {
    Table::from_export_hir(
        &output.output().export,
        &required(output.output().export.module()),
        &mut meter(),
    )
    .unwrap()
}
fn function(export: &hir::ExportHir, name: &str) -> Subject {
    let (id, _) = export
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap();
    match &export.function_identities[id] {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(r)) => {
            Subject::Function(r.id())
        }
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(r)) => {
            Subject::GenericFunction(r.id())
        }
        _ => panic!("source function required"),
    }
}

#[test]
fn default_access_declarations_preserve_all_roles_visibility_and_actual_origins() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();
            let table = table(output);
            assert_eq!(
                table
                    .records()
                    .iter()
                    .map(Record::subject)
                    .collect::<BTreeSet<_>>(),
                required(export)
            );
            if source == SOURCE {
                assert_eq!(
                    table
                        .records()
                        .iter()
                        .map(|r| r.subject().kind_tag())
                        .collect::<BTreeSet<_>>(),
                    (1..=8).collect()
                );
            }
            for record in table.records() {
                let access = record.declaration_access();
                assert_eq!(
                    access.definition_origin().origin(),
                    export
                        .export_definition_origins
                        .get(record.subject())
                        .unwrap()
                        .origin()
                );
                for owner in access.lexical_owners() {
                    let subject = match owner {
                        hir::SourceNominalId::Concrete(id) => Subject::Type(*id),
                        hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(*id),
                    };
                    assert!(table.get(subject).is_some());
                }
            }
        });
    }
}
#[test]
fn default_access_demand_adds_complete_static_nested_owner_chain() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let leaf = function(export, "Scope.Hidden.leaf");
        let actual = Table::from_export_hir(
            &output.output().export,
            &BTreeSet::from([leaf]),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(actual.records().len(), 3);
        let leaf = actual.get(leaf).unwrap().declaration_access();
        assert_eq!(
            leaf.declared_visibility(),
            hir::DeclaredVisibilityV1::Private
        );
        assert!(matches!(
            leaf.lexical_owners(),
            [
                hir::SourceNominalId::GenericTemplate(_),
                hir::SourceNominalId::Concrete(_)
            ]
        ));
        let all = table(output);
        for record in actual.records() {
            assert_eq!(all.get(record.subject()), Some(record));
        }
    });
}
#[test]
fn default_access_source_dump_locks_visibility_owner_depth_and_location() {
    with_hir_source(SOURCE, |output, _| {
        let mut rows = table(output)
            .records()
            .iter()
            .map(|r| {
                let access = r.declaration_access();
                let start = access.definition_origin().origin().span().start_byte() as usize;
                let line = SOURCE[..start].bytes().filter(|b| *b == b'\n').count() + 1;
                format!(
                    "line={line:02} role={} {:?} owners={}\n",
                    r.subject().kind_tag(),
                    access.declared_visibility(),
                    access.lexical_owners().len()
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        assert_eq!(
            rows.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/access-declarations.snap"
            ))
        );
    });
}
