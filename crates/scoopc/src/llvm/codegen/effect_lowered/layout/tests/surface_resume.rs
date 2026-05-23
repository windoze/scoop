//! Surface-resume layout / dispatch / wrapper-completion tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn llvm_surface_resume_layout_keeps_shared_schema_multi_case_object_publications() {
    with_fixture_query_result(
        "effect_lowered_step_enum_single_case.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("shared-schema fixture 应可物化 surface-resume ABI");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("worker step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("worker c0 应存在")
                .continuation_schema();
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let surface_layout = query
                .surface_resume_layout(shared_schema)
                .expect("shared schema surface-resume layout 应可查询");
            let bindings = continuation_layout
                .surface_resume_bindings(shared_schema)
                .expect("object-side shared schema surface publication 应可查询");

            assert_eq!(
                surface_layout.dispatch_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(bindings.len(), 2);
            assert!(bindings.iter().any(|binding| {
                binding.case_tag() == CaseTag::new(0)
                    && binding.reachability()
                        == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Reachable
            }));
            assert!(bindings.iter().any(|binding| {
                binding.case_tag() == CaseTag::new(1)
                    && binding.reachability()
                        == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Unreachable
            }));
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_layout_resolves_resume_site_contracts() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("resume fixture 应可物化 surface-resume ABI");
            let mut checked_resume_site = false;
            for callable in inputs.lir_stage_output.program().callables() {
                if !callable.has_control_body() {
                    continue;
                }
                for boundary in callable.boundary_map().entries() {
                    let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                    else {
                        continue;
                    };
                    let facts = lowering.facts();
                    let surface_layout = query
                        .surface_resume_layout(facts.continuation_schema())
                        .expect("ResumeSiteEffectFacts 所需的 surface-resume layout 应已发布");

                    assert_eq!(
                        surface_layout.continuation_schema(),
                        facts.continuation_schema()
                    );
                    assert_eq!(surface_layout.resume_tuple_ty(), facts.resume_tuple_ty());
                    assert_eq!(surface_layout.answer_ty(), facts.answer_ty());
                    assert_eq!(surface_layout.return_step_schema(), facts.out_step_schema());
                    assert_eq!(surface_layout.param_count(), 2);
                    assert!(
                        !surface_layout.resume_payload_abi().is_elided(),
                        "Int resume payload 不应被零载荷退化"
                    );
                    assert!(
                        module.get_function(surface_layout.symbol_name()).is_some(),
                        "surface-resume symbol 应被声明到 module 中"
                    );
                    assert_eq!(
                        surface_layout.dispatch_source_kind(),
                        crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
                    );
                    checked_resume_site = true;
                }
            }
            assert!(
                checked_resume_site,
                "fixture 应至少包含一个 resume boundary"
            );
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_layout_rejects_missing_published_contract() {
    with_fixture_query_result(
        "effect_lowered_dynamic_invoke_unit_payload.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.unitWorker")
                .expect("callable 应存在");
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == callable.continuation_object() {
                        clone_continuation_object_with_surface_resumes(candidate, Vec::new())
                    } else {
                        candidate.clone()
                    }
                })
                .collect::<Vec<_>>();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 published surface-resume contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("surface-resume 发布"),
                "错误消息应指出缺失的是 surface-resume contract: {message}"
            );
            assert!(
                message.contains("owner step schema"),
                "错误消息应指出缺失 contract 所属的 owner step schema: {message}"
            );
            assert!(
                message.contains("continuation schema k"),
                "错误消息应指出缺失的 continuation schema: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_resolves_object_method_target() {
    with_fixture_query_result(
        "effect_lowered_step_enum_single_case.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("shared schema 应可发布 owner dispatch query");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("worker step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("worker c0 应存在")
                .continuation_schema();
            let surface_layout = query
                .surface_resume_layout(shared_schema)
                .expect("surface-resume layout 应可查询");
            let dispatch = query
                .surface_resume_dispatch_layout(shared_schema)
                .expect("owner dispatch contract 应可查询");

            assert_eq!(dispatch.continuation_schema(), shared_schema);
            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(dispatch.method_targets().len(), 1);

            let lookup = dispatch.method_targets()[0];
            assert_eq!(lookup.continuation_object(), callable.continuation_object());
            let continuation_layout = query
                .continuation_layout(lookup.continuation_object())
                .expect("continuation layout 应可查询");
            assert_eq!(
                continuation_layout.field_index_for_packing(lookup.packing_interface_id()),
                Some(lookup.packing_field_index())
            );
            let method_layout = query
                .surface_resume_method_layout(lookup)
                .expect("surface-resume packing method layout 应可直接查询");
            assert_eq!(lookup.vtable_index(), method_layout.vtable_index());
            assert_eq!(
                method_layout.return_step_schema(),
                surface_layout.return_step_schema()
            );

            match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(
                        trampoline.owner_root_fqn(),
                        "fixtures.build.singleCaseWorker"
                    );
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(trampoline.resume_boundary_sites().is_empty());
                    assert!(trampoline.handle_binder_routes().is_empty());
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared schema object-method fixture 不应是 unreachable dispatch")
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("shared schema object-method fixture 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_resolves_handle_binder_owner_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("handle-binder schema 应可发布 owner trampoline query");
            let callable = inputs
                .abi_visibility_program
                .callable("executeCase")
                .expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let binder = contract
                .handled_arms()
                .iter()
                .find_map(|arm| arm.continuation_binder())
                .expect("fixture 应至少包含一个 continuation binder");
            let dispatch = query
                .surface_resume_dispatch_layout(binder.continuation_schema())
                .expect("handle-binder schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
            assert!(dispatch.method_targets().is_empty());
            match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(trampoline.owner_root_fqn(), "executeCase");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(trampoline.resume_boundary_sites().is_empty());
                    assert_eq!(trampoline.handle_binder_routes().len(), 1);
                    assert_eq!(trampoline.handle_binder_routes()[0].site_id(), site_id);
                    assert_eq!(trampoline.handle_binder_routes()[0].arm_ordinal(), 0);
                    assert_eq!(
                        trampoline.handle_binder_routes()[0].handled_case(),
                        CaseTag::new(0)
                    );
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("handle-binder-only schema 不应是 unreachable dispatch")
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("handle-binder-only schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_resolves_multi_site_resume_owner_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("multi-resume-site schema 应可发布 owner trampoline query");
            let callable = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let resume_lowering = callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                    _ => None,
                })
                .expect("fixture 应至少包含一个 resume boundary");
            let resume_schema = resume_lowering.facts().continuation_schema();
            let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                handle_state.terminator()
            else {
                panic!("main 顶层 handle 应保持 HandleDispatch terminator");
            };
            let binder = contract.handled_arms()[0]
                .continuation_binder()
                .expect("Ask handle arm 应发布 continuation binder");
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("resume schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
            );
            assert!(dispatch.method_targets().is_empty());
            match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    let sites = trampoline
                        .resume_boundary_sites()
                        .iter()
                        .map(|site_id| site_id.as_u32())
                        .collect::<Vec<_>>();
                    assert_eq!(trampoline.owner_root_fqn(), "main");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert_eq!(sites, vec![33, 38, 43, 48]);
                    assert!(!trampoline.handle_binder_routes().is_empty());
                    let projection = trampoline.wrapper_projection().expect(
                        "shared wrapper schema 应发布 owner-step -> wrapper-step projection",
                    );
                    let outward = projection
                        .outward_cases()
                        .first()
                        .expect("shared wrapper projection 应至少包含一个 outward case");
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
                        outward.owner_case_tag().as_u32(),
                        2,
                        "fixture 应把 owner runtime-error case 投影回 wrapper c0"
                    );
                    assert_eq!(outward.wrapper_case_tag().as_u32(), 0);
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("resume-boundary-only schema 不应是 unreachable dispatch")
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("single-owner resume schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_resolves_cross_owner_wrapper_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "continuation_escape_binder_resume_effect_row_runtime_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("cross-owner wrapper schema 应可发布 owner dispatch query");
            let entry = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find(|entry| {
                    let Some(projection) = entry.wrapper_projection() else {
                        return false;
                    };
                    let Some((underlying_owner, underlying_object)) =
                        surface_resume_publication_owner_identity(
                            projection.underlying_route().publication(),
                        )
                    else {
                        return false;
                    };
                    entry.publications().iter().any(|publication| {
                        matches!(
                            publication,
                            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                                owner_version_key,
                                owner_continuation_object,
                                ..
                            } if owner_version_key != underlying_owner
                                || *owner_continuation_object != underlying_object
                        )
                    })
                })
                .expect("fixture 应发布跨 owner 的 wrapper surface-resume schema");
            let dispatch = query
                .surface_resume_dispatch_layout(entry.continuation_schema())
                .expect("cross-owner wrapper schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
            );
            let ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) =
                dispatch.target()
            else {
                panic!("cross-owner fixture 应发布单一 underlying owner trampoline")
            };
            assert_eq!(trampoline.owner_root_fqn(), "start");
            assert!(
                trampoline.resume_boundary_sites().is_empty(),
                "跨 owner wrapper trampoline 使用 underlying handle binder，不应要求 wrapper owner 的 resume site"
            );
            assert_eq!(trampoline.handle_binder_routes().len(), 1);
            assert!(
                trampoline.wrapper_projection().is_some(),
                "跨 owner wrapper trampoline 必须携带 owner-step -> wrapper-step 投影"
            );
            assert!(module.get_function(trampoline.symbol_name()).is_some());
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_resolves_multi_owner_trampolines() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("multi-owner schema 应可发布 owner dispatch query");
            let entry = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find(|entry| entry.wrapper_projections().len() >= 2)
                .expect("fixture 应发布 owner-aware wrapper projections");
            let dispatch = query
                .surface_resume_dispatch_layout(entry.continuation_schema())
                .expect("multi-owner schema 的 owner dispatch contract 应可查询");

            assert_eq!(entry.wrapper_projections().len(), 2);
            let ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets) =
                dispatch.target()
            else {
                panic!("multi-owner schema 应发布多个 owner trampoline target");
            };
            let roots = targets
                .iter()
                .map(|target| target.owner_root_fqn().to_string())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                roots,
                [
                    "run_direct_indirect_direct".to_string(),
                    "run_indirect_direct".to_string(),
                ]
                .into_iter()
                .collect::<BTreeSet<_>>()
            );
            for target in targets {
                assert!(
                    target.wrapper_projection().is_some(),
                    "每个 owner trampoline 都必须携带 owner-specific wrapper projection: {}",
                    target.owner_root_fqn()
                );
                assert!(
                    module.get_function(target.symbol_name()).is_some(),
                    "owner trampoline symbol 应声明到 module: {}",
                    target.symbol_name()
                );
            }
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_rejects_missing_wrapper_projection_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
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
                .expect("fixture 应至少包含一个 resume boundary schema");
            let inventory = program
                .surface_resume_dispatch_inventory()
                .iter()
                .map(|entry| {
                    LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                        entry.continuation_schema(),
                        entry.contract(),
                        entry.source_kind(),
                        entry.publications().to_vec(),
                        if entry.continuation_schema() == resume_schema {
                            None
                        } else {
                            entry.wrapper_projection().cloned()
                        },
                    )
                })
                .collect::<Vec<_>>();
            program.with_surface_resume_dispatch_inventory(inventory)
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 shared wrapper projection contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("owner-step -> wrapper-step projection contract"),
                "错误消息应指出缺失的是 shared wrapper projection contract: {message}"
            );
            assert!(
                message.contains("underlying route k3"),
                "错误消息应指出缺失投影所依赖的 underlying route: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_wrapper_completion_resolves_payload_source() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("ABI materialization 应成功");
            let callable = inputs
                .lir_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
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
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("shared wrapper dispatch 应可查询");

            let trampoline = match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    trampoline.as_ref()
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets)
                    if targets.len() == 1 =>
                {
                    &targets[0]
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("该 fixture 应只有一个 owner trampoline")
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared wrapper schema 应发布 owner trampoline")
                }
            };
            let projection = trampoline
                .wrapper_projection()
                .expect("shared wrapper schema 应发布 wrapper projection");

            assert_eq!(projection.complete().owner_answer_ty().as_u32(), 2);
            assert_eq!(projection.complete().wrapper_answer_ty().as_u32(), 5);
            assert!(matches!(
                projection.complete().payload_source(),
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(
                    LateLoweredCompletionPayloadSource::Operand(source)
                ) if source.source_ty() == projection.complete().wrapper_answer_ty()
                    && matches!(source.value(), LateLoweredOperandValueSource::Local(_))
            ));
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_wrapper_completion_uses_owner_complete_for_matching_answer_type()
{
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("ABI materialization 应成功");
            let resume_schema = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find_map(|entry| {
                    let projection = entry.wrapper_projection()?;
                    (projection.complete().owner_answer_ty()
                        == projection.complete().wrapper_answer_ty())
                    .then_some(entry.continuation_schema())
                })
                .expect("fixture 应包含 owner/wrapper answer type 相同的 wrapper projection");
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("shared wrapper dispatch 应可查询");

            let trampoline = match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    trampoline.as_ref()
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets)
                    if targets.len() == 1 =>
                {
                    &targets[0]
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("该 fixture 应只有一个 owner trampoline")
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared wrapper schema 应发布 owner trampoline")
                }
            };
            let projection = trampoline
                .wrapper_projection()
                .expect("shared wrapper schema 应发布 wrapper projection");

            assert_eq!(
                projection.complete().owner_answer_ty(),
                projection.complete().wrapper_answer_ty()
            );
            assert!(matches!(
                projection.complete().payload_source(),
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty }
                    if *answer_ty == projection.complete().wrapper_answer_ty()
            ));
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_wrapper_completion_rejects_type_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
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
                .expect("fixture 应至少包含一个 resume boundary schema");
            let inventory = program
                .surface_resume_dispatch_inventory()
                .iter()
                .map(|entry| {
                    let wrapper_projection = if entry.continuation_schema() == resume_schema {
                        entry.wrapper_projection().map(|projection| {
                            LateLoweredSurfaceResumeWrapperProjection::new(
                                projection.underlying_route().clone(),
                                projection.owner_step_schema(),
                                projection.wrapper_step_schema(),
                                LateLoweredSurfaceResumeWrapperCompleteProjection::new(
                                    projection.complete().owner_answer_ty(),
                                    projection.complete().wrapper_answer_ty(),
                                    LateLoweredSurfaceResumeWrapperCompletePayloadSource::wrapper_payload(
                                        LateLoweredCompletionPayloadSource::unit(
                                            projection.complete().wrapper_answer_ty(),
                                        ),
                                    ),
                                ),
                                projection.outward_cases().to_vec(),
                            )
                        })
                    } else {
                        entry.wrapper_projection().cloned()
                    };
                    LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                        entry.continuation_schema(),
                        entry.contract(),
                        entry.source_kind(),
                        entry.publications().to_vec(),
                        wrapper_projection,
                    )
                })
                .collect::<Vec<_>>();
            program.with_surface_resume_dispatch_inventory(inventory)
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => {
                    panic!("non-Unit wrapper answer 的 Unit payload source 必须 fail fast")
                }
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("wrapper complete payload")
                    || message.contains("wrapper-step projection contract 漂移"),
                "错误消息应指出 wrapper complete payload contract 漂移: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_rejects_missing_internal_method_target() {
    with_fixture_query_result(
        "effect_lowered_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == callable.continuation_object() {
                        clone_continuation_object_with_methods(candidate, Vec::new())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 internal method target 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("ContinuationObjectMethod"),
                "错误消息应指出 source kind 与 method target 缺失的关系: {message}"
            );
            assert!(
                message.contains("reachable internal method target"),
                "错误消息应指出缺失的是 reachable internal method target: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_keeps_multi_method_lookup_set() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("多 method 共享 schema 应可发布 owner dispatch contract");
            let callable = inputs
                .abi_visibility_program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("callValue step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("c0 应存在")
                .continuation_schema();
            let dispatch = query
                .surface_resume_dispatch_layout(shared_schema)
                .expect("多 method 共享 schema 的 dispatch contract 应可查询");
            let method_keys = dispatch
                .method_targets()
                .iter()
                .map(|lookup| {
                    (
                        lookup.packing_interface_id().as_u32(),
                        lookup.case_tag().as_u32(),
                    )
                })
                .collect::<Vec<_>>();

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(method_keys, vec![(0, 0), (1, 1)]);
            match dispatch.target() {
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(trampoline.owner_root_fqn(), "sample.callValue");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                ContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("多 method 共享 schema 不应是 unreachable dispatch")
                }
                ContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("多 method 单 owner schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
pub(super) fn llvm_surface_resume_dispatch_layout_rejects_multi_object_publication() {
    with_fixture_query_result(
        "effect_lowered_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let next_object_id = ContinuationObjectId::new(
                program
                    .continuation_objects()
                    .iter()
                    .map(|object| object.object_id().as_u32())
                    .max()
                    .map(|raw| raw.saturating_add(1))
                    .unwrap_or(0),
            );
            let duplicated_object = program
                .continuation_object(callable.continuation_object())
                .map(|object| clone_continuation_object_with_id(object, next_object_id))
                .expect("continuation object 应存在");
            let mut continuation_objects = program.continuation_objects().to_vec();
            continuation_objects.push(duplicated_object);

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("多 object 共享同一 schema 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("多个 continuation object 共享同一 schema"),
                "错误消息应指出 multi-object publication 歧义: {message}"
            );
        },
    );
}
