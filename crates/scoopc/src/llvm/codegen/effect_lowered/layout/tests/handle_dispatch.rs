//! Handle-dispatch contract / region-routing / handle-arm continuation binding / completion-state tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn handle_dispatch_contract_publishes_llvm_query_layout() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("HandleDispatch contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let site_id = SiteId::from_raw(1);
            let contract = handle_dispatch_contract(callable, site_id);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 HandleDispatch contract");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");

            assert_eq!(published.owner_step_schema(), callable.step_schema());
            assert_eq!(published.site_id(), site_id);
            assert_eq!(published.lowered_contract(), contract);
            assert_eq!(
                published.state_tag_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::StateTag)
                    .expect("frame 应保留 StateTag")
            );
            assert_eq!(
                published.completion_tag_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::CompletionTag)
                    .expect("frame 应保留 CompletionTag")
            );
            assert_eq!(
                published.payload_carrier_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::ResumePayloadCarrier)
                    .expect("frame 应保留 ResumePayloadCarrier")
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
                    .is_some()
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::ReturnFromFunction)
                    .is_some()
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::PropagateOutward(
                        crate::effect_facts::CaseTag::new(1),
                    ))
                    .is_none()
            );
        },
    );
}

#[test]
pub(super) fn llvm_handle_dispatch_publishes_pending_payload_transport_layout() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_pending_payload_transport.scoop",
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
        )),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("pending payload transport 应可发布到 HandleDispatch LLVM query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.propagate_before_finally")
                .expect("sample.propagate_before_finally callable 应存在");
            let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 pending payload transport contract");
            let pending_case = *contract
                .body_outward_cases()
                .first()
                .expect("fixture 应发布 body outward case");
            let transport = published
                .pending_payload_transport_layout(
                    LateLoweredHandlePendingCompletion::PropagateOutward(pending_case),
                )
                .expect("pending outward case 应发布 typed payload transport layout");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");
            let slot = callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::HandlePendingPayload {
                    site_id,
                    case_tag: pending_case,
                })
                .expect("frame schema 应保留 HandlePendingPayload slot");

            assert_eq!(
                transport.completion(),
                LateLoweredHandlePendingCompletion::PropagateOutward(pending_case)
            );
            assert_eq!(transport.frame_slot(), slot.slot_id());
            assert_eq!(
                transport.frame_field_index(),
                frame_layout
                    .field_index_for_slot(slot.slot_id())
                    .expect("frame layout 应可回查 pending payload field")
            );
            assert_eq!(
                transport.payload_tuple_ty(),
                contract
                    .outward_emission(pending_case)
                    .expect("pending outward case 应保留 outward emission")
                    .payload_tuple_ty()
            );
            assert!(
                published
                    .pending_payload_transport_layout(
                        LateLoweredHandlePendingCompletion::ContinueToExit,
                    )
                    .is_none()
            );
        },
    );
}

#[test]
pub(super) fn llvm_handle_dispatch_rejects_missing_pending_payload_transport() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_pending_payload_transport_missing.scoop",
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
        )),
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.propagate_before_finally")
                .expect("callable 应存在");
            let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
            let broken_contract = LateLoweredHandleDispatchContract::new(
                contract.carrier(),
                contract.body_complete_target(),
                contract.arm_complete_target(),
                contract.finally_complete_target(),
                contract.body_completion_payload_source().cloned(),
                contract.handled_arms().to_vec(),
                contract.body_outward_cases().to_vec(),
                contract.finally_outward_cases().to_vec(),
                contract.outward_emissions().to_vec(),
                contract.pending_completions().to_vec(),
                contract.pending_completion_origins().to_vec(),
                Vec::new(),
                contract.state_regions().to_vec(),
                contract.boundary_routings().to_vec(),
                contract.abandon_target(),
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 pending payload transport 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("pending payload transport"),
                "错误消息应指出缺失的是 pending payload transport contract: {message}"
            );
            assert!(
                message.contains("sample.propagate_before_finally")
                    && message.contains("handle site"),
                "错误消息应指出出错 callable 和 site: {message}"
            );
        },
    );
}

#[test]
pub(super) fn handle_dispatch_region_routing_publishes_query_lookup() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("handle region routing contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("executeCase")
                .expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 handle region routing contract");
            let perform_boundary = callable
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::Site {
                            kind: BoundarySiteKind::Perform,
                            ..
                        }
                    )
                })
                .expect("fixture 应发布 body perform boundary");
            let routing = published
                .boundary_routing(perform_boundary.boundary_id())
                .expect("perform boundary 应可通过 query 回查 routing contract");
            let handled_arm = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm");
            let handled_route = routing
                .case_routing(handled_arm.handled_case())
                .expect("handled perform case 应发布 consume-to-arm routing");

            assert_eq!(
                routing.owner_region(),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert_eq!(
                published.state_region(routing.owner_state()),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert_eq!(
                published.state_region(routing.resume_state()),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert!(matches!(
                handled_route.action(),
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    continuation_resume_state,
                } if arm_state == handled_arm.arm_state()
                    && arm_ordinal == handled_arm.arm_ordinal()
                    && continuation_resume_state == routing.resume_state()
            ));
        },
    );
}

#[test]
pub(super) fn handle_dispatch_region_routing_rejects_resume_state_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("executeCase")
                .expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let handled_case = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm")
                .handled_case();
            let broken_routings = contract
                .boundary_routings()
                .iter()
                .map(|routing| {
                    let broken_case_routings = routing
                        .case_routings()
                        .iter()
                        .map(|route| {
                            if route.case_tag() != handled_case {
                                return *route;
                            }
                            let broken_action = match route.action() {
                                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                    arm_state,
                                    arm_ordinal,
                                    ..
                                } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                    arm_state,
                                    arm_ordinal,
                                    continuation_resume_state: contract.body_complete_target(),
                                },
                                other => other,
                            };
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(
                                route.case_tag(),
                                broken_action,
                            )
                        })
                        .collect::<Vec<_>>();
                    crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                        routing.boundary_id(),
                        routing.owner_state(),
                        routing.owner_region(),
                        routing.resume_state(),
                        broken_case_routings,
                    )
                })
                .collect::<Vec<_>>();
            let broken_contract = clone_handle_dispatch_contract_with_regions_and_routes(
                contract,
                contract.state_regions().to_vec(),
                broken_routings,
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("handle boundary routing resume_state 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("boundary-routing contract 漂移")
                    || message.contains("consume_to_arm")
                    || message.contains("resume=st"),
                "错误消息应指出 published routing 与 state graph/boundary map 不一致: {message}"
            );
        },
    );
}

#[test]
pub(super) fn handle_arm_binding_contract_publishes_llvm_query_layout() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_arm_binding_single.scoop",
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
        )),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("handle arm binder contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.run")
                .expect("sample.run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 HandleDispatch arm binder contract");
            let arm = published
                .handled_arms()
                .first()
                .expect("单 arm fixture 应发布唯一 handled arm layout");

            assert_eq!(arm.arm_ordinal(), 0);
            assert_eq!(arm.payload_binders().len(), 2);
            assert_eq!(arm.payload_binders()[0].ordinal(), 0);
            assert_eq!(arm.payload_binders()[1].ordinal(), 1);
            let continuation_binder = arm
                .continuation_binder()
                .expect("escape continuation arm 应发布 continuation binder layout");
            assert_eq!(
                continuation_binder.continuation_object(),
                callable.continuation_object()
            );
            assert_eq!(
                continuation_binder.surface_resume_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
            assert_eq!(
                continuation_binder.surface_resume_return_step_schema(),
                callable.step_schema()
            );
        },
    );
}

#[test]
pub(super) fn handle_arm_continuation_binding_publishes_mixed_multi_arm_query_layout() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("mixed multi-arm handle 应可发布 arm binder query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 mixed handle arm binder contract");

            assert_eq!(published.handled_arms().len(), 2);
            let escape_arm = published
                .handled_arms()
                .iter()
                .find(|arm| arm.continuation_binder().is_some())
                .expect("mixed fixture 应发布带 continuation binder 的 arm layout");
            let payload_only_arm = published
                .handled_arms()
                .iter()
                .find(|arm| arm.continuation_binder().is_none())
                .expect("mixed fixture 应发布纯 payload arm layout");

            assert_eq!(escape_arm.payload_binders().len(), 1);
            assert_eq!(payload_only_arm.payload_binders().len(), 1);
            let continuation_binder = escape_arm
                .continuation_binder()
                .expect("escape arm 应带 continuation binder layout");
            assert_eq!(
                continuation_binder.surface_resume_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
        },
    );
}

#[test]
pub(super) fn completion_state_contract_rejects_missing_completion_tag_slot() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(
                callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .filter(|slot| {
                        slot.kind()
                            != LateLoweredFrameSlotKind::System(SystemSlotKind::CompletionTag)
                    })
                    .cloned()
                    .collect(),
            )
            .with_resume_payload_bindings(
                callable.frame_schema().resume_payload_bindings().to_vec(),
            )
            .with_completion_payload_bindings(
                callable
                    .frame_schema()
                    .completion_payload_bindings()
                    .to_vec(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 CompletionTag system field 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("缺少 CompletionTag system field"),
                "错误消息应指出缺失的是 CompletionTag 槽位: {message}"
            );
            assert!(
                message.contains("sample.nested_may_suspend_outward"),
                "错误消息应指出出错 callable: {message}"
            );
        },
    );
}

#[test]
pub(super) fn handle_arm_binding_contract_rejects_payload_binder_order_drift() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_arm_binding_order_drift.scoop",
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
        )),
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.run")
                .expect("sample.run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let original_arm = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm");
            let mut swapped_binders = original_arm.payload_binders().to_vec();
            swapped_binders.swap(0, 1);
            let broken_arm = crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                original_arm.handled_case(),
                original_arm.arm_state(),
                original_arm.arm_ordinal(),
                original_arm.payload_tuple_ty(),
                original_arm.completion_payload_source().clone(),
                swapped_binders,
                original_arm.continuation_binder(),
                original_arm.arm_outward_cases().to_vec(),
            );
            let broken_contract =
                clone_handle_dispatch_contract_with_handled_arms(contract, vec![broken_arm]);
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("payload binder 次序漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("payload binder ordinal 漂移")
                    || message.contains("payload binder #0 local 漂移"),
                "错误消息应指出 payload binder 顺序漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn handle_dispatch_contract_rejects_missing_handled_arm_mapping() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let site_id = SiteId::from_raw(1);
            let contract = handle_dispatch_contract(callable, site_id);
            let broken_contract = LateLoweredHandleDispatchContract::new(
                contract.carrier(),
                contract.body_complete_target(),
                contract.arm_complete_target(),
                contract.finally_complete_target(),
                contract.body_completion_payload_source().cloned(),
                Vec::new(),
                contract.body_outward_cases().to_vec(),
                contract.finally_outward_cases().to_vec(),
                contract.outward_emissions().to_vec(),
                contract.pending_completions().to_vec(),
                contract.pending_completion_origins().to_vec(),
                contract.pending_payload_transports().to_vec(),
                contract.state_regions().to_vec(),
                contract.boundary_routings().to_vec(),
                contract.abandon_target(),
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 handled-arm 映射时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("handled-arm 数量"),
                "错误消息应指出缺失的是 handled-arm mapping: {message}"
            );
            assert!(
                message.contains("handle site 1") || message.contains("site 1"),
                "错误消息应指出出错 site: {message}"
            );
        },
    );
}

#[test]
pub(super) fn handle_arm_continuation_binding_rejects_missing_published_continuation_binder() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let broken_arms = contract
                .handled_arms()
                .iter()
                .map(|arm| {
                    crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                        arm.handled_case(),
                        arm.arm_state(),
                        arm.arm_ordinal(),
                        arm.payload_tuple_ty(),
                        arm.completion_payload_source().clone(),
                        arm.payload_binders().to_vec(),
                        None,
                        arm.arm_outward_cases().to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            let broken_contract =
                clone_handle_dispatch_contract_with_handled_arms(contract, broken_arms);
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 published continuation binder 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("underlying continuation route")
                    && message.contains("HandleContinuationBinder"),
                "错误消息应指出缺失的是 continuation binder contract: {message}"
            );
        },
    );
}
