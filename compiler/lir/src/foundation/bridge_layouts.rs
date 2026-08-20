use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiLayoutFingerprint,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprint,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, GeneratedBridgeUnitId,
    GeneratedBridgeUnitKey, NativeExternAbi, NativeExternalContract,
    NativeExternalContractFingerprint, NativeExternalContractRecord,
};

use super::BridgeUnitRecord;

pub(crate) fn required_generated_bridge_layouts(
    units: impl IntoIterator<Item = (GeneratedBridgeUnitId, GeneratedBridgeUnitKey)>,
    contracts: &[NativeExternalContractRecord],
    signatures: &[CanonicalCAbiSignatureFingerprintRecord],
    layouts: &[CanonicalCAbiLayoutFingerprintRecord],
) -> Result<
    BTreeMap<GeneratedBridgeUnitId, BTreeSet<CanonicalCAbiLayoutFingerprint>>,
    GeneratedBridgeLayoutClosureError,
> {
    let contracts = contracts
        .iter()
        .map(|record| (record.fingerprint(), record.contract()))
        .collect::<BTreeMap<_, _>>();
    let signatures = signatures
        .iter()
        .map(|record| (record.fingerprint(), record.signature()))
        .collect::<BTreeMap<_, _>>();
    let layouts = layouts
        .iter()
        .map(|record| (record.fingerprint(), record))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for (unit, key) in units {
        let mut closure = LayoutClosure {
            unit,
            layouts: &layouts,
            visiting: BTreeSet::new(),
            required: BTreeSet::new(),
        };
        match key {
            GeneratedBridgeUnitKey::OutboundFunction(contract, _) => {
                let contract = require_contract(unit, contract, &contracts)?;
                let NativeExternalContract::Function {
                    abi: NativeExternAbi::C(signature),
                    ..
                } = contract
                else {
                    return Err(GeneratedBridgeLayoutClosureError::ContractKindMismatch { unit });
                };
                closure.signature(signature)?;
            }
            GeneratedBridgeUnitKey::GlobalRead(contract)
            | GeneratedBridgeUnitKey::GlobalAddress(contract) => {
                closure.storage(require_data_storage(
                    unit,
                    require_contract(unit, contract, &contracts)?,
                    false,
                )?)?;
            }
            GeneratedBridgeUnitKey::GlobalWrite(contract) => {
                closure.storage(require_data_storage(
                    unit,
                    require_contract(unit, contract, &contracts)?,
                    true,
                )?)?;
            }
            GeneratedBridgeUnitKey::CallbackTrampoline { signature, .. }
            | GeneratedBridgeUnitKey::StaticCallbackTrampoline { signature, .. } => {
                let signature = signatures.get(&signature).copied().ok_or(
                    GeneratedBridgeLayoutClosureError::MissingSignature { unit, signature },
                )?;
                closure.signature(signature)?;
            }
        }
        result.insert(unit, closure.required);
    }
    Ok(result)
}

pub(crate) fn bridge_unit_keys(
    units: &[BridgeUnitRecord],
) -> impl Iterator<Item = (GeneratedBridgeUnitId, GeneratedBridgeUnitKey)> + '_ {
    units.iter().map(|record| (record.id(), *record.key()))
}

fn require_contract<'a>(
    unit: GeneratedBridgeUnitId,
    fingerprint: NativeExternalContractFingerprint,
    contracts: &'a BTreeMap<NativeExternalContractFingerprint, &'a NativeExternalContract>,
) -> Result<&'a NativeExternalContract, GeneratedBridgeLayoutClosureError> {
    contracts.get(&fingerprint).copied().ok_or(
        GeneratedBridgeLayoutClosureError::MissingNativeContract {
            unit,
            contract: fingerprint,
        },
    )
}

fn require_data_storage(
    unit: GeneratedBridgeUnitId,
    contract: &NativeExternalContract,
    require_mutable: bool,
) -> Result<CanonicalCStorageType, GeneratedBridgeLayoutClosureError> {
    match contract {
        NativeExternalContract::ReadOnlyData { storage, .. }
        | NativeExternalContract::ReadOnlyTls { storage, .. }
            if !require_mutable =>
        {
            Ok(*storage)
        }
        NativeExternalContract::MutableData { storage, .. }
        | NativeExternalContract::MutableTls { storage, .. } => Ok(*storage),
        NativeExternalContract::Function { .. }
        | NativeExternalContract::ReadOnlyData { .. }
        | NativeExternalContract::ReadOnlyTls { .. } => {
            Err(GeneratedBridgeLayoutClosureError::ContractKindMismatch { unit })
        }
    }
}

struct LayoutClosure<'a> {
    unit: GeneratedBridgeUnitId,
    layouts: &'a BTreeMap<CanonicalCAbiLayoutFingerprint, &'a CanonicalCAbiLayoutFingerprintRecord>,
    visiting: BTreeSet<CanonicalCAbiLayoutFingerprint>,
    required: BTreeSet<CanonicalCAbiLayoutFingerprint>,
}

impl LayoutClosure<'_> {
    fn signature(
        &mut self,
        signature: &CanonicalCAbiFunctionSignature,
    ) -> Result<(), GeneratedBridgeLayoutClosureError> {
        for parameter in signature.parameters() {
            self.storage(parameter.storage())?;
        }
        if let CanonicalCAbiReturn::Value { storage, .. } = signature.result() {
            self.storage(storage)?;
        }
        Ok(())
    }

    fn storage(
        &mut self,
        storage: CanonicalCStorageType,
    ) -> Result<(), GeneratedBridgeLayoutClosureError> {
        let CanonicalCStorageType::Struct { layout, .. } = storage else {
            return Ok(());
        };
        self.layout(layout)
    }

    fn layout(
        &mut self,
        layout: CanonicalCAbiLayoutFingerprint,
    ) -> Result<(), GeneratedBridgeLayoutClosureError> {
        if self.required.contains(&layout) {
            return Ok(());
        }
        if !self.visiting.insert(layout) {
            return Err(GeneratedBridgeLayoutClosureError::LayoutCycle {
                unit: self.unit,
                layout,
            });
        }
        let record = self.layouts.get(&layout).copied().ok_or(
            GeneratedBridgeLayoutClosureError::MissingLayout {
                unit: self.unit,
                layout,
            },
        )?;
        for field in record.layout().fields() {
            self.storage(field.storage())?;
        }
        self.visiting.remove(&layout);
        self.required.insert(layout);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedBridgeLayoutClosureError {
    MissingNativeContract {
        unit: GeneratedBridgeUnitId,
        contract: NativeExternalContractFingerprint,
    },
    ContractKindMismatch {
        unit: GeneratedBridgeUnitId,
    },
    MissingSignature {
        unit: GeneratedBridgeUnitId,
        signature: CanonicalCAbiSignatureFingerprint,
    },
    MissingLayout {
        unit: GeneratedBridgeUnitId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
    LayoutCycle {
        unit: GeneratedBridgeUnitId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
}

impl fmt::Display for GeneratedBridgeLayoutClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid generated bridge static-assert layout closure: {self:?}"
        )
    }
}

impl std::error::Error for GeneratedBridgeLayoutClosureError {}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use scoop_identity::{
        CLayoutOverride, CallbackParameterIndex, CanonicalCAbiLayout,
        CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiParameter, CanonicalCAbiReturn,
        CanonicalCAbiSignatureFingerprintRecord, CborIdentityRecord, CoreBuiltinNominal,
        ExactTypeKey, GeneratedBridgeUnitKey, NativeExternalContract,
        NativeExternalContractFingerprint, NativeExternalSymbolKey, NativeLibraryBinding,
        PersistentExactTypeId, PersistentNativeExternalSymbolId, SourceNativeSymbol,
    };

    use super::*;

    #[test]
    fn callback_unit_closes_every_by_value_layout() {
        let exact = unit_exact_type();
        let layout = CanonicalCAbiLayoutFingerprintRecord::new(CanonicalCAbiLayout::new(
            exact,
            0,
            NonZeroU64::new(1).unwrap(),
            CLayoutOverride::Natural,
            CLayoutOverride::Natural,
            Vec::new(),
        ))
        .unwrap();
        let signature =
            CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
                vec![
                    CanonicalCAbiParameter::new(
                        exact,
                        CanonicalCStorageType::Struct {
                            exact_type: exact,
                            layout: layout.fingerprint(),
                        },
                    )
                    .unwrap(),
                ],
                CanonicalCAbiReturn::Void,
            ))
            .unwrap();
        let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: signature.fingerprint(),
            context_index: CallbackParameterIndex::new(0),
        })
        .unwrap();

        let closure = required_generated_bridge_layouts(
            [(unit.id(), *unit.key())],
            &[],
            &[signature],
            std::slice::from_ref(&layout),
        )
        .unwrap();

        assert_eq!(closure[&unit.id()], BTreeSet::from([layout.fingerprint()]));
    }

    #[test]
    fn write_unit_rejects_a_read_only_data_contract() {
        let exact = unit_exact_type();
        let symbol = NativeExternalSymbolKey::darwin_macho_external(
            &SourceNativeSymbol::new("value").unwrap(),
        )
        .unwrap();
        let contract = NativeExternalContract::read_only_data(
            NativeLibraryBinding::DefaultNativeNamespace,
            CanonicalCStorageType::Boolean { exact_type: exact },
        );
        let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
            PersistentNativeExternalSymbolId::from_key(&symbol).unwrap(),
            &contract,
        )
        .unwrap();
        let unit =
            CborIdentityRecord::from_key(GeneratedBridgeUnitKey::GlobalWrite(fingerprint)).unwrap();

        assert_eq!(
            require_data_storage(unit.id(), &contract, true),
            Err(GeneratedBridgeLayoutClosureError::ContractKindMismatch { unit: unit.id() })
        );
    }

    fn unit_exact_type() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }
}
