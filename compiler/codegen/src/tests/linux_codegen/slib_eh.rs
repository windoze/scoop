use object::ObjectSymbol;
use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates};
use super::*;

#[test]
fn elf_exception_personality_pointer_belongs_to_its_callable() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for odr in [false, true] {
            let mut module = for_target(exceptions_module(), target);
            if odr {
                for (index, function) in module.functions.iter_mut().enumerate() {
                    function.callable_body = odr_callable_body(&format!("elf_eh_{index}"));
                    refresh_test_safepoints(function);
                }
            }
            let semantics =
                scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
            let fixture = SlibObjects::new(module, directory.path());
            let builtins = fixture.builtins(&fixture.objects);
            let stackmaps =
                read_scoop_stackmaps_v1(builtins, semantics, &candidates(&fixture.objects))
                    .unwrap();
            let callables = fixture.callables(stackmaps);
            assert!(!callables.fingerprints().is_empty());
            let mut pointers = 0;
            for member in &fixture.emitted.members {
                let bytes = member.bytes.clone();
                let file = object::File::parse(bytes.as_slice()).unwrap();
                assert!(file.symbol_by_name("DW.ref.scoop_eh_personality").is_none());
                let Some(pointer) = file.section_by_name(".data.rel.ro.scoop.personality") else {
                    continue;
                };
                pointers += 1;
                assert_eq!(pointer.size(), 8);
                assert_eq!(pointer.relocations().count(), 1);
                if odr {
                    let member_path = directory.path().join("callable.o");
                    std::fs::write(&member_path, &member.bytes).unwrap();
                    let linked = directory.path().join("merged-eh.o");
                    let result = std::process::Command::new("/usr/bin/ld")
                        .arg("-r")
                        .arg(&member_path)
                        .arg(&member_path)
                        .arg("-o")
                        .arg(&linked)
                        .output()
                        .unwrap();
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    let bytes = std::fs::read(&linked).unwrap();
                    let merged = object::File::parse(bytes.as_slice()).unwrap();
                    assert_eq!(
                        merged
                            .section_by_name(".data.rel.ro.scoop.personality")
                            .unwrap()
                            .size(),
                        8
                    );
                    let function = file
                        .symbols()
                        .find(|symbol| {
                            symbol.kind() == object::SymbolKind::Text && symbol.size() != 0
                        })
                        .unwrap();
                    assert_eq!(
                        merged
                            .symbols()
                            .filter(|symbol| symbol.name() == function.name())
                            .count(),
                        1
                    );
                }
            }
            assert!(pointers > 0);
        }
    }
}
