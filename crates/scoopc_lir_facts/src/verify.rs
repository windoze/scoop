//! Structural verifier for LIR fact products.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use scoopc_ids::{BodyVersionKey, LirCallableId, StableCanonicalKey};

use crate::{
    LirCallTargetMode, LirCallableContract, LirCallableRef, LirConeInitRoutineKey,
    LirControlBodyFacts, LirEffectPrecision, LirFacts, LirGlobalRootKey, LirGlobalRootKind,
    LirInitializerBodyKind, LirTypeContextBridgeMode,
};

/// Result type returned by LIR fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before LIR facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    CallableCountMismatch {
        expected: usize,
        actual: usize,
    },
    StepTypeCountMismatch {
        expected: usize,
        actual: usize,
    },
    ResumePackingCountMismatch {
        expected: usize,
        actual: usize,
    },
    ContinuationObjectCountMismatch {
        expected: usize,
        actual: usize,
    },
    SurfaceResumeDispatchCountMismatch {
        expected: usize,
        actual: usize,
    },
    GlobalRootCountMismatch {
        expected: usize,
        actual: usize,
    },
    ObjectOnceCountMismatch {
        expected: usize,
        actual: usize,
    },
    TopLevelEagerInitCountMismatch {
        expected: usize,
        actual: usize,
    },
    ConeInitRoutineCountMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutClassCountMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutEnumCountMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutInterfaceCountMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutClassItableCountMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutCallableSymbolCountMismatch {
        expected: usize,
        actual: usize,
    },
    OptRevisionMismatch {
        summary: u64,
        pipeline: u64,
    },
    EmptyGlobalRoot,
    MismatchedGlobalRootKey {
        key: String,
        root: String,
    },
    MissingGlobalRootDependency {
        root: String,
        dependency: String,
    },
    MissingStoragePolicy {
        root: String,
        kind: &'static str,
    },
    MismatchedGlobalInitializerPresence {
        root: String,
        contract: &'static str,
        root_has_initializer: bool,
        contract_has_initializer: bool,
    },
    MismatchedGlobalStoragePolicy {
        root: String,
        contract: &'static str,
        root_storage: String,
        contract_storage: String,
    },
    MissingExternGlobalContract {
        root: String,
    },
    UnexpectedExternGlobalContract {
        root: String,
        kind: &'static str,
    },
    InvalidExternGlobalInitializerAbsence {
        root: String,
    },
    ExternGlobalInitializerPresent {
        root: String,
    },
    MissingInitializerBodyContract {
        root: String,
    },
    UnexpectedInitializerBodyContract {
        root: String,
        kind: &'static str,
    },
    MismatchedInitializerBodyRoot {
        key: String,
        body_root: String,
    },
    MismatchedInitializerBodyKind {
        root: String,
        expected: &'static str,
        actual: &'static str,
    },
    EmptyInitializerBodySource {
        root: String,
    },
    MissingGlobalContractRoot {
        root: String,
        contract: &'static str,
    },
    MisclassifiedObjectOnceRoot {
        root: String,
        kind: &'static str,
    },
    MisclassifiedTopLevelEagerRoot {
        root: String,
        kind: &'static str,
    },
    GlobalInitClassConflict {
        root: String,
    },
    MissingConeInitRoot {
        routine: u32,
        root: String,
    },
    DuplicateConeInitRoot {
        routine: u32,
        root: String,
    },
    MismatchedConeInitRoutineKey {
        key: u32,
        routine: u32,
    },
    ConeInitRootConeMismatch {
        routine: u32,
        root: String,
    },
    MissingConeInitRoutineForRoot {
        root: String,
    },
    MissingFinalEntryRoutine {
        routine: u32,
    },
    DuplicateFinalEntryRoutine {
        routine: u32,
    },
    UnscheduledConeInitRoutine {
        routine: u32,
    },
    InvalidConeInitDependencyOrder {
        root: String,
        dependency: String,
    },
    InvalidConeInitRoutineSourceOrder {
        routine: u32,
        previous_routine: u32,
    },
    EmptyCallableRoot {
        key: String,
    },
    EmptyStableInstanceKey {
        key: String,
    },
    EmptyLayoutClass {
        key: String,
    },
    EmptyLayoutEnum {
        key: String,
    },
    EmptyCallableSymbolRoot {
        key: String,
    },
    MismatchedCallableSymbolKey {
        key: String,
        callable: String,
    },
    MismatchedCallableSymbolSignature {
        key: String,
    },
    InvalidSourceSignature {
        key: String,
    },
    InvalidAbiSymbol {
        key: String,
        reason: &'static str,
    },
    InvalidLayoutName {
        key: String,
        reason: &'static str,
    },
    InvalidClosureIdentity {
        key: String,
        reason: &'static str,
    },
    InvalidExactCalleeBinding {
        callable: String,
        reason: &'static str,
    },
    InvalidClassCtorInit {
        key: String,
        reason: &'static str,
    },
    InvalidClassCtorCallSite {
        key: String,
        reason: &'static str,
    },
    InvalidReflectionCallSite {
        key: String,
        reason: &'static str,
    },
    InvalidTypeContextBridge {
        mode: &'static str,
    },
    MissingStableWireFormatOwner,
    MismatchedCallableParamTypes {
        callable: String,
    },
    MismatchedCallableReturnType {
        callable: String,
    },
    MissingControlStepType {
        callable: String,
        step_schema: u32,
    },
    MissingContinuationObject {
        callable: String,
        object_id: u32,
    },
    MissingResumePacking {
        callable: String,
        packing_id: u32,
    },
    MissingDynamicInvokeOwner {
        key: String,
    },
    InvalidSourceCallSite {
        key: String,
        reason: &'static str,
    },
    MissingDynamicInvokeTargetStep {
        key: String,
        step_schema: u32,
    },
    MissingDynamicInvokeDispatch {
        key: String,
    },
    MissingDispatchOwner {
        key: String,
    },
    MissingContinuationObjectPacking {
        object_id: u32,
        packing_id: u32,
    },
    MissingSurfaceResumeOutStep {
        continuation_schema: u32,
        step_schema: u32,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} callables but inventory contains {actual}"
            ),
            Self::EmptyCallableRoot { key } => {
                write!(f, "LIR callable `{key}` has an empty root FQN")
            }
            Self::StepTypeCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} step types but facts contain {actual}"
            ),
            Self::ResumePackingCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} resume packings but facts contain {actual}"
            ),
            Self::ContinuationObjectCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} continuation objects but facts contain {actual}"
            ),
            Self::SurfaceResumeDispatchCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} surface-resume dispatches but facts contain {actual}"
            ),
            Self::GlobalRootCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} global roots but facts contain {actual}"
            ),
            Self::ObjectOnceCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} object-once roots but facts contain {actual}"
            ),
            Self::TopLevelEagerInitCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} top-level eager init roots but facts contain {actual}"
            ),
            Self::ConeInitRoutineCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} cone init routines but facts contain {actual}"
            ),
            Self::LayoutClassCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} layout classes but facts contain {actual}"
            ),
            Self::LayoutEnumCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} layout enums but facts contain {actual}"
            ),
            Self::LayoutInterfaceCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} layout interfaces but facts contain {actual}"
            ),
            Self::LayoutClassItableCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} class itables but facts contain {actual}"
            ),
            Self::LayoutCallableSymbolCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} callable symbols but facts contain {actual}"
            ),
            Self::OptRevisionMismatch { summary, pipeline } => write!(
                f,
                "LIR summary opt revision {summary} does not match pipeline revision {pipeline}"
            ),
            Self::EmptyGlobalRoot => write!(f, "LIR global init facts contain an empty root key"),
            Self::MismatchedGlobalRootKey { key, root } => write!(
                f,
                "LIR global root map key `{key}` does not match embedded root `{root}`"
            ),
            Self::MissingGlobalRootDependency { root, dependency } => write!(
                f,
                "LIR global root `{root}` depends on unpublished root `{dependency}`"
            ),
            Self::MissingStoragePolicy { root, kind } => write!(
                f,
                "LIR global root `{root}` of kind `{kind}` is missing a storage policy"
            ),
            Self::MismatchedGlobalInitializerPresence {
                root,
                contract,
                root_has_initializer,
                contract_has_initializer,
            } => write!(
                f,
                "LIR {contract} contract for `{root}` reports has_initializer={contract_has_initializer} but root reports has_initializer={root_has_initializer}"
            ),
            Self::MismatchedGlobalStoragePolicy {
                root,
                contract,
                root_storage,
                contract_storage,
            } => write!(
                f,
                "LIR {contract} contract for `{root}` reports storage={contract_storage} but root reports storage={root_storage}"
            ),
            Self::MissingExternGlobalContract { root } => {
                write!(f, "LIR extern global `{root}` is missing extern contract")
            }
            Self::UnexpectedExternGlobalContract { root, kind } => write!(
                f,
                "LIR non-extern global root `{root}` of kind `{kind}` carries extern contract"
            ),
            Self::InvalidExternGlobalInitializerAbsence { root } => write!(
                f,
                "LIR extern global `{root}` must publish initializer_absent=true"
            ),
            Self::ExternGlobalInitializerPresent { root } => write!(
                f,
                "LIR extern global `{root}` must not publish an initializer"
            ),
            Self::MissingInitializerBodyContract { root } => write!(
                f,
                "LIR global root `{root}` has an initializer but no initializer body contract"
            ),
            Self::UnexpectedInitializerBodyContract { root, kind } => write!(
                f,
                "LIR global root `{root}` of kind `{kind}` must not carry an initializer body contract"
            ),
            Self::MismatchedInitializerBodyRoot { key, body_root } => write!(
                f,
                "LIR initializer body map root `{body_root}` does not match global root `{key}`"
            ),
            Self::MismatchedInitializerBodyKind {
                root,
                expected,
                actual,
            } => write!(
                f,
                "LIR initializer body for `{root}` has kind `{actual}` but root requires `{expected}`"
            ),
            Self::EmptyInitializerBodySource { root } => write!(
                f,
                "LIR initializer body for `{root}` has an empty source path"
            ),
            Self::MissingGlobalContractRoot { root, contract } => write!(
                f,
                "LIR {contract} contract references missing global root `{root}`"
            ),
            Self::MisclassifiedObjectOnceRoot { root, kind } => write!(
                f,
                "LIR object-once contract references `{root}` with incompatible kind `{kind}`"
            ),
            Self::MisclassifiedTopLevelEagerRoot { root, kind } => write!(
                f,
                "LIR top-level eager init contract references `{root}` with incompatible kind `{kind}`"
            ),
            Self::GlobalInitClassConflict { root } => write!(
                f,
                "LIR global root `{root}` is both object-once and top-level eager init"
            ),
            Self::MissingConeInitRoot { routine, root } => write!(
                f,
                "LIR cone init routine r{routine} references missing eager init root `{root}`"
            ),
            Self::DuplicateConeInitRoot { routine, root } => write!(
                f,
                "LIR cone init routine r{routine} lists eager init root `{root}` more than once"
            ),
            Self::MismatchedConeInitRoutineKey { key, routine } => write!(
                f,
                "LIR cone init routine map key r{key} does not match embedded routine r{routine}"
            ),
            Self::ConeInitRootConeMismatch { routine, root } => write!(
                f,
                "LIR cone init routine r{routine} references eager init root `{root}` from another cone"
            ),
            Self::MissingConeInitRoutineForRoot { root } => write!(
                f,
                "LIR top-level eager init root `{root}` is not assigned to a cone init routine"
            ),
            Self::MissingFinalEntryRoutine { routine } => write!(
                f,
                "LIR final entry init order references missing cone init routine r{routine}"
            ),
            Self::DuplicateFinalEntryRoutine { routine } => write!(
                f,
                "LIR final entry init order lists cone init routine r{routine} more than once"
            ),
            Self::UnscheduledConeInitRoutine { routine } => write!(
                f,
                "LIR cone init routine r{routine} is missing from final entry init order"
            ),
            Self::InvalidConeInitDependencyOrder { root, dependency } => write!(
                f,
                "LIR eager init root `{root}` is scheduled before dependency `{dependency}`"
            ),
            Self::InvalidConeInitRoutineSourceOrder {
                routine,
                previous_routine,
            } => write!(
                f,
                "LIR cone init routine r{routine} is scheduled before earlier source-cone routine r{previous_routine}"
            ),
            Self::EmptyStableInstanceKey { key } => {
                write!(f, "LIR callable `{key}` has an empty stable instance key")
            }
            Self::EmptyLayoutClass { key } => {
                write!(f, "LIR class layout `{key}` has an empty identity")
            }
            Self::EmptyLayoutEnum { key } => {
                write!(f, "LIR enum layout `{key}` has an empty identity")
            }
            Self::EmptyCallableSymbolRoot { key } => {
                write!(f, "LIR callable symbol `{key}` has an empty root FQN")
            }
            Self::MismatchedCallableSymbolKey { key, callable } => write!(
                f,
                "LIR callable symbol map key `{key}` does not match embedded callable `{callable}`"
            ),
            Self::MismatchedCallableSymbolSignature { key } => write!(
                f,
                "LIR callable symbol `{key}` signature drifts from callable inventory"
            ),
            Self::InvalidSourceSignature { key } => {
                write!(
                    f,
                    "LIR source signature `{key}` has inconsistent identity or arity"
                )
            }
            Self::InvalidAbiSymbol { key, reason } => {
                write!(f, "LIR ABI symbol `{key}` is invalid: {reason}")
            }
            Self::InvalidLayoutName { key, reason } => {
                write!(f, "LIR layout name `{key}` is invalid: {reason}")
            }
            Self::InvalidClosureIdentity { key, reason } => {
                write!(f, "LIR closure identity `{key}` is invalid: {reason}")
            }
            Self::InvalidExactCalleeBinding { callable, reason } => write!(
                f,
                "LIR callable `{callable}` has an invalid exact callee binding: {reason}"
            ),
            Self::InvalidClassCtorInit { key, reason } => {
                write!(f, "LIR class ctor init `{key}` is invalid: {reason}")
            }
            Self::InvalidClassCtorCallSite { key, reason } => {
                write!(f, "LIR class ctor call-site `{key}` is invalid: {reason}")
            }
            Self::InvalidReflectionCallSite { key, reason } => {
                write!(f, "LIR reflection call-site `{key}` is invalid: {reason}")
            }
            Self::InvalidTypeContextBridge { mode } => write!(
                f,
                "LIR type context bridge mode `{mode}` is inconsistent with published fingerprints"
            ),
            Self::MissingStableWireFormatOwner => write!(
                f,
                "LIR type context stable wire-format decision is missing an owner"
            ),
            Self::MismatchedCallableParamTypes { callable } => write!(
                f,
                "LIR callable `{callable}` source parameter types drift from ABI contract"
            ),
            Self::MismatchedCallableReturnType { callable } => write!(
                f,
                "LIR callable `{callable}` source return type drifts from plain ABI contract"
            ),
            Self::MissingControlStepType {
                callable,
                step_schema,
            } => write!(
                f,
                "LIR callable `{callable}` references missing control StepSchema s{step_schema}"
            ),
            Self::MissingContinuationObject {
                callable,
                object_id,
            } => write!(
                f,
                "LIR callable `{callable}` references missing continuation object cont_obj#{object_id}"
            ),
            Self::MissingResumePacking {
                callable,
                packing_id,
            } => write!(
                f,
                "LIR callable `{callable}` references missing resume packing packing#{packing_id}"
            ),
            Self::MissingDynamicInvokeOwner { key } => {
                write!(
                    f,
                    "LIR dynamic invoke `{key}` references a missing owner callable"
                )
            }
            Self::InvalidSourceCallSite { key, reason } => {
                write!(f, "LIR source call-site `{key}` is invalid: {reason}")
            }
            Self::MissingDynamicInvokeTargetStep { key, step_schema } => write!(
                f,
                "LIR dynamic invoke `{key}` references missing target StepSchema s{step_schema}"
            ),
            Self::MissingDynamicInvokeDispatch { key } => write!(
                f,
                "LIR dynamic invoke `{key}` references a missing dispatch contract"
            ),
            Self::MissingDispatchOwner { key } => {
                write!(
                    f,
                    "LIR dispatch `{key}` references a missing owner callable"
                )
            }
            Self::MissingContinuationObjectPacking {
                object_id,
                packing_id,
            } => write!(
                f,
                "LIR continuation object cont_obj#{object_id} references missing resume packing packing#{packing_id}"
            ),
            Self::MissingSurfaceResumeOutStep {
                continuation_schema,
                step_schema,
            } => write!(
                f,
                "LIR surface-resume dispatch k{continuation_schema} references missing out StepSchema s{step_schema}"
            ),
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the LIR stage.
pub fn verify_lir_facts(facts: &LirFacts) -> Result<()> {
    verify_opt_pipeline_binding(facts)?;
    verify_summary_counts(facts)?;
    verify_global_init_contracts(facts)?;
    verify_physical_layout_contracts(facts)?;
    verify_source_signature_contracts(facts)?;
    verify_intrinsic_callable_contracts(facts)?;
    verify_class_ctor_init_contracts(facts)?;
    verify_class_ctor_call_site_contracts(facts)?;
    verify_reflection_call_site_contracts(facts)?;
    verify_type_context_contract(facts)?;
    verify_callable_inventory(facts)?;
    verify_source_call_site_contracts(facts)?;
    verify_dynamic_invoke_contracts(facts)?;
    verify_dispatch_contracts(facts)?;
    verify_continuation_objects(facts)?;
    verify_surface_resume_dispatches(facts)?;
    Ok(())
}

fn verify_opt_pipeline_binding(facts: &LirFacts) -> Result<()> {
    if facts.summary.opt_revision != facts.opt_pipeline.revision {
        return Err(VerifyError::OptRevisionMismatch {
            summary: facts.summary.opt_revision,
            pipeline: facts.opt_pipeline.revision,
        });
    }
    Ok(())
}

fn verify_summary_counts(facts: &LirFacts) -> Result<()> {
    if facts.summary.global_root_count != facts.global_init.roots.len() {
        return Err(VerifyError::GlobalRootCountMismatch {
            expected: facts.summary.global_root_count,
            actual: facts.global_init.roots.len(),
        });
    }
    if facts.summary.object_once_count != facts.global_init.object_once.len() {
        return Err(VerifyError::ObjectOnceCountMismatch {
            expected: facts.summary.object_once_count,
            actual: facts.global_init.object_once.len(),
        });
    }
    if facts.summary.top_level_eager_init_count != facts.global_init.top_level_eager_inits.len() {
        return Err(VerifyError::TopLevelEagerInitCountMismatch {
            expected: facts.summary.top_level_eager_init_count,
            actual: facts.global_init.top_level_eager_inits.len(),
        });
    }
    if facts.summary.cone_init_routine_count != facts.global_init.cone_init_routines.len() {
        return Err(VerifyError::ConeInitRoutineCountMismatch {
            expected: facts.summary.cone_init_routine_count,
            actual: facts.global_init.cone_init_routines.len(),
        });
    }
    if facts.summary.layout_class_count != facts.physical_layout.classes.len() {
        return Err(VerifyError::LayoutClassCountMismatch {
            expected: facts.summary.layout_class_count,
            actual: facts.physical_layout.classes.len(),
        });
    }
    if facts.summary.layout_enum_count != facts.physical_layout.enums.len() {
        return Err(VerifyError::LayoutEnumCountMismatch {
            expected: facts.summary.layout_enum_count,
            actual: facts.physical_layout.enums.len(),
        });
    }
    if facts.summary.layout_interface_count != facts.physical_layout.interfaces.len() {
        return Err(VerifyError::LayoutInterfaceCountMismatch {
            expected: facts.summary.layout_interface_count,
            actual: facts.physical_layout.interfaces.len(),
        });
    }
    if facts.summary.layout_class_itable_count != facts.physical_layout.class_itables.len() {
        return Err(VerifyError::LayoutClassItableCountMismatch {
            expected: facts.summary.layout_class_itable_count,
            actual: facts.physical_layout.class_itables.len(),
        });
    }
    if facts.summary.layout_callable_symbol_count != facts.physical_layout.callable_symbols.len() {
        return Err(VerifyError::LayoutCallableSymbolCountMismatch {
            expected: facts.summary.layout_callable_symbol_count,
            actual: facts.physical_layout.callable_symbols.len(),
        });
    }
    if facts.summary.callable_count != facts.callables.len() {
        return Err(VerifyError::CallableCountMismatch {
            expected: facts.summary.callable_count,
            actual: facts.callables.len(),
        });
    }
    if facts.summary.step_type_count != facts.step_types.len() {
        return Err(VerifyError::StepTypeCountMismatch {
            expected: facts.summary.step_type_count,
            actual: facts.step_types.len(),
        });
    }
    if facts.summary.resume_packing_count != facts.resume_packings.len() {
        return Err(VerifyError::ResumePackingCountMismatch {
            expected: facts.summary.resume_packing_count,
            actual: facts.resume_packings.len(),
        });
    }
    if facts.summary.continuation_object_count != facts.continuation_objects.len() {
        return Err(VerifyError::ContinuationObjectCountMismatch {
            expected: facts.summary.continuation_object_count,
            actual: facts.continuation_objects.len(),
        });
    }
    if facts.summary.surface_resume_dispatch_count != facts.surface_resume_dispatches.len() {
        return Err(VerifyError::SurfaceResumeDispatchCountMismatch {
            expected: facts.summary.surface_resume_dispatch_count,
            actual: facts.surface_resume_dispatches.len(),
        });
    }
    Ok(())
}

fn verify_global_init_contracts(facts: &LirFacts) -> Result<()> {
    for (key, root) in &facts.global_init.roots {
        if key.as_str().is_empty() || root.root.as_str().is_empty() {
            return Err(VerifyError::EmptyGlobalRoot);
        }
        if key != &root.root {
            return Err(VerifyError::MismatchedGlobalRootKey {
                key: key.as_str().to_string(),
                root: root.root.as_str().to_string(),
            });
        }
        for dependency in &root.dependencies {
            if !facts.global_init.roots.contains_key(&dependency.target) {
                return Err(VerifyError::MissingGlobalRootDependency {
                    root: key.as_str().to_string(),
                    dependency: dependency.target.as_str().to_string(),
                });
            }
        }
        if matches!(
            root.kind,
            LirGlobalRootKind::TopLevelMutableVar | LirGlobalRootKind::ExternGlobal
        ) && root.storage.is_none()
        {
            return Err(VerifyError::MissingStoragePolicy {
                root: key.as_str().to_string(),
                kind: root.kind.stable_name(),
            });
        }
        if root.kind == LirGlobalRootKind::ExternGlobal && root.has_initializer {
            return Err(VerifyError::ExternGlobalInitializerPresent {
                root: key.as_str().to_string(),
            });
        }
        verify_initializer_body_contract(key, root)?;
        match (root.kind, &root.extern_global) {
            (LirGlobalRootKind::ExternGlobal, Some(extern_global)) => {
                if !extern_global.initializer_absent {
                    return Err(VerifyError::InvalidExternGlobalInitializerAbsence {
                        root: key.as_str().to_string(),
                    });
                }
            }
            (LirGlobalRootKind::ExternGlobal, None) => {
                return Err(VerifyError::MissingExternGlobalContract {
                    root: key.as_str().to_string(),
                });
            }
            (_, Some(_)) => {
                return Err(VerifyError::UnexpectedExternGlobalContract {
                    root: key.as_str().to_string(),
                    kind: root.kind.stable_name(),
                });
            }
            (_, None) => {}
        }
    }

    for (key, contract) in &facts.global_init.object_once {
        let Some(root) = facts.global_init.roots.get(key) else {
            return Err(VerifyError::MissingGlobalContractRoot {
                root: key.as_str().to_string(),
                contract: "object-once",
            });
        };
        if root.kind != LirGlobalRootKind::ObjectSingleton {
            return Err(VerifyError::MisclassifiedObjectOnceRoot {
                root: key.as_str().to_string(),
                kind: root.kind.stable_name(),
            });
        }
        if contract.root != *key {
            return Err(VerifyError::MismatchedGlobalRootKey {
                key: key.as_str().to_string(),
                root: contract.root.as_str().to_string(),
            });
        }
        if contract.has_initializer != root.has_initializer {
            return Err(VerifyError::MismatchedGlobalInitializerPresence {
                root: key.as_str().to_string(),
                contract: "object-once",
                root_has_initializer: root.has_initializer,
                contract_has_initializer: contract.has_initializer,
            });
        }
    }

    for (key, contract) in &facts.global_init.top_level_eager_inits {
        let Some(root) = facts.global_init.roots.get(key) else {
            return Err(VerifyError::MissingGlobalContractRoot {
                root: key.as_str().to_string(),
                contract: "top-level eager init",
            });
        };
        if !matches!(
            root.kind,
            LirGlobalRootKind::TopLevelImmutableVal | LirGlobalRootKind::TopLevelMutableVar
        ) {
            return Err(VerifyError::MisclassifiedTopLevelEagerRoot {
                root: key.as_str().to_string(),
                kind: root.kind.stable_name(),
            });
        }
        if contract.root != *key {
            return Err(VerifyError::MismatchedGlobalRootKey {
                key: key.as_str().to_string(),
                root: contract.root.as_str().to_string(),
            });
        }
        if contract.storage != root.storage {
            return Err(VerifyError::MismatchedGlobalStoragePolicy {
                root: key.as_str().to_string(),
                contract: "top-level eager init",
                root_storage: storage_text(root.storage),
                contract_storage: storage_text(contract.storage),
            });
        }
        if contract.has_initializer != root.has_initializer {
            return Err(VerifyError::MismatchedGlobalInitializerPresence {
                root: key.as_str().to_string(),
                contract: "top-level eager init",
                root_has_initializer: root.has_initializer,
                contract_has_initializer: contract.has_initializer,
            });
        }
        if facts.global_init.object_once.contains_key(key) {
            return Err(VerifyError::GlobalInitClassConflict {
                root: key.as_str().to_string(),
            });
        }
    }

    let mut root_to_routine = BTreeMap::<LirGlobalRootKey, LirConeInitRoutineKey>::new();
    for (routine_key, routine) in &facts.global_init.cone_init_routines {
        if routine.routine != *routine_key {
            return Err(VerifyError::MismatchedConeInitRoutineKey {
                key: routine_key.as_u32(),
                routine: routine.routine.as_u32(),
            });
        }
        let mut seen = BTreeSet::new();
        for root in &routine.roots {
            if !facts.global_init.top_level_eager_inits.contains_key(root) {
                return Err(VerifyError::MissingConeInitRoot {
                    routine: routine_key.as_u32(),
                    root: root.as_str().to_string(),
                });
            }
            let root_facts = facts
                .global_init
                .roots
                .get(root)
                .expect("top-level eager roots are verified against roots above");
            if root_facts.cone != routine.cone {
                return Err(VerifyError::ConeInitRootConeMismatch {
                    routine: routine_key.as_u32(),
                    root: root.as_str().to_string(),
                });
            }
            if !seen.insert(root.clone()) {
                return Err(VerifyError::DuplicateConeInitRoot {
                    routine: routine_key.as_u32(),
                    root: root.as_str().to_string(),
                });
            }
            if root_to_routine.insert(root.clone(), *routine_key).is_some() {
                return Err(VerifyError::DuplicateConeInitRoot {
                    routine: routine_key.as_u32(),
                    root: root.as_str().to_string(),
                });
            }
        }
    }

    for root in facts.global_init.top_level_eager_inits.keys() {
        if !root_to_routine.contains_key(root) {
            return Err(VerifyError::MissingConeInitRoutineForRoot {
                root: root.as_str().to_string(),
            });
        }
    }

    let mut scheduled_routines = BTreeSet::new();
    for routine in &facts.global_init.final_entry_order.routines {
        if !facts.global_init.cone_init_routines.contains_key(routine) {
            return Err(VerifyError::MissingFinalEntryRoutine {
                routine: routine.as_u32(),
            });
        }
        if !scheduled_routines.insert(*routine) {
            return Err(VerifyError::DuplicateFinalEntryRoutine {
                routine: routine.as_u32(),
            });
        }
    }

    for routine in facts.global_init.cone_init_routines.keys() {
        if !scheduled_routines.contains(routine) {
            return Err(VerifyError::UnscheduledConeInitRoutine {
                routine: routine.as_u32(),
            });
        }
    }

    let mut previous_routine: Option<LirConeInitRoutineKey> = None;
    let mut previous_order: Option<u32> = None;
    for routine_key in &facts.global_init.final_entry_order.routines {
        let routine = facts
            .global_init
            .cone_init_routines
            .get(routine_key)
            .expect("final-entry routines are verified against routine map above");
        if previous_order.is_some_and(|order| routine.source_cone_order < order) {
            return Err(VerifyError::InvalidConeInitRoutineSourceOrder {
                routine: routine_key.as_u32(),
                previous_routine: previous_routine
                    .expect("previous routine should exist when previous_order exists")
                    .as_u32(),
            });
        }
        previous_routine = Some(*routine_key);
        previous_order = Some(routine.source_cone_order);
    }

    verify_cone_init_dependency_order(facts, &root_to_routine)?;

    Ok(())
}

fn verify_cone_init_dependency_order(
    facts: &LirFacts,
    root_to_routine: &BTreeMap<LirGlobalRootKey, LirConeInitRoutineKey>,
) -> Result<()> {
    let routine_order = facts
        .global_init
        .final_entry_order
        .routines
        .iter()
        .enumerate()
        .map(|(index, routine)| (*routine, index))
        .collect::<BTreeMap<_, _>>();

    for (routine_key, routine) in &facts.global_init.cone_init_routines {
        let root_order = routine
            .roots
            .iter()
            .enumerate()
            .map(|(index, root)| (root.clone(), index))
            .collect::<BTreeMap<_, _>>();
        for root_key in &routine.roots {
            let root = facts
                .global_init
                .roots
                .get(root_key)
                .expect("cone init roots are verified against roots above");
            for dependency in &root.dependencies {
                if !facts
                    .global_init
                    .top_level_eager_inits
                    .contains_key(&dependency.target)
                {
                    continue;
                }
                let Some(dependency_routine) = root_to_routine.get(&dependency.target) else {
                    continue;
                };
                if dependency_routine == routine_key {
                    let root_position = root_order[root_key];
                    let dependency_position = root_order[&dependency.target];
                    if dependency_position >= root_position {
                        return Err(VerifyError::InvalidConeInitDependencyOrder {
                            root: root_key.as_str().to_string(),
                            dependency: dependency.target.as_str().to_string(),
                        });
                    }
                } else {
                    let root_routine_position = routine_order[routine_key];
                    let dependency_routine_position = routine_order[dependency_routine];
                    if dependency_routine_position >= root_routine_position {
                        return Err(VerifyError::InvalidConeInitDependencyOrder {
                            root: root_key.as_str().to_string(),
                            dependency: dependency.target.as_str().to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn verify_initializer_body_contract(
    key: &LirGlobalRootKey,
    root: &crate::LirGlobalRootFacts,
) -> Result<()> {
    let expected = match root.kind {
        LirGlobalRootKind::TopLevelImmutableVal => {
            Some(LirInitializerBodyKind::TopLevelImmutableVal)
        }
        LirGlobalRootKind::TopLevelMutableVar => Some(LirInitializerBodyKind::TopLevelMutableVar),
        LirGlobalRootKind::ObjectSingleton => Some(LirInitializerBodyKind::ObjectSingleton),
        LirGlobalRootKind::ExternGlobal => None,
    };
    let Some(body) = &root.initializer_body else {
        if root.has_initializer && expected.is_some() {
            return Err(VerifyError::MissingInitializerBodyContract {
                root: key.as_str().to_string(),
            });
        }
        return Ok(());
    };
    let Some(expected) = expected else {
        return Err(VerifyError::UnexpectedInitializerBodyContract {
            root: key.as_str().to_string(),
            kind: root.kind.stable_name(),
        });
    };
    if body.root != *key {
        return Err(VerifyError::MismatchedInitializerBodyRoot {
            key: key.as_str().to_string(),
            body_root: body.root.as_str().to_string(),
        });
    }
    if body.kind != expected {
        return Err(VerifyError::MismatchedInitializerBodyKind {
            root: key.as_str().to_string(),
            expected: expected.stable_name(),
            actual: body.kind.stable_name(),
        });
    }
    if body.source_path.is_empty() {
        return Err(VerifyError::EmptyInitializerBodySource {
            root: key.as_str().to_string(),
        });
    }
    Ok(())
}

fn callable_id_text(id: LirCallableId) -> String {
    format!("{:?}", id)
}

fn callable_for_id(facts: &LirFacts, id: LirCallableId) -> Option<&crate::LirCallableFacts> {
    facts.callables.get(&id)
}

fn has_callable_id(facts: &LirFacts, id: LirCallableId) -> bool {
    callable_for_id(facts, id).is_some()
}

fn callable_symbol_for_id(
    facts: &LirFacts,
    id: LirCallableId,
) -> Option<&crate::LirCallableSymbolFacts> {
    facts.physical_layout.callable_symbols.get(&id)
}

fn verify_physical_layout_contracts(facts: &LirFacts) -> Result<()> {
    for (key, class) in &facts.physical_layout.classes {
        if key.is_empty() || class.fqn.is_empty() || class.layout_key.is_empty() {
            return Err(VerifyError::EmptyLayoutClass { key: key.clone() });
        }
    }
    for (key, enum_layout) in &facts.physical_layout.enums {
        if key.is_empty() || enum_layout.fqn.is_empty() {
            return Err(VerifyError::EmptyLayoutEnum { key: key.clone() });
        }
    }
    for (key, symbol) in &facts.physical_layout.callable_symbols {
        let key_text = callable_id_text(*key);
        let Some(callable) = facts.callables.get(key) else {
            return Err(VerifyError::MismatchedCallableSymbolKey {
                key: key_text,
                callable: callable_id_text(symbol.callable),
            });
        };
        if symbol.callable != *key {
            return Err(VerifyError::MismatchedCallableSymbolKey {
                key: key_text,
                callable: callable_id_text(symbol.callable),
            });
        }
        if symbol.root_fqn.is_empty() {
            return Err(VerifyError::EmptyCallableSymbolRoot { key: key_text });
        }
        if callable.root_fqn != symbol.root_fqn
            || callable.param_names != symbol.param_names
            || callable.param_tys != symbol.param_tys
            || callable.return_ty != symbol.return_ty
            || callable_symbol_abi_kind(callable.kind()) != symbol.abi_kind
        {
            return Err(VerifyError::MismatchedCallableSymbolSignature { key: key_text });
        }
    }
    for (key, symbol) in &facts.physical_layout.abi_symbols {
        if key != &symbol.key {
            return Err(VerifyError::InvalidAbiSymbol {
                key: key.clone(),
                reason: "map key does not match embedded key",
            });
        }
        if symbol.symbol.is_empty() || symbol.role.is_empty() {
            return Err(VerifyError::InvalidAbiSymbol {
                key: key.clone(),
                reason: "symbol or role is empty",
            });
        }
        if let Some(callable_ref) = symbol.callable {
            match callable_ref {
                LirCallableRef::Local(callable_id) => {
                    let Some(callable_symbol) = callable_symbol_for_id(facts, callable_id) else {
                        let reason = if has_callable_id(facts, callable_id) {
                            "local callable ref lacks callable symbol facts"
                        } else {
                            "local callable ref references missing callable"
                        };
                        return Err(VerifyError::InvalidAbiSymbol {
                            key: key.clone(),
                            reason,
                        });
                    };
                    if symbol.root_fqn.as_deref() != Some(callable_symbol.root_fqn.as_str()) {
                        return Err(VerifyError::InvalidAbiSymbol {
                            key: key.clone(),
                            reason: "root FQN drifts from callable symbol",
                        });
                    }
                    match symbol.role.as_str() {
                        "callable_export" => {
                            if callable_symbol.exported_symbol.as_deref()
                                != Some(symbol.symbol.as_str())
                            {
                                return Err(VerifyError::InvalidAbiSymbol {
                                    key: key.clone(),
                                    reason: "callable export symbol drifts from callable symbol facts",
                                });
                            }
                        }
                        "native_callable" => {
                            if callable_symbol
                                .native
                                .as_ref()
                                .map(|native| native.symbol.as_str())
                                != Some(symbol.symbol.as_str())
                            {
                                return Err(VerifyError::InvalidAbiSymbol {
                                    key: key.clone(),
                                    reason: "native symbol drifts from callable symbol facts",
                                });
                            }
                        }
                        "extern_callable" => {
                            if callable_symbol
                                .extern_
                                .as_ref()
                                .map(|extern_| extern_.symbol.as_str())
                                != Some(symbol.symbol.as_str())
                            {
                                return Err(VerifyError::InvalidAbiSymbol {
                                    key: key.clone(),
                                    reason: "extern symbol drifts from callable symbol facts",
                                });
                            }
                        }
                        _ => {
                            return Err(VerifyError::InvalidAbiSymbol {
                                key: key.clone(),
                                reason: "unknown ABI symbol role",
                            });
                        }
                    }
                }
                LirCallableRef::ExternalHash(_) => {
                    if symbol.root_fqn.as_deref().unwrap_or_default().is_empty()
                        || !facts
                            .source_signatures
                            .contains_key(symbol.root_fqn.as_deref().unwrap_or_default())
                        || !matches!(
                            symbol.role.as_str(),
                            "callable_export" | "native_callable" | "extern_callable"
                        )
                    {
                        return Err(VerifyError::InvalidAbiSymbol {
                            key: key.clone(),
                            reason: "declaration ABI symbol lacks a published source signature",
                        });
                    }
                }
            }
        } else if symbol.root_fqn.as_deref().unwrap_or_default().is_empty()
            || !matches!(symbol.role.as_str(), "native_callable" | "extern_callable")
        {
            return Err(VerifyError::InvalidAbiSymbol {
                key: key.clone(),
                reason: "body-less ABI symbol must name a native/extern root",
            });
        } else if !facts
            .source_signatures
            .contains_key(symbol.root_fqn.as_deref().unwrap_or_default())
        {
            return Err(VerifyError::InvalidAbiSymbol {
                key: key.clone(),
                reason: "body-less ABI symbol lacks a published source signature",
            });
        }
    }
    for (key, layout) in &facts.physical_layout.layout_names {
        if key != &layout.key || layout.family.is_empty() || layout.layout_name.is_empty() {
            return Err(VerifyError::InvalidLayoutName {
                key: key.clone(),
                reason: "identity, family, or layout name is empty/inconsistent",
            });
        }
    }
    for (key, identity) in &facts.physical_layout.closure_identities {
        let key_text = callable_id_text(*key);
        if !facts.callables.contains_key(key) {
            return Err(VerifyError::InvalidClosureIdentity {
                key: key_text,
                reason: "closure identity references missing callable id",
            });
        }
        if identity.callable != *key {
            return Err(VerifyError::InvalidClosureIdentity {
                key: key_text,
                reason: "map key does not match embedded callable id",
            });
        }
        if identity.root_fqn.is_empty()
            || identity.owner_root_fqn.is_empty()
            || identity.lexical_path.is_empty()
            || !has_callable_id(facts, identity.owner_callable)
        {
            return Err(VerifyError::InvalidClosureIdentity {
                key: key_text,
                reason: "closure or owner identity is missing",
            });
        }
    }
    for (class_fqn, slots) in &facts.physical_layout.class_vtables {
        for slot in slots {
            if !target_has_published_source_and_abi(
                facts,
                slot.impl_member_target,
                &slot.impl_member_fqn,
            ) {
                return Err(VerifyError::InvalidAbiSymbol {
                    key: slot.impl_member_fqn.clone(),
                    reason: "vtable implementation target lacks a target-bound source signature or ABI symbol",
                });
            }
            if class_fqn.is_empty() {
                return Err(VerifyError::InvalidLayoutName {
                    key: class_fqn.clone(),
                    reason: "class vtable owner is empty",
                });
            }
        }
    }
    for (class_fqn, itable) in &facts.physical_layout.class_itables {
        if class_fqn != &itable.class_fqn || class_fqn.is_empty() {
            return Err(VerifyError::InvalidLayoutName {
                key: class_fqn.clone(),
                reason: "class itable owner is empty or inconsistent",
            });
        }
        for entry in &itable.entries {
            if entry.method_impl_fqns.len() != entry.method_impl_targets.len() {
                return Err(VerifyError::InvalidAbiSymbol {
                    key: entry.interface_fqn.clone(),
                    reason: "itable implementation target bindings do not match implementation roots",
                });
            }
            for (impl_fqn, impl_target) in entry
                .method_impl_fqns
                .iter()
                .zip(entry.method_impl_targets.iter())
            {
                if impl_fqn.is_empty() {
                    if impl_target.is_some() {
                        return Err(VerifyError::InvalidAbiSymbol {
                            key: entry.interface_fqn.clone(),
                            reason: "empty itable slot must not publish a target binding",
                        });
                    }
                    continue;
                }
                let Some(impl_target) = impl_target else {
                    return Err(VerifyError::InvalidAbiSymbol {
                        key: impl_fqn.clone(),
                        reason: "itable implementation target lacks a target callable binding",
                    });
                };
                if !target_has_published_source_and_abi(facts, *impl_target, impl_fqn) {
                    return Err(VerifyError::InvalidAbiSymbol {
                        key: impl_fqn.clone(),
                        reason: "itable implementation target lacks a target-bound source signature or ABI symbol",
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_source_signature_contracts(facts: &LirFacts) -> Result<()> {
    for (key, signature) in &facts.source_signatures {
        if key != &signature.root_fqn
            || signature.signature_key.is_empty()
            || signature.root_fqn.is_empty()
            || signature.param_names.len() != signature.param_tys.len()
        {
            return Err(VerifyError::InvalidSourceSignature { key: key.clone() });
        }
    }
    Ok(())
}

fn verify_intrinsic_callable_contracts(facts: &LirFacts) -> Result<()> {
    for (key, intrinsic) in &facts.intrinsic_callables {
        if key != &intrinsic.root_fqn
            || intrinsic.root_fqn.is_empty()
            || intrinsic
                .named_entry_name
                .as_deref()
                .is_some_and(str::is_empty)
        {
            return Err(VerifyError::InvalidSourceSignature { key: key.clone() });
        }
    }
    Ok(())
}

fn verify_class_ctor_init_contracts(facts: &LirFacts) -> Result<()> {
    for (key, init) in &facts.class_ctor_inits {
        if key != &init.key {
            return Err(VerifyError::InvalidClassCtorInit {
                key: key.as_str().to_string(),
                reason: "map key does not match embedded key",
            });
        }
        if init.class_fqn.is_empty() || init.source_path.is_empty() {
            return Err(VerifyError::InvalidClassCtorInit {
                key: key.as_str().to_string(),
                reason: "class identity or source path is empty",
            });
        }
        if init.ctor_span_start.is_some() != init.ctor_span_end.is_some() {
            return Err(VerifyError::InvalidClassCtorInit {
                key: key.as_str().to_string(),
                reason: "ctor span endpoints are incomplete",
            });
        }
        for param in &init.params {
            if param.name.is_empty() {
                return Err(VerifyError::InvalidClassCtorInit {
                    key: key.as_str().to_string(),
                    reason: "parameter name is empty",
                });
            }
            if param.is_property
                && param
                    .property_field_fqn
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty()
            {
                return Err(VerifyError::InvalidClassCtorInit {
                    key: key.as_str().to_string(),
                    reason: "property parameter is missing field identity",
                });
            }
        }
        for step in &init.steps {
            if step.source_span_start > step.source_span_end {
                return Err(VerifyError::InvalidClassCtorInit {
                    key: key.as_str().to_string(),
                    reason: "step source span is reversed",
                });
            }
        }
        if let Some(super_call) = &init.implicit_super
            && (super_call.class_fqn.is_empty()
                || !facts.class_ctor_inits.contains_key(&super_call.target))
        {
            return Err(VerifyError::InvalidClassCtorInit {
                key: key.as_str().to_string(),
                reason: "implicit super target is unpublished",
            });
        }
        if let Some(delegation) = &init.delegation
            && (delegation.class_fqn.is_empty()
                || !facts.class_ctor_inits.contains_key(&delegation.target))
        {
            return Err(VerifyError::InvalidClassCtorInit {
                key: key.as_str().to_string(),
                reason: "delegation target is unpublished",
            });
        }
    }
    Ok(())
}

fn verify_class_ctor_call_site_contracts(facts: &LirFacts) -> Result<()> {
    for (owner, callable) in &facts.callables {
        let mut seen_sites = BTreeSet::new();
        for site in &callable.class_ctor_call_sites {
            let key_text = site_key_text(site.owner_callable, site.site_id);
            if site.owner_callable != *owner {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "callable owner and payload identity differ",
                });
            }
            if !seen_sites.insert(site.site_id) {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "duplicate class constructor call-site identity",
                });
            }
            if site.selected_ctor_span_start.is_some() != site.selected_ctor_span_end.is_some() {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "selected ctor span endpoints are incomplete",
                });
            }
            if site.class_fqn.is_empty() {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "class identity is empty",
                });
            }
            let Some(target_init) = facts.class_ctor_inits.get(&site.target_init) else {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "target constructor init body is unpublished",
                });
            };
            if target_init.class_fqn.is_empty() {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "target init class identity is empty",
                });
            }
            if site.arg_mapping.len() != target_init.params.len() {
                return Err(VerifyError::InvalidClassCtorCallSite {
                    key: key_text,
                    reason: "argument mapping arity does not match target constructor params",
                });
            }
            let mut mapped_args = BTreeSet::new();
            for arg_idx in site.arg_mapping.iter().flatten().copied() {
                if !mapped_args.insert(arg_idx) {
                    return Err(VerifyError::InvalidClassCtorCallSite {
                        key: key_text,
                        reason: "argument mapping reuses a source argument",
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_reflection_call_site_contracts(facts: &LirFacts) -> Result<()> {
    for (owner, callable) in &facts.callables {
        let mut seen_sites = BTreeSet::new();
        for site in &callable.reflection_call_sites {
            let key_text = site_key_text(site.owner_callable, site.site_id);
            if site.owner_callable != *owner {
                return Err(VerifyError::InvalidReflectionCallSite {
                    key: key_text,
                    reason: "callable owner and payload identity differ",
                });
            }
            if !seen_sites.insert(site.site_id) {
                return Err(VerifyError::InvalidReflectionCallSite {
                    key: key_text,
                    reason: "duplicate reflection call-site identity",
                });
            }
            if site.intrinsic_name.is_empty() || site.type_args.len() != 1 {
                return Err(VerifyError::InvalidReflectionCallSite {
                    key: key_text,
                    reason: "reflection intrinsic must publish exactly one type argument and a name",
                });
            }
            if !matches!(
                site.intrinsic_name.as_str(),
                "sizeOf" | "alignOf" | "kindOf" | "descOf"
            ) {
                return Err(VerifyError::InvalidReflectionCallSite {
                    key: key_text,
                    reason: "unknown reflection intrinsic name",
                });
            }
        }
    }
    Ok(())
}

fn callable_symbol_abi_kind(kind: crate::LirCallableKind) -> crate::LirCallableAbiKind {
    match kind {
        crate::LirCallableKind::Plain => crate::LirCallableAbiKind::Plain,
        crate::LirCallableKind::EffectStep => crate::LirCallableAbiKind::EffectStep,
    }
}

fn verify_type_context_contract(facts: &LirFacts) -> Result<()> {
    let ctx = &facts.type_context;
    if ctx.primary_fingerprint.is_empty()
        && ctx.materialized_fingerprint.is_empty()
        && ctx.effect_facts_fingerprint.is_empty()
    {
        return Ok(());
    }
    let fingerprints_match = ctx.materialized_fingerprint == ctx.effect_facts_fingerprint;
    match ctx.bridge_mode {
        LirTypeContextBridgeMode::Identical if !fingerprints_match => {
            return Err(VerifyError::InvalidTypeContextBridge { mode: "identical" });
        }
        LirTypeContextBridgeMode::ExplicitDisplayNameRemap if fingerprints_match => {
            return Err(VerifyError::InvalidTypeContextBridge {
                mode: "explicit_display_name_remap",
            });
        }
        LirTypeContextBridgeMode::Identical
        | LirTypeContextBridgeMode::ExplicitDisplayNameRemap => {}
    }
    if ctx.stable_wire_format.owner.is_empty() {
        return Err(VerifyError::MissingStableWireFormatOwner);
    }
    Ok(())
}

fn storage_text(storage: Option<crate::LirGlobalStoragePolicy>) -> String {
    storage
        .map(|storage| storage.stable_name().to_string())
        .unwrap_or_else(|| "<none>".to_string())
}

fn verify_callable_inventory(facts: &LirFacts) -> Result<()> {
    let mut body_versions = BTreeSet::new();
    for (key, callable) in &facts.callables {
        let key_text = callable_id_text(*key);
        if callable.root_fqn().is_empty() {
            return Err(VerifyError::EmptyCallableRoot { key: key_text });
        }
        if callable.body_version.key.owner_canonical_text().is_empty() {
            return Err(VerifyError::InvalidExactCalleeBinding {
                callable: callable.root_fqn().to_string(),
                reason: "body-version owner is empty",
            });
        }
        if !body_versions.insert(callable.body_version.key.canonical_text()) {
            return Err(VerifyError::InvalidExactCalleeBinding {
                callable: callable.root_fqn().to_string(),
                reason: "duplicate body-version key",
            });
        }
        if callable.stable_instance_key.is_empty() {
            return Err(VerifyError::EmptyStableInstanceKey { key: key_text });
        }
        match &callable.contract {
            LirCallableContract::Plain(plain) => {
                if callable.param_tys != plain.param_tys {
                    return Err(VerifyError::MismatchedCallableParamTypes {
                        callable: callable.root_fqn().to_string(),
                    });
                }
                if callable.return_ty != plain.return_ty {
                    return Err(VerifyError::MismatchedCallableReturnType {
                        callable: callable.root_fqn().to_string(),
                    });
                }
                for site in &plain.call_sites {
                    verify_call_site_contract(facts, callable.root_fqn(), &site.contract)?;
                }
                if let Some(control) = &plain.local_effect_control {
                    verify_control_body(
                        facts,
                        callable.root_fqn(),
                        &callable.body_version.key,
                        control,
                    )?;
                }
            }
            LirCallableContract::EffectStep(effect) => {
                if callable.param_tys != effect.param_tys {
                    return Err(VerifyError::MismatchedCallableParamTypes {
                        callable: callable.root_fqn().to_string(),
                    });
                }
                verify_control_body(
                    facts,
                    callable.root_fqn(),
                    &callable.body_version.key,
                    &effect.control_body,
                )?;
                if !facts
                    .step_types
                    .contains_key(&effect.dynamic_invoke_entry.step_schema)
                {
                    return Err(VerifyError::MissingControlStepType {
                        callable: callable.root_fqn().to_string(),
                        step_schema: effect.dynamic_invoke_entry.step_schema.as_u32(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_call_site_contract(
    facts: &LirFacts,
    owner: &str,
    contract: &crate::LirCallSiteContract,
) -> Result<()> {
    if matches!(contract.precision, LirEffectPrecision::SignatureFallback) {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "call-site still uses signature-fallback precision",
        });
    }
    match contract.target_mode {
        LirCallTargetMode::KnownInstance => {
            let Some(exact) = &contract.exact_callee else {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "known-instance call is missing exact callee binding",
                });
            };
            if contract.target_callables.as_slice() != std::slice::from_ref(&exact.target_callable)
            {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "target callable list does not match exact callee key",
                });
            }
            let binding = target_binding(contract, &exact.target_callable, owner)?;
            if binding.root_fqn != exact.root_fqn
                || binding.abi_symbol != exact.abi_symbol
                || binding.signature_key != exact.signature_key
            {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "exact callee binding drifts from target binding",
                });
            }
            verify_published_call_target(facts, exact.target_callable, binding, owner)?;
        }
        LirCallTargetMode::CandidateSet => {
            if contract.exact_callee.is_some() {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "non-known-instance call must not publish exact callee binding",
                });
            }
            if contract.target_callables.is_empty() {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "candidate-set call must publish at least one target callable",
                });
            }
            for target in &contract.target_callables {
                let binding = target_binding(contract, target, owner)?;
                verify_published_call_target(facts, *target, binding, owner)?;
            }
        }
        LirCallTargetMode::DynamicFallback => {
            if contract.exact_callee.is_some() {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "non-known-instance call must not publish exact callee binding",
                });
            }
            if matches!(
                contract.kind,
                crate::LirCallSiteKind::Direct
                    | crate::LirCallSiteKind::Virtual
                    | crate::LirCallSiteKind::Interface
            ) {
                return Err(VerifyError::InvalidExactCalleeBinding {
                    callable: owner.to_string(),
                    reason: "direct and dispatch call sites must not use dynamic fallback targets",
                });
            }
            for target in &contract.target_callables {
                let binding = target_binding(contract, target, owner)?;
                verify_published_call_target(facts, *target, binding, owner)?;
            }
        }
    }
    Ok(())
}

fn target_binding<'a>(
    contract: &'a crate::LirCallSiteContract,
    target: &LirCallableRef,
    owner: &str,
) -> Result<&'a crate::LirCallTargetBinding> {
    contract
        .target_bindings
        .iter()
        .find(|binding| binding.target_callable == *target)
        .ok_or_else(|| VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "target callable lacks a published target binding",
        })
}

fn verify_published_call_target(
    facts: &LirFacts,
    target: LirCallableRef,
    binding: &crate::LirCallTargetBinding,
    owner: &str,
) -> Result<()> {
    if let LirCallableRef::Local(id) = target
        && !has_callable_id(facts, id)
    {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "target local callable is missing from callable inventory",
        });
    }
    if binding.target_callable != target {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "target binding key does not match target callable",
        });
    }
    let signature = facts.source_signatures.get(&binding.root_fqn);
    if binding.root_fqn.is_empty()
        || binding.abi_symbol.is_empty()
        || signature.is_none_or(|signature| signature.signature_key != binding.signature_key)
        || !facts.physical_layout.abi_symbols.values().any(|symbol| {
            symbol.symbol == binding.abi_symbol
                && symbol.root_fqn.as_deref() == Some(binding.root_fqn.as_str())
                && symbol.callable == Some(target)
                && matches!(
                    symbol.role.as_str(),
                    "callable_export" | "native_callable" | "extern_callable"
                )
        })
    {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "target callable lacks a target-bound source signature or ABI symbol",
        });
    }
    Ok(())
}

fn verify_dispatch_target(facts: &LirFacts, target: LirCallableRef, owner: &str) -> Result<()> {
    let Some(root_fqn) = target_bound_root_fqn(facts, target) else {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "dispatch target callable is unpublished and has no target-bound ABI root",
        });
    };
    if !target_has_published_source_and_abi(facts, target, root_fqn) {
        return Err(VerifyError::InvalidExactCalleeBinding {
            callable: owner.to_string(),
            reason: "dispatch target callable lacks a target-bound source signature or ABI symbol",
        });
    }
    Ok(())
}

fn target_bound_root_fqn(facts: &LirFacts, target: LirCallableRef) -> Option<&str> {
    match target {
        LirCallableRef::Local(id) => callable_for_id(facts, id).map(|callable| callable.root_fqn()),
        LirCallableRef::ExternalHash(_) => {
            facts
                .physical_layout
                .abi_symbols
                .values()
                .find_map(|symbol| {
                    (symbol.callable == Some(target))
                        .then_some(symbol.root_fqn.as_deref())
                        .flatten()
                })
        }
    }
}

fn target_has_published_source_and_abi(
    facts: &LirFacts,
    target: LirCallableRef,
    root_fqn: &str,
) -> bool {
    !root_fqn.is_empty()
        && facts.source_signatures.contains_key(root_fqn)
        && facts.physical_layout.abi_symbols.values().any(|symbol| {
            symbol.callable == Some(target)
                && symbol.root_fqn.as_deref() == Some(root_fqn)
                && matches!(
                    symbol.role.as_str(),
                    "callable_export" | "native_callable" | "extern_callable"
                )
        })
}

fn verify_control_body(
    facts: &LirFacts,
    callable: &str,
    owner_body_version: &BodyVersionKey,
    control: &LirControlBodyFacts,
) -> Result<()> {
    if !facts.step_types.contains_key(&control.step_schema) {
        return Err(VerifyError::MissingControlStepType {
            callable: callable.to_string(),
            step_schema: control.step_schema.as_u32(),
        });
    }
    let Some(continuation_object) = facts.continuation_objects.get(&control.continuation_object)
    else {
        return Err(VerifyError::MissingContinuationObject {
            callable: callable.to_string(),
            object_id: control.continuation_object.as_u32(),
        });
    };
    if &continuation_object.owner_body_version != owner_body_version {
        return Err(VerifyError::MissingContinuationObject {
            callable: callable.to_string(),
            object_id: control.continuation_object.as_u32(),
        });
    }
    for packing_id in &control.resume_packings {
        if !facts.resume_packings.contains_key(packing_id) {
            return Err(VerifyError::MissingResumePacking {
                callable: callable.to_string(),
                packing_id: packing_id.as_u32(),
            });
        }
    }
    Ok(())
}

fn verify_source_call_site_contracts(facts: &LirFacts) -> Result<()> {
    for (owner, callable) in &facts.callables {
        let mut seen_sites = BTreeSet::new();
        for site in &callable.source_call_sites {
            let key_text = site_key_text(site.owner_callable, site.site_id);
            if site.owner_callable != *owner {
                return Err(VerifyError::InvalidSourceCallSite {
                    key: key_text,
                    reason: "callable owner and payload identity differ",
                });
            }
            if !seen_sites.insert(site.site_id) {
                return Err(VerifyError::InvalidSourceCallSite {
                    key: key_text,
                    reason: "duplicate source call-site identity",
                });
            }
            if site
                .semantic_root_fqn
                .as_ref()
                .is_some_and(|root| root.is_empty())
            {
                return Err(VerifyError::InvalidSourceCallSite {
                    key: key_text,
                    reason: "semantic root is empty",
                });
            }
            if site
                .named_entry_name
                .as_ref()
                .is_some_and(|entry| entry.is_empty())
            {
                return Err(VerifyError::InvalidSourceCallSite {
                    key: key_text,
                    reason: "named intrinsic entry is empty",
                });
            }
            verify_call_site_contract(
                facts,
                &callable_id_text(site.owner_callable),
                &site.contract,
            )?;
        }
    }
    Ok(())
}

fn verify_dynamic_invoke_contracts(facts: &LirFacts) -> Result<()> {
    for callable in facts.callables.values() {
        for contract in callable.dynamic_invoke_contracts() {
            let key_text = site_key_text(contract.owner_callable, contract.site_id);
            if !has_callable_id(facts, contract.owner_callable) {
                return Err(VerifyError::MissingDynamicInvokeOwner { key: key_text });
            }
            verify_call_site_contract(
                facts,
                &callable_id_text(contract.owner_callable),
                &contract.call,
            )?;
            if let Some(step_schema) = contract.call.callee_step_schema
                && !facts.step_types.contains_key(&step_schema)
            {
                return Err(VerifyError::MissingDynamicInvokeTargetStep {
                    key: key_text,
                    step_schema: step_schema.as_u32(),
                });
            }
            if let Some(dispatch) = &contract.carrier.dispatch {
                if dispatch.owner_callable != contract.owner_callable
                    || dispatch.site_id != contract.site_id
                {
                    return Err(VerifyError::MissingDynamicInvokeDispatch { key: key_text });
                }
                verify_dispatch_contract(facts, dispatch)?;
            }
            if contract.target_body_versions.len() != contract.call.target_callables.len() {
                return Err(VerifyError::MissingDynamicInvokeTargetStep {
                    key: key_text,
                    step_schema: 0,
                });
            }
            for (target, body_version) in contract
                .call
                .target_callables
                .iter()
                .zip(contract.target_body_versions.iter())
            {
                let Some(target_id) = target.local_id() else {
                    return Err(VerifyError::MissingDynamicInvokeTargetStep {
                        key: key_text,
                        step_schema: 0,
                    });
                };
                let Some(callable) = facts.callables.get(&target_id) else {
                    return Err(VerifyError::MissingDynamicInvokeTargetStep {
                        key: key_text,
                        step_schema: 0,
                    });
                };
                if body_version.owner_canonical_text()
                    != callable.body_version.key.owner_canonical_text()
                {
                    return Err(VerifyError::MissingDynamicInvokeTargetStep {
                        key: key_text,
                        step_schema: 0,
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_dispatch_contracts(facts: &LirFacts) -> Result<()> {
    for callable in facts.callables.values() {
        for dispatch in callable.dispatch_contracts() {
            verify_dispatch_contract(facts, dispatch)?;
        }
    }
    Ok(())
}

fn verify_dispatch_contract(facts: &LirFacts, dispatch: &crate::LirDispatchContract) -> Result<()> {
    if !has_callable_id(facts, dispatch.owner_callable) {
        return Err(VerifyError::MissingDispatchOwner {
            key: site_key_text(dispatch.owner_callable, dispatch.site_id),
        });
    }
    for target in &dispatch.candidate_targets {
        verify_dispatch_target(facts, *target, &dispatch.member_fqn)?;
    }
    Ok(())
}

fn verify_continuation_objects(facts: &LirFacts) -> Result<()> {
    let published_body_versions = facts
        .callables
        .values()
        .map(|callable| callable.body_version.key.canonical_text())
        .collect::<BTreeSet<_>>();
    for (object_id, object) in &facts.continuation_objects {
        if object_id != &object.object_id {
            return Err(VerifyError::MissingContinuationObjectPacking {
                object_id: object_id.as_u32(),
                packing_id: object.object_id.as_u32(),
            });
        }
        if !published_body_versions.contains(&object.owner_body_version.canonical_text()) {
            return Err(VerifyError::MissingContinuationObjectPacking {
                object_id: object_id.as_u32(),
                packing_id: 0,
            });
        }
        for packing_id in &object.implemented_packings {
            if !facts.resume_packings.contains_key(packing_id) {
                return Err(VerifyError::MissingContinuationObjectPacking {
                    object_id: object_id.as_u32(),
                    packing_id: packing_id.as_u32(),
                });
            }
        }
    }
    Ok(())
}

fn verify_surface_resume_dispatches(facts: &LirFacts) -> Result<()> {
    for (schema, dispatch) in &facts.surface_resume_dispatches {
        if !facts.step_types.contains_key(&dispatch.out_step_schema) {
            return Err(VerifyError::MissingSurfaceResumeOutStep {
                continuation_schema: schema.as_u32(),
                step_schema: dispatch.out_step_schema.as_u32(),
            });
        }
    }
    Ok(())
}

fn site_key_text(owner_callable: scoopc_ids::LirCallableId, site_id: scoopc_ids::SiteId) -> String {
    format!(
        "{}:site{}",
        callable_id_text(owner_callable),
        site_id.as_u32()
    )
}
