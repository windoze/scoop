use super::*;

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
        })
        .unwrap()
    };
    let first = build();
    assert!(!first.cache_hit());
    assert_eq!(
        first.objects().objects().len(),
        target.runtime_build().runtime_sources().len()
    );
    assert!(
        first
            .objects()
            .symbols()
            .definitions
            .contains_key("_scoop_rt_run_program")
    );
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
        .filter(|object| {
            !object
                .info()
                .definitions
                .contains_key("_scoop_rt_run_program")
        })
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
fn runtime_cache_checks_sdk_and_compiler_resource_header_contents() {
    let target = ResolvedTargetProfile::resolve_host().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut inputs = inputs::Inputs::read(&RuntimeBuildRequest {
        target: &target,
        runtime_root: &runtime_root(),
        cache_root: directory.path(),
        optimization: RuntimeOptimization::None,
    })
    .unwrap();
    inputs.sdk = directory.path().join("sdk");
    inputs.resource = Some(directory.path().join("resource"));
    for root in [&inputs.sdk, inputs.resource.as_ref().unwrap()] {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("header.h"), "#define VALUE 1\n").unwrap();
    }
    inputs.sdk = std::fs::canonicalize(&inputs.sdk).unwrap();
    inputs.resource = inputs
        .resource
        .map(|path| std::fs::canonicalize(path).unwrap());
    let dependencies = dependencies::Dependencies::collect(
        &inputs,
        &directory.path().join("runtime"),
        vec![Some(format!(
            "runtime.o: {}/header.h {}/header.h\n",
            inputs.sdk.display(),
            inputs.resource.as_ref().unwrap().display()
        ))],
    );
    assert!(dependencies.is_current(&inputs));
    let saved = scoop_wire::encode(&dependencies).unwrap();
    let saved: dependencies::Dependencies = scoop_wire::decode_canonical(&saved).unwrap();
    for root in [&inputs.sdk, inputs.resource.as_ref().unwrap()] {
        std::fs::write(root.join("header.h"), "#define VALUE 2\n").unwrap();
        assert!(!saved.is_current(&inputs));
        std::fs::write(root.join("header.h"), "#define VALUE 1\n").unwrap();
    }
    assert!(
        !dependencies::Dependencies::collect(&inputs, directory.path(), vec![None])
            .is_current(&inputs)
    );
}
