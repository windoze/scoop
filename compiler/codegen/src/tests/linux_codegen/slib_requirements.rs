//! The libc target remains distinct through object and requirement closures.

use super::slib_support::SlibObjects;
use super::*;
use scoop_slib::*;

#[test]
fn elf_allocation_tls_is_a_runtime_requirement_with_a_platform_resolver() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_lir::LirTargetProfile::LINUX_X86_64_GNU,
        scoop_lir::LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        let module = super::super::objects::heap_module_for(target);
        let semantics = scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
        let fixture = SlibObjects::new(module, directory.path());
        let stackmaps = verify_scoop_lir_stackmaps_v1(
            fixture.builtins(&fixture.objects),
            semantics,
            &super::slib_support::candidates(&fixture.objects),
        )
        .unwrap();
        let callables = fixture.callables(stackmaps);
        let requirements = callables
            .body_objects()
            .undefined_requirements()
            .requirements();
        for (symbol, runtime) in [
            (b"scoop_rt_allocation_context".as_slice(), true),
            (b"__tls_get_addr".as_slice(), false),
        ] {
            let uses = requirements
                .iter()
                .filter(|requirement| requirement.use_site().symbol() == symbol)
                .collect::<Vec<_>>();
            assert!(!uses.is_empty());
            for usage in uses {
                assert!(match usage.requirement() {
                    FinalUndefinedSymbolRequirementV1::RuntimeAbi { .. } => runtime,
                    FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport { .. } => !runtime,
                    _ => false,
                });
            }
        }
    }
}

#[test]
fn elf_strong_closure_rejects_mixed_libc_members_and_requirement_targets() {
    let directory = tempfile::tempdir().unwrap();
    let mut closures = Vec::new();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let fixture = SlibObjects::new(
            for_target(
                moving_gc::qualification::stackmap_qualification_module(),
                target,
            ),
            directory.path(),
        );
        let builtins = fixture.builtins(&fixture.objects);
        let closure = builtins.strong_relocations().clone();
        assert_eq!(closure.target().id(), target);
        let wrong_target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
        assert!(matches!(
            verify_dependency_strong_requirements_v1(wrong_target, closure.clone(), &[]),
            Err(CrossConeStrongRequirementValidationError::TargetMismatch { .. })
        ));
        closures.push(closure);
    }
    let first = closures[0].members()[0].clone();
    let second = closures[1]
        .members()
        .iter()
        .find(|member| member.member() != first.member())
        .unwrap()
        .clone();
    assert!(matches!(
        verify_current_cone_strong_relocation_closure_v1(vec![first, second]),
        Err(StrongRelocationClosureValidationError::MixedTarget { .. })
    ));
}
