//! Canonical HIR production contracts shared by ordinary and core Cones.

use std::fmt;

use scoop_identity::{
    ConeIdentity, CoreBuiltinNominal, DecodedExecutableSourceEntryIdentity, DecodedPersistentId,
    ExactOrdinaryNoArgUnitSignature, ExactTypeKey, ExecutableSourceEntryIdentity,
    ExecutableSourceEntryIdentityError, PersistentExportBindingId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use crate::{
    CanonicalHirFoundation, CanonicalPublicExportBindingsV1, ConeOutputKind, ExportHir,
    ValidatedHirFoundation,
};

mod core_interface;
pub use core_interface::*;
mod core_protocols;
pub use core_protocols::*;
mod core_protocol_surface;
#[cfg(test)]
pub(crate) use core_protocol_surface::test_support as core_protocol_test_support;
pub use core_protocol_surface::*;
mod public_nominal_shapes;
pub use public_nominal_shapes::*;
mod core_well_known;
pub use core_well_known::*;
mod const_values;
pub use const_values::*;
mod cross_cone_section;
pub use cross_cone_section::*;
mod callable_interfaces;
pub use callable_interfaces::*;
mod callable_source_interfaces;
pub use callable_source_interfaces::*;
mod definition_sources;
pub(crate) use definition_sources::project_definition_source;
pub use definition_sources::{
    ExportDefinitionSourceProductionError, HirDefinitionSourceProjectionError,
};
mod external_references;
pub use external_references::*;
mod default_templates;
pub use default_templates::*;
mod nominal_dispatch;
mod nominal_interfaces;
pub use nominal_interfaces::*;
mod property_interfaces;
pub use property_interfaces::*;
mod signatures;
pub use signatures::HirInterfaceSignatureProjectionError;
mod type_alias_interfaces;
pub use type_alias_interfaces::*;
mod type_semantics;
pub use type_semantics::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOutputContractV1 {
    Library,
    Executable(Box<ExecutableSourceEntryIdentity>),
}

impl HirOutputContractV1 {
    pub fn from_output_kind(output: &ConeOutputKind) -> Self {
        match output {
            ConeOutputKind::Library => Self::Library,
            ConeOutputKind::Executable { local_entry } => {
                Self::Executable(Box::new(local_entry.identity().clone()))
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
            Self::Executable(entry) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                entry.as_ref().encode(encoder)
            }
        }
    }
}

#[derive(Debug)]
pub enum DecodedHirOutputContractV1 {
    Library,
    Executable(Box<DecodedExecutableSourceEntryIdentity>),
}

impl DecodedHirOutputContractV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<HirOutputContractV1, HirOutputContractValidationError> {
        self.validate_against(foundation.artifact(), foundation.canonical())
    }

    fn validate_against(
        self,
        artifact: ConeIdentity,
        foundation: &CanonicalHirFoundation,
    ) -> Result<HirOutputContractV1, HirOutputContractValidationError> {
        match self {
            Self::Library => Ok(HirOutputContractV1::Library),
            Self::Executable(decoded) => {
                let declaration = foundation
                    .function_record_by_bytes(decoded.declaration().as_array())
                    .ok_or(HirOutputContractValidationError::UnknownEntry(
                        *decoded.declaration().as_array(),
                    ))?;
                let unit_key =
                    ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
                let unit = foundation
                    .exact_type_id_by_key(&unit_key)
                    .ok_or(HirOutputContractValidationError::MissingUnitExactType)?;
                let expected = ExecutableSourceEntryIdentity::try_new(
                    declaration,
                    ExactOrdinaryNoArgUnitSignature::new(unit),
                )
                .map_err(HirOutputContractValidationError::InvalidEntry)?;
                if expected.root_cone() != artifact {
                    return Err(HirOutputContractValidationError::ForeignEntry {
                        artifact,
                        entry: expected.root_cone(),
                    });
                }
                let actual_bytes =
                    encode(decoded.as_ref()).map_err(HirOutputContractValidationError::Encode)?;
                let expected_bytes =
                    encode(&expected).map_err(HirOutputContractValidationError::Encode)?;
                if actual_bytes != expected_bytes {
                    return Err(HirOutputContractValidationError::EntryMismatch);
                }
                Ok(HirOutputContractV1::Executable(Box::new(expected)))
            }
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
            Self::Executable(entry) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                entry.as_ref().encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedHirOutputContractV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
                    .field(1, DecodedExecutableSourceEntryIdentity::decode)
                    .map(Box::new)
                    .map(Self::Executable)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOutputContractValidationError {
    UnknownEntry([u8; 32]),
    MissingUnitExactType,
    InvalidEntry(ExecutableSourceEntryIdentityError),
    ForeignEntry {
        artifact: ConeIdentity,
        entry: ConeIdentity,
    },
    Encode(scoop_wire::cbor::EncodeError),
    EntryMismatch,
}

impl fmt::Display for HirOutputContractValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEntry(id) => write!(
                formatter,
                "HIR executable source entry {} is absent from the identity foundation",
                HexIdentity(id)
            ),
            Self::MissingUnitExactType => {
                formatter.write_str("HIR executable output has no trusted core Unit exact type")
            }
            Self::InvalidEntry(error) => error.fmt(formatter),
            Self::ForeignEntry { artifact, entry } => write!(
                formatter,
                "HIR executable source entry belongs to Cone {entry}, not artifact Cone {artifact}"
            ),
            Self::Encode(error) => error.fmt(formatter),
            Self::EntryMismatch => formatter.write_str(
                "HIR executable output payload does not match its declaration and Unit identity",
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
        Self::from_public_bindings(&export.public_export_bindings)
    }

    fn from_public_bindings(
        public_bindings: &CanonicalPublicExportBindingsV1,
    ) -> Result<Self, DirectPublicSurfaceBuildError> {
        let bindings = public_bindings
            .records()
            .iter()
            .filter_map(|record| {
                matches!(
                    record.source(),
                    crate::ExportBindingSourceV1::DeclaredCurrent { .. }
                )
                .then_some(record.binding())
            })
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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

fn require_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
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
        BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, ExportBindingKey, PackagePath, PersistentExactTypeId,
        PersistentExportBindingId, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite,
    };
    use scoop_wire::{WireEncode, decode_canonical, encode};

    use super::{
        CanonicalDirectPublicSurfaceV1, DecodedCanonicalDirectPublicSurfaceV1,
        DecodedHirOutputContractV1, DirectPublicSurfaceValidationError, HirOutputContractV1,
        HirOutputContractValidationError,
    };
    use crate::CanonicalHirFoundation;
    use crate::{
        CanonicalPublicExportBindingsV1, CanonicalReexportRoutesV1, ExportBindingSourceV1,
        PublicExportBindingRecordV1, ReexportRouteHopV1, ReexportRouteV1,
    };

    #[test]
    fn output_contract_variants_have_fixed_wire_vectors() {
        assert_eq!(
            hex(&encode(&HirOutputContractV1::Library).unwrap()),
            "a10001"
        );

        let (function, _) = function_and_binding("main");
        let executable = executable_contract(&function);
        assert_eq!(
            hex(&encode(&executable).unwrap()),
            "a2000201a50158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02582092c7d9b606546e8e3a46c17972ace56a987446fecd6c88b62c454eea4196152e03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c92570558202c5339a89711e2f13f989f6f69c1889195f4e138bc8e8dcfb1cf49a99b05e65c"
        );
    }

    #[test]
    fn output_contract_reader_rejects_unknown_missing_and_extra_fields() {
        for bytes in [
            vec![0xa1, 0x00, 0x03],
            vec![0xa0],
            vec![0xa2, 0x00, 0x01, 0x01, 0x01],
        ] {
            assert!(decode_canonical::<DecodedHirOutputContractV1>(&bytes).is_err());
        }
    }

    #[test]
    fn executable_contract_is_rebuilt_from_the_foundation() {
        let (known, _) = function_and_binding("main");
        let unknown = function(ConeIdentity::SINGLE_FILE, "main");
        let unit = unit_exact_record();
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_functions(vec![known.clone()]).unwrap();
        foundation.set_exact_types(vec![unit]).unwrap();
        let known_contract = executable_contract(&known);
        let known_bytes = encode(&known_contract).unwrap();
        let known_decoded = decode_canonical::<DecodedHirOutputContractV1>(&known_bytes).unwrap();
        assert_eq!(
            known_decoded.validate_against(ConeIdentity::CORE, &foundation),
            Ok(known_contract)
        );

        let bytes = encode(&executable_contract(&unknown)).unwrap();
        let decoded = decode_canonical::<DecodedHirOutputContractV1>(&bytes).unwrap();

        assert_eq!(
            decoded.validate_against(ConeIdentity::CORE, &foundation),
            Err(HirOutputContractValidationError::UnknownEntry(
                *unknown.id().as_array()
            ))
        );
    }

    #[test]
    fn executable_contract_rejects_the_removed_id_only_wire() {
        let (function, _) = function_and_binding("main");
        let mut bytes = vec![0xa2, 0x00, 0x02, 0x01, 0x58, 0x20];
        bytes.extend_from_slice(function.id().as_array());
        assert!(decode_canonical::<DecodedHirOutputContractV1>(&bytes).is_err());
    }

    #[test]
    fn executable_contract_rejects_a_tampered_derived_body() {
        let (function, _) = function_and_binding("main");
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_functions(vec![function.clone()]).unwrap();
        foundation
            .set_exact_types(vec![unit_exact_record()])
            .unwrap();
        let mut bytes = encode(&executable_contract(&function)).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        let decoded = decode_canonical::<DecodedHirOutputContractV1>(&bytes).unwrap();

        assert_eq!(
            decoded.validate_against(ConeIdentity::CORE, &foundation),
            Err(HirOutputContractValidationError::EntryMismatch)
        );
    }

    #[test]
    fn executable_contract_requires_the_artifact_cone() {
        let (function, _) = function_and_binding("main");
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_functions(vec![function.clone()]).unwrap();
        foundation
            .set_exact_types(vec![unit_exact_record()])
            .unwrap();
        let decoded = decode_canonical::<DecodedHirOutputContractV1>(
            &encode(&executable_contract(&function)).unwrap(),
        )
        .unwrap();

        assert_eq!(
            decoded.validate_against(ConeIdentity::SINGLE_FILE, &foundation),
            Err(HirOutputContractValidationError::ForeignEntry {
                artifact: ConeIdentity::SINGLE_FILE,
                entry: ConeIdentity::CORE,
            })
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
    fn direct_public_surface_excludes_reexports() {
        let (_, direct) = function_and_binding("direct");
        let (_, forwarded) = function_and_binding("forwarded");
        let route = ReexportRouteV1::try_new(
            ConeIdentity::CORE,
            vec![ReexportRouteHopV1::new(ConeIdentity::CORE, forwarded.id())],
        )
        .unwrap();
        let public = CanonicalPublicExportBindingsV1::try_new(vec![
            PublicExportBindingRecordV1::new(
                direct.id(),
                ExportBindingSourceV1::DeclaredCurrent {
                    declaration: direct.key().target(),
                },
            ),
            PublicExportBindingRecordV1::new(
                forwarded.id(),
                ExportBindingSourceV1::Reexport {
                    routes: CanonicalReexportRoutesV1::try_new(vec![route]).unwrap(),
                },
            ),
        ])
        .unwrap();

        assert_eq!(
            CanonicalDirectPublicSurfaceV1::from_public_bindings(&public)
                .unwrap()
                .bindings(),
            &[direct.id()]
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
        decode_canonical(&bytes).unwrap()
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
        let function = function(ConeIdentity::CORE, name.as_str());
        let declaration = function.key();
        let target = BindingTarget::function(declaration).unwrap();
        let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            name,
            target,
        ))
        .unwrap();
        (function, binding)
    }

    fn function(
        cone: ConeIdentity,
        name: &str,
    ) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                cone,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
    }

    fn unit_exact_record() -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn executable_contract(
        declaration: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    ) -> HirOutputContractV1 {
        let entry = ExecutableSourceEntryIdentity::try_new(
            declaration,
            ExactOrdinaryNoArgUnitSignature::new(unit_exact_record().id()),
        )
        .unwrap();
        HirOutputContractV1::Executable(Box::new(entry))
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
