use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_enum(
        &mut self,
        target: concrete::TypeId,
        id: concrete::EnumId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let shape = self.metadata_shape(target).unwrap();
        if let export::EnumRepresentation::Integral { kind, values } =
            self.enums[id].representation.clone()
        {
            let underlying = self.lower_integer_type(kind, &[]);
            let underlying = self.type_reference_value(underlying, fields[0], origin);
            let record = self.metadata_element(fields[1]);
            let variant_fields = self.metadata_fields(record);
            let variants = shape
                .variants()
                .zip(values)
                .enumerate()
                .map(|(index, (variant, bits))| {
                    let id = self.metadata_identity(
                        variant_fields[0],
                        format!("variant:{}", variant.identity()),
                        origin,
                    );
                    let owner = self.type_reference_value(target, variant_fields[1], origin);
                    let name = self.metadata_string(variant.name().to_owned(), origin);
                    let annotations = self.metadata_annotations(
                        variant.annotations(self.dependencies).iter(),
                        variant_fields[3],
                        origin,
                    );
                    let index = self.metadata_index(index, origin);
                    let value =
                        self.metadata_integer(integer_bits(kind, bits), variant_fields[5], origin);
                    self.metadata_record(
                        record,
                        vec![id, owner, name, annotations, index, value],
                        origin,
                    )
                })
                .collect();
            let variants = self.metadata_array(fields[1], variants, origin);
            let interfaces = self.metadata_interfaces(target, shape, fields[2], origin);
            return self.metadata_record(result, vec![underlying, variants, interfaces], origin);
        }
        let record = self.metadata_element(fields[0]);
        let variants = shape
            .variants()
            .enumerate()
            .map(|(index, variant)| {
                self.metadata_variant_info(target, variant, index, record, origin)
            })
            .collect();
        let variants = self.metadata_array(fields[0], variants, origin);
        let interfaces = self.metadata_interfaces(target, shape, fields[1], origin);
        self.metadata_record(result, vec![variants, interfaces], origin)
    }

    fn metadata_variant_info(
        &mut self,
        owner: concrete::TypeId,
        variant: export::StaticVariantShape<'_>,
        index: usize,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let id =
            self.metadata_identity(fields[0], format!("variant:{}", variant.identity()), origin);
        let owner_value = self.type_reference_value(owner, fields[1], origin);
        let name = self.metadata_string(variant.name().to_owned(), origin);
        let annotations = self.metadata_annotations(
            variant.annotations(self.dependencies).iter(),
            fields[3],
            origin,
        );
        let index = self.metadata_index(index, origin);
        let constructor = variant.constructor(self.dependencies);
        let payload = if variant.style() == export::VariantStyle::Unit {
            self.metadata_case(fields[5], "Unit", Vec::new(), origin)
        } else {
            let positional = variant.style() == export::VariantStyle::Positional;
            let name = if positional { "Positional" } else { "Named" };
            let array = self.metadata_variant(fields[5], name).1[0];
            let record = self.metadata_element(array);
            let values = variant
                .fields()
                .enumerate()
                .map(|(index, field)| {
                    self.metadata_field(
                        owner,
                        field,
                        (index, positional),
                        Some(constructor),
                        record,
                        origin,
                    )
                })
                .collect();
            let values = self.metadata_array(array, values, origin);
            self.metadata_case(fields[5], name, vec![values], origin)
        };
        let constructor = self.metadata_constructor(owner, constructor, fields[6], origin);
        self.metadata_record(
            result,
            vec![
                id,
                owner_value,
                name,
                annotations,
                index,
                payload,
                constructor,
            ],
            origin,
        )
    }
}

fn integer_bits(kind: export::IntegerKind, bits: u64) -> export::HirIntegerConstant {
    use export::{HirIntegerConstant as V, IntegerKind as K};
    match kind {
        K::SIGNED_8 => V::Signed8(bits as u8),
        K::SIGNED_16 => V::Signed16(bits as u16),
        K::SIGNED_32 => V::Signed32(bits as u32),
        K::SIGNED_64 => V::Signed64(bits),
        K::UNSIGNED_8 => V::Unsigned8(bits as u8),
        K::UNSIGNED_16 => V::Unsigned16(bits as u16),
        K::UNSIGNED_32 => V::Unsigned32(bits as u32),
        K::UNSIGNED_64 => V::Unsigned64(bits),
    }
}
