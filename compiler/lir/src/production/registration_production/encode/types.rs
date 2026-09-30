use super::*;

impl<D: crate::StrongDescriptorReference, C: WireEncode> WireEncode
    for crate::StrongTypeRegistrationPlan<D, C>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(29)?;
        encode_field(encoder, 1, &self.exact_type())?;
        encode_field(encoder, 2, &self.runtime_type())?;
        encode_field(encoder, 3, &self.symbol())?;
        encode_field(encoder, 4, &self.definition_plan())?;
        encode_field(encoder, 5, &self.primary_atom())?;
        encode_field(encoder, 6, &self.descriptor_symbol())?;
        encode_field(encoder, 7, &self.descriptor_definition_plan())?;
        encode_field(encoder, 8, &self.descriptor_primary_atom())?;
        encode_field(encoder, 9, &self.layout())?;
        encode_field(encoder, 10, &self.layout_symbol())?;
        encode_field(encoder, 11, &self.layout_definition_plan())?;
        encode_field(encoder, 12, &self.layout_primary_atom())?;
        encode_field(encoder, 13, &self.registration_object_node())?;
        encode_field(encoder, 14, &self.descriptor_definition_node())?;
        encode_field(encoder, 15, &self.layout_fingerprint_node())?;
        encode_field(encoder, 16, &self.registration_fingerprint_node())?;
        encode_field(encoder, 17, &self.registration_definition_patch())?;
        encode_field(encoder, 18, &self.descriptor_definition_patch())?;
        encode_field(encoder, 19, &self.layout_fingerprint_patch())?;
        encoder.field(20)?;
        encoder.text(semantic.diagnostic_name())?;
        encode_field(encoder, 21, &semantic.instance_scan())?;
        encoder.field(22)?;
        encode_type_instance_shape(encoder, semantic.instance_shape())?;
        encoder.field(23)?;
        D::encode_optional(semantic.parent(), encoder)?;
        encoder.field(24)?;
        encode_type_vtable(encoder, semantic.vtable())?;
        encoder.field(25)?;
        encoder.array(semantic.itables().len() as u64)?;
        for itable in semantic.itables() {
            encode_type_itable(encoder, itable)?;
        }
        encode_field(encoder, 26, &self.diagnostic_atom())?;
        encoder.field(27)?;
        encode_type_descriptor_inline_scan(encoder, semantic.inline_scan())?;
        encoder.field(28)?;
        encode_type_descriptor_itable_directory(encoder, self.itable_directory())?;
        encoder.field(29)?;
        semantic
            .relations()
            .encode_with(encoder, |reference, encoder| {
                D::encode_optional(*reference, encoder)
            })
    }
}

fn encode_type_descriptor_itable_directory(
    encoder: &mut Encoder,
    directory: crate::TypeDescriptorITableDirectoryV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match directory {
        crate::TypeDescriptorITableDirectoryV1::Null => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encode_unsigned_field(encoder, 1, 0)
        }
        crate::TypeDescriptorITableDirectoryV1::Defined(atom) => {
            encode_value_sum(encoder, 2, &atom)
        }
    }
}

fn encode_type_descriptor_inline_scan(
    encoder: &mut Encoder,
    inline_scan: crate::TypeDescriptorInlineScanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match inline_scan {
        crate::TypeDescriptorInlineScanV1::Null => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encode_unsigned_field(encoder, 1, 0)
        }
        crate::TypeDescriptorInlineScanV1::Defined(scan) => encode_value_sum(encoder, 2, &scan),
    }
}

fn encode_type_instance_shape(
    encoder: &mut Encoder,
    shape: &crate::TypeInstanceShapeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(10)?;
    encode_unsigned_field(encoder, 1, u64::from(shape.instance_kind().tag()))?;
    encode_unsigned_field(encoder, 2, u64::from(shape.inline_storage_kind().tag()))?;
    encode_unsigned_field(encoder, 3, shape.minimum_size())?;
    encode_unsigned_field(encoder, 4, shape.instance_alignment())?;
    encode_unsigned_field(encoder, 5, shape.inline_offset())?;
    encode_unsigned_field(encoder, 6, shape.inline_size())?;
    encode_unsigned_field(encoder, 7, shape.inline_stride())?;
    encode_unsigned_field(encoder, 8, shape.inline_alignment())?;
    encoder.field(9)?;
    encode_ref_scan(encoder, shape.object_scan())?;
    encoder.field(10)?;
    encode_ref_scan(encoder, shape.inline_scan())
}

fn encode_type_vtable<C: WireEncode>(
    encoder: &mut Encoder,
    vtable: &crate::StrongTypeVtableSemanticPlan<C>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_field(encoder, 1, &vtable.table())?;
    encoder.field(2)?;
    encoder.array(vtable.slots().len() as u64)?;
    for slot in vtable.slots() {
        slot.encode(encoder)?;
    }
    Ok(())
}

fn encode_type_itable<D: crate::StrongDescriptorReference, C: WireEncode>(
    encoder: &mut Encoder,
    itable: &crate::StrongTypeItableSemanticPlan<D, C>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_field(encoder, 1, &itable.table())?;
    encoder.field(2)?;
    itable.interface().encode(encoder)?;
    encoder.field(3)?;
    encoder.array(itable.slots().len() as u64)?;
    for slot in itable.slots() {
        slot.encode(encoder)?;
    }
    Ok(())
}
