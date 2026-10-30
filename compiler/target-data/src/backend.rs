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
        llvm_target_backend: 1,
        unwind_provider: 1,
        artifact_inspection: NativeObjectFormat::MachO64,
        lsda: BackendLsdaEncodingsV1 {
            lp_start: 0xff,
            type_table: 0x9b,
            call_site: 0x01,
            omitted_type_table: None,
        },
        machine_frame: None,
    },
);

pub const LLVM_22_1_LINUX_X86_64: BackendProfile = BackendProfile::new(
    &id("llvm-22-1-linux-x86-64"),
    &BackendProfileContract {
        cpu: "x86-64",
        llvm_target_backend: 2,
        unwind_provider: 2,
        artifact_inspection: NativeObjectFormat::Elf64,
        lsda: BackendLsdaEncodingsV1 {
            omitted_type_table: Some(0xff),
            ..LLVM_22_1.contract().lsda
        },
        machine_frame: Some(BackendMachineFrameV1::FixedFrame {
            stack_pointer_register: 7,
            frame_pointer_register: 6,
            root_slot_bytes: 8,
            frame_record_bytes: 16,
        }),
        ..LLVM_22_1.contract()
    },
);

pub const LLVM_22_1_WINDOWS_X86_64: BackendProfile = BackendProfile::new(
    &id("llvm-22-1-windows-x86-64"),
    &BackendProfileContract {
        unwind_provider: 3,
        artifact_inspection: NativeObjectFormat::Coff64,
        lsda: BackendLsdaEncodingsV1 {
            lp_start: 0xff,
            type_table: 0x00,
            call_site: 0x01,
            omitted_type_table: Some(0xff),
        },
        machine_frame: Some(BackendMachineFrameV1::WindowsSeh {
            stack_pointer_register: 7,
            frame_pointer_register: 6,
            root_slot_bytes: 8,
        }),
        ..LLVM_22_1_LINUX_X86_64.contract()
    },
);
