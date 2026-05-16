//! Source-slice classification, stable-id, ABI / frame / step layout baseline tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn refactor_llvm_source_slice_classification_rejects_missing_handoff() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let mut removed = false;
            let callables = program
                .callables()
                .iter()
                .map(|callable| {
                    if !removed && !callable.source_statement_classifications().is_empty() {
                        removed = true;
                        clone_callable_with_source_statement_classifications(callable, Vec::new())
                    } else {
                        callable.clone()
                    }
                })
                .collect();
            assert!(
                removed,
                "fixture 应发布至少一个 source statement classification"
            );
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 classification handoff 必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains("source-slice statement"));
            assert!(message.contains("classification"));
        },
    );
}

#[test]
pub(super) fn refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell() {
    with_fixture_query(
        "effect_refactor_step_enum_no_outward.scoop",
        |inputs, query, module| {
            for fqn in ["fixtures.build.helper", "fixtures.build.main"] {
                let callable = inputs
                    .abi_visibility_program
                    .callable(fqn)
                    .expect("plain callable 应存在");
                assert!(callable.plain_abi().is_some());
                assert!(callable.body_step_schema().is_none());

                let layout = query
                    .plain_callable_layout_by_version_key(callable.body_version_key())
                    .expect("plain callable layout 应可查询");
                assert_eq!(layout.root_fqn(), fqn);
                let direct_symbol = layout.direct_entry().symbol_name();
                let expected_prefix = if fqn == "fixtures.build.main" {
                    "__scoop_abi0_fun__fixtures_build_main__h"
                } else {
                    "__scoop_abi0_fun__fixtures_build_helper__h"
                };
                assert!(
                    direct_symbol.starts_with(expected_prefix),
                    "source-level plain callable direct entry 应切到 AbiMangler namespace: {direct_symbol}"
                );
                assert!(module.get_function(direct_symbol).is_some());
                assert!(
                    query
                        .callable_layout_by_version_key(callable.body_version_key())
                        .is_err(),
                    "plain callable 不应发布 effect-step callable layout"
                );
            }

            let ir = module.print_to_string().to_string();
            assert!(
                !ir.contains("__scoop_priv0__refactor_step_case_tag_complete__h")
                    && !ir.contains("__scoop_priv0__refactor_direct_invoke__h")
                    && !ir.contains("__scoop_priv0__refactor_dynamic_invoke__h")
                    && !ir.contains("%scoop.refactor.Step__h"),
                "plain callable 不应发布 effect-step type/case-tag/dynamic-entry 家族：\n{ir}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_step_layout_keeps_canonical_case_set_for_single_case_callable() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            assert_eq!(callable.impl_plan(), ImplPlan::SingleCase(CaseTag::new(0)));

            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应可查询");
            assert_eq!(step_layout.complete_variant().tag_value(), 0);
            assert_eq!(step_layout.cases().len(), 3);
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(0))
                    .expect("case0 应存在")
                    .variant()
                    .tag_value(),
                1
            );
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(1))
                    .expect("case1 应存在")
                    .variant()
                    .tag_value(),
                2
            );
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(2))
                    .expect("runtime-error case 应存在")
                    .variant()
                    .tag_value(),
                3
            );
            assert!(
                module
                    .get_global(step_layout.complete_tag_constant_name())
                    .is_some()
            );
            assert!(
                module
                    .get_global(
                        step_layout
                            .case_layout(CaseTag::new(1))
                            .expect("case1 应存在")
                            .tag_constant_name(),
                    )
                    .is_some()
            );
            assert!(
                module
                    .get_global(
                        step_layout
                            .case_layout(CaseTag::new(2))
                            .expect("runtime-error case 应存在")
                            .tag_constant_name(),
                    )
                    .is_some()
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_frame_layout_preserves_slot_indices_and_system_fields() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");

            assert_eq!(
                frame_layout.fields()[0].kind(),
                RefactorFrameFieldKind::Header
            );
            for (ordinal, slot) in callable.frame_schema().slots().iter().enumerate() {
                let expected_field_index = ordinal as u32 + 1;
                assert_eq!(
                    frame_layout.field_index_for_slot(slot.slot_id()),
                    Some(expected_field_index)
                );
                if let LateLoweredFrameSlotKind::System(kind) = slot.kind() {
                    assert_eq!(
                        frame_layout.field_index_for_system(kind),
                        Some(expected_field_index)
                    );
                }
            }
            for required in [
                SystemSlotKind::StateTag,
                SystemSlotKind::ResumePayloadCarrier,
                SystemSlotKind::CleanupFlag,
                SystemSlotKind::OneShotFlag,
                SystemSlotKind::CompletionTag,
                SystemSlotKind::CurrentEffectCtx,
            ] {
                assert!(
                    frame_layout.field_index_for_system(required).is_some(),
                    "frame layout 缺少系统字段 {required:?}"
                );
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_keeps_full_method_set() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(0), CaseTag::new(1)],
            )
        },
        |inputs, result, module| {
            let query = result.expect("published full method set 应可物化 ABI");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let callable_layout = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询");
            let interface_id = *callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");

            assert_eq!(interface_layout.methods().len(), 2);
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(0))
                    .expect("case0 method 应存在")
                    .vtable_index(),
                0
            );
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(1))
                    .expect("case1 method 应存在")
                    .vtable_index(),
                1
            );
            assert!(
                continuation_layout
                    .field_index_for_packing(interface_id)
                    .is_some()
            );
            assert!(
                module
                    .get_function(
                        interface_layout
                            .method(CaseTag::new(1))
                            .expect("case1 method 应存在")
                            .symbol_name(),
                    )
                    .is_some()
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_uses_codegen_owned_fields() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let field_kinds = continuation_layout
                .fields()
                .iter()
                .take(9)
                .map(|field| field.kind())
                .collect::<Vec<_>>();

            assert_eq!(
                field_kinds,
                vec![
                    RefactorContinuationFieldKind::Header,
                    RefactorContinuationFieldKind::ResumedFlag,
                    RefactorContinuationFieldKind::ResumeStateTag,
                    RefactorContinuationFieldKind::CapturedEffectCtxRef,
                    RefactorContinuationFieldKind::StateRef,
                    RefactorContinuationFieldKind::StepFn,
                    RefactorContinuationFieldKind::ResumeWord,
                    RefactorContinuationFieldKind::ResumeGcRef,
                    RefactorContinuationFieldKind::CapturedCalleeSuspendStateRef,
                ]
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_preserves_published_packing_order() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(0), CaseTag::new(1)],
            );
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_object = program
                .continuation_object(callable.continuation_object())
                .expect("continuation object 应存在");
            let step_type = program
                .step_type(callable.step_schema())
                .expect("step type 应存在");
            let ping_interface = program
                .resume_packings()
                .iter()
                .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
                .expect("应存在 Ping resume packing");
            let raise_interface_id = next_resume_interface_id(&program);
            let raise_method = resume_method_for_case(step_type, CaseTag::new(2));
            let raise_interface = LateLoweredResumeInterface::new(
                raise_interface_id,
                raise_method.concrete_op_key().effect_family().clone(),
                callable.step_schema(),
                vec![raise_method],
            );
            let reversed_interfaces = vec![raise_interface_id, ping_interface.interface_id()];
            let resume_interfaces = program
                .resume_packings()
                .iter()
                .cloned()
                .chain(std::iter::once(raise_interface))
                .collect();

            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_interfaces(candidate, reversed_interfaces.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == continuation_object.object_id() {
                        clone_continuation_object_with_interfaces(
                            candidate,
                            reversed_interfaces.clone(),
                        )
                    } else {
                        candidate.clone()
                    }
                })
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                resume_interfaces,
                continuation_objects,
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |inputs, result, _module| {
            let query = result.expect("reordered published resume packings 应仍可物化 ABI");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let callable_layout = query
                .callable_layout_by_version_key(callable.body_version_key())
                .expect("callable layout 应可查询");
            let ping_interface_id = callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .copied()
                .expect("应存在 Ping resume packing");
            let raise_interface_id = callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "scoop.core.Raise"
                        })
                })
                .copied()
                .expect("应存在 Raise resume packing");
            let expected_order = vec![raise_interface_id, ping_interface_id];

            assert_eq!(callable_layout.resume_packings(), expected_order.as_slice());

            let continuation_layout = query
                .continuation_layout(callable_layout.continuation_object())
                .expect("continuation layout 应可查询");
            let first_index = continuation_layout
                .field_index_for_packing(expected_order[0])
                .expect("首个 published packing 应有 field");
            let second_index = continuation_layout
                .field_index_for_packing(expected_order[1])
                .expect("次个 published packing 应有 field");
            assert!(
                first_index < second_index,
                "continuation field 顺序必须跟随 published packing 顺序"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_preserves_authoritative_method_order() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(1), CaseTag::new(0)],
            )
        },
        |inputs, result, _module| {
            let query = result.expect("reordered authoritative methods 应仍可物化 ABI");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let interface_id = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询")
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .copied()
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");

            assert_eq!(
                interface_layout
                    .method(CaseTag::new(1))
                    .expect("case1 method 应存在")
                    .vtable_index(),
                0
            );
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(0))
                    .expect("case0 method 应存在")
                    .vtable_index(),
                1
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_rejects_missing_published_packing() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let dropped_interface = callable
                .resume_packings()
                .first()
                .copied()
                .expect("fixture 应至少发布一个 packing");
            let resume_interfaces = program
                .resume_packings()
                .iter()
                .filter(|interface| interface.interface_id() != dropped_interface)
                .cloned()
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                resume_interfaces,
                program.continuation_objects().to_vec(),
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |inputs, result, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let dropped_interface = callable
                .resume_packings()
                .first()
                .copied()
                .expect("fixture 应至少发布一个 packing");
            let err = match result {
                Ok(_) => panic!("缺失 published packing 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains(&format!("resume packing {}", dropped_interface.as_u32())),
                "错误消息应指出缺失的 published packing: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_continuation_layout_rejects_missing_authoritative_method() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| single_case_worker_program_with_ping_method_order(inputs, &[CaseTag::new(0)]),
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 authoritative method 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("authoritative method cases [1]"),
                "错误消息应指出缺失的 authoritative case tag: {message}"
            );
            assert!(
                message.contains("effect family `fixtures.build.Ping`"),
                "错误消息应指出缺失方法所属的 interface family: {message}"
            );
            assert!(
                message.contains("step schema"),
                "错误消息应指出缺失方法对应的 step schema: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_call_target_query_preserves_known_instance_direct_entries() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_handle_hidden_suspend_virtual_helper_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("known-instance direct call 应可回查 callable entry");
            let program = inputs.effect_lowered_stage_output.program();
            let main = program.callable("main").expect("main callable 应存在");
            let helper = program.callable("helper").expect("helper callable 应存在");
            let main_plain = main.plain_abi().expect("main 应保持 plain callable ABI");
            let call_facts = main_plain
                .call_sites()
                .iter()
                .map(|site| site.facts())
                .find(|facts| matches!(facts.target(), CallSiteTarget::KnownInstance(target) if target.template.fqn == "helper"))
                .expect("main plain source slice 应发布 helper known-instance call facts");

            assert_eq!(call_facts.target_mode(), CallTargetMode::KnownInstance);
            if helper.effect_step_abi().is_some() {
                let target = query
                    .callable_layout_by_version_key(helper.body_version_key())
                    .expect("effect-step helper 应可按 body version key 回查 callable entry");
                assert_eq!(target.root_fqn(), "helper");
                assert_eq!(target.body_version_key(), helper.body_version_key());
            } else {
                let target = query
                    .plain_callable_layout_by_version_key(helper.body_version_key())
                    .expect("NoOutward helper 应可按 body version key 回查 plain entry");
                assert_eq!(target.root_fqn(), "helper");
                assert_eq!(target.body_version_key(), helper.body_version_key());
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_callable_version_query_resolves_layout_by_body_version_key() {
    with_fixture_query(
        "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
        |inputs, query, _module| {
            for callable in inputs.abi_visibility_program.callables() {
                if callable.effect_step_abi().is_some() {
                    let layout = query
                        .callable_layout_by_version_key(callable.body_version_key())
                        .expect("effect-step callable version 应可按 body version key 回查");
                    assert_eq!(layout.root_fqn(), callable.root_fqn());
                    assert_eq!(layout.step_schema(), callable.step_schema());
                    assert_eq!(layout.continuation_object(), callable.continuation_object());
                } else {
                    let layout = query
                        .plain_callable_layout_by_version_key(callable.body_version_key())
                        .expect("plain callable version 应可按 body version key 回查");
                    assert_eq!(layout.root_fqn(), callable.root_fqn());
                }
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_known_instance_version_selection_resolves_generic_instance_keys() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("generic known-instance callable 应可回查 callable version");
            let println_int = inputs
                .abi_visibility_program
                .callables()
                .iter()
                .find(|callable| callable.root_fqn() == "scoop.core.println::<Int>")
                .expect("fixture 应发布 println::<Int> callable shell");
            let target = query
                .plain_callable_layout_by_version_key(println_int.body_version_key())
                .expect("NoOutward generic callable 应发布 plain version layout");

            assert_eq!(target.root_fqn(), println_int.root_fqn());
            assert_eq!(target.body_version_key(), println_int.body_version_key());
            assert_eq!(target.surface_instance(), println_int.instance_key());
            assert!(
                query
                    .callable_layout_by_version_key(println_int.body_version_key())
                    .is_err(),
                "NoOutward generic callable 不应发布 effect-step callable layout"
            );
            let direct_symbol = target.direct_entry().symbol_name();
            assert!(
                direct_symbol.starts_with("__scoop_abi0_fun__scoop_core_println__h"),
                "materialized generic plain callable 应切到 AbiMangler namespace: {direct_symbol}"
            );
            assert!(module.get_function(direct_symbol).is_some());
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_resolves_direct_call_anchor_and_args() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("direct call boundary operand contract 应成功发布");
            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let boundary = site_boundary(main, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .call_boundary_operand_layout(
                    main.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("direct call boundary 应可回查 published operand contract");
            let RefactorCallTargetQuery::KnownInstance(_) = query
                .call_target_layout(main.step_schema(), site_id, lowering.facts())
                .expect("direct call target contract 应成功")
            else {
                panic!("known-instance direct call 不应走 dynamic invoke contract");
            };

            assert_eq!(layout.owner_step_schema(), main.step_schema());
            assert_eq!(layout.site_id(), site_id);
            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement {
                    consumes_last_statement: true,
                    ..
                }
            ));
            assert!(layout.contract().carrier_source().is_none());
            assert_eq!(layout.contract().arg_sources().len(), 1);
            assert_eq!(
                inputs
                    .effect_lowered_stage_output
                    .types()
                    .display(layout.contract().arg_sources()[0].source_ty())
                    .to_string(),
                "Bool"
            );
            assert!(matches!(
                layout.contract().arg_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Bool(_))
            ));
            assert!(layout.contract().arg_sources()[0].span().is_some());
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_resolves_dynamic_call_carrier() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("dynamic call boundary operand contract 应成功发布");
            let call_value = inputs
                .abi_visibility_program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary = site_boundary(call_value, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .call_boundary_operand_layout(
                    call_value.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("dynamic call boundary 应可回查 published operand contract");
            let RefactorCallTargetQuery::DynamicInvoke(_) = query
                .call_target_layout(call_value.step_schema(), site_id, lowering.facts())
                .expect("dynamic call target contract 应成功")
            else {
                panic!("non-KnownInstance call 应走 dynamic invoke contract");
            };

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement { .. }
            ));
            assert_eq!(layout.contract().arg_sources().len(), 0);
            assert!(matches!(
                layout
                    .contract()
                    .carrier_source()
                    .expect("dynamic call 应发布 carrier source")
                    .value(),
                LateLoweredOperandValueSource::Local(_)
            ));
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_resolves_perform_and_resume_sources() {
    with_phase_fixture_query_result(
        "effect_facts",
        "handle_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("perform boundary operand contract 应成功发布");
            let main = inputs
                .abi_visibility_program
                .callable("a.main")
                .expect("a.main callable 应存在");
            let boundary = site_boundary(main, BoundarySiteKind::Perform);
            let lowering = perform_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .perform_boundary_operand_layout(
                    main.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("perform boundary 应可回查 published operand contract");

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Terminator { .. }
            ));
            assert_eq!(layout.contract().payload_sources().len(), 1);
            assert!(matches!(
                layout.contract().payload_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
            ));
            assert!(layout.contract().payload_sources()[0].span().is_some());
        },
    );

    with_phase_fixture_query_result(
        "effect_facts",
        "dispatch_and_resume_call.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("resume boundary operand contract 应成功发布");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.mir.resumeBoom")
                .expect("fixtures.mir.resumeBoom callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Resume);
            let lowering = resume_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .resume_boundary_operand_layout(
                    callable.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("resume boundary 应可回查 published operand contract");

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement {
                    consumes_last_statement: true,
                    ..
                }
            ));
            assert!(matches!(
                layout.contract().continuation_source().value(),
                LateLoweredOperandValueSource::Local(_)
            ));
            assert_eq!(layout.contract().arg_sources().len(), 1);
            assert!(matches!(
                layout.contract().arg_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
            ));
            assert!(layout.contract().arg_sources()[0].span().is_some());
            let route = layout.contract().underlying_continuation_route();
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
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("readback resume boundary provenance 应成功发布到 LLVM query");
            let callable = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                handle_state.terminator()
            else {
                panic!("main 顶层 handle 应保持 HandleDispatch terminator");
            };
            let binder = contract.handled_arms()[0]
                .continuation_binder()
                .expect("Ask handle arm 应发布 continuation binder");

            let routes = callable
                .boundary_map()
                .entries()
                .iter()
                .filter_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some((boundary_site_id(boundary), lowering))
                    }
                    _ => None,
                })
                .map(|(site_id, lowering)| {
                    let layout = query
                        .resume_boundary_operand_layout(
                            callable.step_schema(),
                            site_id,
                            lowering.operand_contract(),
                        )
                        .unwrap_or_else(|err| {
                            panic!(
                                "resume site{} 应可回查 boundary operand contract: {err}",
                                site_id.as_u32()
                            )
                        });
                    let route = layout.contract().underlying_continuation_route();
                    (site_id, route)
                })
                .collect::<Vec<_>>();

            assert_eq!(
                routes
                    .iter()
                    .map(|(site_id, _)| site_id.as_u32())
                    .collect::<Vec<_>>(),
                vec![26, 31, 36, 41]
            );
            for (_site_id, route) in routes {
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
        },
    );
}

#[test]
pub(super) fn refactor_llvm_resume_payload_binding_resolves_boundary_and_state_queries() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("call/resume boundary 的 resumed local/home contract 应成功发布");

            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let call_boundary = site_boundary(main, BoundarySiteKind::Call);
            let call_binding = main
                .frame_schema()
                .resume_payload_binding(call_boundary.boundary_id())
                .expect("call boundary 应发布 resumed local/home binding");
            let call_layout = query
                .resume_payload_binding_layout(main.step_schema(), call_binding)
                .expect("call boundary 应可回查 resumed local/home contract");
            let call_frame_layout = query
                .frame_layout(main.step_schema())
                .expect("callable frame layout 应可查询");
            let call_home_slot = call_binding
                .consumer_frame_slot()
                .expect("call boundary 应发布 frame home slot");

            assert_eq!(call_layout.boundary_id(), call_boundary.boundary_id());
            assert_eq!(call_layout.resume_state(), call_boundary.resume_state());
            assert_eq!(call_layout.consumer_local(), call_binding.consumer_local());
            assert_eq!(
                call_layout.frame_field_index(),
                call_frame_layout.field_index_for_slot(call_home_slot),
            );

            let run = inputs
                .abi_visibility_program
                .callable("run")
                .expect("run callable 应存在");
            let resume_boundary = site_boundary(run, BoundarySiteKind::Resume);
            let resume_binding = run
                .frame_schema()
                .resume_payload_binding(resume_boundary.boundary_id())
                .expect("resume boundary 应发布 resumed local/home binding");
            let resume_layout = query
                .resume_payload_binding_layout(run.step_schema(), resume_binding)
                .expect("resume boundary 应可回查 resumed local/home contract");
            let state_layout = query
                .resume_payload_binding_for_state(run.step_schema(), resume_boundary.resume_state())
                .expect("resume state 应可直接回查 resumed local/home contract");

            assert_eq!(
                resume_layout.consumer_local(),
                resume_binding.consumer_local()
            );
            assert_eq!(
                state_layout.consumer_frame_slot(),
                resume_binding.consumer_frame_slot(),
            );
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("perform/runtime-error 的 resumed local/home contract 应成功发布");

            let fetch = inputs
                .abi_visibility_program
                .callable("fetch")
                .expect("fetch callable 应存在");
            let perform_boundary = site_boundary(fetch, BoundarySiteKind::Perform);
            let perform_binding = fetch
                .frame_schema()
                .resume_payload_binding(perform_boundary.boundary_id())
                .expect("perform boundary 应发布 resumed local/home binding");
            let perform_layout = query
                .resume_payload_binding_layout(fetch.step_schema(), perform_binding)
                .expect("perform boundary 应可回查 resumed local/home contract");
            let fetch_frame_layout = query
                .frame_layout(fetch.step_schema())
                .expect("fetch frame layout 应可查询");
            let perform_home_slot = perform_binding
                .consumer_frame_slot()
                .expect("perform boundary 应发布 frame home slot");

            assert_eq!(perform_layout.boundary_id(), perform_boundary.boundary_id());
            assert_eq!(
                perform_layout.resume_state(),
                perform_boundary.resume_state()
            );
            assert_eq!(
                perform_layout.frame_field_index(),
                fetch_frame_layout.field_index_for_slot(perform_home_slot),
            );

            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let runtime_error_boundary = main
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::RuntimeError { .. }
                    )
                })
                .expect("main 应存在 runtime-error boundary");
            let runtime_error_binding = main
                .frame_schema()
                .resume_payload_binding(runtime_error_boundary.boundary_id())
                .expect("runtime-error boundary 应发布 resumed local/home binding");
            let runtime_error_layout = query
                .resume_payload_binding_layout(main.step_schema(), runtime_error_binding)
                .expect("runtime-error boundary 应可回查 resumed local/home contract");
            let state_layout = query
                .resume_payload_binding_for_state(
                    main.step_schema(),
                    runtime_error_boundary.resume_state(),
                )
                .expect("runtime-error resume state 应可直接回查 resumed local/home contract");

            assert_eq!(
                runtime_error_layout.consumer_local(),
                runtime_error_binding.consumer_local(),
            );
            assert_eq!(
                state_layout.consumer_frame_slot(),
                runtime_error_binding.consumer_frame_slot(),
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_resume_payload_binding_rejects_missing_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let fetch = program.callable("fetch").expect("fetch callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(fetch.frame_schema().slots().to_vec())
                .with_completion_payload_bindings(
                    fetch.frame_schema().completion_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(fetch.step_schema()) {
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
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 resumed local/home contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("resumed local/home contract"),
                "错误消息应指出缺失的是 resumed local/home contract: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_resume_payload_binding_rejects_runtime_error_binding_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let runtime_error_boundary = main
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::RuntimeError { .. }
                    )
                })
                .expect("main 应存在 runtime-error boundary");
            let replacement = main
                .frame_schema()
                .resume_payload_bindings()
                .iter()
                .find(|binding| {
                    binding.boundary_id() != runtime_error_boundary.boundary_id()
                        && binding.resume_state() != runtime_error_boundary.resume_state()
                })
                .copied()
                .expect("应存在可用于构造 drift 的其它 resumed local/home binding");
            let bindings = main
                .frame_schema()
                .resume_payload_bindings()
                .iter()
                .copied()
                .map(|binding| {
                    if binding.boundary_id() == runtime_error_boundary.boundary_id() {
                        LateLoweredResumePayloadBinding::new(
                            binding.boundary_id(),
                            binding.resume_state(),
                            replacement.consumer_local(),
                            replacement.consumer_frame_slot(),
                        )
                    } else {
                        binding
                    }
                })
                .collect();
            let frame_schema = LateLoweredFrameSchema::new(main.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(bindings)
                .with_completion_payload_bindings(
                    main.frame_schema().completion_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
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
                Ok(_) => panic!("runtime-error binding 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("resumed local/home contract")
                    && (message.contains("runtime-error boundary")
                        || message.contains("漂移")
                        || message.contains("冲突")),
                "错误消息应指出 runtime-error resumed local/home contract 漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_completion_payload_contract_resolves_return_state_query() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("completion payload contract 应成功发布到 LLVM query");
            let run = inputs
                .abi_visibility_program
                .callable("run")
                .expect("run callable 应存在");
            let binding = run
                .frame_schema()
                .completion_payload_bindings()
                .iter()
                .find(|binding| !binding.payload_source().is_unit())
                .expect("run(): Int 应发布 non-Unit completion payload binding");
            let layout = query
                .completion_payload_binding_layout(run.step_schema(), binding)
                .expect("return state 应可回查 completion payload contract");
            let state_layout = query
                .completion_payload_binding_for_state(run.step_schema(), binding.return_state())
                .expect("return state 应可直接回查 completion payload contract");
            let frame_layout = query
                .frame_layout(run.step_schema())
                .expect("run frame layout 应可查询");

            assert_eq!(layout.owner_step_schema(), run.step_schema());
            assert_eq!(layout.return_state(), binding.return_state());
            assert_eq!(layout.complete_state(), run.state_graph().complete_state());
            assert_eq!(state_layout.binding(), binding);
            assert_eq!(layout.payload_source(), binding.payload_source());
            assert_eq!(
                inputs
                    .effect_lowered_stage_output
                    .types()
                    .display(layout.payload_source().source_ty())
                    .to_string(),
                "Int"
            );
            assert!(
                !layout.payload_abi().is_elided(),
                "Int completion payload 不应在 ABI 中被 elide"
            );
            if let Some(slot_id) = binding.payload_frame_slot() {
                assert_eq!(
                    layout.frame_field_index(),
                    frame_layout.field_index_for_slot(slot_id),
                );
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_completion_payload_contract_rejects_missing_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let run = program.callable("run").expect("run callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(
                    run.frame_schema().resume_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(run.step_schema()) {
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
                Ok(_) => panic!("缺失 completion payload contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("completion payload contract"),
                "错误消息应指出缺失的是 completion payload contract: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_completion_payload_contract_rejects_source_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let run = program.callable("run").expect("run callable 应存在");
            let drifted_bindings = run
                .frame_schema()
                .completion_payload_bindings()
                .iter()
                .map(|binding| {
                    if binding.payload_source().is_unit() {
                        binding.clone()
                    } else {
                        LateLoweredCompletionPayloadBinding::new(
                            binding.return_state(),
                            binding.complete_state(),
                            LateLoweredCompletionPayloadSource::unit(
                                binding.payload_source().source_ty(),
                            ),
                            binding.payload_frame_slot(),
                        )
                    }
                })
                .collect();
            let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(run.frame_schema().resume_payload_bindings().to_vec())
                .with_completion_payload_bindings(drifted_bindings);
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(run.step_schema()) {
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
                Ok(_) => panic!("completion payload source 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("completion payload source")
                    || message.contains("completion payload frame home"),
                "错误消息应指出 completion payload contract 漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_rejects_ordered_arg_drift() {
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
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        LateLoweredCallBoundaryOperandContract::new(
                                            lowering.operand_contract().source_consumption(),
                                            None,
                                            Vec::new(),
                                        ),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
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
                Ok(_) => panic!("ordered arg drift 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("ordered args")
                    && (message.contains("contract 非法")
                        || message.contains("单一 source")
                        || message.contains("component")),
                "错误消息应指出 ordered args contract 漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_rejects_missing_dynamic_carrier_source() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let call_value = program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                call_value
                    .boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        LateLoweredCallBoundaryOperandContract::new(
                                            lowering.operand_contract().source_consumption(),
                                            None,
                                            lowering.operand_contract().arg_sources().to_vec(),
                                        ),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
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
                    if candidate.body_step_schema() == Some(call_value.step_schema()) {
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
                Ok(_) => panic!("缺失 dynamic carrier source 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("carrier source contract"),
                "错误消息应指出缺失的是 dynamic carrier source contract: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_boundary_operand_contract_rejects_missing_underlying_continuation_route_publication()
 {
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
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Resume(lowering) => {
                                let route = lowering
                                    .operand_contract()
                                    .underlying_continuation_route();
                                let broken_contract =
                                    crate::effect_lowered::ir::LateLoweredResumeBoundaryOperandContract::new(
                                        lowering.operand_contract().source_consumption(),
                                        lowering.operand_contract().continuation_source().clone(),
                                        lowering.operand_contract().arg_sources().to_vec(),
                                        crate::effect_lowered::ir::LateLoweredContinuationRoute::new(
                                            route.continuation_schema(),
                                            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                                                owner_version_key: main.body_version_key().clone(),
                                                owner_continuation_object: main.continuation_object(),
                                                site_id: SiteId::from_raw(999),
                                                arm_ordinal: 0,
                                                handled_case: CaseTag::new(1),
                                            },
                                        ),
                                        lowering
                                            .operand_contract()
                                            .underlying_route_is_compatible_set(),
                                    );
                                LateLoweredBoundaryLowering::Resume(
                                    crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.runtime_error_boundary(),
                                        broken_contract,
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
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
                Ok(_) => {
                    panic!("缺失 underlying continuation route publication 时必须 fail fast")
                }
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("underlying continuation route")
                    || message.contains("wrapper complete projection")
                    || message.contains("handle binder"),
                "错误消息应指出 underlying continuation route publication 漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn refactor_llvm_dynamic_invoke_query_resolves_fun_value_unit_contract() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("fun-value DynamicFallback 应可物化 dynamic invoke contract");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);

            assert_eq!(
                lowering.facts().target_mode(),
                CallTargetMode::DynamicFallback
            );
            let site_id = boundary_site_id(boundary);
            let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                .expect("fun-value boundary 应可回查 dynamic invoke contract")
            else {
                panic!("DynamicFallback fun-value call 应走 dynamic invoke contract");
            };
            assert_eq!(layout.owner_step_schema(), callable.step_schema());
            assert_eq!(layout.site_id(), site_id);
            assert_eq!(layout.target_mode(), CallTargetMode::DynamicFallback);
            assert_eq!(
                layout.return_step_schema(),
                lowering.facts().callee_schema()
            );
            assert_eq!(
                layout.invoke_args_tuple_ty(),
                lowering.facts().invoke_args_tuple_ty()
            );
            assert!(layout.args_abi().is_elided());
            assert_eq!(layout.param_count(), 1);
            match layout.carrier() {
                RefactorDynamicInvokeCarrierLayout::ClosureObject(carrier) => {
                    assert_eq!(carrier.object_ty().count_fields(), 3);
                    assert_eq!(carrier.env_field_index(), 1);
                    assert_eq!(carrier.fn_field_index(), 2);
                    assert!(!carrier.receiver_abi().is_elided());
                }
                other => {
                    panic!("fun-value dynamic invoke 应发布 closure carrier，而不是 {other:?}")
                }
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_callable_carrier_layout_resolves_virtual_candidate_set_contracts() {
    with_fixture_query_result(
        "effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("candidate-set virtual helper 应可物化 dynamic invoke contract");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);

            assert_eq!(lowering.facts().target_mode(), CallTargetMode::CandidateSet);
            let site_id = boundary_site_id(boundary);
            let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                .expect("candidate-set virtual boundary 应可回查 dynamic invoke contract")
            else {
                panic!("CandidateSet virtual call 应走 dynamic invoke contract");
            };
            assert_eq!(layout.target_mode(), CallTargetMode::CandidateSet);
            assert_eq!(layout.param_count(), 1);
            assert!(layout.args_abi().is_elided());
            assert_eq!(
                layout.return_step_schema(),
                lowering.facts().callee_schema()
            );
            match layout.carrier() {
                RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                    assert_eq!(
                        inputs
                            .effect_lowered_stage_output
                            .types()
                            .display(dispatch.receiver_ty())
                            .to_string(),
                        "fixtures.build.Base"
                    );
                    assert_eq!(dispatch.owner_fqn(), "fixtures.build.Base");
                    assert_eq!(dispatch.member_name(), "ping");
                    assert!(!dispatch.receiver_abi().is_elided());
                }
                other => panic!(
                    "virtual CandidateSet 应发布 receiver-dispatch carrier，而不是 {other:?}"
                ),
            }
        },
    );
}

#[test]
pub(super) fn refactor_llvm_dynamic_invoke_query_resolves_non_boundary_virtual_contract() {
    with_fixture_query(
        "effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop",
        |inputs, query, _module| {
            let helper = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let plain = helper
                .plain_abi()
                .expect("NoOutward helper 应保持 plain callable ABI");
            assert!(plain.local_effect_control().is_none());

            let (site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);
            assert!(
                facts.resolved_cases().is_empty(),
                "non-boundary dynamic call 的 resolved cases 应为空"
            );
            assert_eq!(facts.target_mode(), CallTargetMode::CandidateSet);
            assert!(
                plain
                    .call_sites()
                    .iter()
                    .any(|site| site.site_id() == site_id)
            );
            let layout = query
                .plain_callable_layout_by_version_key(helper.body_version_key())
                .expect("NoOutward helper 应发布 plain callable layout");
            assert_eq!(layout.root_fqn(), helper.root_fqn());
            assert!(
                query
                    .callable_layout_by_version_key(helper.body_version_key())
                    .is_err(),
                "NoOutward helper 不应发布 effect-step callable layout"
            );
        },
    );
}
