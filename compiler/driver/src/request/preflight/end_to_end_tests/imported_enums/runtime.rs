use super::*;
use scoop_identity::{
    ScoopAbiArgument, ScoopAbiReturn, SourceDeclarationKey, StrongCallableDefinitionOwner,
};

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess; 4],
    fixtures: &Path,
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
    let mut harness = std::fs::read_to_string(fixtures.join("runtime.ll")).unwrap();
    let mut enum_abi_seen = false;
    for (index, identity) in identities.iter().enumerate().skip(1) {
        let (sections, link) = closure.artifact(*identity).unwrap();
        let mut symbols = Vec::new();
        for export in sections.lir_cross_cone_bridge().exports() {
            if let StrongCallableDefinitionOwner::Function(id) = export.target() {
                let key = sections
                    .identity_graph()
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap();
                if let scoop_identity::DeclarationName::Named(name) = key.name() {
                    if index == 2 && name.as_str() == "relay" {
                        let abi = export.abi_signature();
                        let (
                            [ScoopAbiArgument::Indirect(argument)],
                            ScoopAbiReturn::Indirect(result),
                        ) = (abi.arguments(), abi.result())
                        else {
                            panic!("unexpected enum ABI: {abi:?}")
                        };
                        assert_eq!(*argument, result);
                        assert_eq!(result.byte_size(), 24);
                        assert_eq!(result.alignment().get(), 8);
                        enum_abi_seen = true;
                    }
                    if index == 3 && name.as_str() == "check" {
                        harness = harness.replace(
                            "@check(",
                            &format!("@\"{}\"(", export.expected_symbol().symbol()),
                        );
                    }
                }
            }
            symbols.push(export.expected_symbol().symbol().to_string());
        }
        symbols.extend(
            sections
                .lir_exports()
                .callables()
                .records()
                .iter()
                .filter(|export| {
                    matches!(
                        export.target(),
                        StrongCallableDefinitionOwner::Constructor(_)
                    )
                })
                .map(|export| export.definition().symbol().symbol().to_string()),
        );
        let mut members = Vec::new();
        for symbol in symbols {
            let symbol = format!("_{symbol}");
            let definition = link
                .defined_symbols()
                .owners()
                .iter()
                .find(|owner| owner.symbol() == symbol.as_bytes())
                .unwrap();
            if members.contains(&definition.member()) {
                continue;
            }
            members.push(definition.member());
            let object = link
                .final_objects()
                .objects()
                .iter()
                .find(|object| object.member() == definition.member())
                .unwrap();
            let path = directory.join(format!("{}.o", objects.len()));
            std::fs::write(&path, object.bytes()).unwrap();
            objects.push(path);
        }
    }
    assert!(enum_abi_seen);
    let source = directory.join("main.ll");
    let executable = directory.join("enums");
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
        "linking actual enum objects failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "dependency enum execution failed: {output:?}"
    );
}
