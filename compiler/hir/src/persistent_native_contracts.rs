//! Target-independent native contracts produced by export HIR.

use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CanonicalNativeLibraryName, GcEffect as PersistentGcEffect, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceCallingConvention, SourceExternFunctionAbi,
    SourceNativeExternalContract, SourceNativeExternalContractKey,
    SourceNativeExternalContractRecord, SourceNativeLibraryBinding, SourceNativeSymbol,
    SourceScoopAbiFunctionSignature,
};

use crate::{
    ExternAbi, ExternFunction, Function, FunctionId, FunctionKind, GcEffect, Global, GlobalId,
    GlobalStorage, HirFunctionIdentities, HirFunctionIdentity, HirPropertyIdentities,
    HirPropertyIdentity, HirSignatureTypeMapper, HirTypeIdentityInputs, Property, PropertyOwner,
    TypeId,
};

mod error;
pub use error::HirSourceNativeContractError;

/// HIR entity that owns one source-native contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSourceNativeContractOwner {
    Function(FunctionId),
    Global(GlobalId),
}

/// One local owner and its target-independent persistent contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceNativeContract {
    owner: HirSourceNativeContractOwner,
    record: SourceNativeExternalContractRecord,
}

impl HirSourceNativeContract {
    pub const fn owner(&self) -> HirSourceNativeContractOwner {
        self.owner
    }

    pub const fn record(&self) -> &SourceNativeExternalContractRecord {
        &self.record
    }
}

pub struct HirSourceNativeContractInputs<'a> {
    pub functions: &'a Arena<Function>,
    pub extern_functions: &'a Arena<ExternFunction>,
    pub globals: &'a Arena<Global>,
    pub properties: &'a Arena<Property>,
    pub function_identities: &'a HirFunctionIdentities,
    pub property_identities: &'a HirPropertyIdentities,
    pub type_inputs: HirTypeIdentityInputs<'a>,
    pub unit: TypeId,
}

/// Complete persistent contract relation for every source extern function and
/// extern global in an export HIR module.
#[derive(Clone, Debug)]
pub struct HirSourceNativeContracts {
    contracts: Vec<HirSourceNativeContract>,
}

impl HirSourceNativeContracts {
    /// Resolve the total contract relation by its typed local owner.
    pub fn get(
        &self,
        owner: HirSourceNativeContractOwner,
    ) -> Option<&SourceNativeExternalContractRecord> {
        self.contracts
            .iter()
            .find(|contract| contract.owner == owner)
            .map(HirSourceNativeContract::record)
    }

    pub fn from_declarations(
        inputs: HirSourceNativeContractInputs<'_>,
    ) -> Result<Self, HirSourceNativeContractError> {
        let mapper = HirSignatureTypeMapper::new(inputs.type_inputs);
        let mut contracts = Vec::new();
        let mut seen_externs = vec![false; inputs.extern_functions.len()];

        for (function_id, function) in inputs.functions.iter() {
            let FunctionKind::Extern(extern_id) = function.kind else {
                continue;
            };
            let extern_index = local_index(extern_id);
            if extern_index >= inputs.extern_functions.len() {
                return Err(HirSourceNativeContractError::UnknownExternFunction {
                    function: function_id,
                    external: raw_index(extern_id),
                });
            }
            let extern_ = &inputs.extern_functions[extern_id];
            if std::mem::replace(&mut seen_externs[extern_index], true) {
                return Err(HirSourceNativeContractError::DuplicateExternFunction {
                    function: function_id,
                    external: raw_index(extern_id),
                });
            }
            validate_function(function_id, function, extern_)?;
            let declaration = match &inputs.function_identities[function_id] {
                HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Plain(record)) => {
                    record.key()
                }
                _ => {
                    return Err(HirSourceNativeContractError::InvalidFunctionIdentity {
                        function: function_id,
                    });
                }
            };
            let key = SourceNativeExternalContractKey::function(declaration).map_err(|error| {
                HirSourceNativeContractError::InvalidContract {
                    owner: HirSourceNativeContractOwner::Function(function_id),
                    error,
                }
            })?;
            let parameters = extern_
                .params
                .iter()
                .copied()
                .enumerate()
                .map(|(index, ty)| {
                    if extern_.abi.is_c() && ty == inputs.unit {
                        return Err(HirSourceNativeContractError::UnitCParameter {
                            function: function_id,
                            parameter: index,
                        });
                    }
                    mapper.map(ty, &[]).map_err(|error| {
                        HirSourceNativeContractError::InvalidSignatureType {
                            owner: HirSourceNativeContractOwner::Function(function_id),
                            error,
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = mapper.map(extern_.return_type, &[]).map_err(|error| {
                HirSourceNativeContractError::InvalidSignatureType {
                    owner: HirSourceNativeContractOwner::Function(function_id),
                    error,
                }
            })?;
            let abi = match extern_.abi {
                ExternAbi::C(_) => SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                    parameters,
                    if extern_.return_type == inputs.unit {
                        SourceCAbiReturn::Void
                    } else {
                        SourceCAbiReturn::Value(result)
                    },
                )),
                ExternAbi::Scoop => SourceExternFunctionAbi::Scoop {
                    signature: SourceScoopAbiFunctionSignature::new(parameters, result),
                    gc_effect: persistent_gc_effect(extern_.gc_effect),
                },
            };
            let contract = SourceNativeExternalContract::Function {
                symbol: source_symbol(
                    HirSourceNativeContractOwner::Function(function_id),
                    &extern_.native_symbol,
                )?,
                library: source_library(
                    HirSourceNativeContractOwner::Function(function_id),
                    &extern_.library,
                )?,
                abi,
                calling_convention: SourceCallingConvention::Cdecl,
            };
            contracts.push(HirSourceNativeContract {
                owner: HirSourceNativeContractOwner::Function(function_id),
                record: SourceNativeExternalContractRecord::new(key, contract).map_err(
                    |error| HirSourceNativeContractError::InvalidContract {
                        owner: HirSourceNativeContractOwner::Function(function_id),
                        error,
                    },
                )?,
            });
        }
        if let Some(external) = seen_externs.iter().position(|seen| !seen) {
            return Err(HirSourceNativeContractError::UnownedExternFunction {
                external: external as u32,
            });
        }

        for (global_id, global) in inputs.globals.iter() {
            let GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } = &global.storage
            else {
                continue;
            };
            if local_index(global.property) >= inputs.properties.len() {
                return Err(HirSourceNativeContractError::UnknownGlobalProperty {
                    global: global_id,
                    property: raw_index(global.property),
                });
            }
            let property = &inputs.properties[global.property];
            if property.owner != PropertyOwner::TopLevel || property.ty != global.ty {
                return Err(HirSourceNativeContractError::InvalidGlobalProperty {
                    global: global_id,
                });
            }
            let HirPropertyIdentity::Ordinary(property_identity) =
                &inputs.property_identities[global.property]
            else {
                return Err(HirSourceNativeContractError::UnexpectedExtensionProperty {
                    global: global_id,
                });
            };
            let owner = HirSourceNativeContractOwner::Global(global_id);
            let key = SourceNativeExternalContractKey::property(property_identity.key())
                .map_err(|error| HirSourceNativeContractError::InvalidContract { owner, error })?;
            let storage = mapper.map(global.ty, &[]).map_err(|error| {
                HirSourceNativeContractError::InvalidSignatureType { owner, error }
            })?;
            let symbol = source_symbol(owner, native_symbol)?;
            let library = source_library(owner, library)?;
            let contract = match (global.mutable, *thread_local) {
                (false, false) => SourceNativeExternalContract::ReadOnlyData {
                    symbol,
                    library,
                    storage,
                },
                (true, false) => SourceNativeExternalContract::MutableData {
                    symbol,
                    library,
                    storage,
                },
                (false, true) => SourceNativeExternalContract::ReadOnlyTls {
                    symbol,
                    library,
                    storage,
                },
                (true, true) => SourceNativeExternalContract::MutableTls {
                    symbol,
                    library,
                    storage,
                },
            };
            contracts.push(HirSourceNativeContract {
                owner,
                record: SourceNativeExternalContractRecord::new(key, contract).map_err(
                    |error| HirSourceNativeContractError::InvalidContract { owner, error },
                )?,
            });
        }

        contracts.sort_by_key(|entry| entry.record.id());
        let mut identities = HashSet::with_capacity(contracts.len());
        for entry in &contracts {
            if !identities.insert(entry.record.id()) {
                return Err(HirSourceNativeContractError::DuplicateIdentity { owner: entry.owner });
            }
        }
        Ok(Self { contracts })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirSourceNativeContract> {
        self.contracts.iter()
    }

    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }
}

fn validate_function(
    id: FunctionId,
    function: &Function,
    extern_: &ExternFunction,
) -> Result<(), HirSourceNativeContractError> {
    if function.method.is_some()
        || function.name != extern_.source_name
        || function.return_ty != extern_.return_type
        || function.attributes.calling_convention != extern_.calling_convention
        || function.attributes.gc_effect != extern_.gc_effect
        || function.attributes.safety != extern_.safety
    {
        return Err(HirSourceNativeContractError::FunctionRelation { function: id });
    }
    Ok(())
}

fn persistent_gc_effect(effect: GcEffect) -> PersistentGcEffect {
    match effect {
        GcEffect::Managed => PersistentGcEffect::Managed,
        GcEffect::NoGc => PersistentGcEffect::NoGc,
    }
}

fn source_symbol(
    owner: HirSourceNativeContractOwner,
    symbol: &str,
) -> Result<SourceNativeSymbol, HirSourceNativeContractError> {
    SourceNativeSymbol::new(symbol)
        .map_err(|error| HirSourceNativeContractError::InvalidSymbol { owner, error })
}

fn source_library(
    owner: HirSourceNativeContractOwner,
    library: &str,
) -> Result<SourceNativeLibraryBinding, HirSourceNativeContractError> {
    if library.is_empty() {
        Ok(SourceNativeLibraryBinding::DefaultNativeNamespace)
    } else {
        CanonicalNativeLibraryName::new(library)
            .map(SourceNativeLibraryBinding::LogicalLibrary)
            .map_err(|error| HirSourceNativeContractError::InvalidLibrary { owner, error })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
