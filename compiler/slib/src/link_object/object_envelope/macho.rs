use super::*;
use crate::link_object::{DarwinArm64SymbolKindV1, ValidatedDarwinArm64ObjectEnvelopeV1};

impl From<ValidatedDarwinArm64ObjectEnvelopeV1> for ValidatedObjectEnvelopeV1 {
    fn from(envelope: ValidatedDarwinArm64ObjectEnvelopeV1) -> Self {
        Self {
            byte_length: envelope.byte_length(),
            content_digest: envelope.content_digest(),
            format: ObjectEnvelopeFormatV1::DarwinArm64 {
                load_command_count: envelope.load_command_count(),
                deployment: envelope.deployment().cloned(),
            },
            sections: envelope
                .sections()
                .iter()
                .map(|s| ObservedObjectSectionV1 {
                    segment_name: s.segment_name().to_vec(),
                    section_name: s.section_name().to_vec(),
                    virtual_address: s.virtual_address(),
                    file_offset: s.file_offset(),
                    flags: ObjectSectionFlagsV1::MachO(s.flags()),
                    byte_size: s.byte_size(),
                    alignment_power: s.alignment_power(),
                })
                .collect(),
            symbols: envelope
                .symbols()
                .iter()
                .map(|s| ObservedObjectSymbolV1 {
                    table_index: s.table_index(),
                    name: s.name().to_vec(),
                    kind: match s.kind() {
                        DarwinArm64SymbolKindV1::LocalSectionDefinition => {
                            ObjectSymbolKindV1::LocalSectionDefinition
                        }
                        DarwinArm64SymbolKindV1::ExternalStrongDefinition => {
                            ObjectSymbolKindV1::ExternalStrongDefinition
                        }
                        DarwinArm64SymbolKindV1::ExternalWeakDefinition => {
                            ObjectSymbolKindV1::ExternalWeakDefinition
                        }
                        DarwinArm64SymbolKindV1::ExternalUndefined => {
                            ObjectSymbolKindV1::ExternalUndefined
                        }
                    },
                    section_ordinal: s.section_ordinal().map(NonZeroU32::from),
                    value: s.value(),
                    attributes: ObjectSymbolAttributesV1::MachO {
                        no_dead_strip: s.no_dead_strip(),
                        private_external: s.private_external(),
                    },
                })
                .collect(),
            relocations: envelope
                .relocations()
                .iter()
                .map(|r| ObservedObjectRelocationV1 {
                    containing_section_ordinal: r.containing_section_ordinal(),
                    offset: u64::from(r.offset()),
                    encoded_value: r.encoded_value(),
                    shape: ObjectRelocationShapeV1::DarwinArm64(r.shape()),
                })
                .collect(),
        }
    }
}
