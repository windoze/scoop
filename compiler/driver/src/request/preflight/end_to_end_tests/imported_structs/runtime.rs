use super::*;
use scoop_identity::{
    ScoopAbiArgument, ScoopAbiReturn, SourceDeclarationKey, StrongCallableDefinitionOwner,
};

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess; 4],
    directory: &Path,
) {
    let bytes = artifacts.map(|artifact| std::fs::read(artifact.artifact().path()).unwrap());
    let identities = artifacts.map(|artifact| {
        artifact
            .artifact()
            .summary()
            .coordinate()
            .identity()
            .unwrap()
    });
    let mut direct = identities[..3].to_vec();
    direct.sort_unstable();
    let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            identities[3],
            target.lir_target_selection(),
            direct,
            bytes[..3].iter().map(Vec::as_slice).collect(),
            &bytes[3],
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    std::fs::create_dir_all(directory).unwrap();
    let mut objects = Vec::new();
    let mut symbols = Vec::new();
    let mut expected_abis = Vec::new();
    for (index, names) in [
        ["passToken", "passWide"],
        ["keepToken", "keepWide"],
        ["token", "wide"],
    ]
    .into_iter()
    .enumerate()
    {
        let (sections, link) = closure.artifact(identities[index + 1]).unwrap();
        for (kind, name) in names.into_iter().enumerate() {
            let export = sections.lir_cross_cone_bridge().exports().iter().find(|export| {
                let StrongCallableDefinitionOwner::Function(id) = export.target() else { return false; };
                let key = sections.identity_graph().canonical_key::<_, SourceDeclarationKey>(id).unwrap();
                matches!(key.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name)
            }).unwrap();
            let abi = export.abi_signature();
            if index == 0 {
                match (kind, abi.arguments(), abi.result()) {
                    (
                        0,
                        [ScoopAbiArgument::ElidedZst(argument)],
                        ScoopAbiReturn::ElidedZst(result),
                    ) => {
                        assert_eq!(argument.byte_size(), 0);
                        assert_eq!(*argument, result);
                    }
                    (
                        1,
                        [ScoopAbiArgument::Indirect(argument)],
                        ScoopAbiReturn::Indirect(result),
                    ) => {
                        assert_eq!(argument.byte_size(), 24);
                        assert_eq!(argument.alignment().get(), 8);
                        assert_eq!(*argument, result);
                    }
                    _ => panic!("unexpected {name} ABI: {abi:?}"),
                }
                expected_abis.push(abi.clone());
            } else {
                assert_eq!(abi, &expected_abis[kind]);
            }
            let symbol = format!("_{}", export.expected_symbol().symbol());
            let definition = link
                .defined_symbols()
                .owners()
                .iter()
                .find(|owner| owner.symbol() == symbol.as_bytes())
                .unwrap();
            let object = link
                .final_objects()
                .objects()
                .iter()
                .find(|object| object.member() == definition.member())
                .unwrap();
            let path = directory.join(format!("{index}-{name}.o"));
            std::fs::write(&path, object.bytes()).unwrap();
            objects.push(path);
            symbols.push(export.expected_symbol().symbol().to_string());
        }
    }
    // Exercise the canonical Scoop ABI directly, including byval and sret.
    // Only actual function members are linked; this is not multi-image startup.
    let mut harness = std::fs::read_to_string(
        crate::workspace_root().join("tests/fixtures/m23-imported-struct-values/runtime.ll"),
    )
    .unwrap();
    for (index, symbol) in symbols.iter().enumerate() {
        harness = harness.replace(&format!("@f{index}"), &format!("@\"{symbol}\""));
    }
    let source = directory.join("main.ll");
    let executable = directory.join("struct-values");
    std::fs::write(&source, harness).unwrap();
    let output = std::process::Command::new("cc")
        .arg(&source)
        .args(&objects)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "linking actual dependency function objects failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "dependency value ABI execution failed: {output:?}"
    );
}
