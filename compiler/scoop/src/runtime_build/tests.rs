use super::*;

mod index;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux;
mod stress;

fn vendor_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/vendor")
}

fn runtime_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime")
}

fn unwind_prefix() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        Some(PathBuf::from(
            std::env::var_os("SCOOP_TEST_UNWIND_MSVC")
                .expect("SCOOP_TEST_UNWIND_MSVC is required for Windows runtime tests"),
        ))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[test]
fn missing_runtime_vendor_reports_the_selected_path() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing vendor");
    let failure = build_runtime(RuntimeBuildRequest {
        enable_gc_stress: false,
        enable_startup_stats: false,
        profile_runtime: None,
        target: &target,
        runtime_root: &runtime_root(),
        vendor_root: &missing,
        cache_root: directory.path(),
        optimization: RuntimeOptimization::Optimized,
        unwind_prefix: unwind_prefix().as_deref(),
    })
    .unwrap_err();
    assert!(
        failure
            .to_string()
            .contains(&format!("vendor source root {}:", missing.display()))
    );
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn actual_runtime_archive_roundtrips_and_reuses_paths() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let build = || {
        build_runtime(RuntimeBuildRequest {
            enable_gc_stress: false,
            enable_startup_stats: false,
            profile_runtime: None,
            target: &target,
            runtime_root: &runtime_root(),
            vendor_root: &vendor_root(),
            cache_root: cache.path(),
            optimization: RuntimeOptimization::None,
            unwind_prefix: unwind_prefix().as_deref(),
        })
        .unwrap()
    };
    let startup = target
        .lir_target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol("scoop_rt_run_program");
    let first = build();
    assert!(!first.cache_hit());
    let modified = first
        .objects()
        .archive_path()
        .metadata()
        .unwrap()
        .modified()
        .unwrap();
    index::check_relocation_and_compatible_toolchains(&first, &target);
    assert_eq!(
        first.objects().objects().len(),
        target.runtime_build().runtime_sources(false).len()
    );
    assert!(first.objects().symbols().definitions.contains_key(&startup));
    let second = build();
    assert!(second.cache_hit());
    assert_eq!(
        first.objects().fingerprint(),
        second.objects().fingerprint()
    );

    assert_eq!(
        first.objects().input_paths(),
        second.objects().input_paths()
    );
    assert_eq!(
        second
            .objects()
            .archive_path()
            .metadata()
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert_eq!(first.objects().input_paths().len(), 2);
    assert_eq!(
        std::fs::read_dir(first.index().parent().unwrap())
            .unwrap()
            .count(),
        2
    );
    assert!(
        std::fs::read_dir(first.index().parent().unwrap())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".objects-"))
    );
}

#[test]
fn runtime_cache_tracks_sources_headers_candidates_and_flags() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let source = workspace.path().join("runtime source");
    let cache = workspace.path().join("cache");
    copy_tree(&runtime_root(), &source);
    let vendor = workspace.path().join("vendor");
    copy_tree(&vendor_root().join("ryu"), &vendor.join("ryu"));
    #[cfg(windows)]
    copy_tree(&vendor_root().join("gdtoa"), &vendor.join("gdtoa"));
    copy_tree(
        &vendor_root().join("libbacktrace"),
        &vendor.join("libbacktrace"),
    );
    std::fs::write(
        source.join("include/m48_candidate.h"),
        "#define M48_VALUE 1\n",
    )
    .unwrap();
    let rt = source.join("src/rt.c");
    let bytes = std::fs::read_to_string(&rt).unwrap();
    std::fs::write(&rt, format!("#include \"m48_candidate.h\"\n{bytes}")).unwrap();
    let build = |optimization| {
        build_runtime(RuntimeBuildRequest {
            enable_gc_stress: false,
            enable_startup_stats: false,
            profile_runtime: None,
            target: &target,
            runtime_root: &source,
            vendor_root: &vendor,
            cache_root: &cache,
            optimization,
            unwind_prefix: unwind_prefix().as_deref(),
        })
        .unwrap()
    };
    let first = build(RuntimeOptimization::None);
    assert!(build(RuntimeOptimization::None).cache_hit());
    for path in [
        source.join("src/rt.c"),
        source.join("include/scoop_rt.h"),
        vendor.join("ryu/ryu/ryu.h"),
    ] {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"\n/* Runtime cache content change. */\n")
            .unwrap();
        let changed = build(RuntimeOptimization::None);
        assert!(!changed.cache_hit());
        assert_ne!(
            first.objects().configuration().input_key,
            changed.objects().configuration().input_key
        );
    }
    std::fs::write(
        source.join("include/unused_candidate.h"),
        "/* New include candidate. */\n",
    )
    .unwrap();
    assert!(build(RuntimeOptimization::None).cache_hit());
    std::fs::write(source.join("src/m48_candidate.h"), "#define M48_VALUE 2\n").unwrap();
    assert!(!build(RuntimeOptimization::None).cache_hit());
    assert!(!build(RuntimeOptimization::Optimized).cache_hit());
    let records = || {
        std::fs::read_dir(first.index().parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| (p.clone(), std::fs::read(p).unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = records();
    std::fs::write(&rt, "invalid C;\n").unwrap();
    let failed = build_runtime(RuntimeBuildRequest {
        enable_gc_stress: false,
        enable_startup_stats: false,
        profile_runtime: None,
        target: &target,
        runtime_root: &source,
        vendor_root: &vendor,
        cache_root: &cache,
        optimization: RuntimeOptimization::None,
        unwind_prefix: unwind_prefix().as_deref(),
    });
    assert!(failed.is_err());
    assert_eq!(records(), before);
    std::fs::remove_file(&rt).unwrap();
    assert!(
        build_runtime(RuntimeBuildRequest {
            enable_gc_stress: false,
            enable_startup_stats: false,
            profile_runtime: None,
            target: &target,
            runtime_root: &source,
            vendor_root: &vendor,
            cache_root: &cache,
            optimization: RuntimeOptimization::None,
            unwind_prefix: unwind_prefix().as_deref(),
        })
        .is_err()
    );
    assert_eq!(records(), before);
}

#[test]
fn runtime_dependencies_track_actual_metadata_and_alias_selection() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("a.h");
    let b = directory.path().join("b.h");
    std::fs::write(&a, "#define VALUE 1\n").unwrap();
    std::fs::write(&b, "#define VALUE 2\n").unwrap();
    let dependencies = dependencies::Dependencies::collect([a.clone(), b.clone()].into()).unwrap();
    assert!(dependencies.is_current().unwrap());
    let bytes = serde_json::to_vec(&dependencies).unwrap();
    let saved: dependencies::Dependencies = serde_json::from_slice(&bytes).unwrap();
    std::fs::write(&a, "#define VALUE 10\n").unwrap();
    assert!(!saved.is_current().unwrap());
    #[cfg(unix)]
    {
        let alias = directory.path().join("selected.h");
        std::os::unix::fs::symlink(&a, &alias).unwrap();
        let selected = dependencies::Dependencies::collect([alias.clone()].into()).unwrap();
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&b, &alias).unwrap();
        assert!(!selected.is_current().unwrap());
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let output = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &output);
        } else {
            std::fs::copy(entry.path(), output).unwrap();
        }
    }
}
