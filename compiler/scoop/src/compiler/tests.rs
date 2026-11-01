// These compiler fixtures are POSIX shell executables.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use scoop_protocol::encode_machine_capability_frame;

use super::*;

fn write_fake_compiler(path: &Path, frame: &[u8]) {
    let escaped = frame
        .iter()
        .map(|byte| format!("\\{:03o}", byte))
        .collect::<String>();
    crate::test_compiler::write(
        path,
        format!("#!/bin/sh\nprintf 'probe\n' >> probes\nprintf '{escaped}'\n").as_bytes(),
    );
}

#[test]
fn resolves_only_the_exact_shared_machine_identity() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    write_fake_compiler(
        &compiler,
        &encode_machine_capability_frame(&expected).unwrap(),
    );
    let locator = PairedScoopcLocator::new(&compiler).unwrap();

    let resolved = ResolvedPairedScoopc::resolve(&locator, &cache(directory.path())).unwrap();

    assert_eq!(resolved.executable_path(), compiler.canonicalize().unwrap());
    assert_eq!(resolved.protocol(), expected.protocol());
    assert_eq!(
        resolved.fingerprint().toolchain_distribution_id(),
        expected.toolchain_distribution_id()
    );
}

#[test]
fn reports_spawn_errors_and_mismatched_tools() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    crate::test_compiler::write(&compiler, b"not executable");
    std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o600)).unwrap();
    let locator = PairedScoopcLocator::new(&compiler).unwrap();
    assert!(matches!(
        ResolvedPairedScoopc::resolve(&locator, &cache(directory.path())),
        Err(PairedCompilerError::Spawn { .. })
    ));

    let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    let mismatched = ScoopcMachineCapabilityV1::new(
        expected.protocol(),
        scoop_wire::sha256(b"another distribution"),
        expected.compiler_build_identity(),
        expected.identity_abi(),
    );
    write_fake_compiler(
        &compiler,
        &encode_machine_capability_frame(&mismatched).unwrap(),
    );
    assert!(matches!(
        ResolvedPairedScoopc::resolve(&locator, &cache(directory.path())),
        Err(PairedCompilerError::CapabilityMismatch { .. })
    ));
}

fn cache(directory: &Path) -> ArtifactCacheRoot {
    ArtifactCacheRoot::new(directory.join("cache")).unwrap()
}

#[test]
fn reuses_capabilities_until_tool_inputs_change() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    write_fake_compiler(
        &compiler,
        &encode_machine_capability_frame(&expected).unwrap(),
    );
    let locator = PairedScoopcLocator::new(&compiler).unwrap();
    let cache = cache(directory.path());
    let first = ResolvedPairedScoopc::resolve(&locator, &cache).unwrap();
    assert_eq!(
        ResolvedPairedScoopc::resolve(&locator, &cache).unwrap(),
        first
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("probes")).unwrap(),
        "probe\n"
    );

    let mut script = std::fs::read(&compiler).unwrap();
    script.extend_from_slice(b"# Updated compiler input.\n");
    crate::test_compiler::write(&compiler, &script);
    let changed = ResolvedPairedScoopc::resolve(&locator, &cache).unwrap();
    assert_ne!(changed.fingerprint(), first.fingerprint());
    assert_eq!(
        ResolvedPairedScoopc::resolve(&locator, &cache).unwrap(),
        changed
    );

    // A timestamp change alone is an input change, even with the same length.
    std::fs::File::open(&compiler)
        .unwrap()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
        .unwrap();
    let retimed = ResolvedPairedScoopc::resolve(&locator, &cache).unwrap();
    assert_ne!(retimed.fingerprint(), changed.fingerprint());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("probes")).unwrap(),
        "probe\nprobe\nprobe\n"
    );
}

#[test]
fn concurrent_requests_share_one_capability_probe() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    write_fake_compiler(
        &compiler,
        &encode_machine_capability_frame(&expected).unwrap(),
    );
    let locator = PairedScoopcLocator::new(&compiler).unwrap();
    let cache = cache(directory.path());
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    ResolvedPairedScoopc::resolve(&locator, &cache).unwrap()
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert!(results.windows(2).all(|pair| pair[0] == pair[1]));
    });
    assert_eq!(
        std::fs::read_to_string(directory.path().join("probes")).unwrap(),
        "probe\n"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn tool_records_support_non_utf8_paths() {
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir().unwrap();
    let compiler = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"scoopc-\xff".to_vec()));
    let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    write_fake_compiler(
        &compiler,
        &encode_machine_capability_frame(&expected).unwrap(),
    );
    let locator = PairedScoopcLocator::new(&compiler).unwrap();
    let cache = cache(directory.path());
    let first = ResolvedPairedScoopc::resolve(&locator, &cache).unwrap();
    assert_eq!(
        ResolvedPairedScoopc::resolve(&locator, &cache).unwrap(),
        first
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("probes")).unwrap(),
        "probe\n"
    );
}
