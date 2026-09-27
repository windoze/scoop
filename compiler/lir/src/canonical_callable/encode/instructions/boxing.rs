use super::*;

impl Writer<'_, '_> {
    pub(super) fn box_payload(&mut self, payload: &BoxPayload) -> Result {
        match payload {
            BoxPayload::ZeroSized(descriptor) => record!(self, 1; self.boxed_zst(descriptor)),
            BoxPayload::NonZero(place) => record!(self, 2; self.box_place(place)),
        }
    }

    pub(super) fn unbox_result(&mut self, result: &UnboxResult) -> Result {
        match result {
            UnboxResult::ZeroSized { descriptor, out } => {
                record!(self, 1; self.temp(*out), self.boxed_zst(descriptor))
            }
            UnboxResult::NonZero(place) => record!(self, 2; self.box_place(place)),
        }
    }

    fn boxed_zst(&mut self, descriptor: &BoxedZstDescriptor) -> Result {
        record!(self, 1; self.descriptor(descriptor.descriptor()), self.id(&descriptor.descriptor_exact()), self.logical_zst(descriptor.value()))
    }

    fn box_place(&mut self, place: &BoxValuePlace) -> Result {
        let descriptor = place.descriptor();
        record!(self, 1; self.local(place.local()), self.descriptor(descriptor.descriptor()), self.id(&descriptor.descriptor_exact()), self.id(&descriptor.payload_exact()), self.abi_value(descriptor.value()), self.box_rooting(place.rooting()))
    }

    fn box_rooting(&mut self, rooting: &BoxPayloadRooting) -> Result {
        match rooting {
            BoxPayloadRooting::GcFree => record!(self, 1;),
            BoxPayloadRooting::RecursiveRegion(scan) => {
                record!(self, 2; self.scan(scan.as_ref_scan()))
            }
        }
    }
}
