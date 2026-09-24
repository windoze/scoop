use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;
use std::sync::Arc;

use scoop_hir::{
    NativeBoundaryCLayoutPolicy, NativeBoundaryNominalOwner, NativeBoundaryNominalShape,
    NativeBoundaryTypeDefinitionRecord,
};
use scoop_identity::{
    CDataPointee, CPointerStorage, CallableApplicationKey, CallableArguments,
    CallableInstantiationOwner, CallableMaterializationContext, CallbackApplicationKey,
    CallbackRegistrationKey, CanonicalCAbiFunctionSignature, CanonicalCAbiLayout,
    CanonicalCAbiLayoutField, CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayoutFingerprintRecord,
    CanonicalCAbiParameter, CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprint,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, CanonicalNativeLibraryName,
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, CborIdentityRecord,
    ExactCallableSignature, ExactTypeKey, GcEffect, IdentityLayer, InitializationUnitKey,
    NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
    NativeLibraryBinding, NativeLinkRequirementId, NativeLinkRequirementKey, NonEmptyVec,
    OptionalSignatureType, PersistentCallableApplicationId, PersistentCallbackApplicationId,
    PersistentCallbackRegistrationId, PersistentExactTypeId, PersistentInitializationUnitId,
    ScoopAbiArgument, ScoopAbiReturn, ScoopAbiValueShape, SignatureCallableShape, SignatureTypeKey,
    SourceCAbiFunctionSignature, SourceCAbiReturn, SourceExternFunctionAbi,
    SourceNativeExternalContract, SourceNativeExternalContractRecord, SourceNativeLibraryBinding,
    SourceScoopAbiFunctionSignature, TargetCallingConvention,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    NativeBoundaryCompileError, NativeBoundaryFoundationView,
    NativeBoundarySourceValidatedFoundations, index_records, records_by_id,
};
use crate::ValidatedGraphArtifact;

mod errors;
mod layout;
mod nominals;
pub use errors::NativeBoundaryTargetError;
use nominals::AbiNominalDefinition;
mod physical;
mod scoop_abi;
use physical::*;

use scoop_abi::exact_type_records;
pub(crate) use scoop_abi::{AbiReplayDependency, replay_canonical_scoop_abi};

/// Foundation payloads whose source closure and every target-specific native
/// ABI leaf were independently recomputed from canonical identities.
pub struct NativeBoundaryValidatedFoundations<'input> {
    pub(crate) foundations: super::super::StructurallyValidatedFoundations<'input>,
}

impl<'input> NativeBoundarySourceValidatedFoundations<'input> {
    pub fn validate_target(
        mut self,
    ) -> Result<NativeBoundaryValidatedFoundations<'input>, NativeBoundaryCompileError> {
        let view = NativeBoundaryFoundationView {
            source_contracts: self.foundations.hir.source_native_contracts(),
            type_definitions: self.foundations.hir.native_boundary_types(),
            callback_applications: self.foundations.mir.callback_application_records(),
            native_contracts: self.foundations.lir.native_contracts(),
            c_abi_signatures: self.foundations.lir.c_abi_signatures(),
            c_abi_layouts: self.foundations.lir.c_abi_layouts(),
            callback_bridges: self.foundations.lir.callback_bridges(),
        };
        validate_target_normalization(
            &mut self.foundations.graph,
            &self.foundations.identities,
            &view,
        )?;
        Ok(NativeBoundaryValidatedFoundations {
            foundations: self.foundations,
        })
    }
}

impl NativeBoundaryValidatedFoundations<'_> {
    pub const fn foundations(&self) -> &super::super::StructurallyValidatedFoundations<'_> {
        &self.foundations
    }
}

pub(super) fn validate_target_normalization(
    artifact: &mut ValidatedGraphArtifact<'_>,
    graph: &scoop_identity::ValidatedIdentityGraph,
    view: &NativeBoundaryFoundationView<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    let target = artifact.target_selection().target();
    let meter = artifact.envelope.meter_mut();
    let exact_types = exact_type_records(graph, meter)?;
    let callable_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallableApplicationId, CallableApplicationKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(17),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(17),
    )?;
    let initialization_units = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentInitializationUnitId, InitializationUnitKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(21),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(21),
    )?;
    let callback_registrations = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(25),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(25),
    )?;
    let callback_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(
                    IdentityLayer::Mir,
                    meter,
                    &WirePath::root().field(9),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(9),
    )?;
    let definitions = nominals::native_definitions(view.type_definitions, meter)?;

    let actual_contracts = index_records(
        view.native_contracts,
        NativeExternalContractRecord::source,
        meter,
        &WirePath::root().field(14),
    )?;
    let application_records = index_records(
        view.callback_applications,
        scoop_mir::CallbackApplicationRecord::application,
        meter,
        &WirePath::root().field(10),
    )?;
    let actual_signatures = index_records(
        view.c_abi_signatures,
        CanonicalCAbiSignatureFingerprintRecord::fingerprint,
        meter,
        &WirePath::root().field(15),
    )?;
    let actual_layouts = index_records(
        view.c_abi_layouts,
        CanonicalCAbiLayoutFingerprintRecord::fingerprint,
        meter,
        &WirePath::root().field(16),
    )?;
    let actual_requirements = records_by_id(
        std::iter::once(
            graph
                .records::<NativeLinkRequirementId, NativeLinkRequirementKey>(
                    IdentityLayer::Lir,
                    meter,
                    &WirePath::root().field(20),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        meter,
        &WirePath::root().field(20),
    )?;
    let mut expected_contracts = HashMap::new();
    meter
        .try_reserve_map_slots(
            &mut expected_contracts,
            view.source_contracts.len(),
            &WirePath::root().field(14),
        )
        .map_err(NativeBoundaryCompileError::Resource)?;
    let mut normalizer = NativeBoundaryNormalizer::new(
        target,
        meter,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );

    for source in view.source_contracts {
        let expected = normalizer.normalize_external(source)?;
        expected_contracts.insert(expected.source(), expected);
    }
    require_equal_records(
        &expected_contracts,
        &actual_contracts,
        normalizer.meter,
        &WirePath::root().field(14),
        NativeBoundaryTargetError::NativeContractMismatch,
    )?;

    for bridge in view.callback_bridges {
        charge_relations(normalizer.meter, 1, &WirePath::root().field(13))?;
        let application = callback_applications.get(&bridge.application()).ok_or(
            NativeBoundaryTargetError::MissingCallbackApplication {
                application: bridge.application(),
            },
        )?;
        charge_relations(normalizer.meter, 1, &WirePath::root().field(25))?;
        let registration = callback_registrations
            .get(&application.registration())
            .ok_or(NativeBoundaryTargetError::MissingCallbackRegistration {
                registration: application.registration(),
            })?;
        let binders = normalizer.binders(application.context())?;
        let expected = normalizer.c_signature(registration.source_signature(), &binders)?;
        if expected.fingerprint() != bridge.signature() {
            return Err(NativeBoundaryTargetError::CallbackSignatureMismatch {
                application: bridge.application(),
            }
            .into());
        }
        insert_metered(
            normalizer.meter,
            &mut normalizer.expected_signatures,
            expected.fingerprint(),
            expected,
            &WirePath::root().field(15),
        )?;

        let expected_managed =
            normalizer.managed_signature(registration.managed_signature(), &binders)?;
        charge_relations(normalizer.meter, 1, &WirePath::root().field(10))?;
        let actual = application_records.get(&bridge.application()).ok_or(
            NativeBoundaryTargetError::MissingCallbackApplication {
                application: bridge.application(),
            },
        )?;
        if actual.managed_signature() != &expected_managed {
            return Err(
                NativeBoundaryTargetError::ManagedCallbackSignatureMismatch {
                    application: bridge.application(),
                }
                .into(),
            );
        }
    }

    require_equal_records(
        &normalizer.expected_signatures,
        &actual_signatures,
        normalizer.meter,
        &WirePath::root().field(15),
        NativeBoundaryTargetError::CAbiSignatureSetMismatch,
    )?;

    require_equal_records(
        &normalizer.expected_layouts,
        &actual_layouts,
        normalizer.meter,
        &WirePath::root().field(16),
        NativeBoundaryTargetError::CAbiLayoutSetMismatch,
    )?;

    require_equal_records(
        &normalizer.expected_requirements,
        &actual_requirements,
        normalizer.meter,
        &WirePath::root().field(20),
        NativeBoundaryTargetError::NativeRequirementSetMismatch,
    )
}

fn require_equal_records<K, V, A>(
    expected: &HashMap<K, V>,
    actual: &HashMap<K, A>,
    meter: &mut BudgetMeter,
    path: &WirePath,
    error: NativeBoundaryTargetError,
) -> Result<(), NativeBoundaryCompileError>
where
    K: Eq + std::hash::Hash,
    V: Eq,
    A: std::borrow::Borrow<V>,
{
    charge_relations(meter, expected.len(), path)?;
    if expected.len() != actual.len()
        || expected
            .iter()
            .any(|(key, value)| actual.get(key).map(std::borrow::Borrow::borrow) != Some(value))
    {
        Err(error.into())
    } else {
        Ok(())
    }
}

fn insert_metered<K, V>(
    meter: &mut BudgetMeter,
    records: &mut HashMap<K, V>,
    key: K,
    value: V,
    path: &WirePath,
) -> Result<Option<V>, NativeBoundaryCompileError>
where
    K: Eq + std::hash::Hash,
{
    if !records.contains_key(&key) {
        meter
            .try_reserve_map_slots(records, 1, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
    }
    Ok(records.insert(key, value))
}

fn charge_relations(
    meter: &mut BudgetMeter,
    count: usize,
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    let count = u64::try_from(count).map_err(|_| {
        NativeBoundaryCompileError::Resource(scoop_wire::WireError::new(
            scoop_wire::WireErrorKind::IntegerOutOfRange,
            path.clone(),
            None,
        ))
    })?;
    meter
        .charge_edges(count, path)
        .map_err(NativeBoundaryCompileError::Resource)
}

fn metered_vec<T>(
    meter: &mut BudgetMeter,
    capacity: usize,
    path: &WirePath,
) -> Result<Vec<T>, NativeBoundaryCompileError> {
    let mut values = Vec::new();
    meter
        .try_reserve_collection_slots(&mut values, capacity, path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    Ok(values)
}

fn clone_c_signature(
    meter: &mut BudgetMeter,
    signature: &CanonicalCAbiFunctionSignature,
    path: &WirePath,
) -> Result<CanonicalCAbiFunctionSignature, NativeBoundaryCompileError> {
    let mut parameters = metered_vec(meter, signature.parameters().len(), path)?;
    parameters.extend_from_slice(signature.parameters());
    Ok(CanonicalCAbiFunctionSignature::cdecl(
        parameters,
        signature.result(),
    ))
}

fn push_binder_group(
    meter: &mut BudgetMeter,
    binders: &mut Vec<Vec<PersistentExactTypeId>>,
    arguments: &[PersistentExactTypeId],
    path: &WirePath,
) -> Result<(), NativeBoundaryCompileError> {
    charge_relations(meter, arguments.len(), path)?;
    meter
        .try_reserve_collection_slots(binders, 1, path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    let mut group = metered_vec(meter, arguments.len(), path)?;
    group.extend_from_slice(arguments);
    binders.push(group);
    Ok(())
}

struct MeteredVisitingSet<T> {
    entries: HashSet<T>,
    charged_capacity: usize,
}

impl<T> MeteredVisitingSet<T>
where
    T: Copy + Eq + std::hash::Hash,
{
    fn new() -> Self {
        Self {
            entries: HashSet::new(),
            charged_capacity: 0,
        }
    }

    fn push(
        &mut self,
        value: T,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, NativeBoundaryCompileError> {
        if self.entries.contains(&value) {
            return Ok(false);
        }
        let next_len = self.entries.len().checked_add(1).ok_or_else(|| {
            NativeBoundaryCompileError::Resource(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        let depth = u64::try_from(next_len).map_err(|_| {
            NativeBoundaryCompileError::Resource(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        meter
            .check_semantic_depth(depth, path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        if next_len > self.charged_capacity {
            meter
                .try_reserve_set_slots(&mut self.entries, next_len - self.charged_capacity, path)
                .map_err(NativeBoundaryCompileError::Resource)?;
            self.charged_capacity = next_len;
        }
        self.entries.insert(value);
        Ok(true)
    }

    fn remove(&mut self, value: &T) {
        self.entries.remove(value);
    }
}

struct NativeBoundaryNormalizer<'a> {
    target: scoop_lir::LirTargetProfile,
    meter: &'a mut BudgetMeter,
    exact_types: &'a HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    callable_applications:
        &'a HashMap<PersistentCallableApplicationId, Arc<CallableApplicationKey>>,
    initialization_units: &'a HashMap<PersistentInitializationUnitId, Arc<InitializationUnitKey>>,
    definitions: &'a HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
    expected_signatures:
        HashMap<CanonicalCAbiSignatureFingerprint, CanonicalCAbiSignatureFingerprintRecord>,
    expected_layouts: HashMap<CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayoutFingerprintRecord>,
    layouts_by_type: HashMap<PersistentExactTypeId, CanonicalCAbiLayoutFingerprint>,
    visiting_c_layouts: MeteredVisitingSet<PersistentExactTypeId>,
    scoop_layouts: HashMap<PersistentExactTypeId, PhysicalType>,
    visiting_scoop_layouts: MeteredVisitingSet<PersistentExactTypeId>,
    expected_requirements: HashMap<NativeLinkRequirementId, NativeLinkRequirementKey>,
}

impl<'a> NativeBoundaryNormalizer<'a> {
    fn new(
        target: scoop_lir::LirTargetProfile,
        meter: &'a mut BudgetMeter,
        exact_types: &'a HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
        callable_applications: &'a HashMap<
            PersistentCallableApplicationId,
            Arc<CallableApplicationKey>,
        >,
        initialization_units: &'a HashMap<
            PersistentInitializationUnitId,
            Arc<InitializationUnitKey>,
        >,
        definitions: &'a HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
    ) -> Self {
        Self {
            target,
            meter,
            exact_types,
            callable_applications,
            initialization_units,
            definitions,
            expected_signatures: HashMap::new(),
            expected_layouts: HashMap::new(),
            layouts_by_type: HashMap::new(),
            visiting_c_layouts: MeteredVisitingSet::new(),
            scoop_layouts: HashMap::new(),
            visiting_scoop_layouts: MeteredVisitingSet::new(),
            expected_requirements: HashMap::new(),
        }
    }

    fn normalize_external(
        &mut self,
        source: &SourceNativeExternalContractRecord,
    ) -> Result<NativeExternalContractRecord, NativeBoundaryCompileError> {
        let (symbol, source_library) = source_target(source.contract());
        let symbol = match self.target.id() {
            scoop_lir::TargetProfileId::DarwinAarch64 => {
                let path = WirePath::root().field(14);
                let normalized_length =
                    NativeExternalSymbolKey::darwin_macho_external_length(symbol)
                        .map_err(NativeBoundaryTargetError::NativeSymbol)?;
                self.meter
                    .charge_owned_bytes(normalized_length, &path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                NativeExternalSymbolKey::darwin_macho_external(symbol)
                    .map_err(NativeBoundaryTargetError::NativeSymbol)?
            }
        };
        let library = self.library(source_library)?;
        let contract = match source.contract() {
            SourceNativeExternalContract::Function { abi, .. } => match abi {
                SourceExternFunctionAbi::C(signature) => {
                    let record = self.c_signature(signature, &[])?;
                    let signature = clone_c_signature(
                        self.meter,
                        record.signature(),
                        &WirePath::root().field(15),
                    )?;
                    insert_metered(
                        self.meter,
                        &mut self.expected_signatures,
                        record.fingerprint(),
                        record,
                        &WirePath::root().field(15),
                    )?;
                    NativeExternalContract::c_function(library, signature)
                }
                SourceExternFunctionAbi::Scoop {
                    signature,
                    gc_effect,
                } => NativeExternalContract::scoop_function(
                    library,
                    self.scoop_signature(signature, *gc_effect, &[])?,
                    TargetCallingConvention::Cdecl,
                ),
            },
            SourceNativeExternalContract::ReadOnlyData { storage, .. } => {
                let exact = self.signature_exact(storage, &[])?;
                NativeExternalContract::read_only_data(library, self.c_storage(exact)?)
            }
            SourceNativeExternalContract::MutableData { storage, .. } => {
                let exact = self.signature_exact(storage, &[])?;
                NativeExternalContract::mutable_data(library, self.c_storage(exact)?)
            }
            SourceNativeExternalContract::ReadOnlyTls { storage, .. } => {
                let exact = self.signature_exact(storage, &[])?;
                NativeExternalContract::read_only_tls(library, self.c_storage(exact)?)
            }
            SourceNativeExternalContract::MutableTls { storage, .. } => {
                let exact = self.signature_exact(storage, &[])?;
                NativeExternalContract::mutable_tls(library, self.c_storage(exact)?)
            }
        };
        for stream_length in NativeExternalContractRecord::hash_stream_lengths(&symbol, &contract)
            .map_err(NativeBoundaryTargetError::Hash)?
        {
            self.meter
                .charge_sha256(stream_length, &WirePath::root().field(14))
                .map_err(NativeBoundaryCompileError::Resource)?;
        }
        NativeExternalContractRecord::new(source.id(), symbol, contract)
            .map_err(NativeBoundaryTargetError::Hash)
            .map_err(Into::into)
    }

    fn library(
        &mut self,
        source: &SourceNativeLibraryBinding,
    ) -> Result<NativeLibraryBinding, NativeBoundaryCompileError> {
        match source {
            SourceNativeLibraryBinding::DefaultNativeNamespace => {
                Ok(NativeLibraryBinding::DefaultNativeNamespace)
            }
            SourceNativeLibraryBinding::LogicalLibrary(name) => {
                let name = CanonicalNativeLibraryName::from_owned(
                    self.meter
                        .try_copy_str(name.as_str(), &WirePath::root().field(20))
                        .map_err(NativeBoundaryCompileError::Resource)?,
                )
                .map_err(NativeBoundaryTargetError::NativeName)?;
                let key = NativeLinkRequirementKey::target_default(name);
                let stream_length = NativeLinkRequirementId::hash_stream_length(&key)
                    .map_err(NativeBoundaryTargetError::Hash)?;
                self.meter
                    .charge_sha256(stream_length, &WirePath::root().field(20))
                    .map_err(NativeBoundaryCompileError::Resource)?;
                let record =
                    CborIdentityRecord::from_key(key).map_err(NativeBoundaryTargetError::Hash)?;
                let id = record.id();
                insert_metered(
                    self.meter,
                    &mut self.expected_requirements,
                    id,
                    record.into_key(),
                    &WirePath::root().field(20),
                )?;
                Ok(NativeLibraryBinding::Requirement(id))
            }
        }
    }

    fn c_signature(
        &mut self,
        source: &SourceCAbiFunctionSignature,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<CanonicalCAbiSignatureFingerprintRecord, NativeBoundaryCompileError> {
        let path = WirePath::root().field(15);
        let mut parameters = metered_vec(self.meter, source.parameters().len(), &path)?;
        for source in source.parameters() {
            let exact = self.signature_exact(source, binders)?;
            parameters.push(
                CanonicalCAbiParameter::new(exact, self.c_storage(exact)?)
                    .map_err(NativeBoundaryTargetError::CanonicalCAbi)?,
            );
        }
        let result = match source.result() {
            SourceCAbiReturn::Void => CanonicalCAbiReturn::Void,
            SourceCAbiReturn::Value(source) => {
                let exact = self.signature_exact(source, binders)?;
                CanonicalCAbiReturn::value(exact, self.c_storage(exact)?)
                    .map_err(NativeBoundaryTargetError::CanonicalCAbi)?
            }
        };
        let signature = CanonicalCAbiFunctionSignature::cdecl(parameters, result);
        let stream_length = CanonicalCAbiSignatureFingerprint::hash_stream_length(&signature)
            .map_err(NativeBoundaryTargetError::Hash)?;
        self.meter
            .charge_sha256(stream_length, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        CanonicalCAbiSignatureFingerprintRecord::new(signature)
            .map_err(NativeBoundaryTargetError::Hash)
            .map_err(Into::into)
    }

    fn scoop_signature(
        &mut self,
        source: &SourceScoopAbiFunctionSignature,
        gc_effect: GcEffect,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
        let path = WirePath::root().field(14);
        let mut parameters = metered_vec(self.meter, source.parameters().len(), &path)?;
        for parameter in source.parameters() {
            parameters.push(self.signature_exact(parameter, binders)?);
        }
        let result = self.signature_exact(source.result(), binders)?;
        let mut arguments = metered_vec(self.meter, parameters.len(), &path)?;
        for exact in &parameters {
            arguments.push(self.scoop_argument(*exact)?);
        }
        let exact_signature =
            ExactCallableSignature::new(scoop_identity::Effect::Ordinary, None, parameters, result);
        let result = if self.is_unit(result) {
            ScoopAbiReturn::UnitVoid
        } else {
            self.scoop_return(result)?
        };
        CanonicalScoopAbiFunctionSignature::new(exact_signature, arguments, result, gc_effect)
            .map_err(NativeBoundaryTargetError::ScoopAbi)
            .map_err(Into::into)
    }

    fn managed_signature(
        &mut self,
        source: &SignatureCallableShape,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<ExactCallableSignature, NativeBoundaryCompileError> {
        let receiver = match source.receiver() {
            OptionalSignatureType::Absent => None,
            OptionalSignatureType::Present(receiver) => {
                Some(self.signature_exact(receiver, binders)?)
            }
        };
        let path = WirePath::root().field(10);
        let mut parameters = metered_vec(self.meter, source.parameters().len(), &path)?;
        for parameter in source.parameters() {
            parameters.push(self.signature_exact(parameter, binders)?);
        }
        Ok(ExactCallableSignature::new(
            source.effect(),
            receiver,
            parameters,
            self.signature_exact(source.result(), binders)?,
        ))
    }

    fn signature_exact(
        &mut self,
        source: &SignatureTypeKey,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<PersistentExactTypeId, NativeBoundaryCompileError> {
        self.signature_exact_at(source, binders, 1)
    }

    fn signature_exact_at(
        &mut self,
        source: &SignatureTypeKey,
        binders: &[Vec<PersistentExactTypeId>],
        depth: u64,
    ) -> Result<PersistentExactTypeId, NativeBoundaryCompileError> {
        let path = WirePath::root().field(15);
        self.meter
            .check_semantic_depth(depth, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        charge_relations(self.meter, 1, &path)?;
        let key = match source {
            SignatureTypeKey::Binder { depth, index } => {
                let group = binders
                    .get(*depth as usize)
                    .ok_or(NativeBoundaryTargetError::BinderDepthOutOfRange { depth: *depth })?;
                return group
                    .get(*index as usize)
                    .copied()
                    .ok_or(NativeBoundaryTargetError::BinderIndexOutOfRange {
                        depth: *depth,
                        index: *index,
                    })
                    .map_err(Into::into);
            }
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                let mut resolved = metered_vec(self.meter, arguments.as_slice().len(), &path)?;
                for argument in arguments.as_slice() {
                    resolved.push(self.signature_exact_at(argument, binders, depth + 1)?);
                }
                ExactTypeKey::NominalApplication {
                    origin: *origin,
                    arguments: NonEmptyVec::new(resolved)
                        .map_err(|_| NativeBoundaryTargetError::InvalidSignatureShape)?,
                }
            }
            SignatureTypeKey::Tuple(elements) => {
                let mut resolved = metered_vec(self.meter, elements.as_slice().len(), &path)?;
                for element in elements.as_slice() {
                    resolved.push(self.signature_exact_at(element, binders, depth + 1)?);
                }
                ExactTypeKey::Tuple(
                    NonEmptyVec::new(resolved)
                        .map_err(|_| NativeBoundaryTargetError::InvalidSignatureShape)?,
                )
            }
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                let mut resolved = metered_vec(self.meter, parameters.len(), &path)?;
                for parameter in parameters {
                    resolved.push(self.signature_exact_at(parameter, binders, depth + 1)?);
                }
                ExactTypeKey::Function {
                    effect: *effect,
                    parameters: resolved,
                    result: self.signature_exact_at(result, binders, depth + 1)?,
                }
            }
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.signature_exact_at(pointee, binders, depth + 1)?)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                let mut resolved = metered_vec(self.meter, parameters.len(), &path)?;
                for parameter in parameters {
                    resolved.push(self.signature_exact_at(parameter, binders, depth + 1)?);
                }
                ExactTypeKey::NativeFunctionPointer {
                    calling_convention: *calling_convention,
                    parameters: resolved,
                    result: self.signature_exact_at(result, binders, depth + 1)?,
                }
            }
        };
        let stream_length = PersistentExactTypeId::hash_stream_length(&key)
            .map_err(NativeBoundaryTargetError::Hash)?;
        self.meter
            .charge_sha256(stream_length, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        let exact =
            PersistentExactTypeId::from_key(&key).map_err(NativeBoundaryTargetError::Hash)?;
        match self.exact_types.get(&exact) {
            Some(actual) if actual.as_ref() == &key => Ok(exact),
            _ => Err(NativeBoundaryTargetError::MissingExactType { exact }.into()),
        }
    }

    fn binders(
        &mut self,
        context: CallableMaterializationContext,
    ) -> Result<Vec<Vec<PersistentExactTypeId>>, NativeBoundaryCompileError> {
        let mut binders = Vec::new();
        let mut visiting = MeteredVisitingSet::new();
        self.append_context_binders(context, &mut binders, &mut visiting)?;
        Ok(binders)
    }

    fn append_context_binders(
        &mut self,
        context: CallableMaterializationContext,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
        visiting: &mut MeteredVisitingSet<PersistentCallableApplicationId>,
    ) -> Result<(), NativeBoundaryCompileError> {
        match context {
            CallableMaterializationContext::NoSubstitution => Ok(()),
            CallableMaterializationContext::Application(application) => {
                self.append_application_binders(application, binders, visiting)
            }
            CallableMaterializationContext::InitializationApplication(unit) => {
                self.append_initialization_binders(unit, binders)
            }
        }
    }

    fn append_application_binders(
        &mut self,
        application: PersistentCallableApplicationId,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
        visiting: &mut MeteredVisitingSet<PersistentCallableApplicationId>,
    ) -> Result<(), NativeBoundaryCompileError> {
        charge_relations(self.meter, 1, &WirePath::root().field(17))?;
        if !visiting.push(application, self.meter, &WirePath::root().field(17))? {
            return Err(NativeBoundaryTargetError::CallableApplicationCycle { application }.into());
        }
        let key = self
            .callable_applications
            .get(&application)
            .ok_or(NativeBoundaryTargetError::MissingCallableApplication { application })?;
        if let CallableArguments::Arguments(arguments) = key.callable_arguments() {
            push_binder_group(
                self.meter,
                binders,
                arguments.as_slice(),
                &WirePath::root().field(17),
            )?;
        }
        let owner = key.instantiation_owner();
        match owner {
            CallableInstantiationOwner::NoOwner => {}
            CallableInstantiationOwner::ExactNominalOwner(owner) => {
                charge_relations(self.meter, 1, &WirePath::root().field(17))?;
                let key = self
                    .exact_types
                    .get(&owner)
                    .ok_or(NativeBoundaryTargetError::MissingExactType { exact: owner })?;
                if let ExactTypeKey::NominalApplication { arguments, .. } = key.as_ref() {
                    push_binder_group(
                        self.meter,
                        binders,
                        arguments.as_slice(),
                        &WirePath::root().field(17),
                    )?;
                }
            }
            CallableInstantiationOwner::EnclosingCallableApplication(enclosing) => {
                self.append_application_binders(enclosing, binders, visiting)?;
            }
            CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
                self.append_initialization_binders(unit, binders)?;
            }
        }
        visiting.remove(&application);
        Ok(())
    }

    fn append_initialization_binders(
        &mut self,
        unit: PersistentInitializationUnitId,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
    ) -> Result<(), NativeBoundaryCompileError> {
        charge_relations(self.meter, 1, &WirePath::root().field(21))?;
        let key = self
            .initialization_units
            .get(&unit)
            .ok_or(NativeBoundaryTargetError::MissingInitializationUnit { unit })?;
        if let InitializationUnitKey::GenericDelegatedExtensionApplication {
            receiver_arguments,
            ..
        } = key.as_ref()
        {
            push_binder_group(
                self.meter,
                binders,
                receiver_arguments.as_slice(),
                &WirePath::root().field(21),
            )?;
        }
        Ok(())
    }
}

fn source_target(
    contract: &SourceNativeExternalContract,
) -> (
    &scoop_identity::SourceNativeSymbol,
    &SourceNativeLibraryBinding,
) {
    match contract {
        SourceNativeExternalContract::Function {
            symbol, library, ..
        }
        | SourceNativeExternalContract::ReadOnlyData {
            symbol, library, ..
        }
        | SourceNativeExternalContract::MutableData {
            symbol, library, ..
        }
        | SourceNativeExternalContract::ReadOnlyTls {
            symbol, library, ..
        }
        | SourceNativeExternalContract::MutableTls {
            symbol, library, ..
        } => (symbol, library),
    }
}
