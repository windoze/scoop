use super::*;

#[test]
fn default_access_source_bytes_ignore_unrelated_arena_and_source_allocation() {
    let (required, bytes) = with_hir_source(SOURCE, |output, _| {
        (
            required(output.output().export.module()),
            encode(&table(output)).unwrap(),
        )
    });
    super::super::source_dispatch::with_hir_sources(
        &[
            ("src/extra.scoop", "private class Unrelated {}"),
            ("src/main.scoop", SOURCE),
        ],
        |output, _| {
            let actual = Table::from_export_hir(&output.output().export, &required).unwrap();
            assert_eq!(encode(&actual).unwrap(), bytes);
            let mut reversed = actual.records().to_vec();
            reversed.reverse();
            assert_eq!(encode(&Table::try_new(reversed).unwrap()).unwrap(), bytes);
            let empty = Table::from_export_hir(&output.output().export, &BTreeSet::new()).unwrap();
            assert!(empty.records().is_empty());
        },
    );
}
