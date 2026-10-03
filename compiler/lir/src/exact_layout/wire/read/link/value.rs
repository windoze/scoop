use super::*;
use crate::{EnumLayoutFieldInputV1, EnumLayoutVariantInputV1, NominalLayoutFieldInputV1};

impl LayoutReader<'_> {
    pub(super) fn value(
        &mut self,
        identity: ExactLayoutIdentityV1,
        raw: &RawValue,
    ) -> Result<ExactValueLayoutV1, LinkDataError> {
        let value = match raw {
            RawValue::Scalar(kind) => ExactValueLayoutV1::scalar(identity, *kind, self.foundation),
            RawValue::QualifiedPointer(kind) => {
                ExactValueLayoutV1::qualified_pointer(identity, *kind, self.foundation)
            }
            RawValue::Unit => ExactValueLayoutV1::unit(identity, self.foundation),
            RawValue::Tuple(elements) => {
                let values = elements
                    .iter()
                    .map(|element| self.field_value(element.link_storage()))
                    .collect::<Result<Vec<_>, _>>()?;
                ExactValueLayoutV1::tuple(
                    identity,
                    &values.iter().map(|value| &value.value).collect::<Vec<_>>(),
                    self.foundation,
                )
            }
            RawValue::Struct {
                policy,
                interior_mutable,
                fields,
            } => {
                let values = self.nominal_fields(fields)?;
                let fields = values
                    .iter()
                    .map(|(field, value)| NominalLayoutFieldInputV1 {
                        field,
                        value: &value.value,
                    })
                    .collect::<Vec<_>>();
                match policy {
                    RawPolicy::Ordinary => ExactValueLayoutV1::ordinary_struct(
                        identity,
                        *interior_mutable,
                        &fields,
                        self.foundation,
                    ),
                    RawPolicy::CLayout { contract, .. } => {
                        let contract = self.identities.resolve(*contract).map_err(link_error)?;
                        let contract = self
                            .foundation
                            .c_abi_layouts()
                            .iter()
                            .find(|record| record.fingerprint() == contract)
                            .ok_or_else(|| {
                                LinkDataError(format!("missing C layout contract {contract}"))
                            })?;
                        let nested = contract
                            .layout()
                            .fields()
                            .iter()
                            .filter_map(|field| match field.storage() {
                                scoop_identity::CanonicalCStorageType::Struct {
                                    exact_type,
                                    ..
                                } => Some(exact_type),
                                _ => None,
                            })
                            .map(|exact| self.value_by_id(value_layout_id(self.target, exact)?))
                            .collect::<Result<Vec<_>, LinkDataError>>()?;
                        let nested = nested.iter().map(AsRef::as_ref).collect::<Vec<_>>();
                        ExactValueLayoutV1::c_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            contract,
                            &nested,
                            self.foundation,
                        )
                    }
                }
            }
            RawValue::TaggedEnum { variants, .. } => {
                return self.enumeration(
                    identity,
                    &variants.iter().map(|v| &v.variant).collect::<Vec<_>>(),
                );
            }
            RawValue::NicheEnum { variants, .. } => {
                return self.enumeration(identity, &variants.iter().collect::<Vec<_>>());
            }
        };
        value.map_err(link_error)
    }

    fn enumeration(
        &mut self,
        identity: ExactLayoutIdentityV1,
        variants: &[&RawVariant],
    ) -> Result<ExactValueLayoutV1, LinkDataError> {
        let mut records = Vec::new();
        let mut fields = Vec::new();
        for variant in variants {
            let id = self.identities.resolve(variant.id).map_err(link_error)?;
            records.push(self.identities.canonical_record(id).map_err(link_error)?);
            let mut values = Vec::new();
            for field in &variant.fields {
                let id = self.identities.resolve(field.id).map_err(link_error)?;
                values.push((
                    self.identities.canonical_record(id).map_err(link_error)?,
                    self.field_value(&field.storage)?,
                ));
            }
            fields.push(values);
        }
        let inputs = fields
            .iter()
            .map(|fields| {
                fields
                    .iter()
                    .map(|(field, layout)| EnumLayoutFieldInputV1 {
                        field,
                        value: &layout.value,
                        pointer_kind: layout.pointer,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let variants = records
            .iter()
            .zip(&inputs)
            .map(|(variant, fields)| EnumLayoutVariantInputV1 { variant, fields })
            .collect::<Vec<_>>();
        ExactValueLayoutV1::enumeration(identity, &variants, self.foundation).map_err(link_error)
    }
}
