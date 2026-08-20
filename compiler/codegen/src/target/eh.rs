/// Complete exception-handling contract qualified for one target/backend
/// profile.  These are capabilities rather than loosely related flags: a
/// target cannot reach codegen without selecting every part of its unwind,
/// LSDA and artifact-inspection ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EhProfile {
    pub(super) unwind_model: UnwindModel,
    pub(super) personality_abi: PersonalityAbi,
    pub(super) exception_data_registers: u8,
    pub(super) encodings: LsdaEncodingProfile,
    pub(super) unwind_provider: UnwindProvider,
    pub(super) artifact_inspection: EhArtifactInspection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnwindModel {
    ItaniumDwarf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PersonalityAbi {
    ScoopLsdaSubset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LsdaEncodingProfile {
    pub(crate) lp_start: u8,
    pub(crate) type_table: u8,
    pub(crate) call_site: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnwindProvider {
    DarwinLibSystem,
    LlvmLibunwind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EhArtifactInspection {
    MachO,
    Elf,
}

impl EhProfile {
    pub(super) const DARWIN_AARCH64: Self = Self {
        unwind_model: UnwindModel::ItaniumDwarf,
        personality_abi: PersonalityAbi::ScoopLsdaSubset,
        exception_data_registers: 2,
        encodings: LsdaEncodingProfile {
            lp_start: 0xff,
            type_table: 0x9b,
            call_site: 0x01,
        },
        unwind_provider: UnwindProvider::DarwinLibSystem,
        artifact_inspection: EhArtifactInspection::MachO,
    };

    pub(super) const LINUX_X86_64: Self = Self {
        unwind_provider: UnwindProvider::LlvmLibunwind,
        artifact_inspection: EhArtifactInspection::Elf,
        ..Self::DARWIN_AARCH64
    };

    pub(crate) fn encodings(self) -> LsdaEncodingProfile {
        self.encodings
    }
}
