use super::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux;

fn runtime_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime")
}

#[test]
fn actual_runtime_objects_roundtrip_and_corrupt_cache_rebuilds() {
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
        .map(|object| object.bytes().to_vec())
        .collect();
    reversed.reverse();
    let reordered = RuntimeObjectSet::from_objects(
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
        .map(|object| object.bytes().to_vec())
        .collect();
    assert!(
        RuntimeObjectSet::from_objects(
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
        .map(|object| object.bytes().to_vec())
        .collect();
    duplicate.push(duplicate[0].clone());
    assert!(
        RuntimeObjectSet::from_objects(
            target.lir_target(),
            target.c_bridge_toolchain().profile(),
            read.configuration().clone(),
            duplicate
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate object IDs")
    );

    let object_path = first
        .index()
        .parent()
        .unwrap()
        .join(format!("{}.o", first.objects().objects()[0].id()));
    std::fs::write(&object_path, b"truncated").unwrap();
    assert!(
        RuntimeObjectSet::read_index(
            first.index(),
            target.lir_target(),
            target.c_bridge_toolchain().profile()
        )
        .unwrap_err()
        .to_string()
        .contains("length or digest mismatch")
    );
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
    let request = RuntimeBuildRequest {
        target: &target,
        runtime_root: &runtime_root(),
        cache_root: &cache,
        optimization: RuntimeOptimization::None,
        unwind_prefix: None,
    };
    inputs::Inputs::read(&request)
        .unwrap()
        .materialize(&source)
        .unwrap();
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
    assert!(!build(RuntimeOptimization::None).cache_hit());
    assert!(!build(RuntimeOptimization::Optimized).cache_hit());
}

#[test]
fn runtime_cache_checks_external_header_contents_and_symlink_changes() {
    let directory = tempfile::tempdir().unwrap();
    let roots = ["sdk", "resource", "musl-headers"].map(|name| directory.path().join(name));
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("header.h"), "#define VALUE 1\n").unwrap();
    }
    let dependencies = dependencies::Dependencies::collect(
        &directory.path().join("runtime"),
        vec![Some(format!(
            "runtime.o: {}\n",
            roots
                .iter()
                .map(|root| root.join("header.h").display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        ))],
    );
    assert!(dependencies.is_current());
    let saved = scoop_wire::encode(&dependencies).unwrap();
    let saved: dependencies::Dependencies = scoop_wire::decode_canonical(&saved).unwrap();
    for root in &roots {
        std::fs::write(root.join("header.h"), "#define VALUE 2\n").unwrap();
        assert!(!saved.is_current());
        std::fs::write(root.join("header.h"), "#define VALUE 1\n").unwrap();
    }
    assert!(!dependencies::Dependencies::collect(directory.path(), vec![None]).is_current());
    #[cfg(unix)]
    {
        let alias = directory.path().join("selected.h");
        std::os::unix::fs::symlink(roots[0].join("header.h"), &alias).unwrap();
        let dependencies = dependencies::Dependencies::collect(
            &directory.path().join("runtime"),
            vec![Some(format!("runtime.o: {}\n", alias.display()))],
        );
        std::fs::write(roots[1].join("header.h"), "#define VALUE 3\n").unwrap();
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(roots[1].join("header.h"), alias).unwrap();
        assert!(!dependencies.is_current());
    }
}
