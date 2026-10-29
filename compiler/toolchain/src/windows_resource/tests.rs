use super::*;
use object::{Object, ObjectSection};

#[test]
fn windows_resources_compile_real_coff_and_follow_headers_and_assets() {
    let directory = tempfile::Builder::new()
        .prefix("scoop RC 资源 ")
        .tempdir()
        .unwrap();
    let clang = PathBuf::from(std::env::var_os("SCOOP_TEST_CLANG").unwrap());
    for (index, (selection, compiler)) in [
        (scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1, None),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1,
            Some("SCOOP_TEST_MINGW_MSVCRT"),
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1,
            Some("SCOOP_TEST_MINGW_UCRT"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let prefix = compiler.map(|name| {
            PathBuf::from(std::env::var_os(name).unwrap())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_owned()
        });
        let invocation =
            crate::resolve_windows_c_toolchain(selection.target(), &clang, prefix.as_deref())
                .unwrap();
        let target = ResolvedTargetProfile::from_c_toolchain(
            selection,
            invocation,
            Digest256::from_array([index as u8; 32]),
        )
        .unwrap();
        let root = directory.path().join(format!("cone-{index}"));
        let cache = directory.path().join(format!("cache-{index}"));
        std::fs::create_dir_all(root.join("resources")).unwrap();
        std::fs::create_dir_all(root.join("include")).unwrap();
        std::fs::write(root.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"test\"\nname = \"resources\"\nversion = \"0.1.0\"\nkind = \"executable\"\n[windows]\nrc = \"resources/app.rc\"\n[native]\ninclude = [\"include\"]\n").unwrap();
        let script = root.join("resources/app.rc");
        let header = root.join("include/values.h");
        let payload = root.join("resources/数据.bin");
        std::fs::write(
            &script,
            "#include <winres.h>\n#include <values.h>\nRESOURCE_ID RCDATA \"数据.bin\"\n",
        )
        .unwrap();
        std::fs::write(&header, "#define RESOURCE_ID 73\n").unwrap();
        std::fs::write(&payload, b"first-payload").unwrap();
        let manifest =
            crate::load_cone_manifest(&scoop_manifest::ManifestRootLocator::cone_directory(&root))
                .unwrap();
        let build = || {
            prepare(&manifest, &target, CompilationPurpose::Production, &cache)
                .unwrap()
                .unwrap()
        };
        let cold = build();
        assert_payload(&cold.object, b"first-payload");
        let time = cold.object.metadata().unwrap().modified().unwrap();
        let warm = build();
        assert_eq!((cold.key, &cold.object), (warm.key, &warm.object));
        assert_eq!(warm.object.metadata().unwrap().modified().unwrap(), time);
        std::fs::write(&header, "#define RESOURCE_ID 74\n").unwrap();
        let header_changed = build();
        assert_ne!(cold.key, header_changed.key);
        assert_ne!(
            std::fs::read(&cold.object).unwrap(),
            std::fs::read(&header_changed.object).unwrap()
        );
        std::fs::write(&payload, b"second-payload").unwrap();
        let changed = build();
        assert_ne!(changed.key, header_changed.key);
        assert_payload(&changed.object, b"second-payload");
        // A newly created earlier search candidate changes the selected asset.
        std::fs::write(root.join("数据.bin"), b"root-payload").unwrap();
        let shadowed = build();
        assert_payload(&shadowed.object, b"root-payload");
        assert_ne!(shadowed.key, changed.key);
        assert_eq!(build().key, shadowed.key);
        std::fs::write(&script, "invalid resource syntax !\n").unwrap();
        let failure_cache = directory.path().join(format!("failure-{index}"));
        assert!(
            prepare(
                &manifest,
                &target,
                CompilationPurpose::Production,
                &failure_cache
            )
            .is_err()
        );
        assert!(
            std::fs::read_dir(failure_cache.join("native-resources-v1"))
                .unwrap()
                .all(|entry| entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "json"))
        );
        assert!(
            prepare(&manifest, &target, CompilationPurpose::Test, &failure_cache)
                .unwrap()
                .is_none()
        );
    }
}

fn assert_payload(path: &Path, expected: &[u8]) {
    let bytes = std::fs::read(path).unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    assert_eq!(file.architecture(), object::Architecture::X86_64);
    assert!(file.section_by_name(".rsrc$01").is_some());
    let payload = file.section_by_name(".rsrc$02").unwrap().data().unwrap();
    assert!(
        payload
            .windows(expected.len())
            .any(|value| value == expected)
    );
}
