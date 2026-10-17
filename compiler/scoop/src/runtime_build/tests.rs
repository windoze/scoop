use super::*;

mod index;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux;

fn runtime_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime")
}

#[test]
fn actual_runtime_objects_roundtrip_and_reuse_paths() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let build = || {
        build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &runtime_root(),
            cache_root: cache.path(),
            optimization: RuntimeOptimization::None,
            unwind_prefix: None,
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
    index::check_relocation_and_compatible_toolchains(&first, &target);
    assert_eq!(
        first.objects().objects().len(),
        target.runtime_build().runtime_sources().len()
    );
    assert!(first.objects().symbols().definitions.contains_key(&startup));
    let second = build();
    assert!(second.cache_hit());
    assert_eq!(
        first.objects().fingerprint(),
        second.objects().fingerprint()
    );

    let moved = tempfile::tempdir().unwrap();
    let index = first.objects().write_index(moved.path()).unwrap();
    let read = RuntimeObjectSet::read_index(
        &index,
        target.lir_target(),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    assert_eq!(read.fingerprint(), first.objects().fingerprint());
    let mut reversed: Vec<_> = read
        .objects()
        .iter()
        .map(|object| object.path().to_owned())
        .collect();
    reversed.reverse();
    let reordered = RuntimeObjectSet::from_paths(
        target.lir_target(),
        target.c_bridge_toolchain().profile(),
        read.configuration().clone(),
        reversed,
    )
    .unwrap();
    assert_eq!(read.fingerprint(), reordered.fingerprint());

    let missing_entry = read
        .objects()
        .iter()
        .filter(|object| !object.info().definitions.contains_key(&startup))
        .map(|object| object.path().to_owned())
        .collect();
    assert!(
        RuntimeObjectSet::from_paths(
            target.lir_target(),
            target.c_bridge_toolchain().profile(),
            read.configuration().clone(),
            missing_entry
        )
        .unwrap_err()
        .to_string()
        .contains("startup entry")
    );
    let mut duplicate: Vec<_> = read
        .objects()
        .iter()
        .map(|object| object.path().to_owned())
        .collect();
    duplicate.push(duplicate[0].clone());
    assert!(
        RuntimeObjectSet::from_paths(
            target.lir_target(),
            target.c_bridge_toolchain().profile(),
            read.configuration().clone(),
            duplicate
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate object IDs")
    );

    assert_eq!(
        first
            .objects()
            .objects()
            .iter()
            .map(|o| o.path())
            .collect::<Vec<_>>(),
        second
            .objects()
            .objects()
            .iter()
            .map(|o| o.path())
            .collect::<Vec<_>>()
    );
    std::fs::remove_file(first.objects().objects()[0].path()).unwrap();
    let rebuilt = build();
    assert!(!rebuilt.cache_hit());
    assert_eq!(
        first.objects().fingerprint(),
        rebuilt.objects().fingerprint()
    );
}

#[test]
fn runtime_cache_tracks_sources_headers_candidates_and_flags() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let source = workspace.path().join("runtime source");
    let cache = workspace.path().join("cache");
    copy_tree(&runtime_root(), &source);
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
            target: &target,
            runtime_root: &source,
            cache_root: &cache,
            optimization,
            unwind_prefix: None,
        })
        .unwrap()
    };
    let first = build(RuntimeOptimization::None);
    assert!(build(RuntimeOptimization::None).cache_hit());
    for relative in ["src/rt.c", "include/scoop_rt.h"] {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(source.join(relative))
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
        std::fs::read_dir(cache.join("runtime-v4"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| (p.clone(), std::fs::read(p).unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = records();
    std::fs::write(&rt, "invalid C;\n").unwrap();
    let failed = build_runtime(RuntimeBuildRequest {
        target: &target,
        runtime_root: &source,
        cache_root: &cache,
        optimization: RuntimeOptimization::None,
        unwind_prefix: None,
    });
    assert!(failed.is_err());
    assert_eq!(records(), before);
    std::fs::remove_file(&rt).unwrap();
    assert!(
        build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &source,
            cache_root: &cache,
            optimization: RuntimeOptimization::None,
            unwind_prefix: None,
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
