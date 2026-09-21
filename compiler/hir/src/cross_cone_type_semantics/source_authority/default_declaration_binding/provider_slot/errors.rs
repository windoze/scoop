use super::*;

#[derive(Debug)]
pub enum DefaultSourceProviderSlotError {
    Foundation(TypeFoundationBindingError),
    Encoding(scoop_wire::cbor::EncodeError),
    ProviderDeclaration,
    SlotCount {
        expected: usize,
        actual: usize,
    },
    MissingSlot(PersistentDispatchSlotId),
    RootSlot(PersistentDispatchSlotId),
    Witness {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
    },
}
impl std::fmt::Display for SlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::ProviderDeclaration => {
                f.write_str("default provider cannot declare this dispatch slot")
            }
            Self::SlotCount { expected, actual } => write!(
                f,
                "default provider requires {expected} original slots, found {actual}"
            ),
            Self::MissingSlot(slot) => write!(
                f,
                "default provider slot {slot} is missing from its artifact"
            ),
            Self::RootSlot(slot) => write!(
                f,
                "default provider is not the original declaration of slot {slot}"
            ),
            Self::Witness { kind, index } => write!(
                f,
                "default source {kind} occurrence {index} slot call domain differs from its provider declaration"
            ),
        }
    }
}
impl std::error::Error for SlotError {}
