use object::{Object, ObjectSection};

use super::*;

mod source;

#[test]
fn actual_release_hooks_keep_raw_nounwind_machine_bodies() {
    let target = resolved_target().expect("release machine checks require a target");
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);
    let profile =
        scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
            .unwrap();
    for case in [
        "standalone",
        "native-storage",
        "native-aggregate/standalone",
        "generic/standalone",
    ] {
        let (coordinate, output) = source::lower(sysroot.path(), &target, case);
        let hooks = output
            .module()
            .release_hooks
            .iter()
            .map(|(_, hook)| hook.code.callable_body.id())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(!hooks.is_empty(), "{case}");
        let rendered = scoop_codegen::render_llvm_ir_members(
            &output,
            &coordinate,
            &[ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            profile,
        )
        .unwrap();
        let mut checked = 0;
        for member in &rendered {
            if matches!(member.units().kind(), scoop_codegen::ScoopLirObjectKindV1::CallableBody(body) if hooks.contains(&body))
            {
                check_function(member.llvm_ir());
                checked += 1;
            }
        }
        assert_eq!(checked, hooks.len(), "{case}");
        let production = output
            .build_production_section_v2(
                coordinate,
                &[ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                &[],
            )
            .unwrap();
        let emitted =
            scoop_codegen::emit_object_set_v2(&output, production, sysroot.path(), profile)
                .unwrap();
        for member in emitted.members() {
            if !matches!(member.units().kind(), scoop_codegen::ScoopLirObjectKindV1::CallableBody(body) if hooks.contains(&body))
            {
                continue;
            }
            let bytes = std::fs::read(member.path()).unwrap();
            let object = object::File::parse(bytes.as_slice()).unwrap();
            for section in object.sections() {
                let name = section.name().unwrap();
                assert!(
                    !matches!(name, "__gcc_except_tab" | "__llvm_stackmaps"),
                    "release object contains {name}"
                );
            }
        }
    }
}

fn check_function(ir: &str) {
    // Other functions may remain as declarations in this physical module.
    let start = ir.find("\ndefine ").expect("the hook has a definition") + 1;
    let end = start + ir[start..].find("\n}").unwrap() + 2;
    let function = &ir[start..end];
    let header = function.lines().next().unwrap();
    assert!(header.contains("void @"), "{header}");
    assert!(header.contains("(ptr "), "{header}");
    assert!(!header.contains("unnamed_addr"), "{header}");
    for forbidden in [
        "addrspace(1)",
        "addrspacecast",
        "gc.statepoint",
        "gc.relocate",
        "gc.result",
        " gc \"",
        "personality",
        "landingpad",
        "invoke ",
        "scoop_gc_",
        "scoop_rt_",
        "scoop_thread_",
    ] {
        assert!(!function.contains(forbidden), "{forbidden}: {function}");
    }
    let attributes = header
        .split_whitespace()
        .find(|word| word.starts_with('#'))
        .expect("the hook has nounwind attributes");
    let prefix = format!("attributes {attributes} = ");
    let attributes = ir.lines().find(|line| line.starts_with(&prefix)).unwrap();
    assert!(attributes.contains("nounwind"), "{attributes}");
}
