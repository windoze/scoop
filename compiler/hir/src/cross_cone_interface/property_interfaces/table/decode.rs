use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalPropertyInterfacesV1 {
    records: Vec<DecodedPropertyInterfaceRecordV1>,
    support: Vec<DecodedPropertyDeclarationRecordV1>,
}

impl DecodedCanonicalPropertyInterfacesV1 {
    pub fn resolve<R: PropertyInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalPropertyInterfacesV1, PropertyInterfaceSetValidationError<E>> {
        let records = self
            .records
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                record
                    .resolve(resolver)
                    .map_err(|error| PropertyInterfaceSetValidationError::Record { index, error })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let support = self
            .support
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                record.resolve(resolver).map_err(|error| {
                    PropertyInterfaceSetValidationError::SupportRecord { index, error }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ordered(records.iter().map(|r| r.declaration()))?;
        ordered(support.iter().map(|r| r.declaration()))?;
        for record in &support {
            if records
                .binary_search_by_key(
                    &record.declaration(),
                    PropertyInterfaceRecordV1::declaration,
                )
                .is_ok()
            {
                return Err(PropertyInterfaceSetValidationError::DuplicateSupport(
                    record.declaration(),
                ));
            }
        }
        Ok(CanonicalPropertyInterfacesV1 { records, support })
    }
}

fn ordered<E>(
    ids: impl Iterator<Item = PropertyDeclarationId>,
) -> Result<(), PropertyInterfaceSetValidationError<E>> {
    let mut previous: Option<PropertyDeclarationId> = None;
    for (index, declaration) in ids.enumerate() {
        if let Some(previous) = previous {
            match previous.cmp(&declaration) {
                std::cmp::Ordering::Equal => {
                    return Err(PropertyInterfaceSetValidationError::DuplicateDeclaration {
                        index,
                        declaration,
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(PropertyInterfaceSetValidationError::NonCanonicalOrder { index });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        previous = Some(declaration);
    }
    Ok(())
}

impl WireEncode for DecodedCanonicalPropertyInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(self.support.len() as u64)?;
        for record in &self.support {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
impl WireDecode for DecodedCanonicalPropertyInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let value = Self {
            records: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedPropertyInterfaceRecordV1::decode(d))
            })?,
            support: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedPropertyDeclarationRecordV1::decode(d))
            })?,
        };
        let count = (value.records.len() + value.support.len()) as u64;
        let path = decoder.path().clone();
        decoder.meter().charge_collection_slots(count, &path)?;
        decoder
            .meter()
            .charge_work(count.saturating_mul(128), &path)?;
        Ok(value)
    }
}
