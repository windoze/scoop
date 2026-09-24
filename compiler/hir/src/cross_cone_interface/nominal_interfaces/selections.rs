//! Actual dispatch choices owned by the shared source nominal declaration.

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{BudgetMeter, Encoder, WireEncode, WirePath};

use crate::InheritanceSourceSlotSelectionV1;

mod decode;
pub use decode::{
    DecodedCanonicalNominalDispatchSelectionsV1, NominalDispatchSelectionResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominalDispatchSelectionV1 {
    slot: PersistentDispatchSlotId,
    selection: InheritanceSourceSlotSelectionV1,
}

impl NominalDispatchSelectionV1 {
    pub const fn new(
        slot: PersistentDispatchSlotId,
        selection: InheritanceSourceSlotSelectionV1,
    ) -> Self {
        Self { slot, selection }
    }

    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }

    pub const fn selection(&self) -> InheritanceSourceSlotSelectionV1 {
        self.selection
    }

    pub fn callable_target(&self) -> Option<scoop_identity::CallableTemplateOrigin> {
        use crate::InheritanceCallableDeclarationV1 as Declaration;
        use scoop_identity::CallableTemplateOrigin as Target;
        let declaration = match self.selection {
            InheritanceSourceSlotSelectionV1::Abstract => return None,
            InheritanceSourceSlotSelectionV1::Concrete(declaration)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(declaration) => declaration,
        };
        Some(match declaration {
            Declaration::Function(id) => Target::Function(id),
            Declaration::Getter(id) | Declaration::Setter(id) => Target::Accessor(id),
        })
    }
}

impl WireEncode for NominalDispatchSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.selection.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNominalDispatchSelectionsV1 {
    records: Vec<NominalDispatchSelectionV1>,
}

impl CanonicalNominalDispatchSelectionsV1 {
    pub const fn empty() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn try_new(
        mut records: Vec<NominalDispatchSelectionV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, NominalDispatchSelectionError> {
        let count = records.len() as u64;
        let path = WirePath::root();
        meter.check_table_entries(count, &path)?;
        meter.charge_work(
            count.saturating_mul(1 + u64::from(count.max(1).ilog2())),
            &path,
        )?;
        records.sort_unstable_by_key(NominalDispatchSelectionV1::slot);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<NominalDispatchSelectionV1>,
    ) -> Result<Self, NominalDispatchSelectionError> {
        for (index, pair) in records.windows(2).enumerate() {
            match pair[0].slot.cmp(&pair[1].slot) {
                std::cmp::Ordering::Equal => {
                    return Err(NominalDispatchSelectionError::Duplicate(pair[1].slot));
                }
                std::cmp::Ordering::Greater => {
                    return Err(NominalDispatchSelectionError::NonCanonicalOrder {
                        index: index + 1,
                    });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[NominalDispatchSelectionV1] {
        &self.records
    }
}

impl WireEncode for CanonicalNominalDispatchSelectionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum NominalDispatchSelectionError {
    Duplicate(PersistentDispatchSlotId),
    NonCanonicalOrder { index: usize },
    Resource(scoop_wire::WireError),
}

impl From<scoop_wire::WireError> for NominalDispatchSelectionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for NominalDispatchSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(slot) => write!(f, "duplicate nominal dispatch selection for {slot:?}"),
            Self::NonCanonicalOrder { index } => write!(
                f,
                "non-canonical nominal dispatch selection at index {index}"
            ),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for NominalDispatchSelectionError {}
