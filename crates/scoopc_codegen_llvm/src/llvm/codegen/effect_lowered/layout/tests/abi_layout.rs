//! Late-lowered layout binding, payload contract resolution and unit-ABI tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn llvm_layout_binds_pure_direct_entries_without_hir_typestore_fallback() {
    with_fixture_query(
        "effect_lowered_dynamic_entry_publication_emit_llvm.scoop",
        |inputs, query, module| {
            let lambda_root = inputs
                .abi_visibility_program
                .callables()
                .iter()
                .find(|callable| callable.root_fqn().contains("$lambda"))
                .map(|callable| callable.root_fqn().to_string())
                .expect("fixture 应发布 lambda callable shell");
            let roots = vec![
                "fixtures.build.makeClosure".to_string(),
                "fixtures.build.Base.ping".to_string(),
                lambda_root,
            ];

            for root in roots {
                let callable = query
                    .plain_callable_layout_by_root_fqn(&root)
                    .expect("plain callable layout 应存在");
                assert_eq!(
                    callable.direct_entry().param_count(),
                    callable.direct_entry().param_tys().len(),
                    "plain direct entry 形参个数必须来自 P5 plain ABI handoff: {root}"
                );
                assert!(
                    module
                        .get_function(callable.direct_entry().symbol_name())
                        .is_some(),
                    "plain direct entry 应声明普通 LLVM callable symbol: {root}"
                );
                assert!(
                    query.callable_layout_by_root_fqn(&root).is_err(),
                    "NoOutward plain callable 不应发布 effect-step callable layout: {root}"
                );
            }
        },
    );
}

#[test]
pub(super) fn llvm_layout_resolves_unit_case_payload_contract() {
    with_fixture_query(
        "effect_lowered_dynamic_invoke_unit_payload.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .lir_stage_output
                .program()
                .callable("fixtures.build.unitWorker")
                .expect("unitWorker callable 应存在");
            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应存在");
            let case_variant = step_layout
                .case_layout(CaseTag::new(0))
                .expect("case0 layout 应存在")
                .variant();
            let case_payload_layout = query
                .source_value_layout(case_variant.payload_source_ty())
                .expect("case payload source type 应发布 source-type ABI contract");
            let complete_layout = query
                .source_value_layout(step_layout.complete_variant().payload_source_ty())
                .expect("complete payload source type 应发布 source-type ABI contract");

            assert_eq!(case_payload_layout.kind(), SourceAbiLayoutKind::Scalar);
            assert!(case_payload_layout.abi().is_elided());
            assert!(case_payload_layout.fields().is_empty());
            assert!(case_variant.payload_is_elided());
            assert_eq!(case_variant.payload_field_count(), 1);
            assert!(complete_layout.abi().is_elided());
            assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
        },
    );
}

#[test]
pub(super) fn llvm_layout_resolves_tuple_resume_payload_and_answer_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "continuation_resume_surface_named_tuple_and_unit_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("tuple resume fixture 应可发布 source-type ABI contract");
            let pair_surface = inputs
                .abi_visibility_program
                .continuation_objects()
                .iter()
                .flat_map(|object| object.surface_resumes().iter())
                .find(|surface| {
                    inputs
                        .primary_types()
                        .display(surface.resume_tuple_ty())
                        .to_string()
                        == "(Int, String)"
                })
                .expect("fixture 应包含 tuple resume surface");
            let surface_layout = query
                .surface_resume_layout(pair_surface.continuation_schema())
                .expect("surface-resume layout 应可查询");
            let resume_payload_layout = query
                .source_value_layout(surface_layout.resume_tuple_ty())
                .expect("resume tuple source type 应发布 source-type ABI contract");
            let answer_layout = query
                .source_value_layout(surface_layout.answer_ty())
                .expect("resume answer source type 应发布 source-type ABI contract");

            assert_eq!(resume_payload_layout.kind(), SourceAbiLayoutKind::Tuple);
            assert_eq!(resume_payload_layout.fields().len(), 2);
            assert_eq!(resume_payload_layout.abi_field_count(), 2);
            assert_eq!(resume_payload_layout.fields()[0].source_index(), 0);
            assert_eq!(resume_payload_layout.fields()[0].abi_field_index(), Some(0));
            assert_eq!(resume_payload_layout.fields()[1].source_index(), 1);
            assert_eq!(resume_payload_layout.fields()[1].abi_field_index(), Some(1));
            assert!(!resume_payload_layout.fields()[0].is_elided());
            assert!(!resume_payload_layout.fields()[1].is_elided());
            assert_eq!(answer_layout.kind(), SourceAbiLayoutKind::Scalar);
            assert!(answer_layout.abi().is_elided());
        },
    );
}

#[test]
pub(super) fn llvm_layout_rejects_mismatched_source_typestore_before_layout() {
    let inputs = build_fixture_inputs("effect_lowered_step_enum_single_case.scoop");
    let mut source_types = inputs.primary_types().clone();
    let param_ty = source_types.ty_param(TypeParamType {
        name: "SyntheticInvokeArgs".to_string(),
        decl_file: std::path::PathBuf::from("tests/p6_t02i.synthetic"),
        decl_span: dummy_span(),
    });

    with_inputs_query_result_for_source_types(
        inputs,
        move |inputs| {
            let program = &inputs.abi_visibility_program;
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.root_fqn() == "fixtures.build.singleCaseWorker" {
                        clone_callable_with_dynamic_invoke_entry(
                            candidate,
                            LateLoweredDynamicInvokeEntry::new(
                                param_ty,
                                candidate.dynamic_invoke_entry().step_schema(),
                                candidate.dynamic_invoke_entry().entry_state(),
                                candidate.dynamic_invoke_entry().complete_state(),
                            ),
                        )
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
        move |_inputs| source_types,
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => {
                    panic!("mismatched source TypeStore 必须在 physical ABI/layout 入口 fail fast")
                }
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("physical ABI/layout"),
                "错误消息应指出失败发生在 physical ABI/layout verifier: {message}"
            );
            assert!(
                message.contains("handoff TypeStore owner 不一致"),
                "错误消息应指出 LIR facts 与 source TypeStore owner 不一致: {message}"
            );
        },
    );
}

#[test]
pub(super) fn llvm_unit_abi_elides_zero_sized_args_and_resume_payloads() {
    with_fixture_query_result(
        "effect_lowered_dynamic_invoke_unit_payload.scoop",
        unit_worker_program_with_ping_interface,
        |inputs, result, module| {
            let query = result.expect("published unit resume packing 应可物化 ABI");
            let callable = inputs
                .lir_stage_output
                .program()
                .callable("fixtures.build.unitWorker")
                .expect("callable 应存在");
            let callable_layout = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询");
            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应可查询");
            let continuation_object = inputs
                .lir_stage_output
                .program()
                .continuation_object(callable.continuation_object())
                .expect("continuation object 应存在");
            let interface_id = *query
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
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");
            let method_layout = interface_layout
                .method(CaseTag::new(0))
                .expect("case0 method 应存在");
            let surface_resume_schema = continuation_object
                .surface_resumes()
                .iter()
                .find(|surface| surface.case_tag() == CaseTag::new(0))
                .expect("case0 surface resume 应存在")
                .continuation_schema();
            let surface_layout = query
                .surface_resume_layout(surface_resume_schema)
                .expect("surface-resume layout 应可查询");

            assert!(callable_layout.dynamic_entry().args_abi().is_elided());
            assert!(callable_layout.direct_entry().args_abi().is_elided());
            assert_eq!(callable_layout.dynamic_entry().param_count(), 0);
            assert_eq!(callable_layout.direct_entry().param_count(), 0);
            assert!(step_layout.complete_variant().payload_is_elided());
            assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
            assert!(method_layout.resume_payload_abi().is_elided());
            assert_eq!(method_layout.param_count(), 1);
            assert!(surface_layout.resume_payload_abi().is_elided());
            assert_eq!(surface_layout.param_count(), 1);
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(0))
                    .expect("case0 layout 应存在")
                    .variant()
                    .payload_field_count(),
                1
            );
            assert!(
                module
                    .get_function(callable_layout.dynamic_entry().symbol_name())
                    .is_some()
            );
            assert!(module.get_function(surface_layout.symbol_name()).is_some());
        },
    );
}
