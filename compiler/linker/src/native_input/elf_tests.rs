use super::*;
use scoop_process::CommandExt;
use std::process::Command;

#[test]
fn elf_native_files_and_archives_keep_bytes_and_defer_unselected_initializers() {
    let mut input_ids = std::collections::BTreeSet::new();
    for triple in ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"] {
        let profile = ValidatedFinalLinkProfile::resolve(triple).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let source = path.join("api.c");
        std::fs::write(&source, "int api(int value) { return value + 1; }\n").unwrap();
        let object = path.join("api.o");
        checked(
            &mut profile
                .startup_toolchain()
                .object_compilation_command(&source, &object),
        );
        let direct = locate::read(&object, NativeFileKind::Object, &profile, false).unwrap();
        assert!(input_ids.insert(direct.id));
        let (_, index) = direct.candidate("api").unwrap();
        assert!(index.check_selected(&direct.bytes).unwrap().is_none());
        assert_eq!(
            index.info.definitions["api"].kind,
            crate::NativeSymbolKind::Function
        );

        std::fs::write(&source, "static volatile int initialized;\n__attribute__((constructor)) static void init(void) { initialized = 1; }\nint unused(void) { return 2; }\n").unwrap();
        let unused = path.join("unused.o");
        checked(
            &mut profile
                .startup_toolchain()
                .object_compilation_command(&source, &unused),
        );
        let archive = path.join("libsample.a");
        checked(
            Command::new("ar")
                .arg("crs")
                .arg(&archive)
                .args([&object, &unused]),
        );
        let file = locate::read(&archive, NativeFileKind::Archive, &profile, false).unwrap();
        assert_eq!(file.objects().len(), 2);
        let NativeContent::Archive(members) = &file.content else {
            panic!("expected archive")
        };
        assert!(
            members[0]
                .index
                .check_selected(&file.bytes[members[0].range.clone()])
                .unwrap()
                .is_none()
        );
        assert!(
            members[1]
                .index
                .check_selected(&file.bytes[members[1].range.clone()])
                .err()
                .unwrap()
                .to_string()
                .contains("initialization")
        );
        std::fs::write(&archive, b"replaced").unwrap();
        let retained = object::read::archive::ArchiveFile::parse(file.bytes.as_ref()).unwrap();
        assert_eq!(retained.members().count(), 2);
        assert!(locate::read(&archive, NativeFileKind::Archive, &profile, false).is_err());

        let thin = path.join("libthin.a");
        checked(Command::new("ar").arg("crsT").arg(&thin).arg(&object));
        let failure = match locate::read(&thin, NativeFileKind::Archive, &profile, false) {
            Ok(_) => panic!("thin archive was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(failure.contains("thin"), "{failure}");
    }
}

fn checked(command: &mut Command) {
    let result = command.scoop_output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
