use crate::effect_facts::{ConcreteOpKey, ContinuationSchema, ImplPlan, StepSchemaId};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    LateLoweredBodyVersionKey, LateLoweredContinuationContract, LateLoweredStepCase,
    LateLoweredStepType,
};
use crate::llvm::LlvmEmitError;
use crate::stable_id::{
    NoTypeParamResolver, PrivateSymbolMangler, StableCanonicalKey, StableConeKey,
    StableContinuationSchemaKey, StableEffectSchemaKey, canonical_effect_row_text, canonical_list,
    canonical_record, canonical_type_text,
};
use crate::ty::{TypeId, TypeStore};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CanonicalTextKey(String);

impl StableCanonicalKey for CanonicalTextKey {
    fn canonical_text(&self) -> String {
        self.0.clone()
    }
}

pub(super) fn private_name_from_key_text(role: &str, key_text: &str) -> String {
    PrivateSymbolMangler.mangle(role, &CanonicalTextKey(key_text.to_string()))
}

fn private_hash_suffix_from_key_text(role: &str, key_text: &str) -> Result<String, LlvmEmitError> {
    let private_name = private_name_from_key_text(role, key_text);
    private_name
        .rsplit_once("__h")
        .map(|(_, suffix)| suffix.to_string())
        .ok_or_else(|| {
            frontend_error(format!(
                "private name `{private_name}` 缺少 stable hash suffix"
            ))
        })
}

pub(super) fn private_type_name_from_key_text(
    family: &str,
    role: &str,
    key_text: &str,
) -> Result<String, LlvmEmitError> {
    let hash = private_hash_suffix_from_key_text(role, key_text)?;
    Ok(format!("scoop.refactor.{family}__h{hash}"))
}

pub(super) fn callable_version_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    version_key: &LateLoweredBodyVersionKey,
    context: &str,
) -> Result<String, LlvmEmitError> {
    let owner_callable = program
        .callable_by_version_key(version_key)
        .ok_or_else(|| {
            frontend_error(format!(
                "{context} 缺少 owner callable version `{}`",
                version_key.surface_instance().template.fqn
            ))
        })?;
    let surface_instance = owner_callable.stable_instance_key().canonical_text();
    let allowed_row = canonical_effect_row_fragment(
        types,
        version_key.allowed_row(),
        &format!("{context} allowed row"),
    )?;
    let step_schema = owner_callable.body_step_schema();
    let impl_plan = impl_plan_key_text(
        stable_cone_key,
        types,
        program,
        step_schema,
        version_key.impl_plan(),
        &format!("{context} impl plan"),
    )?;
    Ok(canonical_record(
        "callable_version",
        [
            surface_instance,
            allowed_row,
            impl_plan,
            version_key.needs_reentry().to_string(),
        ],
    ))
}

pub(super) fn effect_schema_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    step_type: &LateLoweredStepType,
    context: &str,
) -> Result<String, LlvmEmitError> {
    let owner_key = step_schema_owner_key_text(
        stable_cone_key,
        types,
        program,
        step_type.step_schema(),
        &format!("{context} owner"),
    )?;
    let mut case_fragments = step_type
        .cases()
        .iter()
        .map(|case| {
            step_case_key_text(
                stable_cone_key,
                types,
                program,
                case,
                &format!("{context} case"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    case_fragments.sort();
    let fragments = vec![
        canonical_type_fragment(
            types,
            step_type.invoke_args_tuple_ty(),
            &format!("{context} invoke args tuple"),
        )?,
        canonical_type_fragment(
            types,
            step_type.complete_ty(),
            &format!("{context} complete type"),
        )?,
        canonical_type_fragment(
            types,
            step_type.continuation_obj_ty(),
            &format!("{context} continuation object type"),
        )?,
        canonical_list(&case_fragments),
    ];
    Ok(
        StableEffectSchemaKey::new(&CanonicalTextKey(owner_key), "step_schema", fragments)
            .canonical_text(),
    )
}

pub(super) fn continuation_schema_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    schema: &ContinuationSchema,
    context: &str,
) -> Result<String, LlvmEmitError> {
    continuation_key_text_from_fields(
        stable_cone_key,
        types,
        program,
        schema.resume_tuple_ty(),
        schema.answer_ty(),
        schema.out_step_schema(),
        schema.surface_ty(),
        context,
    )
}

pub(super) fn continuation_contract_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    contract: LateLoweredContinuationContract,
    context: &str,
) -> Result<String, LlvmEmitError> {
    continuation_key_text_from_fields(
        stable_cone_key,
        types,
        program,
        contract.resume_tuple_ty(),
        contract.answer_ty(),
        contract.out_step_schema(),
        contract.surface_ty(),
        context,
    )
}

pub(super) fn effect_transport_box_names(
    types: &TypeStore,
    source_ty: TypeId,
) -> Result<(String, String), LlvmEmitError> {
    let key_text = canonical_record(
        "effect_transport_box",
        [canonical_type_fragment(
            types,
            source_ty,
            &format!("effect transport box t{}", source_ty.as_u32()),
        )?],
    );
    let layout_anchor_name = private_name_from_key_text("refactor_effect_transport_box", &key_text);
    let type_name = private_type_name_from_key_text(
        "EffectTransportBox",
        "refactor_effect_transport_box",
        &key_text,
    )?;
    Ok((type_name, layout_anchor_name))
}

pub(super) fn step_case_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    case: &LateLoweredStepCase,
    context: &str,
) -> Result<String, LlvmEmitError> {
    Ok(canonical_record(
        "step_case",
        [
            concrete_op_key_text(
                stable_cone_key,
                types,
                program,
                case.concrete_op_key(),
                &format!("{context} concrete op"),
            )?,
            canonical_type_fragment(
                types,
                case.payload_tuple_ty(),
                &format!("{context} payload tuple"),
            )?,
            continuation_contract_key_text(
                stable_cone_key,
                types,
                program,
                case.continuation_contract(),
                &format!("{context} continuation contract"),
            )?,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
fn continuation_key_text_from_fields(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    surface_ty: TypeId,
    context: &str,
) -> Result<String, LlvmEmitError> {
    let owner_key = step_schema_owner_key_text(
        stable_cone_key,
        types,
        program,
        out_step_schema,
        &format!("{context} out-step owner"),
    )?;
    let out_step_summary = step_schema_shallow_summary_text(
        stable_cone_key,
        types,
        program,
        out_step_schema,
        &format!("{context} out-step summary"),
    )?;
    let fragments = vec![
        canonical_type_fragment(types, resume_tuple_ty, &format!("{context} resume tuple"))?,
        canonical_type_fragment(types, answer_ty, &format!("{context} answer"))?,
        canonical_record("out_step", [owner_key.clone(), out_step_summary]),
        canonical_type_fragment(types, surface_ty, &format!("{context} surface"))?,
    ];
    Ok(StableContinuationSchemaKey::new(
        &CanonicalTextKey(owner_key),
        "continuation_schema",
        fragments,
    )
    .canonical_text())
}

fn step_schema_owner_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    step_schema: StepSchemaId,
    context: &str,
) -> Result<String, LlvmEmitError> {
    if let Some(callable) = program
        .callables()
        .iter()
        .find(|callable| callable.body_step_schema() == Some(step_schema))
    {
        return callable_version_key_text(
            stable_cone_key,
            types,
            program,
            callable.body_version_key(),
            context,
        );
    }
    Ok(canonical_record(
        "synthetic_step_owner",
        [step_schema_shallow_summary_text(
            stable_cone_key,
            types,
            program,
            step_schema,
            context,
        )?],
    ))
}

fn step_schema_shallow_summary_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    step_schema: StepSchemaId,
    context: &str,
) -> Result<String, LlvmEmitError> {
    let step_type = program.step_type(step_schema).ok_or_else(|| {
        frontend_error(format!(
            "{context} 缺少 step schema {} 对应的 late-lowered step type",
            step_schema.as_u32()
        ))
    })?;
    let mut case_fragments = step_type
        .cases()
        .iter()
        .map(|case| {
            Ok(canonical_record(
                "step_case_shallow",
                [
                    concrete_op_key_text(
                        stable_cone_key,
                        types,
                        program,
                        case.concrete_op_key(),
                        &format!("{context} concrete op"),
                    )?,
                    canonical_type_fragment(
                        types,
                        case.payload_tuple_ty(),
                        &format!("{context} payload tuple"),
                    )?,
                ],
            ))
        })
        .collect::<Result<Vec<_>, LlvmEmitError>>()?;
    case_fragments.sort();
    Ok(canonical_record(
        "step_schema_shallow",
        [
            canonical_type_fragment(
                types,
                step_type.invoke_args_tuple_ty(),
                &format!("{context} invoke args tuple"),
            )?,
            canonical_type_fragment(
                types,
                step_type.complete_ty(),
                &format!("{context} complete type"),
            )?,
            canonical_type_fragment(
                types,
                step_type.continuation_obj_ty(),
                &format!("{context} continuation object type"),
            )?,
            canonical_list(&case_fragments),
        ],
    ))
}

fn impl_plan_key_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    step_schema: Option<StepSchemaId>,
    impl_plan: ImplPlan,
    context: &str,
) -> Result<String, LlvmEmitError> {
    match impl_plan {
        ImplPlan::NoOutward => Ok(canonical_record("impl_plan", ["no_outward".to_string()])),
        ImplPlan::CanonicalFull => Ok(canonical_record(
            "impl_plan",
            ["canonical_full".to_string()],
        )),
        ImplPlan::SingleCase(case_tag) => {
            let step_schema = step_schema.ok_or_else(|| {
                frontend_error(format!(
                    "{context} 的 single-case impl_plan 缺少 owner step schema"
                ))
            })?;
            let step_type = program.step_type(step_schema).ok_or_else(|| {
                frontend_error(format!(
                    "{context} 缺少 step schema {} 对应的 late-lowered step type",
                    step_schema.as_u32()
                ))
            })?;
            let case = step_type.case(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "{context} 的 single-case impl_plan 缺少 case {}",
                    case_tag.as_u32()
                ))
            })?;
            Ok(canonical_record(
                "impl_plan",
                [canonical_record(
                    "single_case",
                    [
                        concrete_op_key_text(
                            stable_cone_key,
                            types,
                            program,
                            case.concrete_op_key(),
                            &format!("{context} concrete op"),
                        )?,
                        canonical_type_fragment(
                            types,
                            case.payload_tuple_ty(),
                            &format!("{context} payload tuple"),
                        )?,
                        continuation_contract_shallow_text(
                            stable_cone_key,
                            types,
                            program,
                            case.continuation_contract(),
                            &format!("{context} continuation contract"),
                        )?,
                    ],
                )],
            ))
        }
    }
}

fn continuation_contract_shallow_text(
    stable_cone_key: &StableConeKey,
    types: &TypeStore,
    program: &LateLoweredProgram,
    contract: LateLoweredContinuationContract,
    context: &str,
) -> Result<String, LlvmEmitError> {
    Ok(canonical_record(
        "continuation_contract_shallow",
        [
            canonical_type_fragment(
                types,
                contract.resume_tuple_ty(),
                &format!("{context} resume tuple"),
            )?,
            canonical_type_fragment(types, contract.answer_ty(), &format!("{context} answer"))?,
            step_schema_shallow_summary_text(
                stable_cone_key,
                types,
                program,
                contract.out_step_schema(),
                &format!("{context} out-step summary"),
            )?,
            canonical_type_fragment(types, contract.surface_ty(), &format!("{context} surface"))?,
        ],
    ))
}

fn concrete_op_key_text(
    _stable_cone_key: &StableConeKey,
    types: &TypeStore,
    _program: &LateLoweredProgram,
    concrete_op: &ConcreteOpKey,
    context: &str,
) -> Result<String, LlvmEmitError> {
    let instance = concrete_op.stable_instance_key().canonical_text();
    let mut family_type_args = concrete_op
        .effect_family()
        .type_args()
        .iter()
        .copied()
        .map(|ty| canonical_type_fragment(types, ty, &format!("{context} effect family arg")))
        .collect::<Result<Vec<_>, _>>()?;
    family_type_args.shrink_to_fit();
    Ok(canonical_record(
        "concrete_op",
        [
            instance,
            canonical_record(
                "effect_family",
                [
                    concrete_op.effect_family().effect_fqn().to_string(),
                    canonical_list(&family_type_args),
                ],
            ),
        ],
    ))
}

fn canonical_type_fragment(
    types: &TypeStore,
    ty: TypeId,
    context: &str,
) -> Result<String, LlvmEmitError> {
    canonical_type_text(types, ty, &NoTypeParamResolver).map_err(|err| {
        frontend_error(format!(
            "{context} 编码 canonical type text 失败（t{}）: {err}",
            ty.as_u32()
        ))
    })
}

fn canonical_effect_row_fragment(
    types: &TypeStore,
    row: &crate::ty::EffectRow,
    context: &str,
) -> Result<String, LlvmEmitError> {
    canonical_effect_row_text(types, row, &NoTypeParamResolver)
        .map_err(|err| frontend_error(format!("{context} 编码 canonical effect row 失败: {err}")))
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::effect_lowered::ir::{LateLoweredCallable, LateLoweredPlainCallable};
    use crate::mir::{InstanceKey, TemplateKey};
    use crate::span::Span;
    use crate::stable_id::{
        NoTypeParamResolver, StableDefKey, StableDefNamespace, StableInstanceKey, StableTemplateKey,
    };

    fn overloaded_instance(
        source_path: &str,
        decl_span: Span,
        builtins: crate::ty::BuiltinTypes,
    ) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: "demo.pick".to_string(),
                source_path: PathBuf::from(source_path),
                decl_span,
            },
            type_args: vec![builtins.int],
            eff_args: Vec::new(),
        }
    }

    fn overloaded_stable_instance(
        cone: &StableConeKey,
        types: &TypeStore,
        instance: &InstanceKey,
        signature_key: &str,
    ) -> StableInstanceKey {
        StableInstanceKey::from_type_arguments(
            StableTemplateKey::new(StableDefKey::new(
                cone.clone(),
                StableDefNamespace::Fun,
                &instance.template.fqn,
                "generic_fun",
                Some(signature_key.to_string()),
            )),
            types,
            &instance.type_args,
            &instance.eff_args,
            &NoTypeParamResolver,
        )
        .expect("overloaded instance 应可构造 stable instance key")
    }

    #[test]
    fn callable_version_key_text_distinguishes_overloaded_instances_with_same_type_args() {
        let stable_cone_key = StableConeKey::new("demo", "0.1.0");
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let left_instance =
            overloaded_instance("/tmp/root-a/demo.scoop", Span::new(1, 10), builtins);
        let right_instance =
            overloaded_instance("/tmp/root-b/demo.scoop", Span::new(20, 30), builtins);
        let left_stable =
            overloaded_stable_instance(&stable_cone_key, &types, &left_instance, "sig$arity1");
        let right_stable =
            overloaded_stable_instance(&stable_cone_key, &types, &right_instance, "sig$arity2");
        let left_version = LateLoweredBodyVersionKey::new(
            left_instance,
            crate::ty::EffectRow::pure(),
            ImplPlan::NoOutward,
            false,
        );
        let right_version = LateLoweredBodyVersionKey::new(
            right_instance,
            crate::ty::EffectRow::pure(),
            ImplPlan::NoOutward,
            false,
        );
        let plain = LateLoweredPlainCallable::new(
            builtins.unit,
            vec![builtins.int],
            builtins.int,
            Vec::new(),
            Vec::new(),
            None,
        );
        let program = LateLoweredProgram::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![
                LateLoweredCallable::new_plain(
                    "demo.pick::<Int>$overload$left".to_string(),
                    left_stable,
                    left_version.clone(),
                    Vec::new(),
                    plain.clone(),
                ),
                LateLoweredCallable::new_plain(
                    "demo.pick::<Int>$overload$right".to_string(),
                    right_stable,
                    right_version.clone(),
                    Vec::new(),
                    plain,
                ),
            ],
        );

        let left_key = callable_version_key_text(
            &stable_cone_key,
            &types,
            &program,
            &left_version,
            "left callable",
        )
        .expect("left overload callable 应可构造 callable version key");
        let right_key = callable_version_key_text(
            &stable_cone_key,
            &types,
            &program,
            &right_version,
            "right callable",
        )
        .expect("right overload callable 应可构造 callable version key");

        assert_ne!(
            left_key, right_key,
            "同名 overloaded generic 的同型实例必须保留不同的 callable version key"
        );
        assert!(!left_key.contains("/tmp/root-a"));
        assert!(!right_key.contains("/tmp/root-b"));
    }
}
