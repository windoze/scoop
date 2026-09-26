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
    let mut symbols = Vec::new();
    for (index, names) in [
        &["sameToken", "copyWide", "total", "plus"][..],
        &["keepToken", "keepWide", "total"][..],
        &["token", "wide", "sum"][..],
    ]
    .into_iter()
    .enumerate()
    {
        let (sections, link) = closure.artifact(identities[index + 1]).unwrap();
        for name in names {
            let export = sections.lir_cross_cone_bridge().exports().iter().find(|export| {
                let StrongCallableDefinitionOwner::Function(id) = export.target() else { return false; };
                let key = sections.identity_graph().canonical_key::<_, SourceDeclarationKey>(id).unwrap();
                matches!(key.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == *name)
            }).unwrap_or_else(|| panic!("missing actual callable {name}"));
            let abi = export.abi_signature();
            if index == 0 {
                match (*name, abi.arguments(), abi.result()) {
                    (
                        "sameToken",
                        [
                            ScoopAbiArgument::ElidedZst(receiver),
                            ScoopAbiArgument::ElidedZst(argument),
                        ],
                        ScoopAbiReturn::ElidedZst(result),
                    ) => {
                        assert_eq!(*receiver, result);
                        assert_eq!(*argument, result);
                    }
                    (
                        "copyWide" | "plus",
                        [
                            ScoopAbiArgument::Indirect(receiver),
                            ScoopAbiArgument::Indirect(argument),
                        ],
                        ScoopAbiReturn::Indirect(result),
                    ) => {
                        assert_eq!(*receiver, result);
                        assert_eq!(*argument, result);
                        assert_eq!(result.byte_size(), 24);
                        assert_eq!(result.alignment().get(), 8);
                    }
                    ("total", [ScoopAbiArgument::Indirect(receiver), _], _) => {
                        assert_eq!(receiver.byte_size(), 24)
                    }
                    _ => panic!("unexpected {name} ABI: {abi:?}"),
                }
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
    for (index, names) in [
        (
            1,
            vec![
                "defaultField",
                "defaultMember",
                "defaultProperty",
                "defaultMarker",
            ],
        ),
        (
            2,
            vec![
                "first",
                "marker",
                "nestedFirst",
                "fieldSum",
                "computedTotal",
                "fromField",
                "fromMember",
                "fromProperty",
                "fromMarker",
            ],
        ),
    ] {
        let (sections, link) = closure.artifact(identities[index]).unwrap();
        for export in sections.lir_cross_cone_bridge().exports() {
            let selected_name = match export.target() {
                StrongCallableDefinitionOwner::PropertyAccessor(_) => index == 1,
                StrongCallableDefinitionOwner::Function(id) => {
                    let key = sections
                        .identity_graph()
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap();
                    matches!(key.name(), scoop_identity::DeclarationName::Named(name) if names.contains(&name.as_str()))
                }
                _ => false,
            };
            if !selected_name {
                continue;
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
            let path = directory.join(format!("extra-{}.o", objects.len()));
            std::fs::write(&path, object.bytes()).unwrap();
            objects.push(path);
        }
        for name in names.into_iter().filter(|_| index == 2) {
            let export = sections.lir_cross_cone_bridge().exports().iter().find(|export| {
                let StrongCallableDefinitionOwner::Function(id) = export.target() else { return false; };
                let key = sections.identity_graph().canonical_key::<_, SourceDeclarationKey>(id).unwrap();
                matches!(key.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name)
            }).unwrap();
            symbols.push(export.expected_symbol().symbol().to_string());
        }
    }
    // The actual exported function objects use Scoop's byval/sret ABI.
    // Linking them does not require multi-image startup or artifact-only program linking.
    let mut harness = std::fs::read_to_string(fixtures.join("runtime.ll")).unwrap();
    for (index, symbol) in symbols.iter().enumerate() {
        harness = harness.replace(&format!("@f{index}("), &format!("@\"{symbol}\"("));
    }
    let source = directory.join("main.ll");
    let executable = directory.join("members");
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
        "linking actual member objects failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "dependency member execution failed: {output:?}"
    );
}
