use std::collections::{BTreeMap, HashMap};

use crate::ast;
use crate::mir::{
    File as MirFile, FunDecl as MirFunDecl, InstanceKey, Item as MirItem, MaterializedMir,
    TemplateKey,
};
use crate::resolve::{FunOverload, Index};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{
    EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore, ValueTypeKind,
};
use crate::typecheck::{TypeEnv, TypeLowering, TypeSymbol};

use super::{
    BodyEffectFacts, CallableEffectFacts, CaseSet, CaseTag, ConcreteOpKey, ContinuationSchema,
    ContinuationSchemaId, EffectFactsError, ImplPlan, MaterializedEffectFacts, MirSnapshotBinding,
    StepCaseFact, StepSchema, StepSchemaId,
};

/// 从 canonical materialized MIR snapshot 生成 P4 facts 容器。
#[derive(Debug)]
pub struct MaterializedEffectFactsBuilder<'a> {
    session: &'a Session,
    source: &'a SourceFile,
    materialized: &'a mut MaterializedMir,
}

#[derive(Debug, Clone)]
struct CallableSeed {
    key: InstanceKey,
    declared_row: EffectRow,
    invoke_arg_components: Vec<TypeId>,
    complete_ty: TypeId,
}

#[derive(Debug, Clone)]
struct StepCaseSeed {
    sort_key: String,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    resume_tuple_ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ContinuationSchemaKey {
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    surface_ty: TypeId,
}

#[derive(Debug)]
struct ConcreteEffectOpContract {
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    resume_tuple_ty: TypeId,
}

#[derive(Debug)]
struct EffectFactsTypeContext {
    index: Index,
    env: TypeEnv,
}

impl<'a> MaterializedEffectFactsBuilder<'a> {
    pub fn from_materialized_snapshot(
        session: &'a Session,
        source: &'a SourceFile,
        materialized: &'a mut MaterializedMir,
    ) -> Self {
        Self {
            session,
            source,
            materialized,
        }
    }

    pub fn build(self) -> Result<MaterializedEffectFacts, EffectFactsError> {
        let snapshot_binding = {
            let pass_view = self.materialized.pass_view();
            MirSnapshotBinding::from_pass_view(&pass_view)
        };
        let type_ctx = EffectFactsTypeContext::build(self.session, self.source)?;
        let callable_seeds = collect_callable_seeds(self.materialized, &type_ctx.index)?;

        let mut step_schemas = BTreeMap::new();
        let mut continuation_schemas = BTreeMap::new();
        let mut continuation_schema_ids = BTreeMap::new();
        let mut callable_facts = HashMap::with_capacity(callable_seeds.len());
        let mut bodies = HashMap::with_capacity(callable_seeds.len());
        let mut next_continuation_schema_id = 0u32;
        let types = &mut self.materialized.types;

        for (callable_index, seed) in callable_seeds.into_iter().enumerate() {
            let step_schema_id = StepSchemaId::new(callable_index as u32);
            let invoke_args_tuple_ty =
                canonical_tuple_carrier_ty(types, &seed.invoke_arg_components);
            let continuation_obj_ty = continuation_object_ty(types, &seed.key);
            let case_seeds = type_ctx.step_case_seeds(types, &seed.declared_row)?;
            let answer_ty = seed.complete_ty;

            let mut cases = Vec::with_capacity(case_seeds.len());
            for (case_index, case_seed) in case_seeds.into_iter().enumerate() {
                let case_tag = CaseTag::new(case_index as u32);
                let surface_ty = continuation_surface_ty(
                    types,
                    case_seed.resume_tuple_ty,
                    answer_ty,
                    &seed.declared_row,
                );
                let continuation_key = ContinuationSchemaKey {
                    resume_tuple_ty: case_seed.resume_tuple_ty,
                    answer_ty,
                    out_step_schema: step_schema_id,
                    surface_ty,
                };
                let continuation_schema_id =
                    if let Some(id) = continuation_schema_ids.get(&continuation_key) {
                        *id
                    } else {
                        let id = ContinuationSchemaId::new(next_continuation_schema_id);
                        next_continuation_schema_id += 1;
                        continuation_schemas.insert(
                            id,
                            ContinuationSchema::new(
                                continuation_key.resume_tuple_ty,
                                continuation_key.answer_ty,
                                continuation_key.out_step_schema,
                                continuation_key.surface_ty,
                            ),
                        );
                        continuation_schema_ids.insert(continuation_key.clone(), id);
                        id
                    };

                cases.push(StepCaseFact::new(
                    case_tag,
                    case_seed.concrete_op_key,
                    case_seed.payload_tuple_ty,
                    continuation_schema_id,
                ));
            }

            let step_schema = StepSchema::new(
                invoke_args_tuple_ty,
                seed.complete_ty,
                continuation_obj_ty,
                cases,
            );
            let resolved_outward_cases = CaseSet::new(
                step_schema_id,
                step_schema
                    .cases()
                    .iter()
                    .map(|case| case.case_tag())
                    .collect(),
            );
            let needs_reentry = !resolved_outward_cases.is_empty();
            let impl_plan = match resolved_outward_cases.tags() {
                [] => ImplPlan::NoOutward,
                [single] => ImplPlan::SingleCase(*single),
                _ => ImplPlan::CanonicalFull,
            };

            callable_facts.insert(
                seed.key.clone(),
                CallableEffectFacts::new(
                    seed.declared_row,
                    invoke_args_tuple_ty,
                    step_schema_id,
                    resolved_outward_cases,
                    needs_reentry,
                    impl_plan,
                ),
            );
            bodies.insert(seed.key, BodyEffectFacts::default());
            step_schemas.insert(step_schema_id, step_schema);
        }

        Ok(MaterializedEffectFacts::new(
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        ))
    }
}

impl EffectFactsTypeContext {
    fn build(session: &Session, source: &SourceFile) -> Result<Self, EffectFactsError> {
        let sources = vec![source.clone()];
        let index = session.build_top_level_index(&sources)?;

        let mut parsed = session.parse(source)?;
        let source_refs = vec![source];
        let mut ast_refs = vec![&mut parsed];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )?;

        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)
            .map_err(|error| EffectFactsError::TypeEnv(Box::new(error)))?;
        env.extend_from_file(source, &parsed, &index)
            .map_err(|error| EffectFactsError::TypeEnv(Box::new(error)))?;
        Ok(Self { index, env })
    }

    fn step_case_seeds(
        &self,
        types: &mut TypeStore,
        declared_row: &EffectRow,
    ) -> Result<Vec<StepCaseSeed>, EffectFactsError> {
        let mut effect_terms = declared_row.terms.clone();
        effect_terms.sort_by(|lhs, rhs| {
            types
                .display(*lhs)
                .to_string()
                .cmp(&types.display(*rhs).to_string())
                .then_with(|| lhs.cmp(rhs))
        });

        let mut cases = Vec::new();
        for effect_ty in effect_terms {
            let effect_display = types.display(effect_ty).to_string();
            let (effect_fqn, effect_type_args) = lower_effect_nominal_identity(types, effect_ty)?;
            let effect_sym = self.env.type_symbol(&effect_fqn).ok_or_else(|| {
                EffectFactsError::MissingEffectTypeSymbol {
                    effect_fqn: effect_fqn.clone(),
                }
            })?;
            let mut ops = effect_op_overloads(&self.index, &effect_fqn);
            ops.sort_by(|(lhs_fqn, _), (rhs_fqn, _)| lhs_fqn.cmp(rhs_fqn));

            for (op_fqn, op) in ops {
                let contract = self.lower_effect_op_contract(
                    types,
                    &effect_fqn,
                    &effect_type_args,
                    &op_fqn,
                    &op,
                    effect_sym,
                )?;
                let sort_key = format!(
                    "{effect_display}::{op_fqn}::{}::{}",
                    types.display(contract.payload_tuple_ty),
                    types.display(contract.resume_tuple_ty)
                );
                cases.push(StepCaseSeed {
                    sort_key,
                    concrete_op_key: contract.concrete_op_key,
                    payload_tuple_ty: contract.payload_tuple_ty,
                    resume_tuple_ty: contract.resume_tuple_ty,
                });
            }
        }

        cases.sort_by(|lhs, rhs| lhs.sort_key.cmp(&rhs.sort_key));
        Ok(cases)
    }

    fn lower_effect_op_contract(
        &self,
        types: &mut TypeStore,
        effect_fqn: &str,
        effect_type_args: &[TypeId],
        op_fqn: &str,
        op: &FunOverload,
        effect_sym: &TypeSymbol,
    ) -> Result<ConcreteEffectOpContract, EffectFactsError> {
        if effect_sym.type_param_names.len() != effect_type_args.len() {
            return Err(EffectFactsError::EffectTypeArgArityMismatch {
                effect_fqn: effect_fqn.to_string(),
                expected: effect_sym.type_param_names.len(),
                found: effect_type_args.len(),
            });
        }

        let mut type_bindings = Vec::new();
        let mut concrete_key_type_args = Vec::new();
        for type_param in &op.sig.type_params {
            let ty = types.ty_param(TypeParamType {
                name: type_param.name.clone(),
                decl_file: op.symbol.decl_file.clone(),
                decl_span: type_param.name_span,
            });
            type_bindings.push((type_param.name.clone(), ty));
            concrete_key_type_args.push(ty);
        }
        for (name, actual_ty) in effect_sym
            .type_param_names
            .iter()
            .zip(effect_type_args.iter().copied())
        {
            type_bindings.push((name.clone(), actual_ty));
            concrete_key_type_args.push(actual_ty);
        }

        let decl_source = self.env.source(&op.symbol.decl_file).ok_or_else(|| {
            EffectFactsError::MissingDeclFileContext {
                path: op.symbol.decl_file.display().to_string(),
            }
        })?;
        let file_ctx = self
            .env
            .file_type_context(&op.symbol.decl_file)
            .ok_or_else(|| EffectFactsError::MissingDeclFileContext {
                path: op.symbol.decl_file.display().to_string(),
            })?;

        let (payload_component_tys, resume_tuple_ty) = {
            let builtins = types.intern_builtins();
            let mut lower = TypeLowering::new_with_ctx(
                decl_source,
                &self.index,
                &self.env,
                types,
                builtins,
                file_ctx.pkg_prefix.clone(),
                file_ctx.imports.clone(),
            );
            let mut payload_component_tys =
                Vec::with_capacity(op.sig.params.len() + usize::from(op.sig.receiver.is_some()));

            if let Some(receiver_ref) = &op.sig.receiver {
                payload_component_tys.push(
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            receiver_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                );
            }

            for param in &op.sig.params {
                let Some(param_ty_ref) = &param.ty else {
                    return Err(EffectFactsError::MalformedEffectOpSignature {
                        op_fqn: op_fqn.to_string(),
                        detail: "missing parameter type",
                    });
                };
                payload_component_tys.push(
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            param_ty_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                );
            }

            let resume_tuple_ty = match &op.sig.return_ty {
                Some(return_ty_ref) => lower
                    .lower_type_ref_in_decl_file_with_scopes(
                        &op.symbol.decl_file,
                        type_bindings.clone(),
                        std::iter::empty::<(String, EffectRow)>(),
                        return_ty_ref,
                    )
                    .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                None => builtins.unit,
            };
            (payload_component_tys, resume_tuple_ty)
        };

        Ok(ConcreteEffectOpContract {
            concrete_op_key: ConcreteOpKey::new(InstanceKey {
                template: TemplateKey {
                    fqn: op_fqn.to_string(),
                    source_path: op.symbol.decl_file.clone(),
                    decl_span: op.symbol.span,
                },
                type_args: concrete_key_type_args,
                eff_args: Vec::new(),
            }),
            payload_tuple_ty: canonical_tuple_carrier_ty(types, &payload_component_tys),
            resume_tuple_ty,
        })
    }
}

fn collect_callable_seeds(
    materialized: &MaterializedMir,
    index: &Index,
) -> Result<Vec<CallableSeed>, EffectFactsError> {
    let pass_view = materialized.pass_view();
    let mut seeds = Vec::with_capacity(pass_view.len());
    for family in pass_view.instances() {
        // effect-op 声明与 compiler-owned `Continuation.resume` surface contract 都会由更专门的
        // metadata/schema 路径承载；它们不应在 P4 被误当成“普通 callable body shell”参与求解。
        if template_decl_is_effect_op(index, &family.key().template)
            || template_decl_is_compiler_owned_resume(&family.key().template)
        {
            continue;
        }
        let root_fun = family
            .root_body()
            .or_else(|| raw_fun_decl(&materialized.file, family.root_fqn()))
            .ok_or_else(|| EffectFactsError::MissingCallableRoot {
                fqn: family.root_fqn().to_string(),
            })?;
        seeds.push(CallableSeed {
            key: family.key().clone(),
            declared_row: declared_effect_row(root_fun, &materialized.types),
            invoke_arg_components: root_fun.params.iter().map(|param| param.ty).collect(),
            complete_ty: root_fun.return_ty,
        });
    }
    Ok(seeds)
}

fn template_decl_is_effect_op(index: &Index, template: &TemplateKey) -> bool {
    index.by_fqn.get(&template.fqn).is_some_and(|symbols| {
        symbols.fun.iter().any(|overload| {
            overload.symbol.decl_file == template.source_path
                && overload.symbol.span == template.decl_span
                && overload.sig.kind == ast::FunDeclKind::EffectOp
        })
    })
}

fn template_decl_is_compiler_owned_resume(template: &TemplateKey) -> bool {
    template.fqn == "scoop.core.Continuation.resume"
}

fn raw_fun_decl<'a>(file: &'a MirFile, fqn: &str) -> Option<&'a MirFunDecl> {
    file.items.iter().find_map(|item| match item {
        MirItem::Fun(fun) if fun.fqn == fqn => Some(fun),
        MirItem::Fun(_) | MirItem::Todo { .. } => None,
    })
}

fn declared_effect_row(fun: &MirFunDecl, types: &TypeStore) -> EffectRow {
    match types.kind(fun.ty) {
        TypeKind::Ref(RefTypeKind::Function(function)) => function.effects.clone(),
        _ => EffectRow::pure(),
    }
}

fn lower_effect_nominal_identity(
    types: &TypeStore,
    effect_ty: TypeId,
) -> Result<(String, Vec<TypeId>), EffectFactsError> {
    match types.kind(effect_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            Ok((nominal.fqn.clone(), nominal.args.clone()))
        }
        _ => Err(EffectFactsError::UnsupportedEffectTerm {
            ty: types.display(effect_ty).to_string(),
        }),
    }
}

fn effect_op_overloads(index: &Index, effect_fqn: &str) -> Vec<(String, FunOverload)> {
    let prefix = format!("{effect_fqn}.");
    index
        .by_fqn
        .iter()
        .flat_map(|(fqn, symbols)| {
            let matches_effect = fqn.starts_with(&prefix);
            symbols
                .fun
                .iter()
                .filter(move |overload| {
                    matches_effect && overload.sig.kind == ast::FunDeclKind::EffectOp
                })
                .map(move |overload| (fqn.clone(), overload.clone()))
        })
        .collect()
}

fn canonical_tuple_carrier_ty(types: &mut TypeStore, components: &[TypeId]) -> TypeId {
    let builtins = types.intern_builtins();
    match components {
        [] => builtins.unit,
        [single] => *single,
        _ => types.ty_tuple(components.to_vec()),
    }
}

fn continuation_surface_ty(
    types: &mut TypeStore,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_row: &EffectRow,
) -> TypeId {
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: "scoop.core.Continuation".to_string(),
        args: vec![resume_tuple_ty, answer_ty],
        eff: Some(out_row.clone()),
    })))
}

fn continuation_object_ty(types: &mut TypeStore, key: &InstanceKey) -> TypeId {
    let type_args_suffix = if key.type_args.is_empty() {
        String::new()
    } else {
        format!(
            "::{}",
            key.type_args
                .iter()
                .map(|ty| types.display(*ty).to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let effect_args_suffix = if key.eff_args.is_empty() {
        String::new()
    } else {
        format!(
            "#{}",
            key.eff_args
                .iter()
                .map(|row| effect_row_identity_string(types, row))
                .collect::<Vec<_>>()
                .join("|")
        )
    };
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: format!(
            "scoop.__compiler.ContinuationObject@{}:{}..{}::{}{}{}",
            key.template.source_path.display(),
            key.template.decl_span.start,
            key.template.decl_span.end,
            key.template.fqn,
            type_args_suffix,
            effect_args_suffix,
        ),
        args: Vec::new(),
        eff: None,
    })))
}

fn effect_row_identity_string(types: &TypeStore, row: &EffectRow) -> String {
    match row.terms.as_slice() {
        [] => "Pure".to_string(),
        [term] => types.display(*term).to_string(),
        terms => format!(
            "({})",
            terms
                .iter()
                .map(|term| types.display(*term).to_string())
                .collect::<Vec<_>>()
                .join(" + ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::{MaterializedEffectFactsBuilder, continuation_object_ty};
    use crate::effect_facts::{CanonicalMirQuerySurface, ImplPlan};
    use crate::mir::{InstanceKey, TemplateKey, materialize_for_dump};
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeKind, TypeStore};

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_builder_fixture.scoop",
            r#"
package sample

effect Flag {
    fun ping(): Unit
}

fun <T> pureUnit(_witness: T): Unit {}

fun <T> raiseString(_witness: T): Unit / scoop.core.Raise<String> {
    scoop.core.Raise.raise("boom")
}

fun <T> raiseInt(_witness: T): Unit / scoop.core.Raise<Int> {
    scoop.core.Raise.raise(1)
}

fun <T> pingFlag(_witness: T): Unit / Flag {
    Flag.ping()
}

fun <T> resumeZero(_witness: T, k: scoop.core.Continuation<Unit, Unit, eff Pure>): Unit / scoop.core.Raise<scoop.core.RuntimeError> {
    k.resume()
}

fun exercise(k: scoop.core.Continuation<Unit, Unit, eff Pure>): Unit / (Flag + scoop.core.Raise<String> + scoop.core.Raise<Int> + scoop.core.Raise<scoop.core.RuntimeError>) {
    pureUnit(())
    raiseString(())
    raiseInt(())
    pingFlag(())
    resumeZero((), k)
}
"#,
        )
    }

    fn build_sample_facts() -> (
        crate::mir::MaterializedMir,
        crate::effect_facts::MaterializedEffectFacts,
    ) {
        let session = refactor_session();
        let source = sample_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .build()
        .unwrap();
        (materialized, facts)
    }

    fn callable_facts_for<'a>(
        facts: &'a crate::effect_facts::MaterializedEffectFacts,
        fqn: &str,
    ) -> (
        &'a InstanceKey,
        &'a crate::effect_facts::CallableEffectFacts,
    ) {
        facts
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == fqn)
            .expect("fixture callable 应在 facts 中可见")
    }

    #[test]
    fn materialized_effect_facts_builder_uses_canonical_pass_view_snapshot() {
        let session = refactor_session();
        let source = sample_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let removed_fqn = materialized
            .pass_view()
            .instances()
            .next()
            .expect("fixture 应该产生至少一个 instance")
            .root_fqn()
            .to_string();

        materialized
            .pass_artifacts_mut()
            .remove_callable_body(&removed_fqn);

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .build()
        .unwrap();

        assert_eq!(
            facts.snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(
            facts.snapshot_binding().instance_count(),
            materialized.pass_view().len()
        );
        assert_eq!(facts.callable_facts().len(), facts.bodies().len());
        assert!(
            !facts
                .snapshot_binding()
                .canonical_body_fqns()
                .iter()
                .any(|fqn| fqn == &removed_fqn)
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_skips_effect_op_roots() {
        let (materialized, facts) = build_sample_facts();
        let pass_roots = materialized
            .pass_view()
            .instances()
            .map(|family| family.root_fqn().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            facts.callable_facts().len(),
            5,
            "pass-view roots: {pass_roots:?}"
        );
        assert!(
            facts
                .callable_facts()
                .keys()
                .all(|key| key.template.fqn != "sample.Flag.ping")
        );
        assert!(
            facts
                .callable_facts()
                .keys()
                .all(|key| key.template.fqn != "scoop.core.Raise.raise")
        );
    }

    #[test]
    fn refactor_effect_schema_case_tags_are_stable_and_distinguish_generic_specialized_raise_cases()
    {
        let (materialized, facts) = build_sample_facts();

        let (_, raise_string_facts) = callable_facts_for(&facts, "sample.raiseString");
        let (_, raise_int_facts) = callable_facts_for(&facts, "sample.raiseInt");
        let raise_string_schema = facts
            .step_schemas()
            .get(&raise_string_facts.step_schema())
            .expect("raiseString 应有 step schema");
        let raise_int_schema = facts
            .step_schemas()
            .get(&raise_int_facts.step_schema())
            .expect("raiseInt 应有 step schema");

        let raise_string_case = &raise_string_schema.cases()[0];
        let raise_int_case = &raise_int_schema.cases()[0];
        assert_eq!(raise_string_case.case_tag().as_u32(), 0);
        assert_eq!(raise_int_case.case_tag().as_u32(), 0);
        assert_eq!(
            raise_string_case
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );
        assert_ne!(
            raise_string_case.concrete_op_key(),
            raise_int_case.concrete_op_key(),
            "Raise<String>.raise 与 Raise<Int>.raise 应是不同 concrete op"
        );
        assert_eq!(
            materialized
                .types
                .display(raise_string_case.concrete_op_key().instance_key().type_args[0])
                .to_string(),
            "String"
        );
        assert_eq!(
            materialized
                .types
                .display(raise_int_case.concrete_op_key().instance_key().type_args[0])
                .to_string(),
            "Int"
        );
    }

    #[test]
    fn refactor_continuation_schema_explicitly_records_unit_payload_resume_and_surface_type() {
        let (materialized, facts) = build_sample_facts();

        let (_, ping_flag_facts) = callable_facts_for(&facts, "sample.pingFlag");
        let schema = facts
            .step_schemas()
            .get(&ping_flag_facts.step_schema())
            .expect("pingFlag 应有 step schema");
        let case = &schema.cases()[0];
        let continuation_schema = facts
            .continuation_schemas()
            .get(&case.continuation_schema())
            .expect("pingFlag case 应有 continuation schema");

        assert_eq!(
            materialized
                .types
                .display(case.payload_tuple_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.resume_tuple_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.answer_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.surface_ty())
                .to_string(),
            "scoop.core.Continuation<Unit, Unit, eff sample.Flag>"
        );
        assert!(
            materialized
                .types
                .display(schema.continuation_obj_ty())
                .to_string()
                .contains("sample.pingFlag")
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_uses_final_shape_and_runtime_error_case() {
        let (materialized, facts) = build_sample_facts();

        let (_, pure_facts) = callable_facts_for(&facts, "sample.pureUnit");
        assert!(matches!(pure_facts.impl_plan(), ImplPlan::NoOutward));
        assert!(!pure_facts.needs_reentry());
        assert!(pure_facts.resolved_outward_cases().is_empty());

        let (_, resume_zero_facts) = callable_facts_for(&facts, "sample.resumeZero");
        assert!(resume_zero_facts.needs_reentry());
        assert!(matches!(
            resume_zero_facts.impl_plan(),
            ImplPlan::SingleCase(tag) if tag.as_u32() == 0
        ));

        let schema = facts
            .step_schemas()
            .get(&resume_zero_facts.step_schema())
            .expect("resumeZero 应有 step schema");
        let runtime_case = &schema.cases()[0];
        assert_eq!(
            runtime_case.concrete_op_key().instance_key().template.fqn,
            "scoop.core.Raise.raise"
        );
        assert_eq!(
            materialized
                .types
                .display(runtime_case.payload_tuple_ty())
                .to_string(),
            "scoop.core.RuntimeError"
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_instance_keys_distinguish_allowed_rows() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let raise_string = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.string],
            eff: None,
        })));
        let raise_int = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.int],
            eff: None,
        })));
        let template = TemplateKey {
            fqn: "sample.forward".to_string(),
            source_path: PathBuf::from("<mem>/forward.scoop"),
            decl_span: Span::new(0, 1),
        };
        let string_key = InstanceKey {
            template: template.clone(),
            type_args: Vec::new(),
            eff_args: vec![EffectRow::new(vec![raise_string])],
        };
        let int_key = InstanceKey {
            template,
            type_args: Vec::new(),
            eff_args: vec![EffectRow::new(vec![raise_int])],
        };

        let mut seen = HashMap::new();
        seen.insert(string_key, "string");
        seen.insert(int_key, "int");

        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn refactor_continuation_schema_identity_distinguishes_callable_instances() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let raise_string = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.string],
            eff: None,
        })));
        let raise_int = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.int],
            eff: None,
        })));
        let template = TemplateKey {
            fqn: "sample.forward".to_string(),
            source_path: PathBuf::from("<mem>/forward.scoop"),
            decl_span: Span::new(0, 1),
        };
        let string_key = InstanceKey {
            template: template.clone(),
            type_args: vec![builtins.string],
            eff_args: vec![EffectRow::new(vec![raise_string])],
        };
        let int_key = InstanceKey {
            template,
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::new(vec![raise_int])],
        };

        let string_cont_ty = continuation_object_ty(&mut types, &string_key);
        let int_cont_ty = continuation_object_ty(&mut types, &int_key);

        assert_ne!(string_cont_ty, int_cont_ty);
        assert_ne!(
            types.display(string_cont_ty).to_string(),
            types.display(int_cont_ty).to_string()
        );
    }
}
