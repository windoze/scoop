use super::*;

#[derive(Clone, Copy)]
pub(super) enum Component {
    Position,
    Signature,
    Implementation,
    Abi,
}

pub(super) struct Modified<'a> {
    pub table: &'a lir::ExactDispatchExportV1,
    pub component: Component,
}
impl WireEncode for Modified<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.table.table().encode(encoder)?;
        encoder.field(2)?;
        self.table.owner_exact().encode(encoder)?;
        encoder.field(3)?;
        self.table.role().encode(encoder)?;
        encoder.field(4)?;
        encoder.array(self.table.entries().len() as u64)?;
        for (index, entry) in self.table.entries().iter().enumerate() {
            if index == 0 {
                encode_entry(entry, self.component, encoder)?;
            } else {
                entry.encode(encoder)?;
            }
        }
        encoder.field(5)?;
        self.table.definition().encode(encoder)
    }
}

fn encode_entry(
    entry: &lir::ExactDispatchEntryV1,
    component: Component,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(5)?;
    encoder.field(1)?;
    encoder.unsigned(
        u64::from(entry.position().into_u32())
            + u64::from(matches!(component, Component::Position)),
    )?;
    encoder.field(2)?;
    entry.slot().encode(encoder)?;
    encoder.field(3)?;
    if matches!(component, Component::Signature) {
        let gc = match entry.slot_signature().gc_effect() {
            scoop_identity::GcEffect::Managed => scoop_identity::GcEffect::NoGc,
            scoop_identity::GcEffect::NoGc => scoop_identity::GcEffect::Managed,
        };
        lir::ExactDispatchSlotSignatureV1::new(entry.slot_signature().exact().clone(), gc)
            .encode(encoder)?;
    } else {
        entry.slot_signature().encode(encoder)?;
    }
    encoder.field(4)?;
    let implementation = if matches!(component, Component::Implementation) {
        match entry.implementation() {
            lir::ExactDispatchImplementationV1::AdjustThunkTarget(target) => {
                lir::ExactDispatchImplementationV1::DirectStrongTarget {
                    target,
                    receiver: lir::ExactDispatchReceiverAdaptationV1::Identity,
                }
            }
            other => lir::ExactDispatchImplementationV1::AdjustThunkTarget(other.target()),
        }
    } else {
        entry.implementation()
    };
    implementation.encode(encoder)?;
    encoder.field(5)?;
    let abi = if matches!(component, Component::Abi) {
        match entry.abi() {
            lir::StrongTypeDispatchCallableRefV2::Local(body) => {
                lir::StrongTypeDispatchCallableRefV2::DependencyExternal {
                    provider: ConeIdentity::CORE,
                    body,
                }
            }
            lir::StrongTypeDispatchCallableRefV2::DependencyExternal { body, .. } => {
                lir::StrongTypeDispatchCallableRefV2::Local(body)
            }
            lir::StrongTypeDispatchCallableRefV2::Runtime(_) => {
                panic!("a source dispatch entry has a checked callable ABI")
            }
        }
    } else {
        entry.abi()
    };
    abi.encode(encoder)
}
