//! Real LLVM assembler and MSVC bigobj output; no SDK headers are needed.

use std::process::Command;
use std::sync::OnceLock;

use object::pe;

use super::*;

struct Objects {
    _directory: tempfile::TempDir,
    ordinary: Vec<u8>,
    big: Vec<u8>,
}

fn objects() -> &'static Objects {
    static OBJECTS: OnceLock<Objects> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("facts.s");
        let output = directory.path().join("facts.obj");
        let mut assembly = String::new();
        for index in 0..300 {
            assembly.push_str(&format!(
                ".section .rdata$scoop_long_section_name_{index},\"dr\"\n.quad {index}\n"
            ));
        }
        assembly.push_str(r#"
.text
.globl entry
.def entry; .scl 2; .type 32; .endef
.seh_proc entry
entry:
subq $40, %rsp
.seh_stackalloc 40
.seh_endprologue
callq external
addq $40, %rsp
retq
.seh_endproc
.data
.globl pointer
pointer: .quad external + 17
.quad external - 17
.section .tls$,"dw"
.globl late_tls
late_tls: .quad 42
.section .rdata$odr,"dr",discard,odr
.globl odr
odr: .quad 7
.section .rdata$assoc,"dr",associative,odr
.quad odr
.weak weak_alias
.set weak_alias, external
"#);
        std::fs::write(&source, assembly).unwrap();
        let clang = std::env::var_os("SCOOP_TEST_CLANG").unwrap_or_else(|| "clang".into());
        let result = Command::new(clang).args(["--target=x86_64-pc-windows-msvc", "-c"])
            .arg(&source).arg("-o").arg(&output).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let ordinary = std::fs::read(&output).unwrap();
        let source = directory.path().join("big.c");
        let output = directory.path().join("big.obj");
        std::fs::write(&source, "__declspec(selectany) int shared_value = 42;\nint get_value(void) { return shared_value; }\n").unwrap();
        let result = Command::new("cl.exe").args(["/nologo", "/c", "/bigobj"])
            .arg(&source).arg(format!("/Fo{}", output.display())).output().unwrap();
        assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        let big = std::fs::read(output).unwrap();
        Objects { _directory: directory, ordinary, big }
    })
}

#[test]
fn coff_preserves_sections_symbols_comdats_and_implicit_addends() {
    let objects = objects();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        let envelope = validate_windows_coff_object_envelope_v1(&objects.ordinary, target).unwrap();
        assert_eq!(envelope.target(), target);
        assert!(
            envelope
                .sections()
                .iter()
                .any(|s| s.section_name() == b".rdata$scoop_long_section_name_299")
        );
        for name in [b".pdata".as_slice(), b".xdata".as_slice()] {
            assert!(envelope.sections().iter().any(|s| s.section_name() == name));
        }
        let symbol = |name: &[u8]| {
            envelope
                .symbols()
                .iter()
                .find(|s| s.name() == name)
                .unwrap()
        };
        assert!(symbol(b"late_tls").section_ordinal().unwrap().get() > 255);
        assert_eq!(
            symbol(b"entry").kind(),
            ObjectSymbolKindV1::ExternalStrongDefinition
        );
        assert_eq!(
            symbol(b"odr").kind(),
            ObjectSymbolKindV1::ExternalWeakDefinition
        );
        assert!(envelope.symbols().iter().any(|s| matches!(
            s.attributes(),
            ObjectSymbolAttributesV1::Coff {
                auxiliary: CoffSymbolAuxiliaryV1::Section {
                    selection: pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE,
                    associative_section: Some(_)
                },
                ..
            }
        )));
        let ObjectSymbolAttributesV1::Coff {
            auxiliary: CoffSymbolAuxiliaryV1::WeakExternal { default_symbol, .. },
            ..
        } = symbol(b"weak_alias").attributes()
        else {
            panic!("weak external fallback");
        };
        assert_eq!(
            envelope.symbols()[default_symbol as usize].name(),
            b"external"
        );
        let absolute = envelope
            .relocations()
            .iter()
            .filter(|r| {
                matches!(
                    r.shape(),
                    ObjectRelocationShapeV1::CoffAmd64 {
                        kind: pe::IMAGE_REL_AMD64_ADDR64,
                        ..
                    }
                )
            })
            .map(|r| r.encoded_value())
            .collect::<Vec<_>>();
        assert!(absolute.contains(&17));
        assert!(absolute.contains(&(-17i64 as u64)));
        assert!(envelope.relocations().iter().any(|r| matches!(
            r.shape(),
            ObjectRelocationShapeV1::CoffAmd64 {
                kind: pe::IMAGE_REL_AMD64_REL32,
                width: 4,
                ..
            }
        )));
        assert!(envelope.relocations().iter().any(|r| matches!(
            r.shape(),
            ObjectRelocationShapeV1::CoffAmd64 {
                kind: pe::IMAGE_REL_AMD64_ADDR32NB,
                width: 4,
                ..
            }
        )));
        for (index, symbol) in envelope.symbols().iter().enumerate() {
            assert_eq!(symbol.table_index(), index as u32);
        }
    }
}

#[test]
fn msvc_bigobj_uses_the_same_native_facts() {
    let objects = objects();
    assert_eq!(
        FileKind::parse(objects.big.as_slice()).unwrap(),
        FileKind::CoffBig
    );
    let envelope = validate_windows_coff_object_envelope_v1(
        &objects.big,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
    )
    .unwrap();
    let shared = envelope
        .symbols()
        .iter()
        .find(|symbol| symbol.name() == b"shared_value")
        .unwrap();
    assert_eq!(shared.kind(), ObjectSymbolKindV1::ExternalWeakDefinition);
    assert!(!envelope.relocations().is_empty());
}

#[test]
fn malformed_relocation_extent_and_auxiliary_targets_are_rejected() {
    let original = &objects().ordinary;
    let file = CoffFile::<&[u8]>::parse(original.as_slice()).unwrap();
    let section = file
        .sections()
        .find(|s| !s.coff_relocations().unwrap().is_empty())
        .unwrap();
    let offset = section
        .coff_section()
        .pointer_to_relocations
        .get(object::LittleEndian) as usize;
    let mut bytes = original.clone();
    bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let target = scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID;
    assert!(
        validate_windows_coff_object_envelope_v1(&bytes, target)
            .unwrap_err()
            .to_string()
            .contains("outside section")
    );
    let mut bytes = original.clone();
    bytes[offset + 4..offset + 8].copy_from_slice(&1u32.to_le_bytes());
    assert!(
        validate_windows_coff_object_envelope_v1(&bytes, target)
            .unwrap_err()
            .to_string()
            .contains("auxiliary symbol")
    );
    assert!(
        validate_windows_coff_object_envelope_v1(original, scoop_target_data::LINUX_X86_64_GNU_ID)
            .is_err()
    );
    let mut bytes = original.clone();
    bytes[..2].copy_from_slice(&pe::IMAGE_FILE_MACHINE_ARM64.to_le_bytes());
    assert!(validate_windows_coff_object_envelope_v1(&bytes, target).is_err());
}
