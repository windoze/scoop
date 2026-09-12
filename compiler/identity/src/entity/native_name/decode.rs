use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CanonicalNativeLibraryName, CanonicalNativeNameError, SourceNativeSymbol,
    SourceNativeSymbolError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNativeLibraryName(String);

impl DecodedCanonicalNativeLibraryName {
    pub fn validate(self) -> Result<CanonicalNativeLibraryName, CanonicalNativeNameError> {
        CanonicalNativeLibraryName::from_owned(self.0)
    }
}

impl WireEncode for DecodedCanonicalNativeLibraryName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedCanonicalNativeLibraryName {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.owned_text().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceNativeSymbol(Vec<u8>);

impl DecodedSourceNativeSymbol {
    pub fn validate(self) -> Result<SourceNativeSymbol, SourceNativeSymbolError> {
        SourceNativeSymbol::from_owned(self.0)
    }
}

impl WireEncode for DecodedSourceNativeSymbol {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl WireDecode for DecodedSourceNativeSymbol {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.owned_bytes().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::{DecodedCanonicalNativeLibraryName, DecodedSourceNativeSymbol};
    use crate::{
        CanonicalNativeLibraryName, CanonicalNativeNameError, SourceNativeSymbol,
        SourceNativeSymbolError,
    };

    #[test]
    fn native_names_round_trip_through_validated_decoders() {
        let library = CanonicalNativeLibraryName::new("Résumé-库").unwrap();
        let decoded = decode_canonical::<DecodedCanonicalNativeLibraryName>(
            &encode(&library).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.validate().unwrap(), library);

        let symbol = SourceNativeSymbol::new("_入口").unwrap();
        let decoded = decode_canonical::<DecodedSourceNativeSymbol>(
            &encode(&symbol).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let allocation = decoded.0.as_ptr();
        let decoded = decoded.validate().unwrap();
        assert_eq!(decoded.as_bytes().as_ptr(), allocation);
        assert_eq!(decoded, symbol);
    }

    #[test]
    fn native_name_decoders_reject_noncanonical_values() {
        let library = decode_canonical::<DecodedCanonicalNativeLibraryName>(
            b"\x64a/bc",
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(
            library.validate(),
            Err(CanonicalNativeNameError::ForbiddenCharacter)
        );

        let invalid_utf8 =
            decode_canonical::<DecodedSourceNativeSymbol>(b"\x42\xc3\x28", DecodeLimits::default())
                .unwrap();
        assert_eq!(
            invalid_utf8.validate(),
            Err(SourceNativeSymbolError::InvalidUtf8)
        );

        let nul =
            decode_canonical::<DecodedSourceNativeSymbol>(b"\x43a\x00b", DecodeLimits::default())
                .unwrap();
        assert_eq!(nul.validate(), Err(SourceNativeSymbolError::Nul));
    }
}
