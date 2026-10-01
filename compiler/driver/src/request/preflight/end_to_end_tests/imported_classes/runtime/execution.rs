use super::*;

pub(in super::super::super) fn execute(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    closure: &scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure,
    runtime: &Path,
    template: &str,
    directory: &Path,
    case: &str,
) {
    std::fs::create_dir_all(directory).unwrap();
    let runner = runner::build_runner(target, artifacts, closure, directory);
    let artifacts = artifacts
        .iter()
        .copied()
        .chain([&runner])
        .collect::<Vec<_>>();
    let program = read(target, &artifacts);
    let executable = link_program(target, &program, runtime, template, directory);
    for stress in [false, true] {
        let mut command = Command::new(&executable);
        command.env_remove("SCOOP_GC_STRESS_MOVE");
        if stress {
            command.env("SCOOP_GC_STRESS_MOVE", "1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{case} at {} (stress={stress}): {output:?}",
            directory.display()
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

pub(in super::super::super) fn link_program(
    target: &scoop_toolchain::ResolvedTargetProfile,
    closure: &scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure,
    runtime: &Path,
    template: &str,
    directory: &Path,
) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let mut objects = Vec::new();
    let mut declarations = String::new();
    let mut images = Vec::new();
    let mut string = None;
    let mut root = None;
    for (index, (sections, link)) in closure.dependency_first().enumerate() {
        let production = sections.lir_strong_production();
        let symbol = production.image_plan().symbol().symbol();
        declarations.push_str(&format!(
            "extern const ScoopImageDescriptorV1 fixture_image_{index} __asm__(\"_{symbol}\");\n"
        ));
        images.push(format!("&fixture_image_{index}"));
        if let scoop_lir::EntryProductionPlanV1::Executable(entry) = production.entry_plan() {
            assert_eq!(sections.identity(), closure.physical_imports().current());
            assert!(
                root.replace(entry.root_descriptor_symbol().symbol())
                    .is_none()
            );
        }
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
        for bytes in link
            .final_objects()
            .objects()
            .iter()
            .map(|object| object.bytes())
            .chain(
                link.object_contents()
                    .generated_objects()
                    .map(|object| object.bytes()),
            )
        {
            let path = directory.join(format!("{}.o", objects.len()));
            std::fs::write(&path, bytes).unwrap();
            objects.push(path);
        }
    }
    let root = root.expect("an actual executable root descriptor");
    let string = string.unwrap();
    declarations.push_str(&format!(
        "extern const ScoopRootEntryDescriptorV1 fixture_root __asm__(\"_{root}\");\n"
    ));
    // Reverse input enumeration: startup derives dependency order from actual records.
    images.reverse();
    declarations.push_str(&format!("__attribute__((section(\"__DATA_CONST,__const\")))\nstatic const ScoopImageDescriptorV1 *const fixture_images[] = {{{}}};\n", images.join(",")));
    let archive = directory.join("program.a");
    archive_objects(&archive, &objects);
    let harness = directory.join("main.c");
    std::fs::write(
        &harness,
        template.replace("SCOOP_FIXTURE_IMAGES", &declarations),
    )
    .unwrap();
    let executable = directory.join("program");
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
        .arg(format!(
            "-Wl,-u,_{string},-alias,_{string},_scoop_td_String"
        ))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "linking actual program objects: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}
