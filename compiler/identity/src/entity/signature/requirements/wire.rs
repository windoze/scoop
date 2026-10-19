use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl<T: WireEncode> WireEncode for MemberRequirementKey<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        match &self.call {
            RequiredMemberKey::Constructor => {
                encoder.array(0)?;
            }
            RequiredMemberKey::Method {
                name,
                type_arguments,
            } => {
                encoder.array(2)?;
                encoder.text(name)?;
                encoder.array(type_arguments.len() as u64)?;
                for ty in type_arguments {
                    encoder.array(u64::from(ty.is_some()))?;
                    if let Some(ty) = ty {
                        ty.encode(encoder)?;
                    }
                }
            }
        }
        encoder.field(3)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            encoder.array(3)?;
            encoder.array(u64::from(argument.name.is_some()))?;
            if let Some(name) = &argument.name {
                encoder.text(name)?;
            }
            encoder.unsigned(u64::from(argument.spread))?;
            argument.ty.encode(encoder)?;
        }
        encoder.field(4)?;
        self.result.encode(encoder)?;
        encoder.field(5)?;
        encoder.unsigned(
            u64::from(self.effects.is_suspend)
                | (u64::from(self.effects.is_unsafe) << 1)
                | (u64::from(self.effects.is_no_gc) << 2),
        )
    }
}

impl<T: WireDecode> WireDecode for MemberRequirementKey<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        expect_length(decoder, length, 5)?;
        let subject = decoder.field(1, T::decode)?;
        let call = decoder.field(2, |decoder| {
            let length = decoder.array()?;
            if length == 0 {
                return Ok(RequiredMemberKey::Constructor);
            }
            expect_length(decoder, length, 2)?;
            Ok(RequiredMemberKey::Method {
                name: decoder.owned_text()?,
                type_arguments: decoder.decode_array(|decoder, _| optional(decoder, T::decode))?,
            })
        })?;
        let arguments = decoder.field(3, |decoder| {
            decoder.decode_array(|decoder, _| {
                let length = decoder.array()?;
                expect_length(decoder, length, 3)?;
                Ok(RequiredArgumentKey {
                    name: optional(decoder, |decoder| decoder.owned_text())?,
                    spread: flag(decoder)?,
                    ty: T::decode(decoder)?,
                })
            })
        })?;
        let result = decoder.field(4, T::decode)?;
        let flags = decoder.field(5, Decoder::unsigned)?;
        if flags > 7 {
            return Err(error(decoder, WireErrorKind::UnknownTag { tag: flags }));
        }
        Ok(Self {
            subject,
            call,
            arguments,
            result,
            effects: RequiredEffectsKey {
                is_suspend: flags & 1 != 0,
                is_unsafe: flags & 2 != 0,
                is_no_gc: flags & 4 != 0,
            },
        })
    }
}

fn optional<T>(
    decoder: &mut Decoder<'_>,
    read: impl FnOnce(&mut Decoder<'_>) -> Result<T, WireError>,
) -> Result<Option<T>, WireError> {
    let length = decoder.array()?;
    if length == 0 {
        return Ok(None);
    }
    expect_length(decoder, length, 1)?;
    read(decoder).map(Some)
}

fn flag(decoder: &mut Decoder<'_>) -> Result<bool, WireError> {
    match decoder.unsigned()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn expect_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
