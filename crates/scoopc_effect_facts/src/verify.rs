//! Structural verifier for effect fact products.

use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fmt;

use crate::facts::{CallableAbiKind, SiteEffectFacts};
use crate::schema::{CaseSet, CaseTag, ContinuationSchemaId, StepSchemaId};
use crate::{EffectFacts, StepSchema};

/// Result type returned by effect fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before effect facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    DuplicateStableInstanceKey(String),
    DuplicateStepCase {
        schema: u32,
        case: u32,
    },
    MissingStepSchema {
        context: String,
        schema: u32,
    },
    MissingContinuationSchema {
        context: String,
        schema: u32,
    },
    MissingCaseTag {
        context: String,
        schema: u32,
        case: u32,
    },
    CallableAbiSchemaMismatch {
        callable: String,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateStableInstanceKey(key) => {
                write!(f, "duplicate effect stable instance key `{key}`")
            }
            Self::DuplicateStepCase { schema, case } => {
                write!(f, "duplicate case c{case} in step schema s{schema}")
            }
            Self::MissingStepSchema { context, schema } => {
                write!(f, "{context} references missing step schema s{schema}")
            }
            Self::MissingContinuationSchema { context, schema } => {
                write!(
                    f,
                    "{context} references missing continuation schema k{schema}"
                )
            }
            Self::MissingCaseTag {
                context,
                schema,
                case,
            } => write!(
                f,
                "{context} references missing case c{case} in step schema s{schema}"
            ),
            Self::CallableAbiSchemaMismatch { callable } => write!(
                f,
                "callable `{callable}` has ABI/schema fields that disagree"
            ),
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the effect-facts stage.
pub fn verify_effect_facts(facts: &EffectFacts) -> Result<()> {
    verify_unique_stable_instance_keys(facts)?;
    verify_schema_graph(facts)?;
    verify_callables(facts)?;
    verify_bodies(facts)?;
    Ok(())
}

fn verify_unique_stable_instance_keys(facts: &EffectFacts) -> Result<()> {
    verify_unique_keys(facts.callables.keys())?;
    verify_unique_keys(facts.bodies.keys())?;
    Ok(())
}

fn verify_unique_keys<'a>(
    keys: impl IntoIterator<Item = &'a scoopc_ids::StableEffectInstanceKey>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for key in keys {
        let canonical = key.as_str().to_string();
        if !seen.insert(canonical.clone()) {
            return Err(VerifyError::DuplicateStableInstanceKey(canonical));
        }
    }
    Ok(())
}

fn verify_schema_graph(facts: &EffectFacts) -> Result<()> {
    for (schema_id, schema) in &facts.step_schemas {
        verify_step_schema(*schema_id, schema, facts)?;
    }
    for (continuation_id, continuation) in &facts.continuation_schemas {
        let context = format!("continuation k{} out_step_schema", continuation_id.as_u32());
        verify_step_schema_exists(facts, continuation.out_step_schema(), context)?;
    }
    Ok(())
}

fn verify_step_schema(
    schema_id: StepSchemaId,
    schema: &StepSchema,
    facts: &EffectFacts,
) -> Result<()> {
    let mut seen_cases = BTreeSet::new();
    for case in schema.cases() {
        if !seen_cases.insert(case.case_tag()) {
            return Err(VerifyError::DuplicateStepCase {
                schema: schema_id.as_u32(),
                case: case.case_tag().as_u32(),
            });
        }
        verify_continuation_schema_exists(
            facts,
            case.continuation_schema(),
            format!(
                "step s{} case c{}",
                schema_id.as_u32(),
                case.case_tag().as_u32()
            ),
        )?;
    }
    Ok(())
}

fn verify_callables(facts: &EffectFacts) -> Result<()> {
    for (key, callable) in &facts.callables {
        let callable_name = key.readable_path().to_string();
        match (callable.call_abi_kind(), callable.body_step_schema()) {
            (CallableAbiKind::Plain, None) => {}
            (CallableAbiKind::EffectStep, Some(schema)) => verify_step_schema_exists(
                facts,
                schema,
                format!("callable {callable_name} body_step_schema"),
            )?,
            _ => {
                return Err(VerifyError::CallableAbiSchemaMismatch {
                    callable: callable_name,
                });
            }
        }
        verify_case_set(
            facts,
            callable.resolved_outward_cases(),
            format!("callable {callable_name} resolved_outward_cases"),
        )?;
    }
    Ok(())
}

fn verify_bodies(facts: &EffectFacts) -> Result<()> {
    for (key, body) in &facts.bodies {
        let body_name = key.readable_path();
        if let Some(schema) = body.local_control_step_schema() {
            verify_step_schema_exists(
                facts,
                schema,
                format!("body {body_name} local_control_step_schema"),
            )?;
        }
        for (block_id, block) in body.blocks() {
            let context = format!("body {body_name} block bb{}", block_id.as_u32());
            verify_case_set(
                facts,
                block.ambient_cases(),
                format!("{context} ambient_cases"),
            )?;
            verify_case_set(
                facts,
                block.outward_cases(),
                format!("{context} outward_cases"),
            )?;
        }
        for (site_id, site) in body.sites() {
            verify_site_facts(
                facts,
                site,
                format!("body {body_name} site{}", site_id.as_u32()),
            )?;
        }
    }
    Ok(())
}

fn verify_site_facts(facts: &EffectFacts, site: &SiteEffectFacts, context: String) -> Result<()> {
    match site {
        SiteEffectFacts::Call(call) => {
            if let Some(schema) = call.callee_step_schema() {
                verify_step_schema_exists(facts, schema, format!("{context} callee_schema"))?;
            }
            verify_case_set(
                facts,
                call.resolved_cases(),
                format!("{context} resolved_cases"),
            )?;
        }
        SiteEffectFacts::ClassCtor(class_ctor) => verify_case_set(
            facts,
            class_ctor.emitted_cases(),
            format!("{context} class_ctor emitted_cases"),
        )?,
        SiteEffectFacts::Perform(perform) => verify_continuation_schema_exists(
            facts,
            perform.captured_cont_schema(),
            format!("{context} perform captured_cont_schema"),
        )?,
        SiteEffectFacts::Resume(resume) => {
            verify_continuation_schema_exists(
                facts,
                resume.continuation_schema(),
                format!("{context} resume continuation_schema"),
            )?;
            verify_step_schema_exists(
                facts,
                resume.out_step_schema(),
                format!("{context} resume out_step_schema"),
            )?;
            verify_case_set(
                facts,
                resume.resolved_cases(),
                format!("{context} resume cases"),
            )?;
        }
        SiteEffectFacts::Handle(handle) => {
            verify_case_set(
                facts,
                handle.handled_cases(),
                format!("{context} handled_cases"),
            )?;
            verify_case_set(
                facts,
                handle.body_outward_cases(),
                format!("{context} body_outward_cases"),
            )?;
            verify_case_set(
                facts,
                handle.finally_outward_cases(),
                format!("{context} finally_outward_cases"),
            )?;
            for (index, arm) in handle.arm_facts().iter().enumerate() {
                verify_continuation_schema_exists(
                    facts,
                    arm.continuation_schema(),
                    format!("{context} arm {index} continuation_schema"),
                )?;
                verify_case_tag(
                    facts,
                    handle.handled_cases().schema(),
                    arm.handled_case(),
                    format!("{context} arm {index} handled_case"),
                )?;
                verify_case_set(
                    facts,
                    arm.arm_outward_cases(),
                    format!("{context} arm {index} outward_cases"),
                )?;
            }
        }
    }
    Ok(())
}

fn verify_case_set(facts: &EffectFacts, case_set: &CaseSet, context: String) -> Result<()> {
    if case_set.is_empty() {
        return Ok(());
    }
    verify_step_schema_exists(facts, case_set.schema(), context.clone())?;
    for tag in case_set.tags() {
        verify_case_tag(facts, case_set.schema(), *tag, context.clone())?;
    }
    Ok(())
}

fn verify_case_tag(
    facts: &EffectFacts,
    schema: StepSchemaId,
    tag: CaseTag,
    context: String,
) -> Result<()> {
    let step_schema =
        facts
            .step_schemas
            .get(&schema)
            .ok_or_else(|| VerifyError::MissingStepSchema {
                context: context.clone(),
                schema: schema.as_u32(),
            })?;
    let exists = step_schema
        .cases()
        .iter()
        .any(|case| case.case_tag() == tag);
    if !exists {
        return Err(VerifyError::MissingCaseTag {
            context,
            schema: schema.as_u32(),
            case: tag.as_u32(),
        });
    }
    Ok(())
}

fn verify_step_schema_exists(
    facts: &EffectFacts,
    schema: StepSchemaId,
    context: String,
) -> Result<()> {
    if facts.step_schemas.contains_key(&schema) {
        return Ok(());
    }
    Err(VerifyError::MissingStepSchema {
        context,
        schema: schema.as_u32(),
    })
}

fn verify_continuation_schema_exists(
    facts: &EffectFacts,
    schema: ContinuationSchemaId,
    context: String,
) -> Result<()> {
    if facts.continuation_schemas.contains_key(&schema) {
        return Ok(());
    }
    Err(VerifyError::MissingContinuationSchema {
        context,
        schema: schema.as_u32(),
    })
}
