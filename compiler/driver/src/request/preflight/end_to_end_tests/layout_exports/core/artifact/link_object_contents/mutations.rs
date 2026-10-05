use super::super::link_archive::symbol_offset;
use super::*;
use object::read::macho::{MachHeader as _, Segment as _};
use object::{Endianness, Object as _, ObjectSection as _, macho};

pub(super) fn cases(proof: &slib::ReplayedLayoutLinkObjectContentsV1) -> Vec<(Failure, Mutation)> {
    let mut cases = Vec::new();
    if let Some(bridge) = proof.generated_objects().next() {
        cases.push((
            Failure::CBridgeEnvelope,
            Mutation::Object {
                member: bridge.member(),
                offset: 4,
                value: bridge.bytes()[4] ^ 1,
            },
        ));
    }
    let byte = |member, offset: u64| {
        let object = proof
            .objects()
            .objects()
            .iter()
            .find(|object| object.member() == member)
            .unwrap();
        Mutation::Object {
            member,
            offset: offset as usize,
            value: object.final_bytes()[offset as usize] ^ 1,
        }
    };
    macro_rules! registration {
        ($access:ident, $failure:ident) => {
            if let Some(record) = proof.$access().registrations().first() {
                cases.push((
                    Failure::$failure,
                    byte(record.member(), record.checked_offset()),
                ));
            }
        };
    }
    registration!(callables, Callables);
    registration!(types, Types);
    registration!(immortals, Immortals);
    registration!(storages, Storages);
    registration!(initializations, Initializations);
    registration!(safepoints, Safepoints);
    let object = &proof.objects().objects()[0];
    cases.push((Failure::Envelope, byte(object.member(), 0)));
    let member = proof
        .patch_sites()
        .builtins()
        .strong_relocations()
        .members()
        .iter()
        .find(|member| member.member() == object.member())
        .unwrap();
    let symbol = &member.definitions().symbols()[0];
    let offset =
        symbol_offset(object.final_bytes(), symbol.table_index()) + symbol.macho_name().len() - 1;
    cases.push((Failure::Symbols, byte(object.member(), offset as u64)));
    for object in proof.objects().objects() {
        if let Some(offset) = relocation_field(object.final_bytes()) {
            cases.push((
                Failure::Relocations,
                Mutation::Object {
                    member: object.member(),
                    offset,
                    value: object.final_bytes()[offset] | 0xf0,
                },
            ));
        }
        let parsed = object::File::parse(object.final_bytes()).unwrap();
        if let Some(section) = parsed
            .sections()
            .find(|section| matches!(section.name(), Ok("__llvm_stackmaps" | ".llvm_stackmaps")))
        {
            cases.push((
                Failure::Stackmaps,
                byte(object.member(), section.file_range().unwrap().0),
            ));
        }
    }
    cases
}

fn relocation_field(bytes: &[u8]) -> Option<usize> {
    if object::FileKind::parse(bytes).unwrap() == object::FileKind::Elf64 {
        use object::read::elf::SectionHeader;
        let file = object::read::elf::ElfFile64::<Endianness>::parse(bytes).unwrap();
        let endian = file.endian();
        return file
            .elf_section_table()
            .enumerate()
            .find_map(|(_, section)| {
                (section.sh_type(endian) == object::elf::SHT_RELA && section.sh_size(endian) != 0)
                    .then(|| section.sh_offset(endian) as usize + 8)
            });
    }
    let header = macho::MachHeader64::<Endianness>::parse(bytes, 0).unwrap();
    let endian = header.endian().unwrap();
    let mut commands = header.load_commands(endian, bytes, 0).unwrap();
    while let Some(command) = commands.next().unwrap() {
        if let Some((segment, section_bytes)) = command.segment_64().unwrap() {
            for section in segment.sections(endian, section_bytes).unwrap() {
                if section.nreloc.get(endian) > 0 {
                    return Some(section.reloff.get(endian) as usize + 7);
                }
            }
        }
    }
    None
}
