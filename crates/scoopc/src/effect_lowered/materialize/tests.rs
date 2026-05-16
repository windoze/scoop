//! Materialize integration tests.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::effect_facts::{
    CallTargetMode, CallableAbiKind, ImplPlan, NestedHandleClassification, SiteEffectFacts,
};
use crate::effect_lowered::LateLoweredProgramBuilder;
use crate::effect_lowered::ir::{
    BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredBoundarySourceConsumption,
    LateLoweredCompletionPayloadSource, LateLoweredContinuationMethodReachability,
    LateLoweredContinuationResumeBody, LateLoweredFrameSlotKind,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandlePendingCompletion,
    LateLoweredHandleStateRegion, LateLoweredOneShotPolicy, LateLoweredOperandValueSource,
    LateLoweredSourceStatementClassificationKind, LateLoweredStateTerminator, LateLoweredStepType,
    LateLoweredSurfaceResumeDispatchPublication, LateLoweredSurfaceResumeDispatchSourceKind,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource, SystemSlotKind,
};
use crate::mir::{CallArg, Operand, Rvalue, SiteId, StatementKind};
use crate::pipeline::load_effect_facts_stage_output_for_dump;
use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;

fn session() -> Session {
    Session::with_options(SessionOptions::new()).unwrap()
}

fn load_fixture(phase: &str, name: &str) -> SourceFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(phase)
        .join(name);
    SourceFile::load(&path).expect("fixture 应可加载")
}

struct RawMaterializedOutput {
    effect_facts_stage_output: crate::pipeline::EffectFactsStageOutput,
    program: crate::effect_lowered::LateLoweredProgram,
}

impl RawMaterializedOutput {
    fn program(&self) -> &crate::effect_lowered::LateLoweredProgram {
        &self.program
    }

    fn types(&self) -> &crate::ty::TypeStore {
        self.effect_facts_stage_output.types()
    }
}

fn load_output(source: &SourceFile) -> RawMaterializedOutput {
    let session = session();
    let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(&session, source)
        .expect("fixture 应可通过 effect-facts stage");
    let program = LateLoweredProgramBuilder::from_canonical_inputs(
        effect_facts_stage_output.materialized_pass_view(),
        effect_facts_stage_output.effect_facts(),
        effect_facts_stage_output.types(),
    )
    .with_nominal_direct_supertypes(
        crate::effect_lowered::builder::collect_nominal_direct_supertypes_from_mir_file(
            effect_facts_stage_output.file(),
        ),
    )
    .build()
    .expect("fixture 应可通过 raw late-lowering builder");
    RawMaterializedOutput {
        effect_facts_stage_output,
        program,
    }
}

fn callable<'a>(
    output: &'a RawMaterializedOutput,
    fqn: &str,
) -> &'a crate::effect_lowered::LateLoweredCallable {
    output
        .program()
        .callable(fqn)
        .unwrap_or_else(|| panic!("late-lowered program 应发布 {fqn}"))
}

fn site_boundary(
    callable: &crate::effect_lowered::LateLoweredCallable,
    kind: BoundarySiteKind,
) -> &crate::effect_lowered::ir::LateLoweredBoundary {
    callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::Site { kind: boundary_kind, .. }
                        if boundary_kind == kind
                )
            })
            .expect("应找到指定 kind 的 boundary")
}

fn handle_dispatch_state(
    callable: &crate::effect_lowered::LateLoweredCallable,
    site_id: SiteId,
) -> &crate::effect_lowered::ir::LateLoweredState {
    callable
        .state_graph()
        .states()
        .iter()
        .find(|state| {
            matches!(
                state.terminator(),
                LateLoweredStateTerminator::HandleDispatch { site_id: state_site, .. }
                    if *state_site == site_id
            )
        })
        .expect("应找到指定 site 的 HandleDispatch state")
}

fn handle_site_facts<'a>(
    output: &'a RawMaterializedOutput,
    callable: &crate::effect_lowered::LateLoweredCallable,
    site_id: SiteId,
) -> &'a crate::effect_facts::HandleSiteEffectFacts {
    let body_facts = output
        .effect_facts_stage_output
        .effect_facts()
        .body(callable.instance_key())
        .expect("callable 应发布 body effect facts");
    match body_facts.site(site_id) {
        Some(SiteEffectFacts::Handle(facts)) => facts,
        other => panic!("应找到指定 site 的 Handle facts，而不是 {other:?}"),
    }
}

#[test]
fn step_materialization_keeps_canonical_cases_and_dynamic_entry_states() {
    let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
    let leaf = callable(&output, "sample.leaf");
    let step_type = output
        .program()
        .step_type(leaf.step_schema())
        .expect("callable 应能回查 canonical Step shell");
    let case_fqns = step_type
        .cases()
        .iter()
        .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        case_fqns,
        [
            "sample.Ping.hit".to_string(),
            "scoop.core.Raise.raise".to_string()
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        leaf.dynamic_invoke_entry().step_schema(),
        leaf.step_schema()
    );
    assert_eq!(
        leaf.dynamic_invoke_entry().entry_state(),
        leaf.state_graph().entry_state()
    );
    assert_eq!(
        leaf.dynamic_invoke_entry().complete_state(),
        leaf.state_graph().complete_state()
    );
}

#[test]
fn resume_interface_completeness_groups_methods_by_effect_family() {
    let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
    let leaf = callable(&output, "sample.leaf");
    let interfaces = leaf
        .resume_packings()
        .iter()
        .map(|interface_id| {
            output
                .program()
                .resume_packing(*interface_id)
                .expect("callable 应能回查 resume interface")
        })
        .collect::<Vec<_>>();

    assert_eq!(interfaces.len(), 2);
    assert_eq!(
        interfaces
            .iter()
            .map(|interface| interface.effect_family().effect_fqn().to_string())
            .collect::<BTreeSet<_>>(),
        ["sample.Ping".to_string(), "scoop.core.Raise".to_string()]
            .into_iter()
            .collect()
    );
    assert!(
        interfaces
            .iter()
            .all(|interface| interface.return_step_schema() == leaf.step_schema())
    );
    assert_eq!(
        interfaces
            .iter()
            .map(|interface| interface.methods().len())
            .sum::<usize>(),
        output
            .program()
            .step_type(leaf.step_schema())
            .expect("callable 应能回查 step shell")
            .cases()
            .len()
    );
}

#[test]
fn continuation_object_materializes_surface_resume_and_one_shot_contracts() {
    let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
    let leaf = callable(&output, "sample.leaf");
    let object = output
        .program()
        .continuation_object(leaf.continuation_object())
        .expect("callable 应能回查 continuation object");

    assert_eq!(object.surface_resumes().len(), 2);
    assert_eq!(object.methods().len(), 2);
    assert_eq!(
        object
            .methods()
            .iter()
            .filter(|method| {
                method.reachability() == LateLoweredContinuationMethodReachability::Reachable
            })
            .count(),
        1
    );
    assert!(object.surface_resumes().iter().any(|surface| {
        output.types().display(surface.surface_ty()).to_string()
            == "scoop.core.Continuation<Unit, Unit, eff sample.Ping>"
            && matches!(
                surface.body(),
                LateLoweredContinuationResumeBody::ResumeCapturedState {
                    repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward
                }
            )
    }));
    assert!(object.surface_resumes().iter().any(|surface| {
        surface.concrete_op_key().instance_key().template.fqn == "scoop.core.Raise.raise"
            && surface.reachability() == LateLoweredContinuationMethodReachability::Unreachable
    }));
}

#[test]
fn surface_resume_dispatch_inventory_marks_shared_schema_object_method_sources() {
    let output = load_output(&load_fixture(
        "build",
        "effect_lowered_step_enum_single_case.scoop",
    ));
    let worker = callable(&output, "fixtures.build.singleCaseWorker");
    let step = output
        .program()
        .step_type(worker.step_schema())
        .expect("worker step schema 应可回查");
    let shared_schema = step
        .case(crate::effect_facts::CaseTag::new(0))
        .expect("worker c0 应存在")
        .continuation_schema();
    assert_eq!(
        shared_schema,
        step.case(crate::effect_facts::CaseTag::new(1))
            .expect("worker c1 应存在")
            .continuation_schema()
    );

    let entry = output
        .program()
        .surface_resume_dispatch(shared_schema)
        .expect("shared schema 应发布 dispatch inventory");
    assert_eq!(
        entry.source_kind(),
        LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
    );

    let mut saw_surface_c0 = false;
    let mut saw_surface_c1 = false;
    let mut saw_method_c0 = false;
    for publication in entry.publications() {
        match publication {
            LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
                object_id,
                case_tag,
                reachability,
            } if *object_id == worker.continuation_object()
                && *case_tag == crate::effect_facts::CaseTag::new(0) =>
            {
                assert_eq!(
                    *reachability,
                    LateLoweredContinuationMethodReachability::Reachable
                );
                saw_surface_c0 = true;
            }
            LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
                object_id,
                case_tag,
                reachability,
            } if *object_id == worker.continuation_object()
                && *case_tag == crate::effect_facts::CaseTag::new(1) =>
            {
                assert_eq!(
                    *reachability,
                    LateLoweredContinuationMethodReachability::Unreachable
                );
                saw_surface_c1 = true;
            }
            LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                object_id,
                case_tag,
                reachability,
                ..
            } if *object_id == worker.continuation_object()
                && *case_tag == crate::effect_facts::CaseTag::new(0) =>
            {
                assert_eq!(
                    *reachability,
                    LateLoweredContinuationMethodReachability::Reachable
                );
                saw_method_c0 = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_surface_c0,
        "shared schema 应保留 c0 surface publication"
    );
    assert!(
        saw_surface_c1,
        "shared schema 应保留 c1 surface publication"
    );
    assert!(
        saw_method_c0,
        "shared schema 应明确发布唯一可达的 internal method source"
    );
}

#[test]
fn surface_resume_dispatch_inventory_covers_resume_site_only_and_handle_binder_schema() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let run = callable(&output, "run");

    let resume_schema = run
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                Some(lowering.facts().continuation_schema())
            }
            _ => None,
        })
        .expect("fixture 应至少包含一个 resume boundary schema");
    let resume_entry = output
        .program()
        .surface_resume_dispatch(resume_schema)
        .expect("resume-site-only schema 应发布 dispatch inventory");
    assert_eq!(
        resume_entry.source_kind(),
        LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
    );
    assert!(resume_entry.publications().iter().any(|publication| {
        matches!(
            publication,
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_continuation_object,
                site_id,
                ..
            } if *owner_continuation_object == run.continuation_object() && site_id.as_u32() == 9
        )
    }));

    let handle_schema = run
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch { contract, .. } => {
                contract.handled_arms().iter().find_map(|arm| {
                    arm.continuation_binder()
                        .map(|binder| binder.continuation_schema())
                })
            }
            _ => None,
        })
        .expect("fixture 应至少包含一个 handle continuation binder schema");
    let handle_entry = output
        .program()
        .surface_resume_dispatch(handle_schema)
        .expect("handle binder schema 应发布 dispatch inventory");
    assert_eq!(
        handle_entry.source_kind(),
        LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
    );
    assert!(handle_entry.publications().iter().any(|publication| {
        matches!(
            publication,
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_continuation_object,
                site_id,
                arm_ordinal,
                handled_case,
                ..
            } if *owner_continuation_object == run.continuation_object()
                && site_id.as_u32() == 0
                && *arm_ordinal == 0
                && *handled_case == crate::effect_facts::CaseTag::new(0)
        )
    }));
}

#[test]
fn boundary_lowering_materializes_effectful_call_dispatch_contract() {
    let output = load_output(&load_fixture(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
    ));
    let call_value = callable(&output, "sample.callValue");
    let boundary = site_boundary(call_value, BoundarySiteKind::Call);
    let LateLoweredBoundaryLowering::Call(lowering) = boundary
        .lowering()
        .expect("call boundary 应发布 lowering contract")
    else {
        panic!("call boundary 应物化成 Call lowering")
    };

    assert_eq!(
        lowering.facts().target_mode(),
        CallTargetMode::DynamicFallback
    );
    assert_eq!(
        lowering.dispatch().input_step_schema(),
        lowering.facts().callee_schema()
    );
    assert_eq!(
        lowering.dispatch().complete().target_state(),
        boundary.resume_state()
    );
    assert_eq!(lowering.dispatch().outward_cases().len(), 2);
    assert!(lowering.consumed_runtime_error_case().is_none());
    assert_eq!(
        lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| {
                forwarding
                    .emission()
                    .concrete_op_key()
                    .instance_key()
                    .template
                    .fqn
                    .clone()
            })
            .collect::<BTreeSet<_>>(),
        ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
            .into_iter()
            .collect()
    );
}

#[test]
fn effect_lowered_boundary_operand_contract_publishes_direct_dynamic_and_perform_sources() {
    let direct_output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let main = callable(&direct_output, "main");
    let direct_boundary = site_boundary(main, BoundarySiteKind::Call);
    let LateLoweredBoundaryLowering::Call(direct_lowering) = direct_boundary
        .lowering()
        .expect("direct call boundary 应发布 lowering contract")
    else {
        panic!("main 的 boundary 应物化成 Call lowering")
    };
    assert!(matches!(
        direct_lowering.operand_contract().source_consumption(),
        LateLoweredBoundarySourceConsumption::Statement {
            consumes_last_statement: true,
            ..
        }
    ));
    assert!(
        direct_lowering
            .operand_contract()
            .carrier_source()
            .is_none()
    );
    assert_eq!(direct_lowering.operand_contract().arg_sources().len(), 1);
    assert_eq!(
        direct_output
            .types()
            .display(direct_lowering.operand_contract().arg_sources()[0].source_ty())
            .to_string(),
        "Bool"
    );
    assert!(matches!(
        direct_lowering.operand_contract().arg_sources()[0].value(),
        LateLoweredOperandValueSource::Local(_)
            | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Bool(_))
    ));
    assert!(
        direct_lowering.operand_contract().arg_sources()[0]
            .span()
            .is_some()
    );

    let dynamic_output = load_output(&load_fixture(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
    ));
    let call_value = callable(&dynamic_output, "sample.callValue");
    let dynamic_boundary = site_boundary(call_value, BoundarySiteKind::Call);
    let LateLoweredBoundaryLowering::Call(dynamic_lowering) = dynamic_boundary
        .lowering()
        .expect("dynamic call boundary 应发布 lowering contract")
    else {
        panic!("callValue 的 boundary 应物化成 Call lowering")
    };
    assert_eq!(dynamic_lowering.operand_contract().arg_sources().len(), 0);
    assert!(matches!(
        dynamic_lowering.operand_contract().source_consumption(),
        LateLoweredBoundarySourceConsumption::Statement { .. }
    ));
    assert!(matches!(
        dynamic_lowering
            .operand_contract()
            .carrier_source()
            .expect("dynamic call 应发布 carrier source")
            .value(),
        LateLoweredOperandValueSource::Local(_)
    ));

    let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
    let handled_main = callable(&perform_output, "a.main");
    let perform_boundary = site_boundary(handled_main, BoundarySiteKind::Perform);
    let LateLoweredBoundaryLowering::Perform(perform_lowering) = perform_boundary
        .lowering()
        .expect("perform boundary 应发布 lowering contract")
    else {
        panic!("perform boundary 应物化成 Perform lowering")
    };
    assert!(matches!(
        perform_lowering.operand_contract().source_consumption(),
        LateLoweredBoundarySourceConsumption::Terminator { .. }
    ));
    assert_eq!(
        perform_lowering.operand_contract().payload_sources().len(),
        1
    );
    assert_eq!(
        perform_output
            .types()
            .display(perform_lowering.operand_contract().payload_sources()[0].source_ty())
            .to_string(),
        "Int"
    );
    assert!(matches!(
        perform_lowering.operand_contract().payload_sources()[0].value(),
        LateLoweredOperandValueSource::Local(_)
            | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
    ));
    assert!(
        perform_lowering.operand_contract().payload_sources()[0]
            .span()
            .is_some()
    );
}

#[test]
fn effect_lowered_boundary_operand_contract_publishes_known_closure_env_sources() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_callee_suspend_matrix.scoop",
    ));
    let main = callable(&output, "main");
    let closure_boundary = main
        .boundary_map()
        .entries()
        .iter()
        .find(|boundary| {
            matches!(
                boundary.lowering(),
                Some(LateLoweredBoundaryLowering::Call(lowering))
                    if matches!(
                        lowering.facts().kind(),
                        crate::effect_facts::CallSiteKind::Closure
                            | crate::effect_facts::CallSiteKind::FunValue
                    )
                        && lowering.facts().target_mode() == CallTargetMode::KnownInstance
            )
        })
        .expect("fixture 应包含 known-instance closure/fun-value call boundary");
    let LateLoweredBoundaryLowering::Call(lowering) = closure_boundary
        .lowering()
        .expect("closure boundary 应发布 lowering contract")
    else {
        panic!("closure boundary 应物化成 Call lowering")
    };

    assert!(
        lowering.operand_contract().carrier_source().is_some(),
        "closure call 仍应发布 callable carrier source"
    );
    assert_eq!(
        lowering.operand_contract().arg_sources().len(),
        1,
        "known-instance closure direct args 应由 closure env carrier 发布为单一 source"
    );
    assert_eq!(
        lowering.operand_contract().arg_sources()[0].source_ty(),
        lowering.facts().invoke_args_tuple_ty()
    );
    assert!(matches!(
        lowering.operand_contract().arg_sources()[0].value(),
        LateLoweredOperandValueSource::Local(_)
    ));
}

#[test]
fn effect_lowered_boundary_operand_contract_publishes_resume_sources() {
    let output = load_output(&load_fixture(
        "effect_facts",
        "dispatch_and_resume_call.scoop",
    ));
    let callable = callable(&output, "fixtures.mir.resumeBoom");
    let resume_boundary = site_boundary(callable, BoundarySiteKind::Resume);
    let LateLoweredBoundaryLowering::Resume(resume_lowering) = resume_boundary
        .lowering()
        .expect("resume boundary 应发布 lowering contract")
    else {
        panic!("resume boundary 应物化成 Resume lowering")
    };
    assert!(matches!(
        resume_lowering.operand_contract().source_consumption(),
        LateLoweredBoundarySourceConsumption::Statement {
            consumes_last_statement: true,
            ..
        }
    ));
    assert!(matches!(
        resume_lowering
            .operand_contract()
            .continuation_source()
            .value(),
        LateLoweredOperandValueSource::Local(_)
    ));
    assert!(
        output
            .types()
            .display(
                resume_lowering
                    .operand_contract()
                    .continuation_source()
                    .source_ty(),
            )
            .to_string()
            .contains("Continuation")
    );
    assert_eq!(resume_lowering.operand_contract().arg_sources().len(), 1);
    assert_eq!(
        output
            .types()
            .display(resume_lowering.operand_contract().arg_sources()[0].source_ty())
            .to_string(),
        "Int"
    );
    assert!(matches!(
        resume_lowering.operand_contract().arg_sources()[0].value(),
        LateLoweredOperandValueSource::Local(_)
            | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
    ));
    assert!(
        resume_lowering.operand_contract().arg_sources()[0]
            .span()
            .is_some()
    );
}

#[test]
fn boundary_operand_contract_accepts_nominal_upcast_direct_arg_sources() {
    let source = SourceFile::new_virtual(
        "<mem>/nominal_upcast_boundary.scoop",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

open class Base() {
    open fun ping(): Int / (Ask) {
        Ask.ask(1)
    }
}

class Derived() : Base() {
    override fun ping(): Int / (Ask) {
        Ask.ask(41)
    }
}

fun helper(base: Base): Int / (Ask) {
    return base.ping()
}

fun main(): Int {
    return handle {
        helper(Derived())
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );
    let session = session();
    let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(&session, &source)
        .expect("nominal upcast sample 应可通过 effect-facts stage");
    let pass_view = effect_facts_stage_output.materialized_pass_view();
    let main_family = pass_view
        .instances()
        .find(|family| family.root_fqn() == "a.main")
        .expect("main family should exist");
    let body = main_family
        .root_body()
        .and_then(|fun| fun.body.as_ref())
        .expect("main should have a root body");
    let body_facts = effect_facts_stage_output
        .effect_facts()
        .body(main_family.key())
        .expect("main body facts should exist");
    let helper_call_site = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                value:
                    Rvalue::Call {
                        site_id,
                        kind: crate::mir::CallKind::Direct { callee_fqn },
                        ..
                    },
                ..
            } if callee_fqn == "a.helper" => Some(*site_id),
            _ => None,
        })
        .expect("helper(...) call 应被发布为 direct call site");
    let call_facts = match body_facts.site(helper_call_site) {
        Some(crate::effect_facts::SiteEffectFacts::Call(facts)) => facts,
        other => panic!(
            "helper call site {} 应为 call facts，实际为: {other:?}",
            helper_call_site.as_u32()
        ),
    };
    let arg_local = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                value: Rvalue::Call { site_id, args, .. },
                ..
            } if *site_id == helper_call_site => match args.as_slice() {
                [
                    CallArg {
                        value: Operand::Local(local),
                        ..
                    },
                ] => Some(*local),
                _ => None,
            },
            _ => None,
        })
        .expect("helper call site 应发布单一 local arg");

    let nominal_direct_supertypes =
        crate::effect_lowered::builder::collect_nominal_direct_supertypes_from_mir_file(
            effect_facts_stage_output.file(),
        );
    let expected_ty = call_facts.invoke_args_tuple_ty();
    let local_ty = body.locals[arg_local.as_u32() as usize].ty;

    assert!(
        super::nominal_source_type_compatible(
            effect_facts_stage_output.types(),
            local_ty,
            expected_ty,
            &nominal_direct_supertypes,
        ),
        "raw late-lowering 应接受 direct nominal upcast source；local=t{} ({})，expected=t{} ({})，supertypes={nominal_direct_supertypes:?}",
        local_ty.as_u32(),
        effect_facts_stage_output.types().display(local_ty),
        expected_ty.as_u32(),
        effect_facts_stage_output.types().display(expected_ty),
    );

    super::operand_source_with_expected_ty(
        "a.main",
        SiteId::from_raw(1),
        "Call",
        body,
        effect_facts_stage_output.types(),
        &nominal_direct_supertypes,
        &Operand::Local(arg_local),
        expected_ty,
        None,
    )
    .expect("late-lowering 应接受 direct nominal upcast arg source");
}

#[test]
fn boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let main = callable(&output, "main");
    let step_type = output
        .program()
        .step_type(main.step_schema())
        .expect("main 应能回查 canonical Step shell");
    let call_boundaries = main
        .boundary_map()
        .entries()
        .iter()
        .filter_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Call(lowering)) => Some((boundary, lowering)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(main.resolved_outward_cases().is_empty());
    assert!(step_type.cases().is_empty());
    assert_eq!(call_boundaries.len(), 2);
    assert!(
        call_boundaries
            .iter()
            .all(|(_, lowering)| lowering.dispatch().outward_cases().is_empty())
    );
    for (boundary, lowering) in call_boundaries {
        let runtime_error_case = lowering
            .consumed_runtime_error_case()
            .expect("pure caller 的 call boundary 应显式发布本地 runtime-error contract");
        assert_eq!(runtime_error_case.input_case_tag().as_u32(), 1);
        assert_eq!(
            runtime_error_case
                .input_concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );
        assert_eq!(
            output
                .types()
                .display(runtime_error_case.payload_tuple_ty())
                .to_string(),
            "scoop.core.RuntimeError"
        );
        assert_eq!(
            runtime_error_case.terminal_action(),
            crate::effect_lowered::ir::LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
                runtime_entry:
                    crate::effect_lowered::ir::LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
            }
        );
        let target_state = main
            .state_graph()
            .states()
            .iter()
            .find(|state| state.state_id() == runtime_error_case.target_state())
            .expect("本地 runtime-error contract 应发布 dedicated target state");
        assert!(main.state_graph().states().iter().any(|state| {
            state.state_id() == boundary.owner_state()
                && matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend {
                        local_runtime_error_states,
                        ..
                    } if local_runtime_error_states.contains(&runtime_error_case.target_state())
                )
        }));
        assert!(matches!(
            target_state.terminator(),
            crate::effect_lowered::ir::LateLoweredStateTerminator::LocalRuntimeError {
                payload_tuple_ty,
                terminal_action,
            } if *payload_tuple_ty == runtime_error_case.payload_tuple_ty()
                && *terminal_action == runtime_error_case.terminal_action()
        ));
    }
}

#[test]
fn boundary_lowering_materializes_resume_and_runtime_error_contracts() {
    let output = load_output(&load_fixture(
        "mir_lowered",
        "dispatch_and_resume_call.scoop",
    ));
    let callable = callable(&output, "fixtures.mir.resumeBoom");
    let resume_boundary = site_boundary(callable, BoundarySiteKind::Resume);
    let runtime_error_boundary = callable
        .boundary_map()
        .entries()
        .iter()
        .find(|boundary| {
            matches!(
                boundary.source(),
                crate::effect_lowered::ir::LateLoweredBoundarySource::RuntimeError { .. }
            )
        })
        .expect("resume callable 应发布 runtime-error boundary");

    let LateLoweredBoundaryLowering::Resume(resume_lowering) = resume_boundary
        .lowering()
        .expect("resume boundary 应发布 lowering contract")
    else {
        panic!("resume boundary 应物化成 Resume lowering")
    };
    let LateLoweredBoundaryLowering::RuntimeError(runtime_error_lowering) = runtime_error_boundary
        .lowering()
        .expect("runtime-error boundary 应发布 lowering contract")
    else {
        panic!("runtime-error boundary 应物化成 RuntimeError lowering")
    };

    assert_eq!(
        resume_lowering.runtime_error_boundary(),
        runtime_error_boundary.boundary_id()
    );
    assert_eq!(
        runtime_error_lowering.resume_boundary(),
        resume_boundary.boundary_id()
    );
    assert_eq!(
        resume_lowering.dispatch().input_step_schema(),
        resume_lowering.facts().out_step_schema()
    );
    assert_eq!(resume_lowering.dispatch().outward_cases().len(), 2);
    assert_eq!(
        runtime_error_lowering
            .emitted_step()
            .concrete_op_key()
            .instance_key()
            .template
            .fqn,
        "scoop.core.Raise.raise"
    );
}

#[test]
fn boundary_lowering_publishes_member_readback_resume_route() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let callable = callable(&output, "main");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("main 顶层 handle 应保持 HandleDispatch terminator");
    };
    let binder = contract.handled_arms()[0]
        .continuation_binder()
        .expect("Ask handle arm 应发布 continuation binder");

    let resume_routes = callable
        .boundary_map()
        .entries()
        .iter()
        .filter_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                let crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Resume,
                } = boundary.source()
                else {
                    return None;
                };
                Some((site_id, lowering))
            }
            _ => None,
        })
        .map(|(site_id, lowering)| {
            let route = lowering.operand_contract().underlying_continuation_route();
            (site_id, route)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        resume_routes
            .iter()
            .map(|(site_id, _)| site_id.as_u32())
            .collect::<Vec<_>>(),
        vec![26, 31, 36, 41]
    );
    for (_site_id, route) in resume_routes {
        assert_eq!(route.continuation_schema(), binder.continuation_schema());
        assert!(matches!(
            route.publication(),
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_continuation_object,
                site_id,
                arm_ordinal,
                handled_case,
                ..
            } if *owner_continuation_object == callable.continuation_object()
                && site_id.as_u32() == 1
                && *arm_ordinal == 0
                && *handled_case == contract.handled_arms()[0].handled_case()
        ));
    }
}

#[test]
fn boundary_lowering_publishes_local_option_continuation_readback_route() {
    let output = load_output(&load_fixture(
        "run-pass",
        "continuation_resume_continuation.scoop",
    ));
    let callable = callable(&output, "main");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("main site0 应保持 Outer.getK HandleDispatch terminator");
    };
    let binder = contract.handled_arms()[0]
        .continuation_binder()
        .expect("Outer.getK arm 应发布 continuation binder");
    let route = callable
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.source() {
            crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } if site_id.as_u32() == 15 => match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    Some(lowering.operand_contract().underlying_continuation_route())
                }
                _ => None,
            },
            _ => None,
        })
        .expect("ok.resume(ik) 应发布 resume boundary route");

    assert_eq!(route.continuation_schema(), binder.continuation_schema());
    assert!(matches!(
        route.publication(),
        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_continuation_object,
            site_id,
            arm_ordinal,
            handled_case,
            ..
        } if *owner_continuation_object == callable.continuation_object()
            && site_id.as_u32() == 0
            && *arm_ordinal == 0
            && *handled_case == contract.handled_arms()[0].handled_case()
    ));
}

#[test]
fn surface_resume_dispatch_inventory_publishes_shared_wrapper_projection() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let callable = callable(&output, "main");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("main 顶层 handle 应保持 HandleDispatch terminator");
    };
    let binder = contract.handled_arms()[0]
        .continuation_binder()
        .expect("Ask handle arm 应发布 continuation binder");
    let resume_lowering = callable
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
            _ => None,
        })
        .expect("fixture 应至少包含一个 resume boundary");
    let wrapper_schema = resume_lowering.facts().continuation_schema();
    let inventory_entry = output
        .program()
        .surface_resume_dispatch(wrapper_schema)
        .expect("shared wrapper schema 应发布 authoritative inventory");
    let projection = inventory_entry
        .wrapper_projection()
        .expect("shared wrapper schema 应发布 owner-step -> wrapper-step projection");
    let outward = projection
        .outward_cases()
        .first()
        .expect("wrapper projection 应至少包含一个 outward case");
    let forwarded = resume_lowering
        .dispatch()
        .outward_cases()
        .first()
        .expect("resume boundary dispatch 应至少包含一个 forwarded outward case");

    assert_eq!(
        projection.underlying_route().continuation_schema(),
        binder.continuation_schema()
    );
    assert!(matches!(
        projection.underlying_route().publication(),
        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_continuation_object,
            site_id,
            arm_ordinal,
            handled_case,
            ..
        } if *owner_continuation_object == callable.continuation_object()
            && site_id.as_u32() == 1
            && *arm_ordinal == 0
            && *handled_case == contract.handled_arms()[0].handled_case()
    ));
    assert_eq!(projection.owner_step_schema(), callable.step_schema());
    assert_eq!(
        projection.wrapper_step_schema(),
        resume_lowering.facts().out_step_schema()
    );
    assert_eq!(
        projection.complete().wrapper_answer_ty(),
        resume_lowering.dispatch().complete().answer_ty()
    );
    assert_eq!(outward.owner_case_tag(), forwarded.emission().case_tag());
    assert_eq!(
        outward.owner_concrete_op_key(),
        forwarded.emission().concrete_op_key()
    );
    assert_eq!(outward.wrapper_case_tag(), forwarded.input_case_tag());
    assert_eq!(
        outward.wrapper_concrete_op_key(),
        forwarded.input_concrete_op_key()
    );
    assert_eq!(
        outward.wrapper_continuation_contract().out_step_schema(),
        resume_lowering.facts().out_step_schema()
    );
}

#[test]
fn surface_resume_dispatch_inventory_publishes_wrapper_outward_continuation_schema() {
    let output = load_output(&load_fixture(
        "build",
        "effect_lowered_direct_handle_resume_emit_llvm.scoop",
    ));
    let callable = callable(&output, "fixtures.build.main");
    let resume_lowering = callable
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
            _ => None,
        })
        .expect("fixture 应包含 resume boundary");
    let projection = output
        .program()
        .surface_resume_dispatch(resume_lowering.facts().continuation_schema())
        .and_then(|entry| entry.wrapper_projection())
        .expect("resume wrapper schema 应发布 owner-step -> wrapper-step projection");
    let outward = projection
        .outward_cases()
        .first()
        .expect("wrapper projection 应发布 outward case continuation contract");
    let contract = outward.wrapper_continuation_contract();
    let entry = output
        .program()
        .surface_resume_dispatch(contract.continuation_schema())
        .expect("wrapper outward continuation schema 应发布 surface-resume inventory");

    assert_eq!(
        entry.contract().resume_tuple_ty(),
        contract.resume_tuple_ty()
    );
    assert_eq!(entry.contract().answer_ty(), contract.answer_ty());
    assert_eq!(
        entry.contract().out_step_schema(),
        contract.out_step_schema()
    );
    assert_eq!(entry.wrapper_projection(), Some(projection));
    assert_eq!(
        entry.source_kind(),
        LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
    );
    assert!(entry.publications().iter().any(|publication| matches!(
        publication,
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_continuation_object,
            ..
        } if *owner_continuation_object == callable.continuation_object()
    )));
}

#[test]
fn surface_resume_dispatch_dump_exposes_shared_wrapper_projection() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let dump = output.program().stable_dump();

    assert!(dump.contains("wrapper_projection:"));
    assert!(dump.contains("underlying_route: continuation_schema=cont#h"));
    assert!(dump.contains("owner_step_schema: step#h"));
    assert!(dump.contains("wrapper_step_schema: step#h"));
    assert!(
        dump.lines().any(|line| {
            line.contains("owner case#h")
                && line.contains("op=scoop.core.Raise.raise<")
                && line.contains("payload_tuple_ty=")
                && line.contains(" -> wrapper case#h")
        }),
        "shared wrapper projection 应直接暴露 owner -> wrapper 映射\n{dump}"
    );
}

#[test]
fn effect_lowered_surface_resume_wrapper_completion_publishes_handle_arm_payload_source() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let callable = callable(&output, "main");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("main 顶层 handle 应保持 HandleDispatch terminator");
    };
    let arm_source = contract.handled_arms()[0].completion_payload_source();
    let resume_schema = callable
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                Some(lowering.facts().continuation_schema())
            }
            _ => None,
        })
        .expect("fixture 应包含 shared wrapper resume schema");
    let projection = output
        .program()
        .surface_resume_dispatch(resume_schema)
        .and_then(|entry| entry.wrapper_projection())
        .expect("shared wrapper schema 应发布 complete projection");

    assert_eq!(
        projection.complete().wrapper_answer_ty(),
        arm_source.source_ty()
    );
    assert_eq!(
        projection
            .complete()
            .payload_source()
            .wrapper_payload_source(),
        Some(arm_source),
        "wrapper Complete(Int) 应直接引用 top-level handle arm 的 completion payload source"
    );
    assert!(matches!(
        arm_source,
        LateLoweredCompletionPayloadSource::Operand(source)
            if matches!(source.value(), LateLoweredOperandValueSource::Local(_))
    ));
    let dump = output.program().stable_dump();
    assert!(dump.contains("complete: owner_answer_ty="));
    assert!(dump.contains("payload=local#h"));
    assert!(dump.contains("completion_payload: local#h"));
}

#[test]
fn effect_lowered_surface_resume_wrapper_completion_uses_owner_complete_for_matching_answer_type() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let projection = output
        .program()
        .surface_resume_dispatch_inventory()
        .iter()
        .find_map(|entry| entry.wrapper_projection())
        .expect("matching answer type fixture 应发布 wrapper projection");

    assert_eq!(
        projection.complete().owner_answer_ty(),
        projection.complete().wrapper_answer_ty(),
        "fixture 应覆盖 owner/wrapper answer type 相同的投影路径"
    );
    assert!(matches!(
        projection.complete().payload_source(),
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty }
            if *answer_ty == projection.complete().wrapper_answer_ty()
    ));
    assert!(
        projection
            .complete()
            .payload_source()
            .wrapper_payload_source()
            .is_none(),
        "同型 Complete 投影应直接复用 owner Complete payload，而不是发布 wrapper payload source"
    );
    let dump = output.program().stable_dump();
    assert!(dump.contains("payload=owner_complete:"));
}

#[test]
fn effect_lowered_resume_boundary_self_route_publishes_step_projection() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_escape_continuation_resume_later_exit.scoop",
    ));
    let callable = callable(&output, "main");
    let resume_lowering = callable
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
            _ => None,
        })
        .expect("fixture 应包含 resume boundary");
    let projection = output
        .program()
        .surface_resume_dispatch(resume_lowering.facts().continuation_schema())
        .and_then(|entry| entry.wrapper_projection())
        .expect("same-schema 但不同 StepSchema 的 resume boundary 应发布 wrapper projection");

    assert_eq!(projection.owner_step_schema(), callable.step_schema());
    assert_eq!(
        projection.wrapper_step_schema(),
        resume_lowering.facts().out_step_schema()
    );
    assert_ne!(
        projection.owner_step_schema(),
        projection.wrapper_step_schema()
    );
    assert!(matches!(
        projection.complete().payload_source(),
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. }
    ));
}

#[test]
fn boundary_lowering_publishes_direct_resume_self_route() {
    let output = load_output(&load_fixture(
        "effect_facts",
        "dispatch_and_resume_call.scoop",
    ));

    for callable_fqn in ["fixtures.mir.resumeBoom", "fixtures.mir.resumeOnce"] {
        let callable = callable(&output, callable_fqn);
        let boundary = site_boundary(callable, BoundarySiteKind::Resume);
        let site_id = match boundary.source() {
            crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } => site_id,
            other => panic!("{callable_fqn} 应发布 resume boundary，而不是 {other:?}"),
        };
        let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
            panic!("{callable_fqn} resume boundary 应带 lowering");
        };

        let route = lowering.operand_contract().underlying_continuation_route();
        assert_eq!(
            route.continuation_schema(),
            lowering.facts().continuation_schema()
        );
        assert!(matches!(
            route.publication(),
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_version_key,
                owner_continuation_object,
                site_id: route_site_id,
            } if owner_version_key == callable.body_version_key()
                && *owner_continuation_object == callable.continuation_object()
                && *route_site_id == site_id
        ));
    }
}

#[test]
fn effect_lowered_resume_payload_binding_covers_call_and_resume_boundaries() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));

    let main = callable(&output, "main");
    let call_boundary = site_boundary(main, BoundarySiteKind::Call);
    let call_binding = main
        .frame_schema()
        .resume_payload_binding(call_boundary.boundary_id())
        .expect("call boundary 应发布 resumed local/home contract");
    let call_slot = main
        .frame_schema()
        .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
            boundary: call_boundary.boundary_id(),
            local: call_binding.consumer_local(),
        })
        .expect("call boundary 应保留 BoundaryResult home slot");

    assert_eq!(call_binding.resume_state(), call_boundary.resume_state());
    assert_eq!(
        call_binding.consumer_frame_slot(),
        Some(call_slot.slot_id())
    );

    let run = callable(&output, "run");
    let resume_boundary = site_boundary(run, BoundarySiteKind::Resume);
    let resume_binding = run
        .frame_schema()
        .resume_payload_binding(resume_boundary.boundary_id())
        .expect("resume boundary 应发布 resumed local/home contract");
    let resume_slot = run
        .frame_schema()
        .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
            boundary: resume_boundary.boundary_id(),
            local: resume_binding.consumer_local(),
        })
        .expect("resume boundary 应保留 BoundaryResult home slot");

    assert_eq!(
        resume_binding.resume_state(),
        resume_boundary.resume_state()
    );
    assert_eq!(
        resume_binding.consumer_frame_slot(),
        Some(resume_slot.slot_id())
    );
}

#[test]
fn effect_lowered_resume_payload_binding_covers_perform_and_runtime_error_paths() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));

    let fetch = callable(&output, "fetch");
    let perform_boundary = site_boundary(fetch, BoundarySiteKind::Perform);
    let perform_binding = fetch
        .frame_schema()
        .resume_payload_binding(perform_boundary.boundary_id())
        .expect("perform boundary 应发布 resumed local/home contract");
    let perform_slot = fetch
        .frame_schema()
        .slot_for_kind(LateLoweredFrameSlotKind::BoundaryResult {
            boundary: perform_boundary.boundary_id(),
            local: perform_binding.consumer_local(),
        })
        .expect("perform boundary 应保留 PerformResult 对应的 BoundaryResult slot");

    assert_eq!(
        perform_binding.resume_state(),
        perform_boundary.resume_state()
    );
    assert_eq!(
        perform_binding.consumer_frame_slot(),
        Some(perform_slot.slot_id())
    );

    let main = callable(&output, "main");
    let resume_boundary = site_boundary(main, BoundarySiteKind::Resume);
    let runtime_error_boundary = main
        .boundary_map()
        .entries()
        .iter()
        .find(|boundary| {
            matches!(
                boundary.source(),
                crate::effect_lowered::ir::LateLoweredBoundarySource::RuntimeError {
                    origin_site
                } if origin_site == SiteId::from_raw(26)
            )
        })
        .expect("首个 resume site 的 paired runtime-error boundary 应存在");
    let resume_binding = main
        .frame_schema()
        .resume_payload_binding(resume_boundary.boundary_id())
        .expect("resume boundary 应发布 resumed local/home contract");
    let runtime_error_binding = main
        .frame_schema()
        .resume_payload_binding(runtime_error_boundary.boundary_id())
        .expect("runtime-error boundary 应显式继承 resumed local/home contract");

    assert_eq!(
        runtime_error_binding.resume_state(),
        runtime_error_boundary.resume_state()
    );
    assert_eq!(
        runtime_error_binding.consumer_local(),
        resume_binding.consumer_local()
    );
    assert_eq!(
        runtime_error_binding.consumer_frame_slot(),
        resume_binding.consumer_frame_slot(),
    );
}

#[test]
fn effect_lowered_call_boundary_continuation_composition() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));

    let main = callable(&output, "main");
    let (boundary, lowering) = main
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| {
            let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                return None;
            };
            (!lowering.continuation_compositions().is_empty()).then_some((boundary, lowering))
        })
        .expect("main 的 fetch call boundary 应发布 continuation composition");
    let input_step = output
        .program()
        .step_type(lowering.dispatch().input_step_schema())
        .expect("call boundary input step schema 应可回查");
    let result_binding = main
        .frame_schema()
        .resume_payload_binding(boundary.boundary_id())
        .expect("call boundary 应发布 caller result home binding");

    assert_eq!(result_binding.resume_state(), boundary.resume_state());
    assert_eq!(result_binding.consumer_local(), lowering.result_local());
    assert_eq!(
        lowering.continuation_compositions().len(),
        lowering.dispatch().outward_cases().len(),
        "每个 call-boundary outward forwarding 都必须有 composition contract"
    );

    for composition in lowering.continuation_compositions() {
        assert_eq!(composition.boundary_id(), boundary.boundary_id());
        assert_eq!(composition.input_step_schema(), input_step.step_schema());
        assert_eq!(composition.caller_resume_state(), boundary.resume_state());
        assert_eq!(
            composition.caller_result_local(),
            result_binding.consumer_local()
        );
        assert_eq!(
            composition.caller_result_frame_slot(),
            result_binding.consumer_frame_slot()
        );
        assert_eq!(composition.caller_result_ty(), input_step.complete_ty());

        let input_case = input_step
            .case(composition.input_case_tag())
            .expect("composition input case 应存在于 callee Step_F");
        let forwarding = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
            .expect("composition 应对应一个 dispatch forwarding");

        assert_eq!(
            composition.callee_continuation_contract(),
            input_case.continuation_contract()
        );
        assert_eq!(
            composition.output_case_tag(),
            forwarding.emission().case_tag()
        );
        assert_eq!(
            composition.caller_continuation_contract(),
            forwarding.emission().continuation_contract()
        );
    }

    let rendered = crate::effect_lowered::render_late_lowered_program(output.program());
    assert!(
        rendered.contains("continuation_compositions:"),
        "dump-effect-lowered 应渲染 call-boundary continuation composition handoff"
    );
}

#[test]
fn effect_lowered_resume_boundary_continuation_composition_for_cross_call_escape() {
    let output = load_output(&load_fixture(
        "run-pass",
        "continuation_escape_binder_resume_effect_row_runtime_basic.scoop",
    ));

    let main = callable(&output, "main");
    let (boundary, lowering) = main
        .boundary_map()
        .entries()
        .iter()
        .find_map(|boundary| {
            let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
                return None;
            };
            let route = lowering.operand_contract().underlying_continuation_route();
            matches!(
                route.publication(),
                LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                    owner_version_key,
                    ..
                } if owner_version_key.surface_instance().template.fqn == "start"
            )
            .then_some((boundary, lowering))
        })
        .expect("main 的 saved continuation resume boundary 应接回 start 的 binder route");
    let input_step = output
        .program()
        .step_type(lowering.dispatch().input_step_schema())
        .expect("resume boundary input step schema 应可回查");
    let result_binding = main
        .frame_schema()
        .resume_payload_binding(boundary.boundary_id())
        .expect("resume boundary 应发布 caller result home binding");

    assert_eq!(result_binding.resume_state(), boundary.resume_state());
    assert_eq!(result_binding.consumer_local(), lowering.result_local());
    assert_eq!(
        lowering.continuation_compositions().len(),
        lowering.dispatch().outward_cases().len(),
        "每个 resume-boundary outward forwarding 都必须有 composition contract"
    );

    for composition in lowering.continuation_compositions() {
        assert_eq!(composition.boundary_id(), boundary.boundary_id());
        assert_eq!(composition.input_step_schema(), input_step.step_schema());
        assert_eq!(composition.caller_resume_state(), boundary.resume_state());
        assert_eq!(
            composition.caller_result_local(),
            result_binding.consumer_local()
        );
        assert_eq!(
            composition.caller_result_frame_slot(),
            result_binding.consumer_frame_slot()
        );
        assert_eq!(composition.caller_result_ty(), input_step.complete_ty());

        let input_case = input_step
            .case(composition.input_case_tag())
            .expect("composition input case 应存在于 resume wrapper Step_F");
        let forwarding = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
            .expect("composition 应对应一个 resume dispatch forwarding");

        assert_eq!(
            composition.callee_continuation_contract(),
            input_case.continuation_contract()
        );
        assert_eq!(
            composition.output_case_tag(),
            forwarding.emission().case_tag()
        );
        assert_eq!(
            composition.caller_continuation_contract(),
            forwarding.emission().continuation_contract()
        );
    }

    let rendered = crate::effect_lowered::render_late_lowered_program(output.program());
    assert!(
        rendered.contains("lowering: Resume") && rendered.contains("continuation_compositions:"),
        "dump-effect-lowered 应渲染 resume-boundary continuation composition handoff"
    );
}

#[test]
fn effect_lowered_resume_payload_binding_dump_exposes_consumers() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let dump = output.program().stable_dump();

    assert!(dump.contains("resume_payload_bindings:"));
    assert!(dump.contains("boundary#h"));
    assert!(dump.contains("resume=state#h"));
    assert!(dump.contains("home=slot#h"));
}

#[test]
fn effect_lowered_completion_payload_contract_publishes_non_unit_return_source() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let run = callable(&output, "run");
    let step_type = output
        .program()
        .step_type(run.step_schema())
        .expect("run 应能回查 Step shell");
    let (return_state, payload_source, complete_state) = run
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::Return {
                payload_source,
                complete_state,
            } => Some((state.state_id(), payload_source, *complete_state)),
            _ => None,
        })
        .expect("run(): Int 应发布 Return terminator");
    let binding = run
        .frame_schema()
        .completion_payload_binding_for_state(return_state)
        .expect("return state 应发布 completion payload binding");

    assert_eq!(complete_state, run.state_graph().complete_state());
    assert_eq!(binding.complete_state(), complete_state);
    assert_eq!(binding.payload_source(), payload_source);
    assert_eq!(payload_source.source_ty(), step_type.complete_ty());
    assert_eq!(
        output
            .types()
            .display(payload_source.source_ty())
            .to_string(),
        "Int"
    );
    assert!(
        !payload_source.is_unit(),
        "non-Unit completion 不应退化成 Unit payload source"
    );
    assert!(matches!(
        payload_source,
        LateLoweredCompletionPayloadSource::Operand(source)
            if matches!(source.value(), LateLoweredOperandValueSource::Local(_))
    ));
}

#[test]
fn effect_lowered_completion_payload_contract_dump_exposes_sources() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let dump = output.program().stable_dump();

    assert!(dump.contains("completion_payload_bindings:"));
    assert!(dump.contains("root: run"));
    assert!(dump.contains("payload=local#h"));
}

#[test]
fn effect_lowered_source_slice_classification_publishes_statement_purposes() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let classes = output
        .program()
        .callables()
        .iter()
        .flat_map(|callable| callable.source_statement_classifications())
        .map(|classification| classification.kind())
        .collect::<Vec<_>>();

    assert!(classes.iter().any(|kind| matches!(
        kind,
        LateLoweredSourceStatementClassificationKind::EffectNeutralValue
    )));
    assert!(classes.iter().any(|kind| matches!(
        kind,
        LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
    )));
    assert!(classes.iter().any(|kind| matches!(
        kind,
        LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
    )));

    let dump = output.program().stable_dump();
    assert!(dump.contains("statement_classification:"));
    assert!(dump.contains("effect-neutral-value"));
    assert!(dump.contains("boundary-consumed-anchor"));
}

#[test]
fn effect_lowered_completion_payload_contract_rejects_type_drift() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let run = callable(&output, "run");
    let step_type = output
        .program()
        .step_type(run.step_schema())
        .expect("run 应能回查 Step shell");
    let builtins = output.types().builtins().expect("builtins 应已 intern");
    let wrong_step_type = LateLoweredStepType::new(
        step_type.step_schema(),
        step_type.invoke_args_tuple_ty(),
        builtins.unit,
        step_type.continuation_obj_ty(),
        step_type.cases().to_vec(),
    );

    let err = super::materialize_completion_payload_bindings(
        run.root_fqn(),
        &wrong_step_type,
        run.state_graph(),
        run.frame_schema(),
        output.types(),
    )
    .expect_err("completion payload type drift 必须 fail fast");
    assert!(
        err.to_string().contains("completion payload contract")
            && err.to_string().contains("complete_ty"),
        "错误消息应指出 completion payload complete_ty 漂移: {err}"
    );
}

#[test]
fn published_continuation_provenance_rejects_ambiguous_member_routes() {
    let mut types = crate::ty::TypeStore::default();
    let builtins = types.intern_builtins();
    let span = crate::span::Span::new(0, 0);
    let step_schema = crate::effect_facts::StepSchemaId::new(0);
    let empty_cases = crate::effect_facts::CaseSet::new(step_schema, Vec::new());

    let mut body = crate::mir::Body::new_empty();
    let cell = body.push_local(crate::mir::LocalDecl {
        span,
        name: Some("cell".to_string()),
        ty: builtins.any,
        source: crate::mir::LocalSourceKind::SourceLocal,
    });
    let k0 = body.push_local(crate::mir::LocalDecl {
        span,
        name: Some("k0".to_string()),
        ty: builtins.any,
        source: crate::mir::LocalSourceKind::SourceLocal,
    });
    let k1 = body.push_local(crate::mir::LocalDecl {
        span,
        name: Some("k1".to_string()),
        ty: builtins.any,
        source: crate::mir::LocalSourceKind::SourceLocal,
    });
    let read_local = body.push_local(crate::mir::LocalDecl {
        span,
        name: Some("read".to_string()),
        ty: builtins.any,
        source: crate::mir::LocalSourceKind::CompilerTemporary,
    });
    let resume_local = body.push_local(crate::mir::LocalDecl {
        span,
        name: Some("resume".to_string()),
        ty: builtins.any,
        source: crate::mir::LocalSourceKind::CompilerTemporary,
    });

    let bb0 = body.push_block(crate::mir::BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Unreachable,
            unwind: crate::mir::UnwindAction::NoUnwind,
        },
    });
    let bb1 = body.push_block(crate::mir::BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Unreachable,
            unwind: crate::mir::UnwindAction::NoUnwind,
        },
    });
    let bb2 = body.push_block(crate::mir::BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Unreachable,
            unwind: crate::mir::UnwindAction::NoUnwind,
        },
    });
    let bb3 = body.push_block(crate::mir::BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Unreachable,
            unwind: crate::mir::UnwindAction::NoUnwind,
        },
    });
    body.start = bb0;

    let member = crate::mir::MemberAccessMetadata {
        name: "k".to_string(),
        receiver_ty: builtins.any,
        resolved: Some(crate::mir::MemberTarget::Value {
            fqn: "Cell.k".to_string(),
        }),
        hidden_effects: crate::ty::EffectRow::pure(),
    };
    body.blocks[bb0.as_u32() as usize].stmts = vec![
        crate::mir::Statement {
            span,
            kind: crate::mir::StatementKind::StoreMember {
                receiver: crate::mir::Operand::Local(cell),
                member: member.clone(),
                value: crate::mir::Operand::Local(k0),
                value_ty: builtins.any,
                continuation_route: crate::mir::StoredContinuationRoutePublication::Unique(
                    crate::mir::StoredContinuationValueRoute {
                        source_local: k0,
                        source_ty: builtins.any,
                        path: vec![crate::mir::PatternBindingStep::VariantField {
                            variant: "Some".to_string(),
                            field_index: 0,
                        }],
                    },
                ),
            },
        },
        crate::mir::Statement {
            span,
            kind: crate::mir::StatementKind::StoreMember {
                receiver: crate::mir::Operand::Local(cell),
                member: member.clone(),
                value: crate::mir::Operand::Local(k1),
                value_ty: builtins.any,
                continuation_route: crate::mir::StoredContinuationRoutePublication::Unique(
                    crate::mir::StoredContinuationValueRoute {
                        source_local: k1,
                        source_ty: builtins.any,
                        path: vec![crate::mir::PatternBindingStep::VariantField {
                            variant: "Some".to_string(),
                            field_index: 0,
                        }],
                    },
                ),
            },
        },
        crate::mir::Statement {
            span,
            kind: crate::mir::StatementKind::Assign {
                target: read_local,
                value: crate::mir::Rvalue::MemberAccess {
                    site_id: None,
                    receiver: crate::mir::Operand::Local(cell),
                    member: member.clone(),
                },
            },
        },
        crate::mir::Statement {
            span,
            kind: crate::mir::StatementKind::Assign {
                target: resume_local,
                value: crate::mir::Rvalue::PatternExtract {
                    subject: crate::mir::Operand::Local(read_local),
                    path: vec![crate::mir::PatternBindingStep::VariantField {
                        variant: "Some".to_string(),
                        field_index: 0,
                    }],
                },
            },
        },
    ];
    body.blocks[bb0.as_u32() as usize].terminator = crate::mir::Terminator {
        span,
        kind: crate::mir::TerminatorKind::Handle {
            site_id: SiteId::from_raw(0),
            metadata: crate::mir::HandleMetadata {
                result_ty: builtins.any,
                body_result_ty: builtins.any,
                finally_result_ty: None,
            },
            arms: vec![
                crate::mir::HandlerArm {
                    op_fqn: "sample.Ask.ask".to_string(),
                    op_type_args: Vec::new(),
                    binder_count: 0,
                    binder_locals: Vec::new(),
                    continuation_local: Some(k0),
                    handled_effect_ty: builtins.any,
                    payload_tuple_ty: Some(builtins.unit),
                    payload_component_tys: Vec::new(),
                    body_ty: builtins.any,
                    kind: crate::mir::HandlerArmKind::EscapeContinuation,
                },
                crate::mir::HandlerArm {
                    op_fqn: "sample.Ask.ask".to_string(),
                    op_type_args: Vec::new(),
                    binder_count: 0,
                    binder_locals: Vec::new(),
                    continuation_local: Some(k1),
                    handled_effect_ty: builtins.any,
                    payload_tuple_ty: Some(builtins.unit),
                    payload_component_tys: Vec::new(),
                    body_ty: builtins.any,
                    kind: crate::mir::HandlerArmKind::EscapeContinuation,
                },
            ],
            has_finally: false,
            body_target: bb1,
            arm_targets: vec![bb2, bb3],
            finally_target: None,
            exit_target: bb1,
        },
        unwind: crate::mir::UnwindAction::NoUnwind,
    };

    let body_facts = crate::effect_facts::BodyEffectFacts::new(
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::from([(
            SiteId::from_raw(0),
            crate::effect_facts::SiteEffectFacts::Handle(
                crate::effect_facts::HandleSiteEffectFacts::new(
                    builtins.any,
                    crate::effect_facts::CaseSet::new(
                        step_schema,
                        vec![
                            crate::effect_facts::CaseTag::new(0),
                            crate::effect_facts::CaseTag::new(1),
                        ],
                    ),
                    empty_cases.clone(),
                    vec![
                        crate::effect_facts::HandleArmEffectFacts::new(
                            crate::effect_facts::CaseTag::new(0),
                            builtins.unit,
                            crate::effect_facts::ContinuationSchemaId::new(0),
                            empty_cases.clone(),
                        ),
                        crate::effect_facts::HandleArmEffectFacts::new(
                            crate::effect_facts::CaseTag::new(1),
                            builtins.unit,
                            crate::effect_facts::ContinuationSchemaId::new(1),
                            empty_cases.clone(),
                        ),
                    ],
                    empty_cases,
                    crate::effect_facts::NestedHandleClassification::SelfContained,
                ),
            ),
        )]),
    );
    let owner_version_key = crate::effect_lowered::ir::LateLoweredBodyVersionKey::new(
        crate::mir::InstanceKey {
            template: crate::mir::TemplateKey {
                fqn: "synthetic.main".to_string(),
                source_path: PathBuf::from("<synthetic>"),
                decl_span: span,
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        },
        crate::ty::EffectRow::pure(),
        crate::effect_facts::ImplPlan::NoOutward,
        false,
    );
    let provenance = super::PublishedContinuationProvenance::build(
        "synthetic.main",
        &body,
        &body_facts,
        &owner_version_key,
        crate::effect_lowered::ir::ContinuationObjectId::new(0),
        None,
    )
    .expect("synthetic provenance builder 应成功");

    let err = provenance
        .resolve_resume_local_route("synthetic.main", SiteId::from_raw(9), resume_local)
        .expect_err("多个不兼容 source route 必须显式拒绝");
    let message = err.to_string();
    assert!(
        message.contains("多条互不兼容") || message.contains("无法唯一确定"),
        "错误消息应指出 member readback provenance 歧义: {message}"
    );
}

#[test]
fn boundary_lowering_materializes_perform_and_handle_contracts() {
    let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
    let handled_main = callable(&perform_output, "a.main");
    let perform_boundary = site_boundary(handled_main, BoundarySiteKind::Perform);
    let LateLoweredBoundaryLowering::Perform(perform_lowering) = perform_boundary
        .lowering()
        .expect("perform boundary 应发布 lowering contract")
    else {
        panic!("perform boundary 应物化成 Perform lowering")
    };
    assert_eq!(
        perform_lowering.facts().emitted_case(),
        perform_lowering.emitted_step().case_tag()
    );
    assert_eq!(
        perform_lowering
            .emitted_step()
            .concrete_op_key()
            .instance_key()
            .template
            .fqn,
        "scoop.core.Raise.raise"
    );

    let handle_output = load_output(&load_fixture(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
    ));
    let outward = callable(&handle_output, "sample.nested_may_suspend_outward");
    let handle_boundary = site_boundary(outward, BoundarySiteKind::Handle);
    let LateLoweredBoundaryLowering::Handle(handle_lowering) = handle_boundary
        .lowering()
        .expect("handle boundary 应发布 lowering contract")
    else {
        panic!("handle boundary 应物化成 Handle lowering")
    };
    assert_eq!(
        handle_lowering.facts().nested_handle_classification(),
        NestedHandleClassification::MaySuspendOutward
    );
    assert_eq!(handle_lowering.outward_emissions().len(), 1);
    assert_eq!(
        handle_lowering.outward_emissions()[0]
            .concrete_op_key()
            .instance_key()
            .template
            .fqn,
        "sample.Outer.again"
    );
}

#[test]
fn handle_dispatch_contract_publishes_body_arm_finally_and_outward_routes() {
    let output = load_output(&load_fixture(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
    ));
    let callable = callable(&output, "sample.nested_may_suspend_outward");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
    let LateLoweredStateTerminator::HandleDispatch {
        arm_states,
        finally_state,
        exit_state,
        contract,
        ..
    } = handle_state.terminator()
    else {
        panic!("指定 state 应保持 HandleDispatch terminator");
    };

    assert_eq!(
        contract.carrier().state_tag_slot(),
        SystemSlotKind::StateTag
    );
    assert_eq!(
        contract.carrier().completion_tag_slot(),
        SystemSlotKind::CompletionTag
    );
    assert_eq!(
        contract.carrier().payload_carrier_slot(),
        SystemSlotKind::ResumePayloadCarrier
    );
    assert_eq!(
        contract.body_complete_target(),
        finally_state.expect("fixture 应保留 finally state")
    );
    assert_eq!(
        contract.arm_complete_target(),
        finally_state.expect("fixture 应保留 finally state")
    );
    assert_eq!(contract.finally_complete_target(), Some(*exit_state));
    assert_eq!(
        contract.abandon_target(),
        callable.state_graph().drop_state()
    );
    assert_eq!(contract.handled_arms().len(), 1);
    assert_eq!(contract.handled_arms()[0].handled_case().as_u32(), 0);
    assert_eq!(contract.handled_arms()[0].arm_state(), arm_states[0]);
    assert!(contract.handled_arms()[0].arm_outward_cases().is_empty());
    assert!(contract.body_outward_cases().is_empty());
    assert_eq!(
        contract.finally_outward_cases(),
        &[crate::effect_facts::CaseTag::new(1)]
    );
    assert!(
        contract
            .outward_emission(crate::effect_facts::CaseTag::new(1))
            .is_some(),
        "finally outward case 应能回查 published outward emission"
    );
    assert!(
        contract
            .pending_completions()
            .contains(&LateLoweredHandlePendingCompletion::ContinueToExit)
    );
    assert!(
        contract
            .pending_completions()
            .contains(&LateLoweredHandlePendingCompletion::ReturnFromFunction)
    );
    assert!(
        !contract.pending_completions().contains(
            &LateLoweredHandlePendingCompletion::PropagateOutward(
                crate::effect_facts::CaseTag::new(1)
            )
        ),
        "仅 finally outward 的 case 不应被误发布成 pending completion tag"
    );
}

#[test]
fn handle_dispatch_region_contract_publishes_body_routing_for_handled_perform() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
    ));
    let callable = callable(&output, "run");
    let (_site_id, contract) = callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("run 应发布 HandleDispatch contract");
    let handled_arm = contract
        .handled_arms()
        .first()
        .expect("single-perform fixture 应发布唯一 handled arm");
    let body_route = contract
        .boundary_routings()
        .iter()
        .find(|routing| {
            matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                && callable
                    .boundary_map()
                    .boundary(routing.boundary_id())
                    .is_some_and(|boundary| {
                        matches!(
                            boundary.source(),
                            crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                                kind: BoundarySiteKind::Perform,
                                ..
                            }
                        )
                    })
        })
        .expect("handle body 内的 perform boundary 应发布 body-region routing");
    let route = body_route
        .case_routing(handled_arm.handled_case())
        .expect("handled perform case 应发布 consume-to-arm routing");

    assert_eq!(
        contract.state_region(body_route.owner_state()),
        LateLoweredHandleStateRegion::Body
    );
    assert_eq!(
        contract.state_region(body_route.resume_state()),
        LateLoweredHandleStateRegion::Body
    );
    assert!(matches!(
        route.action(),
        LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
            arm_state,
            arm_ordinal,
            continuation_resume_state,
        } if arm_state == handled_arm.arm_state()
            && arm_ordinal == handled_arm.arm_ordinal()
            && continuation_resume_state == body_route.resume_state()
    ));
}

#[test]
fn handle_dispatch_region_contract_tracks_multi_resume_routes_and_arm_regions() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let callable = callable(&output, "main");
    let (_site_id, contract) = callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("main 应发布 HandleDispatch contract");
    let ask_arm = contract
        .handled_arms()
        .iter()
        .find(|arm| arm.continuation_binder().is_some())
        .expect("Ask arm 应发布 escape continuation binder");
    let consume_routes = contract
        .boundary_routings()
        .iter()
        .filter_map(|routing| {
            routing
                .case_routing(ask_arm.handled_case())
                .map(|route| (routing, route))
        })
        .collect::<Vec<_>>();
    let resume_states = consume_routes
        .iter()
        .map(|(routing, route)| match route.action() {
            LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                arm_state,
                arm_ordinal,
                continuation_resume_state,
            } => {
                assert_eq!(arm_state, ask_arm.arm_state());
                assert_eq!(arm_ordinal, ask_arm.arm_ordinal());
                assert_eq!(continuation_resume_state, routing.resume_state());
                continuation_resume_state
            }
            other => panic!("Ask handled case 应走 consume-to-arm，而不是 {other:?}"),
        })
        .collect::<BTreeSet<_>>();

    assert!(
        consume_routes.len() >= 2,
        "indirect/direct mixed fixture 应至少发布两个 Ask consume route"
    );
    assert!(
        resume_states.len() >= 2,
        "不同 body boundary 的 continuation resume_state 应被稳定区分"
    );
    assert!(
        resume_states
            .iter()
            .all(|state_id| contract.state_region(*state_id) == LateLoweredHandleStateRegion::Body)
    );
    assert!(contract.state_regions().iter().any(|entry| matches!(
        entry.region(),
        LateLoweredHandleStateRegion::Arm { arm_ordinal: 0, .. }
    )));
    assert!(contract.state_regions().iter().any(|entry| matches!(
        entry.region(),
        LateLoweredHandleStateRegion::Arm { arm_ordinal: 1, .. }
    )));
}

#[test]
fn handle_dispatch_region_contract_tracks_pending_and_finally_routing() {
    let pending_output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_region_pending.scoop",
        r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
    ));
    let pending_callable = callable(&pending_output, "sample.propagate_before_finally");
    let pending_contract =
        match handle_dispatch_state(pending_callable, SiteId::from_raw(1)).terminator() {
            LateLoweredStateTerminator::HandleDispatch { contract, .. } => contract,
            other => panic!("期望 HandleDispatch terminator，而不是 {other:?}"),
        };
    let pending_case = pending_contract.body_outward_cases()[0];
    let pending_route = pending_contract
        .boundary_routings()
        .iter()
        .find(|routing| {
            matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                && routing.case_routing(pending_case).is_some()
        })
        .expect("body outward case 应发布 pending routing");
    assert!(matches!(
        pending_route
            .case_routing(pending_case)
            .expect("pending case 应可回查")
            .action(),
        LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
            completion: LateLoweredHandlePendingCompletion::PropagateOutward(case_tag),
        } if case_tag == pending_case
    ));

    let finally_output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_region_finally_outward.scoop",
        r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun finally_outward(): Int / (Outer) {
    return handle {
        Inner.go()
        0
    } with {
        Inner.go() -> 1
    } finally {
        Outer.again()
    }
}
"#,
    ));
    let finally_callable = callable(&finally_output, "sample.finally_outward");
    let (_site_id, finally_contract) = finally_callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("finally_outward 应发布 HandleDispatch contract");
    let finally_case = finally_contract.finally_outward_cases()[0];
    let finally_route = finally_contract
        .boundary_routings()
        .iter()
        .find(|routing| {
            matches!(
                routing.owner_region(),
                LateLoweredHandleStateRegion::Finally
            ) && routing.case_routing(finally_case).is_some()
        })
        .expect("finally outward case 应发布 finally-region routing");
    assert!(matches!(
        finally_route
            .case_routing(finally_case)
            .expect("finally case 应可回查")
            .action(),
        LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
    ));
}

#[test]
fn handle_arm_binding_contract_publishes_payload_and_escape_continuation_binding() {
    let output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_arm_binding_single.scoop",
        r#"
package sample

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun run(): Int {
    return handle {
        Edge.visit("alpha", 1)
    } with {
        Edge.visit(from, to), k -> {
            k.resume(to + 1)
        }
    }
}

fun main(): Int {
    return 0
}
"#,
    ));
    let callable = callable(&output, "sample.run");
    let (site_id, contract) = callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("run 应发布 HandleDispatch contract");
    let arm = contract
        .handled_arms()
        .first()
        .expect("单 arm fixture 应发布唯一 handled arm");
    let facts = handle_site_facts(&output, callable, site_id);
    let expected = &facts.arm_facts()[0];

    assert_eq!(arm.arm_ordinal(), 0);
    assert_eq!(arm.payload_tuple_ty(), expected.payload_tuple_ty());
    assert_eq!(arm.payload_binders().len(), 2);
    assert_eq!(arm.payload_binders()[0].ordinal(), 0);
    assert_eq!(arm.payload_binders()[1].ordinal(), 1);
    assert_ne!(
        arm.payload_binders()[0].local(),
        arm.payload_binders()[1].local(),
        "不同 payload binder 必须稳定绑定到不同 local"
    );
    let continuation_binder = arm
        .continuation_binder()
        .expect("escape continuation arm 必须发布 continuation binder contract");
    assert_eq!(
        continuation_binder.continuation_schema(),
        expected.continuation_schema()
    );
    assert_eq!(
        continuation_binder.continuation_object(),
        callable.continuation_object()
    );
}

#[test]
fn handle_arm_binding_contract_publishes_mixed_multi_arm_bindings_without_ambiguity() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let callable = callable(&output, "main");
    let (site_id, contract) = callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("main 应发布 HandleDispatch contract");
    let facts = handle_site_facts(&output, callable, site_id);

    assert_eq!(contract.handled_arms().len(), 2);
    let mut arm_ordinals = contract
        .handled_arms()
        .iter()
        .map(|arm| arm.arm_ordinal())
        .collect::<Vec<_>>();
    arm_ordinals.sort();
    assert_eq!(arm_ordinals, vec![0, 1]);

    let escape_arm = contract
        .handled_arms()
        .iter()
        .find(|arm| arm.continuation_binder().is_some())
        .expect("mixed fixture 应发布带 continuation binder 的 arm");
    let payload_only_arm = contract
        .handled_arms()
        .iter()
        .find(|arm| arm.continuation_binder().is_none())
        .expect("mixed fixture 应发布纯 payload arm");
    assert_eq!(escape_arm.payload_binders().len(), 1);
    assert_eq!(payload_only_arm.payload_binders().len(), 1);

    let expected_by_case = facts
        .arm_facts()
        .iter()
        .map(|arm| (arm.handled_case(), arm.continuation_schema()))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        escape_arm
            .continuation_binder()
            .expect("escape arm 应带 continuation binder")
            .continuation_schema(),
        *expected_by_case
            .get(&escape_arm.handled_case())
            .expect("handled case 应能回查 arm facts continuation schema")
    );
    assert_eq!(
        payload_only_arm.payload_tuple_ty(),
        facts
            .arm_facts()
            .iter()
            .find(|arm| arm.handled_case() == payload_only_arm.handled_case())
            .expect("payload-only arm handled case 应能回查 facts")
            .payload_tuple_ty()
    );
}

#[test]
fn completion_state_contract_tracks_body_outward_cases_across_finally() {
    let output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_body_outward_finally.scoop",
        r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
    ));
    let callable = callable(&output, "sample.propagate_before_finally");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("指定 state 应保持 HandleDispatch terminator");
    };

    assert_eq!(contract.body_outward_cases().len(), 1);
    let outward_case = contract.body_outward_cases()[0];
    assert!(contract.finally_outward_cases().is_empty());
    assert!(contract.pending_completions().contains(
        &LateLoweredHandlePendingCompletion::PropagateOutward(outward_case,)
    ));
    assert!(contract.outward_emission(outward_case).is_some());
}

#[test]
fn handle_dispatch_contract_publishes_pending_payload_transport_across_finally() {
    let output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_pending_payload_transport.scoop",
        r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
    ));
    let callable = callable(&output, "sample.propagate_before_finally");
    let (site_id, contract) = callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } if contract.pending_completions().iter().any(|completion| {
                matches!(
                    completion,
                    LateLoweredHandlePendingCompletion::PropagateOutward(_)
                )
            }) =>
            {
                Some((*site_id, contract))
            }
            _ => None,
        })
        .expect("fixture 应发布带 pending outward completion 的 HandleDispatch contract");

    let pending_case = *contract
        .body_outward_cases()
        .first()
        .expect("fixture 应发布 body outward case");
    let transport = contract
        .pending_payload_transport(LateLoweredHandlePendingCompletion::PropagateOutward(
            pending_case,
        ))
        .expect("pending outward case 应发布 typed payload transport");
    let slot = callable
        .frame_schema()
        .slot_for_kind(
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                site_id,
                case_tag: pending_case,
            },
        )
        .expect("frame schema 应保留 HandlePendingPayload slot");
    let emission = contract
        .outward_emission(pending_case)
        .expect("pending outward case 应保留 outward emission contract");

    assert_eq!(transport.frame_slot(), slot.slot_id());
    assert_eq!(transport.payload_tuple_ty(), slot.ty());
    assert_eq!(transport.payload_tuple_ty(), emission.payload_tuple_ty());
    assert!(
        contract
            .pending_payload_transport(LateLoweredHandlePendingCompletion::ContinueToExit)
            .is_none()
    );
}

#[test]
fn handle_dispatch_contract_publishes_origin_aware_pending_completion() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_resume_finally_body_raise_after_resume.scoop",
    ));
    let callable = callable(&output, "main");
    let handle_state = handle_dispatch_state(callable, SiteId::from_raw(2));
    let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
    else {
        panic!("指定 state 应保持 HandleDispatch terminator");
    };
    let body_raise_case = *contract
        .body_outward_cases()
        .first()
        .expect("fixture 应发布 resumed-body outward case");
    let completion = LateLoweredHandlePendingCompletion::PropagateOutward(body_raise_case);

    let mut origins_by_completion = BTreeMap::new();
    for origin in contract.pending_completion_origins() {
        origins_by_completion
            .entry(origin.completion())
            .or_insert_with(BTreeSet::new)
            .insert((origin.boundary_id(), origin.resume_state()));
    }
    let origins = origins_by_completion
        .get(&completion)
        .expect("resumed-body raise 应发布 pending completion origins");

    assert!(
        origins.len() >= 2,
        "同一 Raise<Int> pending completion 必须保留多个 origin/resume-state，而不是按 case 合并：{origins:?}"
    );
    assert!(
        origins
            .iter()
            .map(|(_, resume_state)| *resume_state)
            .collect::<BTreeSet<_>>()
            .len()
            >= 2,
        "pending completion origin 必须区分不同 resume state：{origins:?}"
    );
}

#[test]
fn handle_dispatch_contract_dump_exposes_published_completion_state() {
    let output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_handle_contract_dump.scoop",
        r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
    ));
    let dump = output.program().stable_dump();

    assert!(dump.contains("handle_contract:"));
    assert!(dump.contains("pending_completions:"));
    assert!(dump.contains("pending_completion_origins:"));
    assert!(dump.contains("pending_payload_transports:"));
    assert!(dump.contains("state_regions:"));
    assert!(dump.contains("boundary_routings:"));
    assert!(dump.contains("case_routings:"));
    assert!(dump.contains("PropagateOutward("));
    assert!(dump.contains("HandlePendingPayload("));
    assert!(dump.contains("outward_emissions:"));
}

#[test]
fn handle_arm_binding_contract_dump_exposes_payload_and_continuation_binders() {
    let output = load_output(&load_fixture(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
    ));
    let dump = output.program().stable_dump();

    assert!(dump.contains("payload_binders:"));
    assert!(dump.contains("continuation_binder:"));
    assert!(dump.contains("continuation_schema="));
}

#[test]
fn impl_plan_lowering_keeps_no_outward_single_case_and_canonical_full_distinct() {
    let no_outward_output = load_output(&SourceFile::new_virtual(
        "<mem>/late_lowered_no_outward.scoop",
        "package sample\nfun helper() {}\nfun main() { helper() }\n",
    ));
    let no_outward = callable(&no_outward_output, "sample.main");
    assert_eq!(no_outward.impl_plan(), ImplPlan::NoOutward);
    assert_eq!(no_outward.call_abi_kind(), CallableAbiKind::Plain);
    assert!(no_outward.body_step_schema().is_none());
    assert!(no_outward.effect_step_abi().is_none());
    assert!(no_outward.plain_abi().is_some());

    let single_case_output =
        load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
    let single_case = callable(&single_case_output, "sample.leaf");
    let single_case_object = single_case_output
        .program()
        .continuation_object(single_case.continuation_object())
        .expect("single-case callable 应能回查 continuation object");
    assert!(matches!(single_case.impl_plan(), ImplPlan::SingleCase(_)));
    assert_eq!(
        single_case_object
            .methods()
            .iter()
            .filter(|method| {
                method.reachability() == LateLoweredContinuationMethodReachability::Reachable
            })
            .count(),
        1
    );

    let canonical_output = load_output(&load_fixture(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
    ));
    let canonical = callable(&canonical_output, "sample.callValue");
    let canonical_boundary = site_boundary(canonical, BoundarySiteKind::Call);
    let LateLoweredBoundaryLowering::Call(canonical_lowering) = canonical_boundary
        .lowering()
        .expect("canonical-full boundary 应发布 lowering contract")
    else {
        panic!("canonical-full boundary 应物化成 Call lowering")
    };
    assert_eq!(canonical.impl_plan(), ImplPlan::CanonicalFull);
    assert_eq!(canonical_lowering.dispatch().outward_cases().len(), 2);
}
