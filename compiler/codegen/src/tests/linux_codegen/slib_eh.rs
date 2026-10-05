use object::ObjectSymbol;
use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates};
use super::*;

#[test]
fn elf_exception_personality_pointer_belongs_to_its_callable() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
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
            let expected = statepoint::expectations(&module).unwrap();
            let eh = artifact::eh_expectations(&module).unwrap();
            let fixture = SlibObjects::new(module, directory.path());
            let builtins = fixture.builtins(&fixture.objects);
            verify_scoop_lir_stackmaps_v1(builtins, semantics, &candidates(&fixture.objects))
                .unwrap();
            let mut pointers = 0;
            for member in fixture.emitted.members() {
                let bytes = std::fs::read(member.path()).unwrap();
                let file = object::File::parse(bytes.as_slice()).unwrap();
                assert!(file.symbol_by_name("DW.ref.scoop_eh_personality").is_none());
                let Some(pointer) = file.section_by_name(".data.rel.ro.scoop.personality") else {
                    continue;
                };
                pointers += 1;
                assert_eq!(pointer.size(), 8);
                assert_eq!(pointer.relocations().count(), 1);
                if odr {
                    let linked = directory.path().join("merged-eh.o");
                    let result = std::process::Command::new("/usr/bin/ld")
                        .arg("-r")
                        .arg(member.path())
                        .arg(member.path())
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
                    let symbol = function.name().unwrap();
                    ValidatedBackendProfile::from_selection(fixture.emitted.target_selection())
                        .unwrap()
                        .verify_object(
                            &linked,
                            &expected.for_function(symbol).unwrap(),
                            &eh.for_function(symbol),
                        )
                        .expect("coalesced EH and stackmap references remain complete");
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
