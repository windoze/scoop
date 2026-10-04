use super::*;
use std::process::Command;

fn compile(source: &str, extension: &str, extra: &[&str]) -> Vec<u8> {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join(format!("input.{extension}"));
    let output = directory.path().join("input.o");
    std::fs::write(&input, source).unwrap();
    let result = Command::new("gcc")
        .env("TMPDIR", directory.path())
        .args(["-fPIC", "-fno-common", "-g", "-c"])
        .args(extra)
        .arg(input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(output).unwrap()
}

#[test]
fn elf_native_objects_keep_tls_visibility_weakness_and_debug_relocations() {
    let bytes = compile(
        "_Thread_local long local_tls; extern _Thread_local long foreign_tls;\n\
         extern long foreign(void); const long read_only = 7; long data;\n\
         __attribute__((weak)) long weak_data;\n\
         long function(void) { return foreign() + local_tls + foreign_tls; }\n",
        "c",
        &[],
    );
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let info = read(&bytes, target).unwrap();
        assert_eq!(
            info.definitions["local_tls"].kind,
            NativeSymbolKind::ThreadLocal
        );
        assert_eq!(
            info.definitions["function"].kind,
            NativeSymbolKind::Function
        );
        assert!(!info.definitions["data"].read_only);
        assert!(info.definitions["read_only"].read_only);
        assert!(info.definitions["weak_data"].weak);
        assert!(info.requirements.contains("foreign"));
        assert!(info.requirements.contains("foreign_tls"));
        assert!(info.requirements.contains("__tls_get_addr"));
    }
}

#[test]
fn elf_native_inputs_reject_initializers_common_storage_and_cpp_eh() {
    for (source, flags, expected) in [
        (
            "__attribute__((constructor)) void init(void) {}",
            &[][..],
            "initialization",
        ),
        ("int tentative;", &["-fcommon"][..], "common/tentative"),
        (
            "extern void __cxa_throw(void); void f(void) { __cxa_throw(); }",
            &[][..],
            "C++ EH",
        ),
    ] {
        let bytes = compile(source, "c", flags);
        let error = read(&bytes, TargetProfileId::LinuxX86_64Gnu).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

const RELOCATIONS: &str = ".text\n.globl function\n.type function,@function\n\
function: call foreign\nret\n.size function,.-function\n\
.section .debug_test,\"\",@progbits\n.long foreign\n\
.data\n.globl data\n.type data,@object\ndata: .quad foreign\n.size data,.-data\n\
.section .note.GNU-stack,\"\",@progbits\n";

#[test]
fn elf_reader_checks_actual_relocation_width_and_symbol_indices() {
    let bytes = compile(RELOCATIONS, "S", &[]);
    let parsed = ValidatedElfObject::read(&bytes, TargetProfileId::LinuxX86_64Gnu).unwrap();
    assert!(
        parsed
            .relocations()
            .iter()
            .any(|relocation| relocation.width == 4)
    );
    assert!(
        parsed
            .relocations()
            .iter()
            .any(|relocation| relocation.width == 8)
    );
    let file = parsed.file();
    let endian = file.endian();
    let rela = file
        .elf_section_table()
        .iter()
        .find(|section| {
            section.sh_type(endian) == elf::SHT_RELA
                && file
                    .elf_section_table()
                    .section_name(endian, section)
                    .unwrap()
                    == b".rela.debug_test"
        })
        .unwrap();
    let offset = rela.sh_offset(endian) as usize;
    let mut invalid = bytes.clone();
    invalid[offset..offset + 8].copy_from_slice(&1u64.to_le_bytes());
    let error = read(&invalid, TargetProfileId::LinuxX86_64Gnu).unwrap_err();
    assert!(error.to_string().contains("relocation write"), "{error}");
    let mut invalid = bytes.clone();
    invalid[offset + 12..offset + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    let error = read(&invalid, TargetProfileId::LinuxX86_64Gnu).unwrap_err();
    assert!(error.to_string().contains("missing symbol"), "{error}");
    let mut invalid = bytes.clone();
    invalid[18..20].copy_from_slice(&elf::EM_AARCH64.to_le_bytes());
    assert!(read(&invalid, TargetProfileId::LinuxX86_64Gnu).is_err());
}

#[test]
fn elf_reader_checks_symbol_extent_and_comdat_group_references() {
    let bytes = compile(
        ".section .text.body,\"axG\",@progbits,body,comdat\n\
        .weak body\n.type body,@function\nbody: ret\n.size body,.-body\n\
        .section .note.GNU-stack,\"\",@progbits\n",
        "S",
        &[],
    );
    let parsed = ValidatedElfObject::read(&bytes, TargetProfileId::LinuxX86_64Gnu).unwrap();
    let file = parsed.file();
    let endian = file.endian();
    let group = file
        .elf_section_table()
        .iter()
        .find(|section| section.sh_type(endian) == elf::SHT_GROUP)
        .unwrap();
    let member_offset = group.sh_offset(endian) as usize + 4;
    let mut invalid = bytes.clone();
    invalid[member_offset..member_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(read(&invalid, TargetProfileId::LinuxX86_64Gnu).is_err());
    let symbol = file.symbol_by_name("body").unwrap();
    let symtab = file
        .elf_section_table()
        .section(file.elf_symbol_table().section())
        .unwrap();
    let size_offset = symtab.sh_offset(endian) as usize + symbol.index().0 * 24 + 16;
    let mut invalid = bytes.clone();
    invalid[size_offset..size_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    let error = read(&invalid, TargetProfileId::LinuxX86_64Gnu).unwrap_err();
    assert!(error.to_string().contains("symbol extent"), "{error}");
}
