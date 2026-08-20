//! Signature references in shared nominal declarations.

use super::*;

impl CrossConeHirInterfaceSectionV1 {
    pub(super) fn visit_nominal_signatures<A, E>(
        &self,
        validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let table_path = path.clone().field(2);
        for (record_index, (partition, wire_index, record)) in
            self.nominal_interfaces().wire_records().enumerate()
        {
            let record_path = table_path.clone().field(partition).index(wire_index as u64);
            visit_binder_signatures(
                validator,
                record.type_parameters(),
                &record_path.clone().field(3),
                |binder_index, bound| ExternalHirSignatureUseSiteV1::NominalTypeParameter {
                    record_index,
                    binder_index,
                    bound,
                },
            )?;

            for (signature_index, signature) in
                record.exact_supertypes().values().iter().enumerate()
            {
                validator.visit_signature(
                    signature,
                    ExternalHirSignatureUseSiteV1::NominalSupertype {
                        record_index,
                        signature_index,
                    },
                    &record_path.clone().field(4).index(signature_index as u64),
                )?;
            }

            for (selection_index, selection) in record
                .declaration_details()
                .dispatch_selections()
                .records()
                .iter()
                .enumerate()
            {
                validator.visit_signature(
                    selection.receiver(),
                    ExternalHirSignatureUseSiteV1::NominalDispatchReceiver {
                        record_index,
                        selection_index,
                    },
                    &record_path
                        .clone()
                        .field(9)
                        .field(7)
                        .index(selection_index as u64)
                        .field(3),
                )?;
                if let crate::NominalDispatchSelectionRoleV1::Interface { interface } =
                    selection.role()
                {
                    validator.visit_signature(
                        interface,
                        ExternalHirSignatureUseSiteV1::NominalDispatchInterface {
                            record_index,
                            selection_index,
                        },
                        &record_path
                            .clone()
                            .field(9)
                            .field(7)
                            .index(selection_index as u64)
                            .field(0)
                            .field(1),
                    )?;
                }
            }

            for (field_index, field) in record.source_shape().declared_fields().iter().enumerate() {
                validator.visit_signature(
                    field.value_type(),
                    ExternalHirSignatureUseSiteV1::NominalField {
                        record_index,
                        field_index,
                    },
                    &record_path
                        .clone()
                        .field(8)
                        .field(record.source_shape().declared_fields_wire_field())
                        .index(field_index as u64)
                        .field(2),
                )?;
            }
            match record.source_shape() {
                NominalSourceShapeV1::Enum(shape) => {
                    for (variant_index, variant) in shape.variants().iter().enumerate() {
                        for (field_index, field) in variant.fields().iter().enumerate() {
                            validator.visit_signature(
                                field.value_type(),
                                ExternalHirSignatureUseSiteV1::NominalEnumField {
                                    record_index,
                                    variant_index,
                                    field_index,
                                },
                                &record_path
                                    .clone()
                                    .field(8)
                                    .field(1)
                                    .index(variant_index as u64)
                                    .field(3)
                                    .index(field_index as u64)
                                    .field(2),
                            )?;
                        }
                    }
                }
                NominalSourceShapeV1::Struct(_)
                | NominalSourceShapeV1::Class(_)
                | NominalSourceShapeV1::Interface
                | NominalSourceShapeV1::Object(_)
                | NominalSourceShapeV1::Intrinsic(_) => continue,
            }
        }
        Ok(())
    }
}
