use super::*;

#[test]
fn musl_runtime_captures_selected_unwind_headers_without_needing_the_archive() {
    let target = ResolvedTargetProfile::resolve("x86_64-linux-musl").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let prefix = directory.path().join("unwind");
    let include = prefix.join("include");
    std::fs::create_dir_all(&include).unwrap();
    let installed = scoop_toolchain::runtime_unwind_include(target.id(), None)
        .unwrap()
        .unwrap();
    copy_headers(&installed, &include);
    assert!(!prefix.join("lib/libunwind.a").exists());
    let cache = directory.path().join("cache");
    let build = || {
        build_runtime(RuntimeBuildRequest {
            target: &target,
            runtime_root: &runtime_root(),
            cache_root: &cache,
            optimization: RuntimeOptimization::Optimized,
            unwind_prefix: Some(&prefix),
        })
        .unwrap()
    };
    let first = build();
    assert!(!first.cache_hit());
    assert!(build().cache_hit());
    assert!(first.input_paths().contains(&include.join("unwind.h")));
    assert_eq!(
        first.objects().target(),
        scoop_lir::LirTargetProfile::LINUX_X86_64_MUSL
    );
    let header = include.join("unwind.h");
    let mut changed = std::fs::read(&header).unwrap();
    changed.extend_from_slice(b"\n/* Selected unwind headers changed. */\n");
    std::fs::write(header, changed).unwrap();
    let second = build();
    assert!(!second.cache_hit());
    assert_ne!(
        first.objects().configuration().input_key,
        second.objects().configuration().input_key
    );
}

fn copy_headers(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let output = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_headers(&entry.path(), &output);
        } else {
            std::fs::copy(entry.path(), output).unwrap();
        }
    }
}
