//! Metadata pointer semantics without losing the native relocation form.

use super::{VerifiedObjectRelocationFormV1 as Form, VerifiedObjectRelocationShapeV1 as Shape};

impl Form {
    pub fn is_absolute64(self) -> bool {
        matches!(
            self,
            Self::Unsigned64
                | Self::ElfRela {
                    kind: object::elf::R_X86_64_64,
                    width: 8,
                    ..
                }
        )
    }

    /// Mach-O stores the addend in-place; ELF RELA stores it in the relocation.
    pub fn absolute64_addend(self, encoded_field: u64) -> Option<i128> {
        match self {
            Self::Unsigned64 => Some(i128::from(encoded_field)),
            Self::ElfRela {
                kind: object::elf::R_X86_64_64,
                width: 8,
                addend,
            } => Some(i128::from(addend)),
            _ => None,
        }
    }

    pub fn absolute64_address(self, encoded_field: u64, base: u64) -> Option<u64> {
        u64::try_from(i128::from(base) + self.absolute64_addend(encoded_field)?).ok()
    }
}

impl Shape {
    /// Resolve an absolute pointer within this object, in section coordinates.
    pub fn absolute64_local_address(
        &self,
        encoded_field: u64,
    ) -> Option<(std::num::NonZeroU32, u64)> {
        use super::VerifiedRelocationTargetV1 as Target;
        let (section, base) = match self.absolute64_target()? {
            Target::LocalDefinition {
                section_ordinal,
                value,
                ..
            } => (*section_ordinal, *value),
            Target::SectionBase {
                section_ordinal, ..
            } => (*section_ordinal, 0),
            _ => return None,
        };
        Some((
            section,
            self.form().absolute64_address(encoded_field, base)?,
        ))
    }

    pub fn absolute64_target(&self) -> Option<&super::VerifiedRelocationTargetV1> {
        match self {
            Self::Unsigned64 { target }
            | Self::ElfRela {
                kind: object::elf::R_X86_64_64,
                width: 8,
                target,
                ..
            } => Some(target),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rela_signed_addends_are_separate_from_the_relocation_field() {
        let absolute = |addend| Form::ElfRela {
            kind: object::elf::R_X86_64_64,
            addend,
            width: 8,
        };
        assert_eq!(absolute(-8).absolute64_address(0, 24), Some(16));
        assert_eq!(absolute(17).absolute64_address(0, 24), Some(41));
        assert_eq!(absolute(0).absolute64_addend(17), Some(0));
        assert_eq!(absolute(-8).absolute64_address(0, 0), None);
        assert_eq!(absolute(1).absolute64_address(0, u64::MAX), None);
        assert_eq!(Form::Unsigned64.absolute64_address(17, 24), Some(41));
        assert_eq!(
            Form::Unsigned64.absolute64_address(0, u64::MAX),
            Some(u64::MAX)
        );
        for kind in [
            object::elf::R_X86_64_PC64,
            object::elf::R_X86_64_GOT64,
            object::elf::R_X86_64_TPOFF64,
        ] {
            assert!(
                !Form::ElfRela {
                    kind,
                    addend: 0,
                    width: 8
                }
                .is_absolute64()
            );
        }
    }
}
