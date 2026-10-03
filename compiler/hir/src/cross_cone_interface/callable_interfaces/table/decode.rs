use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCallableInterfacesV1 {
    records: Vec<DecodedCallableInterfaceRecordV1>,
    support: Vec<DecodedCallableDeclarationRecordV1>,
}

impl DecodedCanonicalCallableInterfacesV1 {
    pub fn resolve<R: CallableInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCallableInterfacesV1, CallableInterfaceSetValidationError<E>> {
        let records = self
            .records
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                record
                    .resolve(resolver)
                    .map_err(|error| CallableInterfaceSetValidationError::Record { index, error })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let support = self
            .support
            .into_iter()
            .enumerate()
            .map(|(index, record)| {
                record.resolve(resolver).map_err(|error| {
                    CallableInterfaceSetValidationError::SupportRecord { index, error }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ordered(records.iter().map(|r| r.declaration()))?;
        ordered(support.iter().map(|r| r.declaration()))?;
        for record in &support {
            if records
                .binary_search_by_key(
                    &record.declaration(),
                    CallableInterfaceRecordV1::declaration,
                )
                .is_ok()
            {
                return Err(CallableInterfaceSetValidationError::DuplicateSupport(
                    record.declaration(),
                ));
            }
        }
        Ok(CanonicalCallableInterfacesV1 { records, support })
    }
}

fn ordered<E>(
    ids: impl Iterator<Item = CallableDeclarationId>,
) -> Result<(), CallableInterfaceSetValidationError<E>> {
    let mut previous: Option<CallableDeclarationId> = None;
    for (index, declaration) in ids.enumerate() {
        if let Some(previous) = previous {
            match previous.cmp(&declaration) {
                std::cmp::Ordering::Equal => {
                    return Err(CallableInterfaceSetValidationError::DuplicateDeclaration {
                        index,
                        declaration,
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(CallableInterfaceSetValidationError::NonCanonicalOrder { index });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        previous = Some(declaration);
    }
    Ok(())
}

impl WireEncode for DecodedCanonicalCallableInterfacesV1 {
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
impl WireDecode for DecodedCanonicalCallableInterfacesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let value = Self {
            records: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedCallableInterfaceRecordV1::decode(d))
            })?,
            support: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedCallableDeclarationRecordV1::decode(d))
            })?,
        };

        Ok(value)
    }
}
