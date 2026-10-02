use super::*;
use object::{Object, ObjectSection, macho, read::macho::MachOFile64};

#[test]
fn native_calls_data_and_code_pointers_run_with_debug_and_optimized_objects() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "final",
        "executable",
        &native_fixture("final/root.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "final",
        &[],
        &[],
        "final/root",
    );
    for flags in [&["-O0"][..], &["-O2", "-g"][..]] {
        compile_native(
            directory.path(),
            "m23_final",
            &native_fixture("final/native.c"),
            flags,
        );
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        let (program, _) = link_native(environment, &root, &[], directory.path());
        assert_eq!(run(&program, false), "42\n");
    }
}

#[test]
fn selected_native_objects_reject_corrupt_relocation_tables_and_unwind_records() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "final",
        "executable",
        &native_fixture("final/root.scoop"),
        &[],
    );
    let path = compile_native(
        directory.path(),
        "m23_final",
        &native_fixture("final/native.c"),
        &[],
    );
    let original = std::fs::read(&path).unwrap();
    let file: MachOFile64<'_> = MachOFile64::parse(original.as_slice()).unwrap();
    let section = file.section_by_name("__text").unwrap();
    let header = section.macho_section() as *const _ as usize - original.as_ptr() as usize;
    let relocations = section.macho_relocations().unwrap();
    assert!(relocations.len() >= 2);
    let relocation = relocations.as_ptr() as usize - original.as_ptr() as usize;
    let fields = u32::from_le_bytes(original[relocation + 4..relocation + 8].try_into().unwrap());
    let unwind = file.section_by_name("__compact_unwind").unwrap();
    let unwind_header = unwind.macho_section() as *const _ as usize - original.as_ptr() as usize;
    let mut cases = Vec::new();
    for (offset, word, expected) in [
        (header + 56, u32::MAX, "relocations offset or number"),
        (relocation, 0x100000, "RelocationSiteOutOfBounds"),
        (relocation, 0x80000000, "ScatteredRelocation"),
        (
            relocation + 4,
            fields | 0x00ffffff,
            "SymbolTargetOutOfBounds",
        ),
        (
            relocation + 4,
            fields | 0xf0000000,
            "UnsupportedRelocationKind",
        ),
        (
            relocation + 4,
            fields ^ (1 << 25),
            "InvalidRelocationFields",
        ),
        (
            relocation + 4,
            (fields & 0x0fffffff) | (u32::from(macho::ARM64_RELOC_ADDEND) << 28),
            "InvalidRelocationPair",
        ),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        cases.push((bytes, expected));
    }
    let mut bytes = original.clone();
    bytes[relocation + 8..relocation + 16].copy_from_slice(&original[relocation..relocation + 8]);
    cases.push((bytes, "DuplicateRelocationOffset"));
    let mut bytes = original.clone();
    bytes[unwind_header + 40..unwind_header + 48]
        .copy_from_slice(&(unwind.size() + 1).to_le_bytes());
    cases.push((bytes, "compact unwind record is truncated"));
    for (bytes, expected) in cases {
        std::fs::write(&path, bytes).unwrap();
        reject_native(
            environment,
            &root,
            &[],
            directory.path(),
            &[directory.path().join("native")],
            &["native object", expected],
        );
    }
}
