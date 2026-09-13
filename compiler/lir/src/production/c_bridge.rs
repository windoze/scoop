use scoop_identity::{CBridgeToolchainProfileId, GeneratedBridgeUnitId};
use scoop_wire::{Encoder, WireEncode};

use crate::{
    CBridgeToolchainFingerprint, CBridgeToolchainProfileV1, CanonicalCBridgeFlagFingerprint,
    GeneratedBridgePlanSetV1, GeneratedCSourceTemplateFingerprint,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CBridgeProductionSetV1 {
    NotUsed,
    Used(CBridgeProductionV1),
}

impl CBridgeProductionSetV1 {
    pub fn from_generated_bridge_plan(
        plan: &GeneratedBridgePlanSetV1,
        profile: &CBridgeToolchainProfileV1,
    ) -> Self {
        let units = plan
            .units()
            .iter()
            .map(|unit| unit.unit())
            .collect::<Vec<_>>();
        if units.is_empty() {
            return Self::NotUsed;
        }
        Self::Used(CBridgeProductionV1 {
            profile_id: profile.id().clone(),
            profile_fingerprint: profile.fingerprint(),
            source_template_fingerprint: profile.contract().source_template_fingerprint(),
            canonical_flag_fingerprint: profile.contract().canonical_flag_fingerprint(),
            units,
        })
    }
}

impl WireEncode for CBridgeProductionSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotUsed => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Used(production) => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                production.encode_fields(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeProductionV1 {
    profile_id: CBridgeToolchainProfileId,
    profile_fingerprint: CBridgeToolchainFingerprint,
    source_template_fingerprint: GeneratedCSourceTemplateFingerprint,
    canonical_flag_fingerprint: CanonicalCBridgeFlagFingerprint,
    units: Vec<GeneratedBridgeUnitId>,
}

impl CBridgeProductionV1 {
    pub const fn profile_id(&self) -> &CBridgeToolchainProfileId {
        &self.profile_id
    }

    pub const fn profile_fingerprint(&self) -> CBridgeToolchainFingerprint {
        self.profile_fingerprint
    }

    pub const fn source_template_fingerprint(&self) -> GeneratedCSourceTemplateFingerprint {
        self.source_template_fingerprint
    }

    pub const fn canonical_flag_fingerprint(&self) -> CanonicalCBridgeFlagFingerprint {
        self.canonical_flag_fingerprint
    }

    pub fn units(&self) -> &[GeneratedBridgeUnitId] {
        &self.units
    }

    fn encode_fields(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.profile_id.encode(encoder)?;
        encoder.field(2)?;
        self.profile_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.source_template_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.canonical_flag_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        encoder.array(self.units.len() as u64)?;
        for unit in &self.units {
            unit.encode(encoder)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, ConeIdentity, GeneratedBridgeUnitId,
        GeneratedBridgeUnitKey, NativeExternalContract, NativeExternalContractFingerprint,
        NativeExternalSymbolKey, NativeLibraryBinding, PersistentNativeExternalSymbolId,
        SourceNativeSymbol,
    };
    use scoop_wire::encode;

    use super::*;
    use crate::{
        AppleClangCompilerIdentityV1, CanonicalLirFoundation, DarwinBuildToolIdV1,
        DarwinBuildToolVersionContractV1, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
        OdrFreeLirFoundation,
    };

    #[test]
    fn absent_generated_bridges_produce_the_explicit_not_used_variant() {
        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, CanonicalLirFoundation::empty())
                .unwrap();
        let plan = GeneratedBridgePlanSetV1::from_odr_free_foundation(&foundation).unwrap();
        let production = CBridgeProductionSetV1::from_generated_bridge_plan(&plan, &profile());
        assert_eq!(production, CBridgeProductionSetV1::NotUsed);
        assert_eq!(hex(&encode(&production).unwrap()), "a10001");
    }

    #[test]
    fn used_variant_has_the_frozen_field_order() {
        let production = CBridgeProductionSetV1::Used(CBridgeProductionV1 {
            profile_id: CBridgeToolchainProfileId::darwin_aarch64_apple_clang(),
            profile_fingerprint: profile().fingerprint(),
            source_template_fingerprint: profile().contract().source_template_fingerprint(),
            canonical_flag_fingerprint: profile().contract().canonical_flag_fingerprint(),
            units: vec![unit_id()],
        });
        assert_eq!(
            hex(&encode(&production).unwrap()),
            "a6000201a30178296f72672e73636f6f702d6c616e672e632d6272696467652d746f6f6c636861696e2d70726f66696c6502781a64617277696e2d616172636836342d6170706c652d636c616e6703010258200bd65078dfefea47cdc2cd6d477065dfc2d51ff7bbecf5edfcec09bc6aa32a53035820b75415d9bd34b79931f79031154e81403aa5e0606904b8cd786a3428bf0e91d604582003f639bc4c8be95a9e47cdc620948ee8ca8c7bc62d472ae54d190c68803a93ee0581582065a6a97003171db803e4000a76035475b756279dcc38e3ea75a0bff21c2652ac"
        );
    }

    fn profile() -> CBridgeToolchainProfileV1 {
        let deployment = DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
            DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
            vec![DarwinBuildToolVersionContractV1::new(
                DarwinBuildToolIdV1::Clang,
                DarwinPackedVersionV1::new(0x1000_0200).unwrap(),
            )],
        )
        .unwrap();
        CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
            deployment,
            AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
        )
        .unwrap()
    }

    fn unit_id() -> GeneratedBridgeUnitId {
        let symbol = NativeExternalSymbolKey::darwin_macho_external(
            &SourceNativeSymbol::new("native").unwrap(),
        )
        .unwrap();
        let contract = NativeExternalContract::c_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
        );
        let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
            PersistentNativeExternalSymbolId::from_key(&symbol).unwrap(),
            &contract,
        )
        .unwrap();
        GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::OutboundFunction(fingerprint))
            .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
