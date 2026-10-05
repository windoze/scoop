//! The libc target remains distinct through object and requirement closures.

use super::slib_support::SlibObjects;
use super::*;
use scoop_slib::*;

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
