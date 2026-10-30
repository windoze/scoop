use super::*;

#[test]
fn target_sysroot_headers_precede_local_headers_and_track_only_selected_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cone");
    let original = manifest(
        &root,
        "#include <value.h>\nint answer(void); int answer(void) { return VALUE; }\n",
    );
    let unselected_os = if cfg!(windows) { "darwin" } else { "windows" };
    std::fs::write(original.manifest_path(), original.source_text().replace("include = [\"include\"]", &format!("include = [{{sysroot = 'backend/include'}}, {{sysroot = 'unused/include', when = {{os = '{unselected_os}'}}}}, 'include']"))).unwrap();
    let manifest = load_cone_manifest(&ManifestRootLocator::cone_directory(&root)).unwrap();
    std::fs::write(root.join("include/value.h"), "#error wrong include order\n").unwrap();
    let sysroot = directory.path().join("sysroot");
    std::fs::create_dir_all(sysroot.join("include")).unwrap();
    let selected_root = crate::target_library_root(&sysroot, target().lir_target());
    let include = selected_root.join("backend/include");
    std::fs::create_dir_all(&include).unwrap();
    std::fs::write(include.join("value.h"), "#define VALUE 41\n").unwrap();
    let cache = directory.path().join("cache");
    let build = || {
        prepare_native_objects(
            &manifest,
            target(),
            OptimizationMode::Release,
            &sysroot,
            &cache,
            scoop_protocol::CompilationPurpose::Production,
        )
        .unwrap()
    };
    let cold = build();
    let time = cold.objects()[0].metadata().unwrap().modified().unwrap();
    assert_eq!(build(), cold);
    std::fs::create_dir_all(selected_root.join("unused/include")).unwrap();
    std::fs::write(
        selected_root.join("unused/include/value.h"),
        "#error unselected header\n",
    )
    .unwrap();
    assert_eq!(build(), cold);
    assert_eq!(
        cold.objects()[0].metadata().unwrap().modified().unwrap(),
        time
    );
    std::fs::write(include.join("value.h"), "#define VALUE 42\n").unwrap();
    let changed = build();
    assert_ne!(cold.key(), changed.key());
    assert_ne!(
        std::fs::read(&cold.objects()[0]).unwrap(),
        std::fs::read(&changed.objects()[0]).unwrap()
    );
    let main = directory.path().join("main.c");
    let binary = directory.path().join("program");
    std::fs::write(
        &main,
        "int answer(void); int main(void) { return answer() != 42; }\n",
    )
    .unwrap();
    let output = target()
        .c_bridge_toolchain()
        .driver_command()
        .arg(main)
        .args(changed.objects())
        .arg("-o")
        .arg(&binary)
        .scoop_output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(binary)
            .scoop_output()
            .unwrap()
            .status
            .success()
    );

    let relocated = directory.path().join("another-sysroot");
    std::fs::create_dir_all(relocated.join("include")).unwrap();
    let relocated_include =
        crate::target_library_root(&relocated, target().lir_target()).join("backend/include");
    std::fs::create_dir_all(&relocated_include).unwrap();
    std::fs::copy(include.join("value.h"), relocated_include.join("value.h")).unwrap();
    let preprocess = |sysroot: &Path| {
        prepare_native_inputs(
            &manifest,
            target().id(),
            target().c_bridge_toolchain(),
            OptimizationMode::Release,
            sysroot,
        )
        .unwrap()
    };
    assert_eq!(preprocess(&sysroot).units(), preprocess(&relocated).units());
}
