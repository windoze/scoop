use scoop_identity::BackendProfileId;
use scoop_lir::{
    BackendLsdaEncodingsV1, BackendMachineFrameV1, BackendProfile, BackendProfileContract,
    NativeObjectFormat,
};

const fn id(name: &str) -> BackendProfileId {
    match BackendProfileId::new(name, 1) {
        Ok(id) => id,
        Err(_) => panic!("invalid registered backend identity"),
    }
}

pub const LLVM_22_1: BackendProfile = BackendProfile::new(
    &id("llvm-22-1"),
    &BackendProfileContract {
        llvm_api_version: [22, 1],
        inkwell_binding: [0, 10, 1],
        cpu: "generic",
        features: "",
        llvm_target_backend: scoop_lir::TargetArchitecture::Aarch64,
        unwind_provider: 1,
        artifact_inspection: NativeObjectFormat::MachO64,
        lsda: BackendLsdaEncodingsV1 {
            lp_start: 0xff,
            type_table: 0x9b,
            call_site: 0x01,
            omitted_type_table: None,
        },
        machine_frame: BackendMachineFrameV1::DarwinAarch64,
    },
);

pub const LLVM_22_1_LINUX_X86_64: BackendProfile = BackendProfile::new(
    &id("llvm-22-1-linux-x86-64"),
    &BackendProfileContract {
        cpu: "x86-64",
        llvm_target_backend: scoop_lir::TargetArchitecture::X86_64,
        unwind_provider: 2,
        artifact_inspection: NativeObjectFormat::Elf64,
        lsda: BackendLsdaEncodingsV1 {
            omitted_type_table: Some(0xff),
            ..LLVM_22_1.contract().lsda
        },
        machine_frame: BackendMachineFrameV1::SysvAmd64,
        ..LLVM_22_1.contract()
    },
);
