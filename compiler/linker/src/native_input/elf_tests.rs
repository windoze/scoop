use super::*;
use scoop_process::CommandExt;
use std::process::Command;

#[test]
fn elf_native_files_and_archives_index_member_requirements() {
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
        assert_eq!(
            index.definitions["api"].kind,
            crate::NativeSymbolKind::Function
        );

        std::fs::write(
            &source,
            "extern void __cxa_throw(void);\nint unused(void) { __cxa_throw(); return 2; }\n",
        )
        .unwrap();
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
                .object_index()
                .unwrap()
                .definitions
                .contains_key("api")
        );
        assert!(
            members[1]
                .object_index()
                .unwrap()
                .requirements
                .contains("__cxa_throw")
        );
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
