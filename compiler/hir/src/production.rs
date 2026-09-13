//! Canonical HIR production contracts shared by ordinary and core Cones.

use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentExportBindingId, PersistentFunctionId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CanonicalHirFoundation, ConeOutputKind, ExportHir, ValidatedHirFoundation};

mod core_prelude;
pub use core_prelude::*;
mod core_well_known;
pub use core_well_known::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOutputContractV1 {
    Library,
    ExecutableSourceEntry(PersistentFunctionId),
}

impl HirOutputContractV1 {
    pub const fn from_output_kind(output: &ConeOutputKind) -> Self {
        match output {
            ConeOutputKind::Library => Self::Library,
            ConeOutputKind::Executable { local_entry } => {
                Self::ExecutableSourceEntry(local_entry.declaration())
            }
        }
    }
}

impl WireEncode for HirOutputContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::ExecutableSourceEntry(entry) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                entry.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedHirOutputContractV1 {
    Library,
    ExecutableSourceEntry(DecodedPersistentId<PersistentFunctionId>),
}

impl DecodedHirOutputContractV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<HirOutputContractV1, HirOutputContractValidationError> {
        self.validate_against(foundation.canonical())
    }

    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<HirOutputContractV1, HirOutputContractValidationError> {
        match self {
            Self::Library => Ok(HirOutputContractV1::Library),
            Self::ExecutableSourceEntry(decoded) => foundation
                .function_id_by_bytes(decoded.as_array())
                .map(HirOutputContractV1::ExecutableSourceEntry)
                .ok_or(HirOutputContractValidationError::UnknownEntry(
                    *decoded.as_array(),
                )),
        }
    }
}

impl WireEncode for DecodedHirOutputContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::ExecutableSourceEntry(entry) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                entry.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedHirOutputContractV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 1)?;
                Ok(Self::Library)
            }
            2 => {
                require_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::ExecutableSourceEntry)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOutputContractValidationError {
    UnknownEntry([u8; 32]),
}

impl fmt::Display for HirOutputContractValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEntry(id) => write!(
                formatter,
                "HIR executable source entry {} is absent from the identity foundation",
                HexIdentity(id)
            ),
        }
    }
}

impl std::error::Error for HirOutputContractValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDirectPublicSurfaceV1 {
    bindings: Vec<PersistentExportBindingId>,
}

impl CanonicalDirectPublicSurfaceV1 {
    pub fn try_new(
        mut bindings: Vec<PersistentExportBindingId>,
    ) -> Result<Self, DirectPublicSurfaceBuildError> {
        bindings.sort_unstable();
        if let Some(pair) = bindings.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(DirectPublicSurfaceBuildError::DuplicateBinding(pair[0]));
        }
        Ok(Self { bindings })
    }

    pub fn from_export_hir(export: &ExportHir) -> Result<Self, DirectPublicSurfaceBuildError> {
        let bindings = export
            .export_binding_identities
            .iter()
            .map(|record| record.id())
            .collect();
        Self::try_new(bindings)
    }

    pub fn bindings(&self) -> &[PersistentExportBindingId] {
        &self.bindings
    }
}

impl WireEncode for CanonicalDirectPublicSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bindings.len() as u64)?;
        for binding in &self.bindings {
            binding.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalDirectPublicSurfaceV1 {
    bindings: Vec<DecodedPersistentId<PersistentExportBindingId>>,
}

impl DecodedCanonicalDirectPublicSurfaceV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<CanonicalDirectPublicSurfaceV1, DirectPublicSurfaceValidationError> {
        self.validate_against(foundation.canonical())
    }

    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CanonicalDirectPublicSurfaceV1, DirectPublicSurfaceValidationError> {
        let mut previous: Option<[u8; 32]> = None;
        let mut bindings = Vec::with_capacity(self.bindings.len());
        for (index, decoded) in self.bindings.into_iter().enumerate() {
            if let Some(previous) = previous {
                match previous.cmp(decoded.as_array()) {
                    std::cmp::Ordering::Equal => {
                        return Err(DirectPublicSurfaceValidationError::DuplicateBinding {
                            index,
                            identity: *decoded.as_array(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(DirectPublicSurfaceValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            let binding = foundation
                .export_binding_id_by_bytes(decoded.as_array())
                .ok_or(DirectPublicSurfaceValidationError::UnknownBinding {
                    index,
                    identity: *decoded.as_array(),
                })?;
            previous = Some(*decoded.as_array());
            bindings.push(binding);
        }
        Ok(CanonicalDirectPublicSurfaceV1 { bindings })
    }
}

impl WireEncode for DecodedCanonicalDirectPublicSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bindings.len() as u64)?;
        for binding in &self.bindings {
            binding.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalDirectPublicSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            .map(|bindings| Self { bindings })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPublicSurfaceBuildError {
    DuplicateBinding(PersistentExportBindingId),
}

impl fmt::Display for DirectPublicSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBinding(id) => {
                write!(formatter, "duplicate direct public binding {id}")
            }
        }
    }
}

impl std::error::Error for DirectPublicSurfaceBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPublicSurfaceValidationError {
    DuplicateBinding { index: usize, identity: [u8; 32] },
    NonCanonicalOrder { index: usize },
    UnknownBinding { index: usize, identity: [u8; 32] },
}

impl fmt::Display for DirectPublicSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBinding { index, identity } => write!(
                formatter,
                "duplicate direct public binding {} at index {index}",
                HexIdentity(identity)
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "direct public bindings are not in canonical order at index {index}"
            ),
            Self::UnknownBinding { index, identity } => write!(
                formatter,
                "direct public binding {} at index {index} is absent from the identity foundation",
                HexIdentity(identity)
            ),
        }
    }
}

impl std::error::Error for DirectPublicSurfaceValidationError {}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

struct HexIdentity<'a>(&'a [u8; 32]);

impl fmt::Display for HexIdentity<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, ExportBindingKey, PackagePath, PersistentExportBindingId,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };
    use scoop_wire::{DecodeLimits, WireEncode, decode_canonical, encode};

    use super::{
        CanonicalDirectPublicSurfaceV1, DecodedCanonicalDirectPublicSurfaceV1,
        DecodedHirOutputContractV1, DirectPublicSurfaceValidationError, HirOutputContractV1,
        HirOutputContractValidationError,
    };
    use crate::CanonicalHirFoundation;

    #[test]
    fn output_contract_variants_have_fixed_wire_vectors() {
        assert_eq!(
            hex(&encode(&HirOutputContractV1::Library).unwrap()),
            "a10001"
        );

        let (function, _) = function_and_binding("main");
        let executable = HirOutputContractV1::ExecutableSourceEntry(function.id());
        assert_eq!(
            hex(&encode(&executable).unwrap()),
            "a2000201582092c7d9b606546e8e3a46c17972ace56a987446fecd6c88b62c454eea4196152e"
        );
    }

    #[test]
    fn output_contract_reader_rejects_unknown_missing_and_extra_fields() {
        for bytes in [
            vec![0xa1, 0x00, 0x03],
            vec![0xa0],
            vec![0xa2, 0x00, 0x01, 0x01, 0x01],
        ] {
            assert!(
                decode_canonical::<DecodedHirOutputContractV1>(&bytes, DecodeLimits::default())
                    .is_err()
            );
        }
    }

    #[test]
    fn executable_contract_must_reference_a_foundation_function() {
        let (known, _) = function_and_binding("known");
        let (unknown, _) = function_and_binding("unknown");
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_functions(vec![known.clone()]).unwrap();
        let known_bytes = encode(&HirOutputContractV1::ExecutableSourceEntry(known.id())).unwrap();
        let known_decoded =
            decode_canonical::<DecodedHirOutputContractV1>(&known_bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(
            known_decoded.validate_against(&foundation),
            Ok(HirOutputContractV1::ExecutableSourceEntry(known.id()))
        );

        let bytes = encode(&HirOutputContractV1::ExecutableSourceEntry(unknown.id())).unwrap();
        let decoded =
            decode_canonical::<DecodedHirOutputContractV1>(&bytes, DecodeLimits::default())
                .unwrap();

        assert_eq!(
            decoded.validate_against(&foundation),
            Err(HirOutputContractValidationError::UnknownEntry(
                *unknown.id().as_array()
            ))
        );
    }

    #[test]
    fn direct_public_surface_sorts_producer_input_and_has_a_fixed_wire_vector() {
        let (_, first) = function_and_binding("first");
        let (_, second) = function_and_binding("second");
        let surface =
            CanonicalDirectPublicSurfaceV1::try_new(vec![second.id(), first.id()]).unwrap();
        assert!(surface.bindings()[0] < surface.bindings()[1]);
        assert_eq!(
            hex(&encode(&surface).unwrap()),
            "8258201a553b750119dac566230c7d5c98b447db36232899fc35706905821e88e766fa5820e95a6e2f6ceec0afeeb1e4ee1127cd5794a0a547ace61aacafd7951abe3019d4"
        );
    }

    #[test]
    fn direct_public_surface_reader_rejects_duplicate_noncanonical_and_unknown_ids() {
        let (_, first) = function_and_binding("first");
        let (_, second) = function_and_binding("second");
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_export_bindings(vec![first.clone(), second.clone()])
            .unwrap();
        let (low, high) = if first.id() < second.id() {
            (first.id(), second.id())
        } else {
            (second.id(), first.id())
        };

        let valid = decode_surface(&[low, high]);
        assert_eq!(
            valid.validate_against(&foundation).unwrap().bindings(),
            &[low, high]
        );

        let duplicate = decode_surface(&[low, low]);
        assert!(matches!(
            duplicate.validate_against(&foundation),
            Err(DirectPublicSurfaceValidationError::DuplicateBinding { index: 1, .. })
        ));

        let reversed = decode_surface(&[high, low]);
        assert_eq!(
            reversed.validate_against(&foundation),
            Err(DirectPublicSurfaceValidationError::NonCanonicalOrder { index: 1 })
        );

        let (_, unknown) = function_and_binding("unknown");
        let unknown = decode_surface(&[unknown.id()]);
        assert!(matches!(
            unknown.validate_against(&foundation),
            Err(DirectPublicSurfaceValidationError::UnknownBinding { index: 0, .. })
        ));
    }

    fn decode_surface(
        bindings: &[PersistentExportBindingId],
    ) -> DecodedCanonicalDirectPublicSurfaceV1 {
        let bytes = encode(&RawSurface(bindings)).unwrap();
        decode_canonical(&bytes, DecodeLimits::default()).unwrap()
    }

    struct RawSurface<'a>(&'a [PersistentExportBindingId]);

    impl WireEncode for RawSurface<'_> {
        fn encode(
            &self,
            encoder: &mut scoop_wire::Encoder,
        ) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for binding in self.0 {
                binding.encode(encoder)?;
            }
            Ok(())
        }
    }

    fn function_and_binding(
        name: &str,
    ) -> (
        CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
        CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    ) {
        let name = CanonicalIdentifier::new(name).unwrap();
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            name.clone(),
            0,
            None,
            Vec::new(),
        );
        let function = CborIdentityRecord::from_key(declaration.clone()).unwrap();
        let target = BindingTarget::function(&declaration).unwrap();
        let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            name,
            target,
        ))
        .unwrap();
        (function, binding)
    }

    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }
}
