use super::*;

#[test]
fn map_requires_exact_objects_unique_indices_and_known_symbol_owners() {
    let objects = [
        PathBuf::from("/private/one.o"),
        PathBuf::from("/private/two.o"),
    ];
    let stubs = BTreeMap::from([(
        PathBuf::from("/private/system.tbd"),
        "/usr/lib/libSystem.B.dylib".into(),
    )]);
    let original = "# Object files:\n[ 0] linker synthesized\n[ 1] /private/one.o\n[ 2] /private/two.o\n[ 3] /private/system.tbd\n# Sections:\n# Symbols:\n0x1000 0x10 [ 1] _one\n0x1010 0x20 [ 2] _two\n";
    check(original, &objects, &stubs, &BTreeMap::new()).unwrap();
    for (text, expected) in [
        (
            original.replace("[ 2] /private/two.o\n", ""),
            "unknown object index",
        ),
        (
            original.replace("[ 2] /private/two.o", "[ 1] /private/two.o"),
            "repeats object index",
        ),
        (
            original.replace("/private/two.o", "/private/extra.o"),
            "unexpected or duplicate input",
        ),
        (
            original.replace("[ 2] /private/two.o", "[ 2] /private/one.o"),
            "repeats input",
        ),
        (
            original.replace("[ 2] _two", "[ 9] _two"),
            "unknown object index",
        ),
        (
            original.replace("0x1010 0x20", "0xffffffffffffffff 0x20"),
            "invalid link map symbol range",
        ),
        (
            original.replace(
                "# Sections:",
                "[ 4] /usr/lib/libSystem.B.dylib\n# Sections:",
            ),
            "repeats input",
        ),
    ] {
        let message = check(&text, &objects, &stubs, &BTreeMap::new())
            .err()
            .unwrap()
            .to_string();
        assert!(message.contains(expected), "{message}");
    }
    assert!(
        check(
            "# Object files:\n[ 1] /private/one.o\n",
            &objects,
            &stubs,
            &BTreeMap::new()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("omitted 1 explicit objects")
    );
}

#[test]
fn trace_rejects_extra_repeated_and_missing_inputs() {
    let objects = [PathBuf::from("/private/object.o")];
    let stubs = BTreeMap::from([(PathBuf::from("/private/system.tbd"), "system".into())]);
    trace("/private/object.o\n/private/system.tbd\n", &objects, &stubs).unwrap();
    for text in [
        "/private/object.o\n",
        "/private/object.o\n/private/system.tbd\n/extra.o\n",
        "/private/object.o\n/private/system.tbd\n/private/object.o\n",
    ] {
        assert!(trace(text, &objects, &stubs).is_err());
    }
}

#[test]
fn retained_cone_symbols_keep_their_actual_input_member_and_address() {
    use scoop_slib::{MemberStableKey, SlibMemberRecord, SlibMemberRole};

    let objects = [
        PathBuf::from("/private/left.o"),
        PathBuf::from("/private/right.o"),
    ];
    let cone = ConeIdentity::CORE;
    let member = SlibMemberRecord::new(
        cone,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        &[],
    )
    .unwrap()
    .id();
    let origins = BTreeMap::from([(objects[1].clone(), ObjectOrigin::Cone { cone, member })]);
    let text = "# Object files:\n[ 1] /private/left.o\n[ 2] /private/right.o\n# Symbols:\n0x1000 0x0 [ 2] _shared\n# Dead Stripped Symbols:\n0x0 0x10 [ 1] _shared\n";
    let map = check(text, &objects, &BTreeMap::new(), &origins).unwrap();
    let [row] = map.cones["_shared"].as_slice() else {
        panic!("only the retained symbol belongs to the final map");
    };
    assert_eq!((row.cone, row.member, row.address), (cone, member, 0x1000));
}
