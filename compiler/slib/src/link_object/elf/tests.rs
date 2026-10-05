//! Exercise real GNU assembler output without loading LLVM.

use std::process::Command;

use super::*;
use crate::link_object::{
    ObjectEnvelopeFormatV1, ObjectRelocationShapeV1, ObjectSymbolAttributesV1, ObjectSymbolKindV1,
};

#[test]
fn builtin_elf_preserves_large_section_indices_names_groups_and_signed_addends() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("facts.s");
    let output = directory.path().join("facts.o");
    let mut assembly = String::new();
    for index in 0..300 {
        assembly.push_str(&format!(
            ".section .rodata.scoop_long_section_name_{index},\"a\",@progbits\n.quad {index}\n"
        ));
    }
    assembly.push_str(
        r#"
.section .data,"aw",@progbits
.quad external + 17
.text
.globl entry
.hidden entry
.type entry,@function
entry:
call external
ret
.size entry,.-entry
.section .tdata,"awT",@progbits
.globl late_tls
.hidden late_tls
.type late_tls,@tls_object
late_tls: .quad 42
.size late_tls,8
.section .rodata.odr,"aG",@progbits,odr,comdat
.weak odr
.hidden odr
.type odr,@object
odr: .quad 7
.size odr,8
.section .note.GNU-stack,"",@progbits
"#,
    );
    std::fs::write(&source, assembly).unwrap();
    let result = Command::new("gcc")
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(output).unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let envelope = validate_linux_elf_object_envelope_v1(&bytes, target).unwrap();
        assert_eq!(envelope.target(), target);
        assert!(
            envelope
                .sections()
                .iter()
                .any(|s| s.section_name() == b".rodata.scoop_long_section_name_299")
        );
        let tls = envelope
            .symbols()
            .iter()
            .find(|s| s.name() == b"late_tls")
            .unwrap();
        assert!(tls.section_ordinal().unwrap().get() > 255);
        assert_eq!(tls.kind(), ObjectSymbolKindV1::ExternalStrongDefinition);
        assert!(
            matches!(tls.attributes(), ObjectSymbolAttributesV1::Elf { size: 8, info, other }
            if info & 15 == object::elf::STT_TLS && other == object::elf::STV_HIDDEN)
        );
        let ObjectEnvelopeFormatV1::Elf64 { groups, .. } = envelope.format() else {
            panic!("ELF facts")
        };
        assert_eq!(groups.len(), 1);
        let odr = envelope
            .symbols()
            .iter()
            .find(|s| s.name() == b"odr")
            .unwrap();
        assert_eq!(groups[0].signature_symbol, odr.table_index());
        assert!(groups[0].sections.contains(&odr.section_ordinal().unwrap()));
        for relocation in envelope.relocations() {
            assert_eq!(relocation.encoded_value(), 0);
            let ObjectRelocationShapeV1::ElfRela {
                kind,
                addend,
                width,
                ..
            } = relocation.shape()
            else {
                panic!("RELA")
            };
            match kind {
                object::elf::R_X86_64_64 => assert_eq!((addend, width), (17, 8)),
                object::elf::R_X86_64_PLT32 => assert_eq!((addend, width), (-4, 4)),
                _ => panic!("unexpected relocation {kind}"),
            }
        }
        assert_eq!(envelope.relocations().len(), 2);
    }
}
