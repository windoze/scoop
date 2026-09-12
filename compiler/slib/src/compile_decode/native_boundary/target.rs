use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU64;

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
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType,
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, CborIdentityRecord,
    CoreNativeBoundaryNominal, ExactCallableSignature, ExactTypeKey, GcEffect, IdentityLayer,
    InitializationUnitKey, NativeExternalContract, NativeExternalContractRecord,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementId,
    NativeLinkRequirementKey, NonEmptyVec, OptionalSignatureType, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId, PersistentExactTypeId,
    PersistentInitializationUnitId, ScoopAbiArgument, ScoopAbiReturn, ScoopAbiValueShape,
    SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    SourceExternFunctionAbi, SourceNativeExternalContract, SourceNativeExternalContractRecord,
    SourceNativeLibraryBinding, SourceScoopAbiFunctionSignature, TargetCallingConvention,
};
use scoop_wire::WirePath;

use super::{
    NativeBoundaryCompileError, NativeBoundarySourceValidatedFoundations, index_records,
    records_by_id,
};

mod layout;

/// Foundation payloads whose source closure and every target-specific native
/// ABI leaf were independently recomputed from canonical identities.
pub struct NativeBoundaryValidatedFoundations<'input> {
    pub(crate) foundations: super::super::StructurallyValidatedFoundations<'input>,
}

impl<'input> NativeBoundarySourceValidatedFoundations<'input> {
    pub fn validate_target(
        mut self,
    ) -> Result<NativeBoundaryValidatedFoundations<'input>, NativeBoundaryCompileError> {
        validate_target_normalization(&mut self.foundations)?;
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

fn validate_target_normalization(
    foundations: &mut super::super::StructurallyValidatedFoundations<'_>,
) -> Result<(), NativeBoundaryCompileError> {
    let graph = &foundations.identities;
    let target = foundations.graph.target_selection().target();
    let meter = foundations.graph.envelope.meter_mut();
    let exact_types = records_by_id(
        [
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Hir,
                    meter,
                    &WirePath::root().field(15),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Mir,
                    meter,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Lir,
                    meter,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ],
        meter,
        &WirePath::root().field(15),
    )?;
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
    let definitions = index_records(
        foundations.hir.native_boundary_types(),
        NativeBoundaryTypeDefinitionRecord::owner,
        meter,
        &WirePath::root().field(30),
    )?;

    let mut normalizer = NativeBoundaryNormalizer::new(
        target,
        &exact_types,
        &callable_applications,
        &initialization_units,
        &definitions,
    );

    let actual_contracts = index_records(
        foundations.lir.native_contracts(),
        NativeExternalContractRecord::source,
        meter,
        &WirePath::root().field(14),
    )?;
    let mut expected_contracts = BTreeMap::new();
    for source in foundations.hir.source_native_contracts() {
        let expected = normalizer.normalize_external(source)?;
        expected_contracts.insert(expected.source(), expected);
    }
    require_equal_records(
        &expected_contracts,
        &actual_contracts,
        NativeBoundaryTargetError::NativeContractMismatch,
    )?;

    let application_records = index_records(
        foundations.mir.callback_application_records(),
        scoop_mir::CallbackApplicationRecord::application,
        meter,
        &WirePath::root().field(10),
    )?;
    for bridge in foundations.lir.callback_bridges() {
        let application = callback_applications.get(&bridge.application()).ok_or(
            NativeBoundaryTargetError::MissingCallbackApplication {
                application: bridge.application(),
            },
        )?;
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
        normalizer
            .expected_signatures
            .insert(expected.fingerprint(), expected);

        let expected_managed =
            normalizer.managed_signature(registration.managed_signature(), &binders)?;
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

    let actual_signatures = index_records(
        foundations.lir.c_abi_signatures(),
        CanonicalCAbiSignatureFingerprintRecord::fingerprint,
        meter,
        &WirePath::root().field(15),
    )?;
    require_equal_records(
        &normalizer.expected_signatures,
        &actual_signatures,
        NativeBoundaryTargetError::CAbiSignatureSetMismatch,
    )?;

    let actual_layouts = index_records(
        foundations.lir.c_abi_layouts(),
        CanonicalCAbiLayoutFingerprintRecord::fingerprint,
        meter,
        &WirePath::root().field(16),
    )?;
    require_equal_records(
        &normalizer.expected_layouts,
        &actual_layouts,
        NativeBoundaryTargetError::CAbiLayoutSetMismatch,
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
    if actual_requirements.len() != normalizer.expected_requirements.len()
        || normalizer
            .expected_requirements
            .iter()
            .any(|(key, value)| actual_requirements.get(key) != Some(value))
    {
        return Err(NativeBoundaryTargetError::NativeRequirementSetMismatch.into());
    }
    Ok(())
}

fn require_equal_records<K, V>(
    expected: &BTreeMap<K, V>,
    actual: &HashMap<K, &V>,
    error: NativeBoundaryTargetError,
) -> Result<(), NativeBoundaryCompileError>
where
    K: Eq + std::hash::Hash,
    V: Eq,
{
    if expected.len() != actual.len()
        || expected
            .iter()
            .any(|(key, value)| actual.get(key).copied() != Some(value))
    {
        Err(error.into())
    } else {
        Ok(())
    }
}

struct NativeBoundaryNormalizer<'a> {
    target: scoop_lir::LirTargetProfile,
    exact_types: &'a HashMap<PersistentExactTypeId, ExactTypeKey>,
    callable_applications: &'a HashMap<PersistentCallableApplicationId, CallableApplicationKey>,
    initialization_units: &'a HashMap<PersistentInitializationUnitId, InitializationUnitKey>,
    definitions: &'a HashMap<NativeBoundaryNominalOwner, &'a NativeBoundaryTypeDefinitionRecord>,
    expected_signatures:
        BTreeMap<CanonicalCAbiSignatureFingerprint, CanonicalCAbiSignatureFingerprintRecord>,
    expected_layouts:
        BTreeMap<CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayoutFingerprintRecord>,
    layouts_by_type: BTreeMap<PersistentExactTypeId, CanonicalCAbiLayoutFingerprint>,
    visiting_c_layouts: BTreeSet<PersistentExactTypeId>,
    scoop_layouts: BTreeMap<PersistentExactTypeId, PhysicalType>,
    visiting_scoop_layouts: BTreeSet<PersistentExactTypeId>,
    expected_requirements: BTreeMap<NativeLinkRequirementId, NativeLinkRequirementKey>,
}

impl<'a> NativeBoundaryNormalizer<'a> {
    fn new(
        target: scoop_lir::LirTargetProfile,
        exact_types: &'a HashMap<PersistentExactTypeId, ExactTypeKey>,
        callable_applications: &'a HashMap<PersistentCallableApplicationId, CallableApplicationKey>,
        initialization_units: &'a HashMap<PersistentInitializationUnitId, InitializationUnitKey>,
        definitions: &'a HashMap<
            NativeBoundaryNominalOwner,
            &'a NativeBoundaryTypeDefinitionRecord,
        >,
    ) -> Self {
        Self {
            target,
            exact_types,
            callable_applications,
            initialization_units,
            definitions,
            expected_signatures: BTreeMap::new(),
            expected_layouts: BTreeMap::new(),
            layouts_by_type: BTreeMap::new(),
            visiting_c_layouts: BTreeSet::new(),
            scoop_layouts: BTreeMap::new(),
            visiting_scoop_layouts: BTreeSet::new(),
            expected_requirements: BTreeMap::new(),
        }
    }

    fn normalize_external(
        &mut self,
        source: &SourceNativeExternalContractRecord,
    ) -> Result<NativeExternalContractRecord, NativeBoundaryCompileError> {
        let (symbol, source_library) = source_target(source.contract());
        let symbol = match self.target.id() {
            scoop_lir::TargetProfileId::DarwinAarch64 => {
                NativeExternalSymbolKey::darwin_macho_external(symbol)
                    .map_err(NativeBoundaryTargetError::NativeSymbol)?
            }
        };
        let library = self.library(source_library)?;
        let contract = match source.contract() {
            SourceNativeExternalContract::Function { abi, .. } => match abi {
                SourceExternFunctionAbi::C(signature) => {
                    let record = self.c_signature(signature, &[])?;
                    let signature = record.signature().clone();
                    self.expected_signatures
                        .insert(record.fingerprint(), record);
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
                let key = NativeLinkRequirementKey::target_default(name.clone());
                let record = CborIdentityRecord::from_key(key.clone())
                    .map_err(NativeBoundaryTargetError::Hash)?;
                self.expected_requirements.insert(record.id(), key);
                Ok(NativeLibraryBinding::Requirement(record.id()))
            }
        }
    }

    fn c_signature(
        &mut self,
        source: &SourceCAbiFunctionSignature,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<CanonicalCAbiSignatureFingerprintRecord, NativeBoundaryCompileError> {
        let mut parameters = Vec::with_capacity(source.parameters().len());
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
        CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
            parameters, result,
        ))
        .map_err(NativeBoundaryTargetError::Hash)
        .map_err(Into::into)
    }

    fn scoop_signature(
        &mut self,
        source: &SourceScoopAbiFunctionSignature,
        gc_effect: GcEffect,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<CanonicalScoopAbiFunctionSignature, NativeBoundaryCompileError> {
        let parameters = source
            .parameters()
            .iter()
            .map(|parameter| self.signature_exact(parameter, binders))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.signature_exact(source.result(), binders)?;
        let exact_signature = ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            parameters.clone(),
            result,
        );
        let arguments = parameters
            .into_iter()
            .map(|exact| self.scoop_argument(exact))
            .collect::<Result<Vec<_>, _>>()?;
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
        &self,
        source: &SignatureCallableShape,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<ExactCallableSignature, NativeBoundaryCompileError> {
        let receiver = match source.receiver() {
            OptionalSignatureType::Absent => None,
            OptionalSignatureType::Present(receiver) => {
                Some(self.signature_exact(receiver, binders)?)
            }
        };
        let parameters = source
            .parameters()
            .iter()
            .map(|parameter| self.signature_exact(parameter, binders))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExactCallableSignature::new(
            source.effect(),
            receiver,
            parameters,
            self.signature_exact(source.result(), binders)?,
        ))
    }

    fn signature_exact(
        &self,
        source: &SignatureTypeKey,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<PersistentExactTypeId, NativeBoundaryCompileError> {
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
                let arguments = arguments
                    .as_slice()
                    .iter()
                    .map(|argument| self.signature_exact(argument, binders))
                    .collect::<Result<Vec<_>, _>>()?;
                ExactTypeKey::NominalApplication {
                    origin: *origin,
                    arguments: NonEmptyVec::new(arguments)
                        .expect("validated signature applications are non-empty"),
                }
            }
            SignatureTypeKey::Tuple(elements) => {
                let elements = elements
                    .as_slice()
                    .iter()
                    .map(|element| self.signature_exact(element, binders))
                    .collect::<Result<Vec<_>, _>>()?;
                ExactTypeKey::Tuple(
                    NonEmptyVec::new(elements).expect("validated signature tuples are non-empty"),
                )
            }
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                effect: *effect,
                parameters: parameters
                    .iter()
                    .map(|parameter| self.signature_exact(parameter, binders))
                    .collect::<Result<Vec<_>, _>>()?,
                result: self.signature_exact(result, binders)?,
            },
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.signature_exact(pointee, binders)?)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => ExactTypeKey::NativeFunctionPointer {
                calling_convention: *calling_convention,
                parameters: parameters
                    .iter()
                    .map(|parameter| self.signature_exact(parameter, binders))
                    .collect::<Result<Vec<_>, _>>()?,
                result: self.signature_exact(result, binders)?,
            },
        };
        let exact =
            PersistentExactTypeId::from_key(&key).map_err(NativeBoundaryTargetError::Hash)?;
        match self.exact_types.get(&exact) {
            Some(actual) if actual == &key => Ok(exact),
            _ => Err(NativeBoundaryTargetError::MissingExactType { exact }.into()),
        }
    }

    fn binders(
        &self,
        context: CallableMaterializationContext,
    ) -> Result<Vec<Vec<PersistentExactTypeId>>, NativeBoundaryCompileError> {
        let mut binders = Vec::new();
        let mut visiting = BTreeSet::new();
        self.append_context_binders(context, &mut binders, &mut visiting)?;
        Ok(binders)
    }

    fn append_context_binders(
        &self,
        context: CallableMaterializationContext,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
        visiting: &mut BTreeSet<PersistentCallableApplicationId>,
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
        &self,
        application: PersistentCallableApplicationId,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
        visiting: &mut BTreeSet<PersistentCallableApplicationId>,
    ) -> Result<(), NativeBoundaryCompileError> {
        if !visiting.insert(application) {
            return Err(NativeBoundaryTargetError::CallableApplicationCycle { application }.into());
        }
        let key = self
            .callable_applications
            .get(&application)
            .ok_or(NativeBoundaryTargetError::MissingCallableApplication { application })?;
        if let CallableArguments::Arguments(arguments) = key.callable_arguments() {
            binders.push(arguments.as_slice().to_vec());
        }
        match key.instantiation_owner() {
            CallableInstantiationOwner::NoOwner => {}
            CallableInstantiationOwner::ExactNominalOwner(owner) => {
                if let ExactTypeKey::NominalApplication { arguments, .. } = self.exact(owner)? {
                    binders.push(arguments.as_slice().to_vec());
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
        &self,
        unit: PersistentInitializationUnitId,
        binders: &mut Vec<Vec<PersistentExactTypeId>>,
    ) -> Result<(), NativeBoundaryCompileError> {
        let key = self
            .initialization_units
            .get(&unit)
            .ok_or(NativeBoundaryTargetError::MissingInitializationUnit { unit })?;
        if let InitializationUnitKey::GenericDelegatedExtensionApplication {
            receiver_arguments,
            ..
        } = key
        {
            binders.push(receiver_arguments.as_slice().to_vec());
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct PhysicalType {
    size: u64,
    alignment: u64,
    shape: ScoopAbiValueShape,
    gc_free: bool,
}

fn scalar(layout: scoop_lir::ScalarLayout, gc_free: bool) -> PhysicalType {
    PhysicalType {
        size: layout.size_bytes(),
        alignment: layout.alignment_bytes(),
        shape: ScoopAbiValueShape::Scalar,
        gc_free,
    }
}

fn integer_scalar_kind(bit_width: scoop_identity::IntegerBitWidth) -> scoop_lir::BackendScalarKind {
    match bit_width {
        scoop_identity::IntegerBitWidth::Bits8 => scoop_lir::BackendScalarKind::I8,
        scoop_identity::IntegerBitWidth::Bits16 => scoop_lir::BackendScalarKind::I16,
        scoop_identity::IntegerBitWidth::Bits32 => scoop_lir::BackendScalarKind::I32,
        scoop_identity::IntegerBitWidth::Bits64 => scoop_lir::BackendScalarKind::I64,
    }
}

fn pointer(
    target: scoop_lir::LirTargetProfile,
    kind: scoop_lir::PointerKind,
    gc_free: bool,
) -> PhysicalType {
    let layout = target.pointer_layout(kind);
    PhysicalType {
        size: layout.size_bytes(),
        alignment: layout.alignment_bytes(),
        shape: ScoopAbiValueShape::Scalar,
        gc_free,
    }
}

fn aggregate(
    fields: &[PhysicalType],
    shape: ScoopAbiValueShape,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    aggregate_with_overrides(fields, None, None, shape, exact)
}

fn aggregate_with_policy(
    fields: &[PhysicalType],
    policy: NativeBoundaryCLayoutPolicy,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    match policy {
        NativeBoundaryCLayoutPolicy::NotCLayout => {
            aggregate_with_overrides(fields, None, None, ScoopAbiValueShape::Aggregate, exact)
        }
        NativeBoundaryCLayoutPolicy::CLayout { aligned, packed } => aggregate_with_overrides(
            fields,
            override_bytes(aligned),
            override_bytes(packed),
            ScoopAbiValueShape::Aggregate,
            exact,
        ),
    }
}

fn aggregate_with_overrides(
    fields: &[PhysicalType],
    aligned: Option<u64>,
    packed: Option<u64>,
    shape: ScoopAbiValueShape,
    exact: PersistentExactTypeId,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    let mut size = 0_u64;
    let mut alignment = aligned.unwrap_or(1);
    for field in fields {
        let access_alignment = packed.map_or(field.alignment, |cap| field.alignment.min(cap));
        size = align_up(size, access_alignment)?;
        size = size
            .checked_add(field.size)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        alignment = alignment.max(access_alignment);
    }
    Ok(PhysicalType {
        size: align_up(size, alignment)?,
        alignment,
        shape,
        gc_free: fields.iter().all(|field| field.gc_free),
    })
}

fn align_up(value: u64, alignment: u64) -> Result<u64, NativeBoundaryCompileError> {
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(NativeBoundaryTargetError::ArithmeticOverflow.into())
    }
}

fn override_bytes(value: scoop_identity::CLayoutOverride) -> Option<u64> {
    match value {
        scoop_identity::CLayoutOverride::Natural => None,
        scoop_identity::CLayoutOverride::Bytes(bytes) => Some(u64::from(bytes.get())),
    }
}

fn core_integer(
    key: &ExactTypeKey,
) -> Option<(scoop_identity::Signedness, scoop_identity::IntegerBitWidth)> {
    let ExactTypeKey::Nominal(owner) = key else {
        return None;
    };
    CoreNativeBoundaryNominal::ALL
        .into_iter()
        .find(|role| role.concrete_id() == Some(*owner))
        .and_then(CoreNativeBoundaryNominal::integer)
}

fn is_core_application(key: &ExactTypeKey, role: CoreNativeBoundaryNominal) -> bool {
    matches!(
        key,
        ExactTypeKey::NominalApplication { origin, arguments }
            if Some(*origin) == role.generic_id() && arguments.as_slice().len() == 1
    )
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

#[derive(Debug)]
pub enum NativeBoundaryTargetError {
    MissingExactType {
        exact: PersistentExactTypeId,
    },
    MissingCallbackApplication {
        application: PersistentCallbackApplicationId,
    },
    MissingCallableApplication {
        application: PersistentCallableApplicationId,
    },
    MissingCallbackRegistration {
        registration: PersistentCallbackRegistrationId,
    },
    MissingInitializationUnit {
        unit: PersistentInitializationUnitId,
    },
    BinderDepthOutOfRange {
        depth: u32,
    },
    BinderIndexOutOfRange {
        depth: u32,
        index: u32,
    },
    ExpectedNominal {
        exact: PersistentExactTypeId,
    },
    NotCAbiSafe {
        exact: PersistentExactTypeId,
    },
    CLayoutCycle {
        exact: PersistentExactTypeId,
    },
    ScoopLayoutCycle {
        exact: PersistentExactTypeId,
    },
    CallableApplicationCycle {
        application: PersistentCallableApplicationId,
    },
    LayoutOverflow {
        exact: PersistentExactTypeId,
    },
    MissingComputedCLayout {
        layout: CanonicalCAbiLayoutFingerprint,
    },
    ArithmeticOverflow,
    NativeContractMismatch,
    CallbackSignatureMismatch {
        application: PersistentCallbackApplicationId,
    },
    ManagedCallbackSignatureMismatch {
        application: PersistentCallbackApplicationId,
    },
    CAbiSignatureSetMismatch,
    CAbiLayoutSetMismatch,
    NativeRequirementSetMismatch,
    NativeSymbol(scoop_identity::NativeLinkSymbolError),
    CanonicalCAbi(scoop_identity::CanonicalCAbiError),
    ScoopAbi(scoop_identity::ScoopAbiError),
    Hash(scoop_wire::HashError),
}

impl std::fmt::Display for NativeBoundaryTargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "native boundary target normalization failed: {self:?}"
        )
    }
}

impl std::error::Error for NativeBoundaryTargetError {}

impl From<NativeBoundaryTargetError> for NativeBoundaryCompileError {
    fn from(error: NativeBoundaryTargetError) -> Self {
        Self::Target(error)
    }
}
