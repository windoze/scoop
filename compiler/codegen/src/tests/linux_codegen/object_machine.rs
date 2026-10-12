//! Real ELF objects retain their machine identity at codegen and slib boundaries.

use super::*;
use scoop_slib::{
    ObjectEnvelopeFormatV1, ValidatedElfObject, validate_linux_elf_object_envelope_v1,
};

#[test]
fn elf_machine_and_flags_are_checked_before_relocation_interpretation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("machine.o");
    let mut module = for_target(exceptions_module(), TargetProfileId::LinuxX86_64Gnu);
    module.output = scoop_lir::LirOutput::Library;
    let profile =
        ValidatedBackendProfile::from_selection(scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1)
            .unwrap();
    let expected = statepoint::expectations(&module).unwrap();
    let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let emitted = emit_object_set(
        &input,
        &scoop_lir::ConeCoordinate::reserved_single_file(),
        &[scoop_identity::ConeIdentity::CORE],
        scoop_lir::EntryProductionSourceV1::Library,
        directory.path(),
        profile,
    )
    .unwrap();
    let bytes = std::fs::read(emitted.members()[0].path()).unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let envelope = validate_linux_elf_object_envelope_v1(&bytes, target).unwrap();
        assert!(matches!(
            envelope.format(),
            ObjectEnvelopeFormatV1::Elf64 { flags: 0, .. }
        ));
        let mut flags = bytes.clone();
        flags[48..52].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            ValidatedElfObject::read(&flags, target)
                .unwrap_err()
                .to_string()
                .contains("flags")
        );
        let mut machine = bytes.clone();
        machine[18..20].copy_from_slice(&object::elf::EM_AARCH64.to_le_bytes());
        assert!(
            ValidatedElfObject::read(&machine, target)
                .unwrap_err()
                .to_string()
                .contains("selected target")
        );
        std::fs::write(&path, machine).unwrap();
        assert!(
            check(input.module(), &expected, profile, &path)
                .unwrap_err()
                .0
                .contains("selected machine")
        );
    }
    assert!(
        ValidatedElfObject::read(&bytes, TargetProfileId::DarwinAarch64)
            .unwrap_err()
            .to_string()
            .contains("does not support ELF")
    );
}
