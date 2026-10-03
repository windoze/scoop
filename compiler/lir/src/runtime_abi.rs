//! Closed runtime and target-EH symbol contracts shared by LIR, codegen and
//! link-object verification.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{BackendProfileWireId, TargetProfileWireId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{
    BackendProfileFingerprint, LirTargetProfile, ManagedRuntimeFunction, NoGcRuntimeFunction,
    RuntimeFunction, TargetProfileFingerprint, ValidatedLirTargetSelection,
};

const RUNTIME_ABI_DOMAIN: &str = "scoop-runtime-abi-contract-v1";
const RUNTIME_SYMBOL_CONTRACT_DOMAIN: &str = "scoop-runtime-symbol-contract-v1";
const TARGET_EH_REQUIREMENT_DOMAIN: &str = "scoop-target-eh-requirement-v1";
const INITIAL_SCHEMA: u64 = 1;

mod machine;
pub use machine::{CompilerNativeContractV1, CompilerNativeValueV1};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAbiFingerprint([u8; 32]);

impl RuntimeAbiFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for RuntimeAbiFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for RuntimeAbiFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeAbiContract;

impl RuntimeAbiContract {
    pub fn fingerprint(self) -> Result<RuntimeAbiFingerprint, HashError> {
        domain_separated_cbor_hash(RUNTIME_ABI_DOMAIN, &self)
            .map(|digest| RuntimeAbiFingerprint(*digest.as_array()))
    }
}

impl WireEncode for RuntimeAbiContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        for field in 1..=3 {
            encoder.field(field)?;
            encoder.unsigned(match field {
                1 => 4,
                3 => 2,
                _ => INITIAL_SCHEMA,
            })?;
        }
        Ok(())
    }
}

/// Every compiler-facing symbol supplied by the Scoop runtime ABI.
///
/// The variants are the semantic identity. Callers must obtain object bytes
/// through [`RuntimeSymbolContractV1::object_symbol`] instead of applying a
/// spelling convention themselves.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAbiSymbolV1 {
    LirCall(RuntimeFunction),
    CoreStringTypeDescriptor,
    ArrayClone,
    AllocationContext,
    CardTable,
    FinishTlabAllocation,
    AllocateSlow,
    BeginCatch,
    EndCatch,
    PushCallerRoots,
    PopCallerRoots,
    PushCompilerRoots,
    PopCompilerRoots,
    PopTopCompilerRoots,
    EnterNativeSafe,
    LeaveNativeSafe,
    EnterNativeBorrowed,
    LeaveNativeBorrowed,
    CallbackRegister,
    CallbackRetain,
    CallbackRelease,
    CallbackFailure,
    CallbackState,
    CallbackInvoke,
}

impl RuntimeAbiSymbolV1 {
    pub const ALL: [Self; 50] = [
        Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::Safepoint)),
        Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::Alloc)),
        Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::BoxZst)),
        Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::BoxValue)),
        Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::GcCollect)),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::MaterializeException,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::StringConcat,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::InitializationEnter,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::InitializationSucceed,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::InitializationFail,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::InitializationFailure,
        )),
        Self::LirCall(RuntimeFunction::Managed(
            ManagedRuntimeFunction::InitializationCycleMessage,
        )),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::IsInstance)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::ITableLookup)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Pin)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Unpin)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::GetHandle)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::ReleaseHandle)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::GcStats)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::StringCompare)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Trap)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Throw)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Rethrow)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxZst)),
        Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxValue)),
        Self::LirCall(RuntimeFunction::NoGc(
            NoGcRuntimeFunction::PushRecursiveRegion,
        )),
        Self::LirCall(RuntimeFunction::NoGc(
            NoGcRuntimeFunction::PopRecursiveRegion,
        )),
        Self::CoreStringTypeDescriptor,
        Self::ArrayClone,
        Self::AllocationContext,
        Self::CardTable,
        Self::FinishTlabAllocation,
        Self::AllocateSlow,
        Self::BeginCatch,
        Self::EndCatch,
        Self::PushCallerRoots,
        Self::PopCallerRoots,
        Self::PushCompilerRoots,
        Self::PopCompilerRoots,
        Self::PopTopCompilerRoots,
        Self::EnterNativeSafe,
        Self::LeaveNativeSafe,
        Self::EnterNativeBorrowed,
        Self::LeaveNativeBorrowed,
        Self::CallbackRegister,
        Self::CallbackRetain,
        Self::CallbackRelease,
        Self::CallbackFailure,
        Self::CallbackState,
        Self::CallbackInvoke,
    ];

    pub const fn logical_symbol(self) -> &'static str {
        match self {
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::Safepoint)) => {
                "scoop_rt_safepoint"
            }
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::Alloc)) => {
                "scoop_rt_alloc"
            }
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::BoxZst)) => {
                "scoop_rt_box_zst"
            }
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::BoxValue)) => {
                "scoop_rt_box_value"
            }
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::GcCollect)) => {
                "scoop_rt_gc_collect"
            }
            Self::LirCall(RuntimeFunction::Managed(
                ManagedRuntimeFunction::MaterializeException,
            )) => "scoop_rt_materialize_exception",
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::StringConcat)) => {
                "scoop_rt_string_concat"
            }
            Self::LirCall(RuntimeFunction::Managed(
                ManagedRuntimeFunction::InitializationEnter,
            )) => "scoop_rt_init_enter",
            Self::LirCall(RuntimeFunction::Managed(
                ManagedRuntimeFunction::InitializationSucceed,
            )) => "scoop_rt_init_succeed",
            Self::LirCall(RuntimeFunction::Managed(ManagedRuntimeFunction::InitializationFail)) => {
                "scoop_rt_init_fail"
            }
            Self::LirCall(RuntimeFunction::Managed(
                ManagedRuntimeFunction::InitializationFailure,
            )) => "scoop_rt_init_failure",
            Self::LirCall(RuntimeFunction::Managed(
                ManagedRuntimeFunction::InitializationCycleMessage,
            )) => "scoop_rt_init_cycle_message",
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::IsInstance)) => {
                "scoop_rt_is_instance"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::ITableLookup)) => {
                "scoop_rt_itable_lookup"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Pin)) => "scoop_rt_pin",
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Unpin)) => "scoop_rt_unpin",
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::GetHandle)) => {
                "scoop_rt_get_handle"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::ReleaseHandle)) => {
                "scoop_rt_release_handle"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::GcStats)) => {
                "scoop_rt_gc_stats"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::StringCompare)) => {
                "scoop_rt_string_compare"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Trap)) => "scoop_rt_trap",
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Throw)) => "scoop_rt_throw",
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::Rethrow)) => {
                "scoop_rt_rethrow"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxZst)) => {
                "scoop_rt_unbox_zst"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxValue)) => {
                "scoop_rt_unbox_value"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::PushRecursiveRegion)) => {
                "scoop_rt_push_native_region_roots"
            }
            Self::LirCall(RuntimeFunction::NoGc(NoGcRuntimeFunction::PopRecursiveRegion)) => {
                "scoop_rt_pop_native_region_roots"
            }
            Self::CoreStringTypeDescriptor => "scoop_td_String",
            Self::ArrayClone => "scoop_rt_array_clone",
            Self::AllocationContext => "scoop_rt_allocation_context",
            Self::CardTable => "scoop_gc_card_table",
            Self::FinishTlabAllocation => "scoop_runtime_finish_tlab_alloc",
            Self::AllocateSlow => "scoop_runtime_alloc_slow",
            Self::BeginCatch => "scoop_rt_begin_catch",
            Self::EndCatch => "scoop_rt_end_catch",
            Self::PushCallerRoots => "scoop_rt_push_caller_roots",
            Self::PopCallerRoots => "scoop_rt_pop_caller_roots",
            Self::PushCompilerRoots => "scoop_rt_push_compiler_roots",
            Self::PopCompilerRoots => "scoop_rt_pop_compiler_roots",
            Self::PopTopCompilerRoots => "scoop_rt_pop_top_compiler_roots",
            Self::EnterNativeSafe => "scoop_rt_enter_native_safe",
            Self::LeaveNativeSafe => "scoop_rt_leave_native_safe",
            Self::EnterNativeBorrowed => "scoop_rt_enter_native_borrowed",
            Self::LeaveNativeBorrowed => "scoop_rt_leave_native_borrowed",
            Self::CallbackRegister => "scoop_runtime_callback_register",
            Self::CallbackRetain => "scoop_runtime_callback_retain",
            Self::CallbackRelease => "scoop_runtime_callback_release",
            Self::CallbackFailure => "scoop_runtime_callback_failure",
            Self::CallbackState => "scoop_runtime_callback_state",
            Self::CallbackInvoke => "scoop_runtime_callback_invoke",
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::LirCall(RuntimeFunction::Managed(_)) => 1,
            Self::LirCall(RuntimeFunction::NoGc(_)) => 2,
            Self::CoreStringTypeDescriptor => 3,
            Self::ArrayClone => 4,
            Self::AllocationContext => 5,
            Self::CardTable => 6,
            Self::FinishTlabAllocation => 7,
            Self::AllocateSlow => 8,
            Self::BeginCatch => 9,
            Self::EndCatch => 10,
            Self::PushCallerRoots => 11,
            Self::PopCallerRoots => 12,
            Self::PushCompilerRoots => 13,
            Self::PopCompilerRoots => 14,
            Self::PopTopCompilerRoots => 15,
            Self::EnterNativeSafe => 16,
            Self::LeaveNativeSafe => 17,
            Self::EnterNativeBorrowed => 18,
            Self::LeaveNativeBorrowed => 19,
            Self::CallbackRegister => 20,
            Self::CallbackRetain => 21,
            Self::CallbackRelease => 22,
            Self::CallbackFailure => 23,
            Self::CallbackState => 24,
            Self::CallbackInvoke => 25,
        }
    }
}

impl WireEncode for RuntimeAbiSymbolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::LirCall(RuntimeFunction::Managed(function)) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(self.tag())?;
                encoder.field(1)?;
                encoder.unsigned(RuntimeFunction::Managed(*function).wire_function_tag())
            }
            Self::LirCall(RuntimeFunction::NoGc(function)) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(self.tag())?;
                encoder.field(1)?;
                encoder.unsigned(RuntimeFunction::NoGc(*function).wire_function_tag())
            }
            _ => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(self.tag())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeSymbolContractId([u8; 32]);

impl RuntimeSymbolContractId {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for RuntimeSymbolContractId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for RuntimeSymbolContractId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSymbolContractV1 {
    id: RuntimeSymbolContractId,
    runtime_abi: RuntimeAbiFingerprint,
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    symbol: RuntimeAbiSymbolV1,
}

impl RuntimeSymbolContractV1 {
    pub fn current(
        target: LirTargetProfile,
        symbol: RuntimeAbiSymbolV1,
    ) -> Result<Self, RuntimeRequirementRegistryError> {
        let input = RuntimeSymbolContractInputV1 {
            runtime_abi: RuntimeAbiContract.fingerprint()?,
            target: target.wire_id(),
            target_fingerprint: target.fingerprint()?,
            symbol,
        };
        let digest = domain_separated_cbor_hash(RUNTIME_SYMBOL_CONTRACT_DOMAIN, &input)?;
        Ok(Self {
            id: RuntimeSymbolContractId(*digest.as_array()),
            runtime_abi: input.runtime_abi,
            target: input.target,
            target_fingerprint: input.target_fingerprint,
            symbol,
        })
    }

    pub const fn id(&self) -> RuntimeSymbolContractId {
        self.id
    }

    pub const fn runtime_abi(&self) -> RuntimeAbiFingerprint {
        self.runtime_abi
    }

    pub const fn target(&self) -> &TargetProfileWireId {
        &self.target
    }

    pub const fn target_fingerprint(&self) -> TargetProfileFingerprint {
        self.target_fingerprint
    }

    pub const fn symbol(&self) -> RuntimeAbiSymbolV1 {
        self.symbol
    }

    pub fn object_symbol(&self, target: LirTargetProfile) -> Vec<u8> {
        target
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(self.symbol.logical_symbol())
            .into_bytes()
    }
}

impl WireEncode for RuntimeSymbolContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        RuntimeSymbolContractInputV1 {
            runtime_abi: self.runtime_abi,
            target: self.target.clone(),
            target_fingerprint: self.target_fingerprint,
            symbol: self.symbol,
        }
        .encode(encoder)
    }
}

struct RuntimeSymbolContractInputV1 {
    runtime_abi: RuntimeAbiFingerprint,
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    symbol: RuntimeAbiSymbolV1,
}

impl WireEncode for RuntimeSymbolContractInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.runtime_abi.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.symbol.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSymbolContractRegistryV1 {
    target: LirTargetProfile,
    by_object_symbol: BTreeMap<Vec<u8>, RuntimeSymbolContractV1>,
}

impl RuntimeSymbolContractRegistryV1 {
    pub fn current(target: LirTargetProfile) -> Result<Self, RuntimeRequirementRegistryError> {
        let mut by_object_symbol = BTreeMap::new();
        for symbol in RuntimeAbiSymbolV1::ALL {
            let contract = RuntimeSymbolContractV1::current(target, symbol)?;
            let object_symbol = contract.object_symbol(target);
            if by_object_symbol
                .insert(object_symbol.clone(), contract)
                .is_some()
            {
                return Err(RuntimeRequirementRegistryError::DuplicateObjectSymbol(
                    object_symbol,
                ));
            }
        }
        Ok(Self {
            target,
            by_object_symbol,
        })
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub fn contracts(&self) -> impl ExactSizeIterator<Item = &RuntimeSymbolContractV1> {
        self.by_object_symbol.values()
    }

    pub fn contract_for_object_symbol(&self, symbol: &[u8]) -> Option<&RuntimeSymbolContractV1> {
        self.by_object_symbol.get(symbol)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetEhSupportV1 {
    ScoopPersonality,
    UnwindResume,
}

impl TargetEhSupportV1 {
    pub const ALL: [Self; 2] = [Self::ScoopPersonality, Self::UnwindResume];

    pub const fn logical_symbol(self) -> &'static str {
        match self {
            Self::ScoopPersonality => "scoop_eh_personality",
            Self::UnwindResume => "_Unwind_Resume",
        }
    }
}

impl WireEncode for TargetEhSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ScoopPersonality => 1,
            Self::UnwindResume => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetEhRequirementId([u8; 32]);

impl TargetEhRequirementId {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for TargetEhRequirementId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for TargetEhRequirementId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetEhRequirementV1 {
    id: TargetEhRequirementId,
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    backend: BackendProfileWireId,
    backend_fingerprint: BackendProfileFingerprint,
    runtime_abi: RuntimeAbiFingerprint,
    support: TargetEhSupportV1,
}

impl TargetEhRequirementV1 {
    pub fn current(
        selection: ValidatedLirTargetSelection,
        support: TargetEhSupportV1,
    ) -> Result<Self, RuntimeRequirementRegistryError> {
        let target = selection.target();
        let backend = selection.backend();
        let input = TargetEhRequirementInputV1 {
            target: target.wire_id(),
            target_fingerprint: target.fingerprint()?,
            backend: backend.wire_id(),
            backend_fingerprint: backend.fingerprint()?,
            runtime_abi: RuntimeAbiContract.fingerprint()?,
            support,
        };
        let digest = domain_separated_cbor_hash(TARGET_EH_REQUIREMENT_DOMAIN, &input)?;
        Ok(Self {
            id: TargetEhRequirementId(*digest.as_array()),
            target: input.target,
            target_fingerprint: input.target_fingerprint,
            backend: input.backend,
            backend_fingerprint: input.backend_fingerprint,
            runtime_abi: input.runtime_abi,
            support,
        })
    }

    pub const fn id(&self) -> TargetEhRequirementId {
        self.id
    }

    pub const fn support(&self) -> TargetEhSupportV1 {
        self.support
    }

    pub fn object_symbol(&self, target: LirTargetProfile) -> Vec<u8> {
        target
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(self.support.logical_symbol())
            .into_bytes()
    }
}

impl WireEncode for TargetEhRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        TargetEhRequirementInputV1 {
            target: self.target.clone(),
            target_fingerprint: self.target_fingerprint,
            backend: self.backend.clone(),
            backend_fingerprint: self.backend_fingerprint,
            runtime_abi: self.runtime_abi,
            support: self.support,
        }
        .encode(encoder)
    }
}

struct TargetEhRequirementInputV1 {
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    backend: BackendProfileWireId,
    backend_fingerprint: BackendProfileFingerprint,
    runtime_abi: RuntimeAbiFingerprint,
    support: TargetEhSupportV1,
}

impl WireEncode for TargetEhRequirementInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.backend.encode(encoder)?;
        encoder.field(4)?;
        self.backend_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.runtime_abi.encode(encoder)?;
        encoder.field(6)?;
        self.support.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetEhRequirementRegistryV1 {
    selection: ValidatedLirTargetSelection,
    by_object_symbol: BTreeMap<Vec<u8>, TargetEhRequirementV1>,
}

impl TargetEhRequirementRegistryV1 {
    pub fn current(
        selection: ValidatedLirTargetSelection,
    ) -> Result<Self, RuntimeRequirementRegistryError> {
        let mut by_object_symbol = BTreeMap::new();
        for support in TargetEhSupportV1::ALL {
            let requirement = TargetEhRequirementV1::current(selection, support)?;
            let object_symbol = requirement.object_symbol(selection.target());
            if by_object_symbol
                .insert(object_symbol.clone(), requirement)
                .is_some()
            {
                return Err(RuntimeRequirementRegistryError::DuplicateObjectSymbol(
                    object_symbol,
                ));
            }
        }
        Ok(Self {
            selection,
            by_object_symbol,
        })
    }

    pub const fn selection(&self) -> ValidatedLirTargetSelection {
        self.selection
    }

    pub fn requirements(&self) -> impl ExactSizeIterator<Item = &TargetEhRequirementV1> {
        self.by_object_symbol.values()
    }

    pub fn requirement_for_object_symbol(&self, symbol: &[u8]) -> Option<&TargetEhRequirementV1> {
        self.by_object_symbol.get(symbol)
    }
}

#[derive(Debug)]
pub enum RuntimeRequirementRegistryError {
    Hash(HashError),
    DuplicateObjectSymbol(Vec<u8>),
}

impl From<HashError> for RuntimeRequirementRegistryError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

impl fmt::Display for RuntimeRequirementRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid runtime requirement registry: {self:?}")
    }
}

impl std::error::Error for RuntimeRequirementRegistryError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use scoop_wire::encode;

    use super::*;

    #[test]
    fn runtime_abi_contract_versions_the_unified_initialization_record() {
        assert_eq!(hex(&encode(&RuntimeAbiContract).unwrap()), "a3010402010302");
        assert_eq!(
            RuntimeAbiContract.fingerprint().unwrap().to_string(),
            "b1738954278c8af3c6285bc5513d2a25aef20dfb55efa2481f9632df8c086186"
        );
    }

    #[test]
    fn boxing_runtime_entries_retire_the_old_tag_without_reusing_it() {
        assert_eq!(RuntimeFunction::from_wire_tags(1, 3), None);
        let entries = [
            (
                RuntimeFunction::Managed(ManagedRuntimeFunction::BoxZst),
                1,
                12,
                "scoop_rt_box_zst",
            ),
            (
                RuntimeFunction::Managed(ManagedRuntimeFunction::BoxValue),
                1,
                13,
                "scoop_rt_box_value",
            ),
            (
                RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxZst),
                2,
                12,
                "scoop_rt_unbox_zst",
            ),
            (
                RuntimeFunction::NoGc(NoGcRuntimeFunction::UnboxValue),
                2,
                13,
                "scoop_rt_unbox_value",
            ),
            (
                RuntimeFunction::NoGc(NoGcRuntimeFunction::PushRecursiveRegion),
                2,
                14,
                "scoop_rt_push_native_region_roots",
            ),
            (
                RuntimeFunction::NoGc(NoGcRuntimeFunction::PopRecursiveRegion),
                2,
                15,
                "scoop_rt_pop_native_region_roots",
            ),
        ];
        for (function, protocol, tag, symbol) in entries {
            assert_eq!(
                RuntimeFunction::from_wire_tags(protocol, tag),
                Some(function)
            );
            assert_eq!(function.symbol(), symbol);
            assert!(function.requires_dedicated_operation());
        }
    }

    #[test]
    fn runtime_registry_has_one_contract_per_typed_symbol() {
        let target = LirTargetProfile::DARWIN_AARCH64;
        let registry = RuntimeSymbolContractRegistryV1::current(target).unwrap();
        assert_eq!(registry.contracts().len(), RuntimeAbiSymbolV1::ALL.len());
        assert_eq!(
            registry
                .contracts()
                .map(RuntimeSymbolContractV1::id)
                .collect::<BTreeSet<_>>()
                .len(),
            RuntimeAbiSymbolV1::ALL.len()
        );
        let allocation = registry
            .contract_for_object_symbol(b"_scoop_rt_allocation_context")
            .unwrap();
        assert_eq!(allocation.symbol(), RuntimeAbiSymbolV1::AllocationContext);
        assert_eq!(
            allocation.id().to_string(),
            "3279288a5abe80adfc133b11797600f808f562135cac145c707850b5346b2c6c"
        );
        assert!(
            registry
                .contract_for_object_symbol(b"_scoop_rt_unknown")
                .is_none()
        );
        assert_eq!(
            hex(
                &encode(&RuntimeAbiSymbolV1::LirCall(RuntimeFunction::Managed(
                    ManagedRuntimeFunction::Safepoint,
                )))
                .unwrap()
            ),
            "a200010101"
        );
        assert_eq!(
            hex(&encode(&RuntimeAbiSymbolV1::LirCall(RuntimeFunction::NoGc(
                NoGcRuntimeFunction::Rethrow,
            )))
            .unwrap()),
            "a20002010b"
        );
        assert_eq!(
            hex(&encode(&RuntimeAbiSymbolV1::CallbackInvoke).unwrap()),
            "a1001819"
        );
    }

    #[test]
    fn target_eh_registry_normalizes_each_closed_requirement() {
        let registry = TargetEhRequirementRegistryV1::current(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        assert_eq!(registry.requirements().len(), 2);
        assert_eq!(
            registry
                .requirement_for_object_symbol(b"_scoop_eh_personality")
                .unwrap()
                .support(),
            TargetEhSupportV1::ScoopPersonality
        );
        assert_eq!(
            registry
                .requirement_for_object_symbol(b"__Unwind_Resume")
                .unwrap()
                .support(),
            TargetEhSupportV1::UnwindResume
        );
        assert_eq!(
            registry
                .requirement_for_object_symbol(b"_scoop_eh_personality")
                .unwrap()
                .id()
                .to_string(),
            "03f667982cb2fbb7f8b3b58877d08638730c629e294a7c2ee97c9e0ea45fed07"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
