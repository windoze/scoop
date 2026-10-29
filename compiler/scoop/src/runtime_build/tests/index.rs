use super::*;
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgePlatformContractV1, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, GccCompilerIdentityV1,
};

pub(super) fn check_relocation_and_compatible_toolchains(
    build: &RuntimeBuildOutput,
    target: &ResolvedTargetProfile,
) {
    let relocated = tempfile::tempdir().unwrap();
    let directory = relocated.path().join("installed runtime 空格");
    copy_tree(build.index().parent().unwrap(), &directory);
    let index = directory.join("index.cbor");
    let consumer = compatible_consumer(target);
    assert_ne!(consumer, *target.c_bridge_toolchain().profile());
    let read = RuntimeObjectSet::read_index(&index, target.lir_target(), &consumer).unwrap();
    assert_eq!(read.fingerprint(), build.objects().fingerprint());
    for (original, moved) in build.objects().objects().iter().zip(read.objects()) {
        assert!(moved.path().starts_with(&directory));
        assert_eq!(
            std::fs::read(original.path()).unwrap(),
            std::fs::read(moved.path()).unwrap()
        );
    }
    let wrong_target = if target.lir_target() == scoop_target_data::LINUX_X86_64_GNU {
        scoop_target_data::LINUX_X86_64_MUSL
    } else {
        scoop_target_data::LINUX_X86_64_GNU
    };
    assert!(
        RuntimeObjectSet::read_index(&index, wrong_target, &consumer)
            .unwrap_err()
            .to_string()
            .contains("target")
    );

    if let CBridgePlatformContractV1::Darwin {
        deployment,
        compiler,
    } = consumer.contract().platform()
    {
        let older = DarwinPackedVersionV1::new(deployment.minimum_os().packed() - 1).unwrap();
        let deployment =
            DarwinCBridgeDeploymentContractV1::new(older, deployment.sdk(), Vec::new()).unwrap();
        let older =
            scoop_target_data::darwin_c_bridge_profile(deployment, compiler.clone()).unwrap();
        let message = RuntimeObjectSet::read_index(&index, target.lir_target(), &older)
            .unwrap_err()
            .to_string();
        assert!(message.contains("runtime requires macOS"), "{message}");
    }
}

fn compatible_consumer(target: &ResolvedTargetProfile) -> CBridgeToolchainProfileV1 {
    match target.c_bridge_toolchain().profile().contract().platform() {
        scoop_lir::CBridgePlatformContractV1::Windows { compiler } => {
            scoop_target_data::windows_c_bridge_profile(
                target.lir_target(),
                scoop_lir::WindowsClangCompilerIdentityV1::new(
                    compiler.version(),
                    scoop_wire::sha256(b"different compiler"),
                ),
            )
            .unwrap()
        }
        CBridgePlatformContractV1::Darwin {
            deployment,
            compiler,
        } => {
            let deployment = DarwinCBridgeDeploymentContractV1::new(
                deployment.minimum_os(),
                DarwinPackedVersionV1::new(deployment.sdk().packed() + 1).unwrap(),
                Vec::new(),
            )
            .unwrap();
            let compiler = AppleClangCompilerIdentityV1::new(
                compiler.version_major() + 1,
                0,
                0,
                "different-compatible-compiler",
            )
            .unwrap();
            scoop_target_data::darwin_c_bridge_profile(deployment, compiler).unwrap()
        }
        CBridgePlatformContractV1::Linux { .. } => scoop_target_data::linux_c_bridge_profile(
            target.lir_target(),
            GccCompilerIdentityV1::new("99.1.0", scoop_wire::sha256(b"different compiler inputs"))
                .unwrap(),
        )
        .unwrap(),
    }
}
