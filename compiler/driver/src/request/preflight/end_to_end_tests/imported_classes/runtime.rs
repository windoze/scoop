use super::*;
use scoop_identity::{SourceDeclarationKey, StrongCallableDefinitionOwner};
use std::path::PathBuf;
use std::process::Command;

pub(super) fn build(target: &scoop_toolchain::ResolvedTargetProfile, directory: &Path) -> PathBuf {
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

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess; 4],
    runtime: &Path,
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
    let mut entry = None;
    let mut string = None;
    let mut descriptors = std::collections::BTreeMap::new();
    let mut immortals = Vec::new();
    for (index, identity) in identities.iter().enumerate() {
        let (sections, link) = closure.artifact(*identity).unwrap();
        for descriptor in sections.lir_exports().descriptors().records() {
            assert!(
                descriptors
                    .insert(
                        descriptor.exact(),
                        descriptor.definition().symbol().symbol().to_string(),
                    )
                    .is_none()
            );
        }
        immortals.extend_from_slice(
            sections
                .lir_strong_production()
                .registration_production()
                .immortal_objects()
                .registrations(),
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
        if index == 3 {
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
    for (index, immortal) in immortals.iter().enumerate() {
        let object = immortal.object_symbol().symbol();
        let descriptor = &descriptors[&immortal.type_registration()];
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
    std::fs::write(
        &harness,
        std::fs::read_to_string(fixtures.join("runtime.c"))
            .unwrap()
            .replace("SCOOP_FIXTURE_ENTRY", &format!("_{entry}"))
            .replace("SCOOP_FIXTURE_IMMORTALS", &immortal_declarations),
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
            "dependency class execution (stress={stress}): {output:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
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
