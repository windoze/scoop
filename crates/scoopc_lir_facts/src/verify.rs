//! Structural verifier for LIR fact products.

use std::error::Error;
use std::fmt;

use crate::{LirCallableContract, LirControlBodyFacts, LirFacts};

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
    OptRevisionMismatch {
        summary: u64,
        pipeline: u64,
    },
    EmptyCallableRoot {
        key: String,
    },
    EmptyStableInstanceKey {
        key: String,
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
            Self::OptRevisionMismatch { summary, pipeline } => write!(
                f,
                "LIR summary opt revision {summary} does not match pipeline revision {pipeline}"
            ),
            Self::EmptyStableInstanceKey { key } => {
                write!(f, "LIR callable `{key}` has an empty stable instance key")
            }
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
    verify_callable_inventory(facts)?;
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

fn verify_callable_inventory(facts: &LirFacts) -> Result<()> {
    for (key, callable) in &facts.callables {
        if callable.root_fqn().is_empty() {
            return Err(VerifyError::EmptyCallableRoot {
                key: key.as_str().to_string(),
            });
        }
        if callable.stable_instance_key.is_empty() {
            return Err(VerifyError::EmptyStableInstanceKey {
                key: key.as_str().to_string(),
            });
        }
        match &callable.contract {
            LirCallableContract::Plain(plain) => {
                if let Some(control) = &plain.local_effect_control {
                    verify_control_body(facts, callable.root_fqn(), control)?;
                }
            }
            LirCallableContract::EffectStep(effect) => {
                verify_control_body(facts, callable.root_fqn(), &effect.control_body)?;
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

fn verify_control_body(
    facts: &LirFacts,
    callable: &str,
    control: &LirControlBodyFacts,
) -> Result<()> {
    if !facts.step_types.contains_key(&control.step_schema) {
        return Err(VerifyError::MissingControlStepType {
            callable: callable.to_string(),
            step_schema: control.step_schema.as_u32(),
        });
    }
    if !facts
        .continuation_objects
        .contains_key(&control.continuation_object)
    {
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

fn verify_dynamic_invoke_contracts(facts: &LirFacts) -> Result<()> {
    for (key, contract) in &facts.dynamic_invokes {
        let key_text = dynamic_key_text(key);
        if !facts.callables.contains_key(&key.owner_callable) {
            return Err(VerifyError::MissingDynamicInvokeOwner { key: key_text });
        }
        if let Some(step_schema) = contract.call.callee_step_schema
            && !facts.step_types.contains_key(&step_schema)
        {
            return Err(VerifyError::MissingDynamicInvokeTargetStep {
                key: key_text,
                step_schema: step_schema.as_u32(),
            });
        }
        if let Some(dispatch_key) = &contract.carrier.dispatch
            && !facts.dispatches.contains_key(dispatch_key)
        {
            return Err(VerifyError::MissingDynamicInvokeDispatch { key: key_text });
        }
    }
    Ok(())
}

fn verify_dispatch_contracts(facts: &LirFacts) -> Result<()> {
    for key in facts.dispatches.keys() {
        if !facts.callables.contains_key(&key.owner_callable) {
            return Err(VerifyError::MissingDispatchOwner {
                key: dispatch_key_text(key),
            });
        }
    }
    Ok(())
}

fn verify_continuation_objects(facts: &LirFacts) -> Result<()> {
    for (object_id, object) in &facts.continuation_objects {
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

fn dynamic_key_text(key: &crate::LirDynamicInvokeKey) -> String {
    format!(
        "{}:site{}",
        key.owner_callable.as_str(),
        key.site_id.as_u32()
    )
}

fn dispatch_key_text(key: &crate::LirDispatchKey) -> String {
    format!(
        "{}:site{}",
        key.owner_callable.as_str(),
        key.site_id.as_u32()
    )
}
