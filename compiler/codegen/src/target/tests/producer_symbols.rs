use super::*;
use inkwell::{context::Context, memory_buffer::MemoryBuffer, targets::FileType};
use object::{Object, ObjectComdat, ObjectSection, ObjectSymbol, RelocationTarget};

const END: &str = "scoop$1$be$0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const START: &str = "scoop$1$bs$0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn emit(
    selection: scoop_lir::ValidatedLirTargetSelection,
    end_reference: bool,
    big_coff: bool,
) -> Vec<u8> {
    let profile = ValidatedBackendProfile::for_selection(selection).unwrap();
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let referenced = if end_reference { END } else { START };
    let mut source = format!(
        r#"
@"{START}" = global i64 42
@"{END}" = global i64 0
@after = global ptr @"{referenced}"
@external = external global i64
@reference = global ptr @external
define i64 @read() uwtable {{
    %value = load i64, ptr @"{START}"
    ret i64 %value
}}
"#
    );
    let triple = machine.get_triple();
    let triple = triple.as_str().to_str().unwrap();
    if !triple.contains("apple") {
        source.push_str(
            "\n$odr = comdat any\ndefine linkonce_odr i32 @odr() comdat { ret i32 42 }\n",
        );
    }
    let module = context
        .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
            source.as_bytes(),
            "production-symbols",
        ))
        .unwrap();
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    if triple.contains("linux") {
        module.set_inline_assembly(".addrsig\n.addrsig_sym external\n");
    }
    if big_coff {
        let assembly = (0..66000)
            .map(|index| format!(".section .s{index:05},\"dw\"\n.byte 0\n"))
            .collect::<String>();
        module.set_inline_assembly(&assembly);
    }
    machine
        .write_to_memory_buffer(&module, FileType::Object)
        .unwrap()
        .as_slice()
        .to_vec()
}

fn relocations(file: &object::File<'_>) -> Vec<String> {
    file.sections()
        .flat_map(|section| {
            section.relocations().map(move |(offset, relocation)| {
                let target = match relocation.target() {
                    RelocationTarget::Symbol(index) => format!(
                        "symbol:{}",
                        file.symbol_by_index(index).unwrap().name().unwrap()
                    ),
                    target => format!("{target:?}"),
                };
                format!(
                    "{}:{offset}:{:?}:{target}:{}",
                    section.name().unwrap(),
                    relocation.flags(),
                    relocation.addend()
                )
            })
        })
        .collect()
}

#[test]
fn production_end_cleanup_preserves_native_definitions_sections_and_relocations() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::DARWIN_X86_64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1,
    ] {
        let original = emit(selection, false, false);
        let cleaned = scoop_slib::remove_production_end_symbols(&original).unwrap();
        assert!(cleaned.len() < original.len());
        assert!(
            !cleaned
                .windows(END.len())
                .any(|bytes| bytes == END.as_bytes())
        );
        assert_eq!(
            scoop_slib::remove_production_end_symbols(&cleaned).unwrap(),
            cleaned
        );
        let before = object::File::parse(original.as_slice()).unwrap();
        let after = object::File::parse(cleaned.as_slice()).unwrap();
        assert_eq!(relocations(&before), relocations(&after));
        let groups = |file: &object::File<'_>| {
            file.comdats()
                .map(|group| {
                    (
                        group.name().unwrap().to_owned(),
                        group.sections().collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(groups(&before), groups(&after));
        assert_eq!(
            address_significant_names(&before),
            address_significant_names(&after)
        );
        for symbol in before
            .symbols()
            .filter(|s| !s.name().unwrap().ends_with(END))
        {
            let retained = after
                .symbols()
                .find(|s| {
                    s.name_bytes().unwrap() == symbol.name_bytes().unwrap()
                        && s.section_index() == symbol.section_index()
                        && s.address() == symbol.address()
                })
                .unwrap();
            assert_eq!(
                (
                    symbol.address(),
                    symbol.size(),
                    symbol.section_index(),
                    symbol.flags()
                ),
                (
                    retained.address(),
                    retained.size(),
                    retained.section_index(),
                    retained.flags()
                )
            );
        }
        for section in before.sections() {
            let name = section.name().unwrap();
            if name.contains("strtab")
                || name == ".symtab"
                || name.starts_with(".rel")
                || name == ".llvm_addrsig"
            {
                continue;
            }
            let retained = after.section_by_index(section.index()).unwrap();
            assert_eq!(retained.name().unwrap(), name);
            assert_eq!(retained.data().unwrap(), section.data().unwrap(), "{name}");
        }
        let referenced = emit(selection, true, false);
        assert!(
            scoop_slib::remove_production_end_symbols(&referenced)
                .unwrap_err()
                .to_string()
                .contains("still referenced")
        );
    }
}

fn address_significant_names(file: &object::File<'_>) -> Vec<String> {
    file.section_by_name(".llvm_addrsig")
        .into_iter()
        .flat_map(|section| {
            section.data().unwrap().iter().map(|index| {
                // This small fixture has only single-byte ULEB128 indices.
                assert!(*index < 128);
                file.symbol_by_index(object::SymbolIndex(usize::from(*index)))
                    .unwrap()
                    .name()
                    .unwrap()
                    .to_owned()
            })
        })
        .collect()
}

#[test]
fn production_end_cleanup_preserves_big_coff_section_and_symbol_indices() {
    let original = emit(
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1,
        false,
        true,
    );
    assert_eq!(
        object::FileKind::parse(original.as_slice()).unwrap(),
        object::FileKind::CoffBig
    );
    let cleaned = scoop_slib::remove_production_end_symbols(&original).unwrap();
    let before = object::File::parse(original.as_slice()).unwrap();
    let after = object::File::parse(cleaned.as_slice()).unwrap();
    assert_eq!(relocations(&before), relocations(&after));
    assert_eq!(before.sections().count(), after.sections().count());
    assert_eq!(before.symbols().count(), after.symbols().count() + 1);
    assert!(cleaned.len() < original.len());
    assert!(
        !cleaned
            .windows(END.len())
            .any(|bytes| bytes == END.as_bytes())
    );
}
