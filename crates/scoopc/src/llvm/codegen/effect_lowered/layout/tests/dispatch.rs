//! Callable-carrier layout, dynamic-invoke publication, runtime-error contract tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn llvm_callable_carrier_layout_resolves_non_boundary_virtual_contracts() {
    with_fixture_query(
        "effect_lowered_non_boundary_dynamic_call_emit_llvm.scoop",
        |inputs, query, _module| {
            let helper = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let (_site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);

            assert_eq!(facts.target_mode(), CallTargetMode::CandidateSet);
            let CallSiteTarget::CandidateSet(targets) = facts.target() else {
                panic!("non-boundary virtual call 应保留 CandidateSet target");
            };
            assert!(
                targets
                    .iter()
                    .any(|target| target.template.fqn == "fixtures.build.Base.ping")
            );
            assert!(
                query
                    .plain_callable_layout_by_version_key(helper.body_version_key())
                    .is_ok(),
                "NoOutward non-boundary dynamic call owner 应保持 plain callable layout"
            );
            for target in targets {
                assert!(
                    query
                        .plain_callable_layout_by_root_fqn(&target.template.fqn)
                        .is_ok(),
                    "NoOutward virtual target `{}` 应发布 plain callable layout",
                    target.template.fqn
                );
            }
        },
    );
}

#[test]
pub(super) fn llvm_dynamic_invoke_query_rejects_missing_published_contract() {
    with_fixture_query_result(
        "effect_lowered_dynamic_invoke_candidate_set_emit_llvm.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let helper = program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let bogus_site = crate::mir::SiteId::from_raw(999);
            let rewritten_boundary_map = LateLoweredBoundaryMap::new(
                helper
                    .boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let source = match boundary.source() {
                            LateLoweredBoundarySource::Site {
                                kind: BoundarySiteKind::Call,
                                ..
                            } => LateLoweredBoundarySource::Site {
                                site_id: bogus_site,
                                kind: BoundarySiteKind::Call,
                            },
                            other => other,
                        };
                        let lowered = boundary
                            .lowering()
                            .cloned()
                            .expect("candidate-set helper 的 boundary 应带 lowering");
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            source,
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowered)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(helper.step_schema()) {
                        clone_callable_with_boundary_map(candidate, rewritten_boundary_map.clone())
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
                Ok(_) => panic!("缺失 dynamic-invoke contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("canonical MIR call metadata"),
                "错误消息应指出缺失的是 call-site authoritative metadata: {message}"
            );
            assert!(
                message.contains("dynamic-invoke contract"),
                "错误消息应指出缺失的是 dynamic-invoke contract: {message}"
            );
            assert!(
                message.contains("fixtures.build.helper") && message.contains("999"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_call_boundary_continuation_composition() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("ABI materialization 应成功");
            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let composition = main
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| {
                    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                    else {
                        return None;
                    };
                    lowering.continuation_compositions().first()
                })
                .expect("main 的 fetch call boundary 应发布 composition contract");
            let continuation_layout = query
                .continuation_layout(main.continuation_object())
                .expect("main continuation object layout 应存在");
            assert!(continuation_layout.fields().iter().any(|field| {
                field.kind() == ContinuationFieldKind::CapturedCalleeSuspendStateRef
            }));
            let callee_surface = query
                .surface_resume_layout(composition.callee_continuation_schema())
                .expect("callee continuation surface resume ABI 应发布");
            assert_eq!(
                callee_surface.return_step_schema(),
                composition.input_step_schema()
            );
            assert_eq!(
                callee_surface.resume_tuple_ty(),
                composition.callee_continuation_contract().resume_tuple_ty()
            );
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("main boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.operand_contract().clone(),
                                        lowering.dispatch().clone(),
                                        Vec::new(),
                                        lowering.consumed_runtime_error_case().cloned(),
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
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
                Ok(_) => panic!("缺失 continuation composition 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("continuation composition"),
                "错误消息应指出缺失 call-boundary continuation composition: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_dynamic_entry_publication_declares_closure_vtable_and_itable_targets() {
    with_inputs_query_result_and_codegen(
        build_fixture_inputs("effect_lowered_dynamic_entry_publication_emit_llvm.scoop"),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, codegen, result, module| {
            let query = result.expect("ABI materialization 应成功");
            let make_closure_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.makeClosure")
                .expect("makeClosure callable 应存在");
            let base_ping_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.Base.ping")
                .expect("Base.ping callable 应存在");
            let derived_ping_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.Derived.ping")
                .expect("Derived.ping callable 应存在");

            let make_closure = query
                .plain_callable_layout_by_version_key(make_closure_callable.body_version_key())
                .expect("makeClosure plain callable target 应存在");
            let base_vtable = query
                .plain_callable_layout_by_version_key(base_ping_callable.body_version_key())
                .expect("Base.ping plain callable target 应存在");
            let derived_vtable = query
                .plain_callable_layout_by_version_key(derived_ping_callable.body_version_key())
                .expect("Derived.ping plain callable target 应存在");

            assert_eq!(
                make_closure.body_version_key(),
                make_closure_callable.body_version_key()
            );
            assert_eq!(
                base_vtable.body_version_key(),
                base_ping_callable.body_version_key()
            );
            assert_eq!(
                derived_vtable.body_version_key(),
                derived_ping_callable.body_version_key()
            );

            for (kind, fqn) in [
                (
                    CallableCarrierKind::ClosureObject,
                    "fixtures.build.makeClosure",
                ),
                (CallableCarrierKind::ClassVtable, "fixtures.build.Base.ping"),
                (
                    CallableCarrierKind::InterfaceItable,
                    "fixtures.build.Base.ping",
                ),
                (
                    CallableCarrierKind::ClassVtable,
                    "fixtures.build.Derived.ping",
                ),
                (
                    CallableCarrierKind::InterfaceItable,
                    "fixtures.build.Derived.ping",
                ),
            ] {
                assert!(
                    query.callable_carrier_target_layout(kind, fqn).is_err(),
                    "NoOutward carrier `{fqn}` 不应发布 effect-step dynamic entry target"
                );
                assert!(
                    codegen.plain_callable_carrier_fallback_allowed(kind, fqn),
                    "NoOutward carrier `{fqn}` 应发布 plain callable fallback"
                );
            }

            let _ = codegen
                .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Base")
                .expect("Base vtable 应可物化");
            let _ = codegen
                .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Derived")
                .expect("Derived vtable 应可物化");
            let _ = codegen
                .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Base")
                .expect("Base itable 应可物化");
            let _ = codegen
                .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Derived")
                .expect("Derived itable 应可物化");

            assert!(
                module
                    .get_function(make_closure.direct_entry().symbol_name())
                    .is_some()
            );
            assert!(
                module
                    .get_function(base_vtable.direct_entry().symbol_name())
                    .is_some()
            );
            assert!(
                module
                    .get_function(derived_vtable.direct_entry().symbol_name())
                    .is_some()
            );
        },
    );
}

#[test]
pub(super) fn llvm_callable_carrier_version_selection_rejects_ambiguous_root_targets() {
    with_fixture_query_result(
        "effect_lowered_dynamic_entry_publication_emit_llvm.scoop",
        |inputs| {
            duplicate_no_outward_callable_version(
                &inputs.abi_visibility_program,
                "fixtures.build.makeClosure",
            )
        },
        |_inputs, result, _module| {
            let query = result.expect("duplicated plain versions 应允许物化到 version-key 查询面");
            let err = match query.plain_callable_layout_by_root_fqn("fixtures.build.makeClosure") {
                Ok(_) => panic!("歧义 root 查询必须要求调用方改用 body version key"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("fixtures.build.makeClosure"),
                "错误消息应指出歧义 callable: {message}"
            );
            assert!(
                message.contains("多个 published callable version"),
                "错误消息应指出存在多个 callable version: {message}"
            );
            assert!(
                message.contains("body version key"),
                "错误消息应指出歧义 version key: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_dynamic_entry_publication_rejects_missing_dispatch_callable_shell() {
    with_inputs_query_result_and_codegen(
        build_fixture_inputs("effect_lowered_dynamic_entry_publication_emit_llvm.scoop"),
        |inputs| inputs.abi_visibility_program.clone(),
        |_inputs, codegen, result, _module| {
            let _ = result.expect("ABI materialization 应成功");
            let dummy_fn = codegen.declare_compiler_private_helper_function(
                "__scoop_missing_carrier_target_dummy",
                codegen.context.void_type().fn_type(&[], false),
                Linkage::External,
            );
            let err = match codegen.callable_carrier_target_fn_ptr(
                CallableCarrierKind::ClassVtable,
                "fixtures.build.Missing.ping",
                dummy_fn.as_global_value().as_pointer_value(),
            ) {
                Ok(_) => panic!("缺失 dispatch callable shell 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("fixtures.build.Missing.ping"),
                "错误消息应指出缺失 shell 的 target callable: {message}"
            );
            assert!(
                message.contains("published target entry") || message.contains("class vtable slot"),
                "错误消息应指出问题出在 carrier target 发布: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_local_runtime_error_contract_resolves_pure_call_boundary_targets() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query =
                result.expect("pure caller local runtime-error contract 应可发布到 ABI query");
            let main = inputs
                .effect_lowered_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
            let mut checked = 0usize;

            for boundary in main.boundary_map().entries() {
                let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                    continue;
                };
                let Some(contract) = lowering.consumed_runtime_error_case() else {
                    continue;
                };
                let site_id = boundary_site_id(boundary);
                let published = query
                    .call_local_runtime_error_contract(main.step_schema(), site_id, contract)
                    .expect("call boundary 应可回查 published local runtime-error contract");

                assert_eq!(published.owner_step_schema(), main.step_schema());
                assert_eq!(published.site_id(), site_id);
                assert_eq!(published.input_case_tag(), contract.input_case_tag());
                assert_eq!(published.payload_tuple_ty(), contract.payload_tuple_ty());
                assert_eq!(
                    published.terminal_action().lowered_action(),
                    contract.terminal_action()
                );
                assert_eq!(published.target_state(), contract.target_state());
                assert!(
                    !published.payload_abi().is_elided(),
                    "RuntimeError payload 不应被零载荷退化"
                );
                let runtime_entry = published.terminal_action().runtime_entry();
                assert_eq!(
                    runtime_entry.kind(),
                    LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal
                );
                assert_eq!(runtime_entry.symbol_name(), "scoop_runtime_error_fatal");
                assert_eq!(runtime_entry.param_count(), 1);
                assert!(
                    module.get_function(runtime_entry.symbol_name()).is_some(),
                    "published runtime fatal entry 应声明到 LLVM module 中"
                );
                checked += 1;
            }

            assert_eq!(
                checked, 2,
                "fixture 应包含两个 pure caller call boundary contract"
            );
        },
    );
}

#[test]
pub(super) fn llvm_local_runtime_error_contract_rejects_missing_target_state() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("main boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                let consumed_runtime_error_case = lowering
                                    .consumed_runtime_error_case()
                                    .cloned()
                                    .map(|contract| {
                                        LateLoweredConsumedRuntimeErrorCase::new(
                                            contract.input_case_tag(),
                                            contract.input_concrete_op_key().clone(),
                                            contract.payload_tuple_ty(),
                                            contract.terminal_action(),
                                            StateId::new(999),
                                        )
                                    });
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.operand_contract().clone(),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
                                        consumed_runtime_error_case,
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
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
                Ok(_) => panic!("缺失 local runtime-error target state 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("local runtime-error target state"),
                "错误消息应指出缺失的是 local runtime-error target state: {message}"
            );
            assert!(
                message.contains("main") && message.contains("call site 1"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_local_runtime_error_contract_rejects_non_local_runtime_error_terminator() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let local_runtime_error_states = main
                .boundary_map()
                .entries()
                .iter()
                .filter_map(|boundary| {
                    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                    else {
                        return None;
                    };
                    lowering
                        .consumed_runtime_error_case()
                        .map(|contract| contract.target_state())
                })
                .collect::<BTreeSet<_>>();
            let rewritten_states = main
                .state_graph()
                .states()
                .iter()
                .map(|state| {
                    if !local_runtime_error_states.contains(&state.state_id()) {
                        return state.clone();
                    }
                    crate::effect_lowered::ir::LateLoweredState::new(
                        state.state_id(),
                        state.role(),
                        state.source_slices().to_vec(),
                        crate::effect_lowered::ir::LateLoweredStateTerminator::Unreachable,
                    )
                })
                .collect::<Vec<_>>();
            let state_graph = crate::effect_lowered::ir::LateLoweredStateGraph::new(
                main.state_graph().entry_state(),
                main.state_graph().complete_state(),
                main.state_graph().cleanup_state(),
                main.state_graph().drop_state(),
                rewritten_states,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
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
                Ok(_) => panic!("缺失 LocalRuntimeError terminal contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("不是 LocalRuntimeError terminator"),
                "错误消息应指出 local runtime-error target state 缺少终止 contract: {message}"
            );
            assert!(
                message.contains("main") && message.contains("call site 1"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}
