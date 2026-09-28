use super::*;
use scoop_identity::{SourceDeclarationKey, StrongCallableDefinitionOwner};
use std::path::PathBuf;
use std::process::Command;

pub(in super::super) fn build(
    target: &scoop_toolchain::ResolvedTargetProfile,
    directory: &Path,
) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let workspace = crate::workspace_root();
    let profile = target.runtime_build();
    let mut objects = Vec::new();
    for (index, source) in profile.runtime_sources().iter().enumerate() {
        let object = directory.join(format!("{index}.o"));
        let output = Command::new(target.final_link().linker_driver())
            .args(profile.runtime_c_flags())
            .arg("-Dmain=scoop_fixture_runtime_main")
            .arg("-I")
            .arg(workspace.join("runtime/include"))
            .arg("-c")
            .arg(workspace.join(source))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        objects.push(object);
    }
    let archive = directory.join("runtime.a");
    archive_objects(&archive, &objects);
    archive
}

pub(in super::super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    runtime: &Path,
    fixtures: &Path,
    directory: &Path,
    case: &str,
) -> scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure {
    let bytes = artifacts
        .iter()
        .map(|artifact| std::fs::read(artifact.artifact().path()).unwrap())
        .collect::<Vec<_>>();
    let identities = artifacts
        .iter()
        .map(|artifact| {
            artifact
                .artifact()
                .summary()
                .coordinate()
                .identity()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let current = artifacts.len().checked_sub(1).expect("current artifact");
    let mut direct = artifacts[current]
        .artifact()
        .summary()
        .direct_dependencies()
        .iter()
        .map(scoop_slib::DependencyRecord::identity)
        .collect::<Vec<_>>();
    direct.sort_unstable();
    let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            identities[current],
            target.lir_target_selection(),
            direct,
            bytes[..current].iter().map(Vec::as_slice).collect(),
            &bytes[current],
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    std::fs::create_dir_all(directory).unwrap();
    let mut objects = Vec::new();
    let mut entry = None;
    let mut string = None;
    let mut descriptors = std::collections::BTreeMap::new();
    let mut immortals = std::collections::BTreeMap::new();
    let mut globals = Vec::new();
    for (index, identity) in identities.iter().enumerate() {
        let (sections, link) = closure.artifact(*identity).unwrap();
        if index == 2 && case == "initialization-imported-object" {
            let object_uses = sections
                .mir_type_bridge()
                .exports()
                .initialization_uses()
                .records()
                .iter()
                .filter(|usage| {
                    matches!(
                        usage.cause(),
                        scoop_mir::MirExternalInitializationCauseV1::ObjectValue(_)
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(object_uses.len(), 1);
            assert_eq!(object_uses[0].provider(), identities[1]);
        }
        for descriptor in sections.lir_exports().descriptors().records() {
            let definition = descriptor.physical_definition();
            let value = (
                definition.symbol().symbol().to_string(),
                definition.symbol().linkage(),
                definition.definition(),
            );
            if let Some(previous) = descriptors.insert(descriptor.exact(), value.clone()) {
                assert_eq!(value.1, scoop_identity::LinkageClass::OdrWeak, "{case}");
                assert_eq!(previous, value, "{case}: duplicate ODR descriptors agree");
            }
        }
        for &plan in sections
            .lir_strong_production()
            .registration_production()
            .immortal_objects()
            .registrations()
        {
            if let Some(previous) = immortals.insert(plan.object(), plan) {
                let scoop_lir::RegistrationDefinitionOwner::Odr { group, member } =
                    plan.definition_owner()
                else {
                    panic!("{case}: duplicate Strong immortal objects");
                };
                assert_eq!(previous, plan, "{case}: duplicate ODR immortal plans agree");
                let merged = closure.odr_definitions().get(group, member).unwrap();
                assert!(
                    merged
                        .candidates()
                        .any(|candidate| candidate.provider() == *identity)
                );
            }
        }
        globals.extend(
            sections
                .lir_strong_production()
                .registration_production()
                .static_storages()
                .registrations()
                .iter()
                .filter(|plan| {
                    plan.semantic().scan_kind() == scoop_lir::StaticStorageScanKindV1::Recursive
                })
                .map(|plan| {
                    (
                        plan.semantic().symbol().symbol(),
                        plan.scan_symbol().symbol(),
                    )
                }),
        );
        for ty in sections.mir_type_bridge().exports().types().records() {
            if matches!(
                ty.representation(),
                scoop_mir::MirTypeRepresentationV1::Intrinsic(
                    scoop_mir::MirParamFreeIntrinsicV1::String
                )
            ) {
                let descriptor = sections
                    .lir_exports()
                    .descriptors()
                    .get(ty.exact())
                    .unwrap();
                assert!(
                    string
                        .replace(descriptor.definition().symbol().symbol().to_string())
                        .is_none()
                );
            }
        }
        if index == current {
            for export in sections.lir_cross_cone_bridge().exports() {
                let StrongCallableDefinitionOwner::Function(id) = export.target() else {
                    continue;
                };
                let key = sections
                    .identity_graph()
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap();
                if matches!(key.name(), scoop_identity::DeclarationName::Named(name) if name.as_str() == "check")
                {
                    entry = Some(export.expected_symbol().symbol().to_string());
                }
            }
        }
        for object in link.final_objects().objects() {
            let path = directory.join(format!("{}.o", objects.len()));
            std::fs::write(&path, object.bytes()).unwrap();
            objects.push(path);
        }
    }
    let archive = directory.join("classes.a");
    archive_objects(&archive, &objects);
    let executable = directory.join("classes");
    let entry = entry.unwrap();
    let string = string.unwrap();
    let harness = directory.join("main.c");
    let mut immortal_declarations = String::new();
    let mut immortal_entries = String::new();
    for (index, immortal) in immortals.values().enumerate() {
        let object = immortal.object_symbol().symbol();
        let descriptor = &descriptors[&immortal.type_registration()].0;
        immortal_declarations.push_str(&format!(
            "extern const unsigned char fixture_immortal_{index}[] __asm__(\"_{object}\");\n\
             extern const ScoopTypeDescriptor fixture_immortal_td_{index} __asm__(\"_{descriptor}\");\n"
        ));
        immortal_entries.push_str(&format!(
            "{{fixture_immortal_{index}, {}, &fixture_immortal_td_{index}}},\n",
            immortal.object_size(),
        ));
    }
    if immortals.is_empty() {
        immortal_entries.push_str("{0}");
    }
    immortal_declarations.push_str(&format!(
        "const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[] = {{{immortal_entries}}};\n\
         const uint64_t scoop_image_immortal_object_count = {};\n", immortals.len(),
    ));
    let mut global_declarations = String::new();
    let mut global_entries = String::new();
    for (index, (storage, scan)) in globals.iter().enumerate() {
        global_declarations.push_str(&format!(
            "extern unsigned char fixture_global_{index}[] __asm__(\"_{storage}\");\n\
             extern const uint64_t fixture_global_scan_{index}[] __asm__(\"_{scan}\");\n"
        ));
        global_entries.push_str(&format!(
            "{{fixture_global_{index}, fixture_global_scan_{index}}},\n"
        ));
    }
    if globals.is_empty() {
        global_entries.push_str("{0}");
    }
    global_declarations.push_str(&format!(
        "const ScoopManagedGlobalDescriptor scoop_image_managed_globals[] = {{{global_entries}}};\n\
         const uint64_t scoop_image_managed_global_count = {};\n",
        globals.len(),
    ));
    std::fs::write(
        &harness,
        std::fs::read_to_string(fixtures.join("runtime.c"))
            .unwrap()
            .replace("SCOOP_FIXTURE_ENTRY", &format!("_{entry}"))
            .replace("SCOOP_FIXTURE_IMMORTALS", &immortal_declarations)
            .replace("SCOOP_FIXTURE_GLOBALS", &global_declarations),
    )
    .unwrap();
    let output = Command::new(target.final_link().linker_driver())
        .args(target.final_link().linker_args())
        .args(target.runtime_build().runtime_c_flags())
        .arg("-I")
        .arg(crate::workspace_root().join("runtime/include"))
        .arg("-I")
        .arg(crate::workspace_root().join("runtime/src"))
        .arg(&harness)
        .arg(&archive)
        .arg(runtime)
        .arg(format!("-Wl,-u,_{entry},-alias,_{entry},_scoop_main"))
        .arg(format!(
            "-Wl,-u,_{string},-alias,_{string},_scoop_td_String"
        ))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "linking actual class objects: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for stress in [false, true] {
        let mut command = Command::new(&executable);
        command.env_remove("SCOOP_GC_STRESS_MOVE");
        if stress {
            command.env("SCOOP_GC_STRESS_MOVE", "1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "dependency class execution at {} (stress={stress}): {output:?}",
            directory.display()
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
    closure
}

fn archive_objects(archive: &Path, objects: &[PathBuf]) {
    let output = Command::new("ar")
        .arg("rcs")
        .arg(archive)
        .args(objects)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "archiving actual objects: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
