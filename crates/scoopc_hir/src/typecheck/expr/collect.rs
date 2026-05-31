use std::collections::HashMap;
use std::path::Path;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, TypeId, TypeStore};

use super::call::{type_ref_fn_effect_eff_base, type_ref_nominal_eff_eff_base};
use super::util::package_prefix;

use super::{EffParamSig, ExprInferInputs, ExprTypeError, FunSigOwned, FunWhereConstraintInfo};

use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::eff_row_subst::{EffRowVarSubstPlan, build_eff_row_var_subst_plan};
use super::super::lower::{TypeLowering, build_where_bound_entries};
use super::super::{TypeEnv, val_pat};

struct TopLevelValueCollectionFile<'a> {
    source: &'a SourceFile,
    file: ast::File,
    imports: ImportTable,
    strict: bool,
}

fn is_annotation_class_decl(decl: &ast::TypeDecl) -> bool {
    decl.kind == ast::TypeKind::Class && decl.modifiers.contains(&ast::Modifier::Annotation)
}

/// 收集“当前编译单元内”的顶层 `val/var` 声明类型（FQN → TypeId）。
///
/// 说明：
/// - 普通顶层名字绑定仍直接读取显式类型注解；
/// - 顶层 `val` pattern binding 既支持显式整体类型注解，也支持由 initializer 驱动推断；
/// - 会尽量补齐其它文件中的顶层 pattern binder 类型，以支持跨文件静态引用；
/// - 该表用于处理表达式中的 `ResolvedValueRef::TopLevel`（变量引用）。
pub(super) fn collect_top_level_value_types(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let mut map: HashMap<String, TypeId> = HashMap::new();
    let mut files: Vec<TopLevelValueCollectionFile<'_>> = Vec::new();

    files.push(TopLevelValueCollectionFile {
        source,
        file: file.clone(),
        imports: imports.clone(),
        strict: true,
    });

    for (path, stored_file) in env.files() {
        if path.as_path() == source.path() {
            continue;
        }
        let Some(stored_source) = env.source(path) else {
            continue;
        };
        let mut cloned = stored_file.clone();
        let headers = match crate::resolve::check_file_headers(stored_source, &cloned, index) {
            Ok(headers) => headers,
            Err(_) => continue,
        };
        if crate::resolve::check_file_bodies(stored_source, &mut cloned, index, &headers).is_err() {
            continue;
        }
        files.push(TopLevelValueCollectionFile {
            source: stored_source,
            file: cloned,
            imports: headers.imports,
            strict: false,
        });
    }

    for file_info in &files {
        collect_explicit_top_level_value_types_in_file(
            file_info, index, env, types, builtins, &mut map,
        )?;
    }

    let mut changed = true;
    while changed {
        changed = false;
        for file_info in &files {
            if infer_top_level_unannotated_value_types_in_file(
                file_info, index, env, types, builtins, &mut map,
            )? {
                changed = true;
            }
        }
    }

    Ok(map)
}

fn collect_explicit_top_level_value_types_in_file(
    file_info: &TopLevelValueCollectionFile<'_>,
    index: &Index,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let pkg_prefix = package_prefix(file_info.source, file_info.file.package.as_ref());
    let mut lower = TypeLowering::new(
        file_info.source,
        &file_info.file,
        index,
        &file_info.imports,
        env,
        types,
        builtins,
    );
    if !file_info.strict {
        lower.set_warning_emission_enabled(false);
    }
    let struct_field_types =
        collect_struct_field_types(file_info.source, &file_info.file, &mut lower)?;

    for item in &file_info.file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };

        match &v.binding {
            ast::ValBinding::Name(name) => {
                let Some(ty_ref) = &v.ty else {
                    continue;
                };

                let local_name = file_info.source.slice(name.span);
                let fqn = if pkg_prefix.is_empty() {
                    local_name.to_string()
                } else {
                    format!("{pkg_prefix}.{local_name}")
                };

                let ty = lower.lower_type_ref(ty_ref)?;
                out.insert(fqn, ty);
            }
            ast::ValBinding::Pattern(pattern) => {
                let Some(ty_ref) = &v.ty else {
                    continue;
                };

                let subject_ty = lower.lower_type_ref(ty_ref)?;
                let bindings = val_pat::infer_val_pat_bindings(
                    file_info.source,
                    pattern,
                    subject_ty,
                    &mut lower,
                    builtins,
                    &struct_field_types,
                )?;

                for binder in v.binding.bound_idents() {
                    let Some(ty) = bindings.get(&binder.span).copied() else {
                        continue;
                    };
                    let local_name = binder.text(file_info.source);
                    let fqn = if pkg_prefix.is_empty() {
                        local_name.to_string()
                    } else {
                        format!("{pkg_prefix}.{local_name}")
                    };
                    out.insert(fqn, ty);
                }
            }
        }
    }

    Ok(())
}

fn infer_top_level_unannotated_value_types_in_file(
    file_info: &TopLevelValueCollectionFile<'_>,
    index: &Index,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    out: &mut HashMap<String, TypeId>,
) -> Result<bool, ExprTypeError> {
    let pkg_prefix = package_prefix(file_info.source, file_info.file.package.as_ref());
    let mut lower = TypeLowering::new(
        file_info.source,
        &file_info.file,
        index,
        &file_info.imports,
        env,
        types,
        builtins,
    );
    if !file_info.strict {
        lower.set_warning_emission_enabled(false);
    }
    let struct_field_types =
        collect_struct_field_types(file_info.source, &file_info.file, &mut lower)?;
    let top_level_funs =
        collect_top_level_fun_signatures(file_info.source, &file_info.file, &mut lower, builtins)?;
    let empty_locals = HashMap::new();
    let mut changed = false;

    for item in &file_info.file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };
        if v.ty.is_some() {
            continue;
        }
        let Some(init) = &v.init else {
            continue;
        };

        let all_bound = v.binding.bound_idents().into_iter().all(|binder| {
            let local_name = binder.text(file_info.source);
            let fqn = if pkg_prefix.is_empty() {
                local_name.to_string()
            } else {
                format!("{pkg_prefix}.{local_name}")
            };
            out.contains_key(&fqn)
        });
        if all_bound {
            continue;
        }

        let init_ty = match (ExprInferInputs {
            source: file_info.source,
            builtins,
            locals: &empty_locals,
            mutable_bindings: None,
            lambda_this_decl_span: None,
            top_level_types: out,
            top_level_funs: &top_level_funs,
            member_mutabilities: None,
            struct_field_types: &struct_field_types,
            loop_depth: 0,
            expected_return_ty: None,
        })
        .infer(&mut lower, init)
        {
            Ok(ty) => ty,
            Err(err) => {
                if file_info.strict {
                    match err {
                        ExprTypeError::UnsupportedTopLevelValueType { .. } => continue,
                        other => return Err(other),
                    }
                }
                continue;
            }
        };

        match &v.binding {
            ast::ValBinding::Name(name) => {
                let local_name = name.text(file_info.source);
                let fqn = if pkg_prefix.is_empty() {
                    local_name.to_string()
                } else {
                    format!("{pkg_prefix}.{local_name}")
                };
                if out.insert(fqn, init_ty).is_none() {
                    changed = true;
                }
            }
            ast::ValBinding::Pattern(pattern) => {
                let bindings = match val_pat::infer_val_pat_bindings(
                    file_info.source,
                    pattern,
                    init_ty,
                    &mut lower,
                    builtins,
                    &struct_field_types,
                ) {
                    Ok(bindings) => bindings,
                    Err(err) => {
                        if file_info.strict {
                            return Err(err);
                        }
                        continue;
                    }
                };

                for binder in v.binding.bound_idents() {
                    let Some(ty) = bindings.get(&binder.span).copied() else {
                        continue;
                    };
                    let local_name = binder.text(file_info.source);
                    let fqn = if pkg_prefix.is_empty() {
                        local_name.to_string()
                    } else {
                        format!("{pkg_prefix}.{local_name}")
                    };
                    if out.insert(fqn, ty).is_none() {
                        changed = true;
                    }
                }
            }
        }
    }

    Ok(changed)
}

/// 收集“当前文件内”的顶层 `fun` 声明签名（FQN → FunSig）。
///
/// 当前阶段（最小子集）：
/// - 支持 `fun <T>`：在签名 lowering 时把 `T` 视为 `TypeKind::Param`；
/// - 调用点的最小泛型实参推断见 T0505（当前仅支持单一类型参数）；
/// - 不处理 overload / default param；
/// - 未显式标注 return type 的函数，暂视为 `Unit`；
/// - 扩展函数会被降糖为“receiver 作为第一个参数”的普通顶层函数，用于 `receiver.member()` 与 `receiver?.member()`
///   调用的类型检查（spec §7.4）。
pub(super) fn collect_top_level_fun_signatures(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<HashMap<String, Vec<FunSigOwned>>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, Vec<FunSigOwned>> = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let local_name = source.slice(fun.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        let decl_span = fun.name.span;
        let builtin_flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);

        // fun 自身的 type params 在签名 lowering 语境内可见。
        lower.push_type_params(&fun.type_params);
        let bounds = build_where_bound_entries(source, &fun.type_params, fun.where_clause.as_ref());
        let where_bounds_pushed = if bounds.is_empty() {
            false
        } else {
            lower.push_where_bounds(bounds);
            true
        };
        // T0509：effect row 参数（`<eff E = Pure>`）。
        //
        // 说明：这里先把 `E` 绑定到默认值（缺省为 Pure），以便签名里的 `(...) / E` 能顺利 lowering；
        // 调用点会根据 lambda body 的 required effects 覆盖该默认值并做实例化。
        let eff_param_sig = if let Some(eff_param) = &fun.eff_param {
            let name = source.slice(eff_param.name.span).to_string();
            let default = match eff_param.default.as_ref() {
                Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
                None => EffectRow::pure(),
            };
            lower.push_effect_row_param_binding(name.clone(), default.clone());
            Some(EffParamSig { name, default })
        } else {
            None
        };

        let result: Result<(), ExprTypeError> = (|| {
            let type_params: Vec<TypeId> = fun
                .type_params
                .iter()
                .map(|p| lower.ty_param_from_decl(p))
                .collect();

            let mut param_names = Vec::with_capacity(fun.params.len() + 1);
            let mut param_has_defaults = Vec::with_capacity(fun.params.len() + 1);
            let mut param_is_vararg = Vec::with_capacity(fun.params.len() + 1);
            let mut params = Vec::with_capacity(fun.params.len() + 1);
            let mut param_fn_effect_eff_base: Vec<Option<EffectRow>> =
                Vec::with_capacity(fun.params.len() + 1);
            let mut param_nominal_eff_eff_base: Vec<Option<EffectRow>> =
                Vec::with_capacity(fun.params.len() + 1);
            let mut param_eff_row_var_subst: Vec<EffRowVarSubstPlan> =
                Vec::with_capacity(fun.params.len() + 1);

            // spec §7.4：扩展函数编译为普通静态函数：receiver 作为第一个参数。
            // typecheck 阶段也沿用这一“降糖”形式，便于统一调用检查逻辑。
            let is_extension = fun.receiver.is_some();
            if let Some(receiver) = &fun.receiver {
                // receiver 本身没有名字；这里用占位符保持与 `params` 对齐。
                param_names.push("<receiver>".to_string());
                param_has_defaults.push(false);
                param_is_vararg.push(false);
                let receiver_ty = lower.lower_type_ref(receiver)?;
                params.push(receiver_ty);
                param_fn_effect_eff_base.push(None);
                let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_nominal_eff_eff_base(receiver, &eff_param.name, source, lower)?
                } else {
                    None
                };
                param_nominal_eff_eff_base.push(nominal_eff_base);
                let subst_plan = if let Some(eff_param) = &eff_param_sig {
                    build_eff_row_var_subst_plan(
                        receiver,
                        receiver_ty,
                        &eff_param.name,
                        source,
                        lower,
                    )?
                } else {
                    EffRowVarSubstPlan::None
                };
                param_eff_row_var_subst.push(subst_plan);
            }

            for p in &fun.params {
                let Some(ty_ref) = &p.ty else {
                    // headers check 已保证参数类型注解存在；这里保持健壮性。
                    continue;
                };
                let fn_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_fn_effect_eff_base(ty_ref, &eff_param.name, source, lower)?
                } else {
                    None
                };
                let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                    type_ref_nominal_eff_eff_base(ty_ref, &eff_param.name, source, lower)?
                } else {
                    None
                };
                param_names.push(source.slice(p.name.span).to_string());
                param_has_defaults.push(p.default_value.is_some());
                param_is_vararg.push(p.is_vararg);
                let ty = lower.lower_type_ref(ty_ref)?;
                params.push(ty);
                param_fn_effect_eff_base.push(fn_eff_base);
                param_nominal_eff_eff_base.push(nominal_eff_base);
                let subst_plan = if let Some(eff_param) = &eff_param_sig {
                    build_eff_row_var_subst_plan(ty_ref, ty, &eff_param.name, source, lower)?
                } else {
                    EffRowVarSubstPlan::None
                };
                param_eff_row_var_subst.push(subst_plan);
            }

            let return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => builtins.unit,
            };

            let return_eff_row_var_subst = if let (Some(eff_param), Some(ret_ref)) =
                (eff_param_sig.as_ref(), fun.return_ty.as_ref())
            {
                build_eff_row_var_subst_plan(ret_ref, return_ty, &eff_param.name, source, lower)?
            } else {
                EffRowVarSubstPlan::None
            };

            // T0129：从 AST where_clause 构建 where_constraints。
            let where_constraints =
                build_fun_where_constraints(source, &fun.type_params, fun.where_clause.as_ref());

            map.entry(fqn).or_default().push(FunSigOwned {
                decl_span,
                decl_file: source.path().to_path_buf(),
                is_extension,
                is_operator: fun.modifiers.contains(&ast::Modifier::Operator),
                is_unsafe: builtin_flags.is_unsafe,
                is_nogc: builtin_flags.is_nogc,
                is_extern: builtin_flags.is_extern,
                is_intrinsic: builtin_flags.is_intrinsic,
                intrinsic_entry_name: builtin_flags.intrinsic_entry_name.clone(),
                param_names,
                param_has_defaults,
                param_is_vararg,
                type_params,
                owner_eff_param: None,
                eff_param: eff_param_sig.clone(),
                param_fn_effect_eff_base,
                param_nominal_eff_eff_base,
                param_eff_row_var_subst,
                return_eff_row_var_subst,
                params,
                return_ty,
                effects: fun.effects.clone(),
                where_constraints,
            });
            Ok(())
        })();
        if eff_param_sig.is_some() {
            lower.pop_effect_row_param_binding();
        }
        if where_bounds_pushed {
            lower.pop_where_bounds();
        }
        lower.pop_type_params(&fun.type_params);
        result?;
    }

    // T0112: Collect extension property getter signatures.
    // Extension properties are synthesized as getter functions during HIR lowering.
    // Register their signatures here so typecheck can resolve the return type.
    for item in &file.items {
        let ast::Item::ExtensionProperty(prop) = item else {
            continue;
        };
        if prop.getter.is_none() {
            continue;
        }

        let local_name = source.slice(prop.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        let decl_span = prop.name.span;

        lower.push_type_params(&prop.type_params);
        let bounds = build_where_bound_entries(source, &prop.type_params, None);
        let where_bounds_pushed = if bounds.is_empty() {
            false
        } else {
            lower.push_where_bounds(bounds);
            true
        };
        let result: Result<(), ExprTypeError> = (|| {
            let receiver_ty = lower.lower_type_ref(&prop.receiver)?;
            let return_ty = match &prop.ty {
                Some(t) => lower.lower_type_ref(t)?,
                None => builtins.any,
            };

            map.entry(fqn).or_default().push(FunSigOwned {
                decl_span,
                decl_file: source.path().to_path_buf(),
                is_extension: true,
                is_operator: false,
                is_unsafe: false,
                is_nogc: false,
                is_extern: false,
                is_intrinsic: false,
                intrinsic_entry_name: None,
                param_names: vec!["<receiver>".to_string()],
                param_has_defaults: vec![false],
                param_is_vararg: vec![false],
                type_params: Vec::new(),
                owner_eff_param: None,
                eff_param: None,
                param_fn_effect_eff_base: vec![None],
                param_nominal_eff_eff_base: vec![None],
                param_eff_row_var_subst: vec![EffRowVarSubstPlan::None],
                return_eff_row_var_subst: EffRowVarSubstPlan::None,
                params: vec![receiver_ty],
                return_ty,
                effects: None,
                where_constraints: Vec::new(),
            });
            Ok(())
        })();
        if where_bounds_pushed {
            lower.pop_where_bounds();
        }
        lower.pop_type_params(&prop.type_params);
        result?;
    }

    Ok(map)
}

/// 从 AST `where_clause` + `type_params` 构建 `FunWhereConstraintInfo` 列表（T0129）。
///
/// 此函数在"同文件"和"跨文件"两条签名收集路径中复用。
fn build_fun_where_constraints(
    source: &SourceFile,
    type_params: &[ast::TypeParam],
    where_clause: Option<&ast::WhereClause>,
) -> Vec<FunWhereConstraintInfo> {
    let constraints = ast::generic_constraints(type_params, where_clause);
    if constraints.is_empty() {
        return Vec::new();
    }
    let param_names: Vec<String> = type_params
        .iter()
        .map(|p| p.name.text(source).to_string())
        .collect();
    let mut out = Vec::new();
    for c in constraints {
        let target_name = source.slice(c.ty_param.span).to_string();
        let Some(param_index) = param_names.iter().position(|n| n == &target_name) else {
            // 如果 target 不在当前函数的 type params 中，跳过
            // （where_clause.rs 的 declaration-site 检查会报错）。
            continue;
        };
        out.push(FunWhereConstraintInfo {
            _span: c.span,
            param_index,
            param_name: target_name,
            bound: c.bound.clone(),
        });
    }
    out
}

/// 从 resolve 的 `TypeParamSig` + `WhereClause` 构建 `FunWhereConstraintInfo` 列表（T0129）。
///
/// 用于跨文件签名收集路径：resolver 的 `FunSig.type_params` 已有 param name，
/// `FunSig.where_clause` 保留了 AST where clause。
pub(super) fn build_fun_where_constraints_from_resolve_sig(
    decl_source: &SourceFile,
    type_params: &[crate::resolve::TypeParamSig],
    where_clause: Option<&ast::WhereClause>,
) -> Vec<FunWhereConstraintInfo> {
    let param_names: Vec<&str> = type_params.iter().map(|p| p.name.as_str()).collect();
    let mut out = Vec::new();
    for (param_index, param) in type_params.iter().enumerate() {
        for bound in &param.bounds {
            out.push(FunWhereConstraintInfo {
                _span: Span::new(param.name_span.start, bound.span().end),
                param_index,
                param_name: param.name.clone(),
                bound: bound.clone(),
            });
        }
    }
    if let Some(wc) = where_clause {
        for c in &wc.constraints {
            let target_name = decl_source.slice(c.ty_param.span).to_string();
            let Some(param_index) = param_names.iter().position(|n| *n == target_name) else {
                continue;
            };
            out.push(FunWhereConstraintInfo {
                _span: c.span,
                param_index,
                param_name: target_name,
                bound: c.bound.clone(),
            });
        }
    }
    out
}

pub(super) fn collect_member_mutabilities(
    source: &SourceFile,
    file: &ast::File,
    env: &TypeEnv,
) -> HashMap<String, bool> {
    let mut map: HashMap<String, bool> = HashMap::new();

    collect_member_mutabilities_in_file(source, file, &mut map);

    // 成员赋值需要看到“当前编译单元的其它声明文件”中的 `var`/`val` 信息：
    // 例如 sysroot 中某些 declaration-only surface 定义在 `core.scoop`，实现体位于其它可编译文件。
    let mut foreign_files = env
        .files()
        .filter(|(path, _)| path.as_path() != source.path())
        .filter_map(|(path, stored_file)| {
            let stored_source = env.source(path)?.clone();
            Some((path.clone(), stored_source, stored_file.clone()))
        })
        .collect::<Vec<_>>();
    foreign_files.sort_by(|(lhs, _, _), (rhs, _, _)| lhs.cmp(rhs));

    for (_, foreign_source, foreign_file) in foreign_files {
        collect_member_mutabilities_in_file(&foreign_source, &foreign_file, &mut map);
    }

    map
}

fn collect_member_mutabilities_in_file(
    source: &SourceFile,
    file: &ast::File,
    out: &mut HashMap<String, bool>,
) {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_member_mutabilities_in_type_decl(source, ty, &pkg_prefix, out);
            }
            ast::Item::Object(obj) => {
                collect_member_mutabilities_in_object_decl(source, obj, &pkg_prefix, out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }
}

fn collect_member_mutabilities_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut HashMap<String, bool>,
) {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                // struct value members 始终是 immutable direct field；显式 `var`
                // 会在 `check_file_struct_decls` 提前报错，这里不再把它泄漏成 mutable。
                out.insert(field_fqn, false);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, false);
            }
        }
    }

    if matches!(decl.kind, ast::TypeKind::Class) && !is_annotation_class_decl(decl) {
        // class ctor `val/var` 参数声明同名字段/属性；裸参数不应进入 member 表。
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(kind) = p.kind else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(kind, ast::ValKind::Var));
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, ast::ValKind::Var));
            }
        }
    }

    // 无论外层是否 struct/class，都递归收集 nested type（可能存在 nested struct/class）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_member_mutabilities_in_type_decl(source, nested, &type_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    collect_member_mutabilities_in_object_decl(source, obj, &type_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }
}

fn collect_member_mutabilities_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut HashMap<String, bool>,
) {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                return;
            }
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                let Some(_ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{obj_fqn}.{field_name}");
                out.insert(field_fqn, matches!(p.kind, ast::ValKind::Var));
            }
            ast::TypeMember::Type(nested) => {
                collect_member_mutabilities_in_type_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_member_mutabilities_in_object_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

/// 收集当前编译单元内“可通过成员访问读取”的 value members 声明类型（member FQN → TypeId）。
///
/// 说明：
/// - 初始版本（T0408）仅收集 `struct`（值类型）的字段；
/// - T0438 起额外收集 class 的 ctor `val/var` 参数与 type body 属性，用于最小 member access typecheck；
/// - 字段来源：
///   - 主构造参数（`struct Point(val x: Int)`）：在语义上等价于字段
///   - type body 内的 `val/var` property（`struct Point { val x: Int }`）
/// - 现在会先扫描当前文件 AST，再扫描 `TypeEnv` 中其它已知源文件的 AST，以补齐真实跨文件
///   member access / struct literal 所需的 body property 与 getter-only property；
/// - 对于没有 AST 上下文的外部类型（例如仅通过索引暴露的声明），仍会额外从 `Index` 的
///   primary ctor 信息补全“跨文件 ctor 字段”。
pub(super) fn collect_struct_field_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_struct_field_types_in_type_decl(source, ty, &pkg_prefix, lower, &mut map)?;
            }
            ast::Item::Object(obj) => {
                collect_struct_field_types_in_object_decl(
                    source,
                    obj,
                    &pkg_prefix,
                    lower,
                    &mut map,
                )?;
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    // 先补全“当前编译单元内其它已知源文件”的 value member 类型表。
    //
    // 说明：
    // - 真实 `typecheck_multi/<case>/` 会按“整个编译单元”构建 `TypeEnv`，因此这里需要把
    //   foreign AST 里的 ctor 字段、body property 与 getter-only property 一并收进来；
    // - generic owner 使用 fresh type params 作为占位，后续 member access 读取时再由
    //   `instantiate_member_value_type_from_receiver_ty` 按 receiver 的 concrete args 具体化。
    let mut foreign_files = lower
        .env()
        .files()
        .filter(|(path, _)| path.as_path() != source.path())
        .filter_map(|(path, stored_file)| {
            let stored_source = lower.env().source(path)?.clone();
            Some((path.clone(), stored_source, stored_file.clone()))
        })
        .collect::<Vec<_>>();
    foreign_files.sort_by(|(lhs, _, _), (rhs, _, _)| lhs.cmp(rhs));

    lower.with_warning_emission_suspended(|lower| {
        for (_, foreign_source, foreign_file) in foreign_files {
            collect_struct_field_types_in_foreign_file(
                &foreign_source,
                &foreign_file,
                lower,
                &mut map,
            )?;
        }
        Ok::<(), ExprTypeError>(())
    })?;

    // 额外补全“没有 AST 上下文的跨文件 ctor 字段”：
    //
    // 背景：
    // - 某些外部声明只以索引/符号形式暴露，没有完整 AST 参与当前 `TypeEnv`；
    // - 这类场景下仍至少需要 primary ctor 字段类型，避免 struct literal / member access
    //   在 fallback 路径下退回 `struct_lit_unknown_field` / `unsupported_member_access`。
    //
    // 约定：
    // - 只考虑 primary constructor 的参数（secondary ctor 不是字段声明来源）；
    // - 仅当 `{TypeFqn}.{field}` 在 value namespace 中存在时才视为字段：
    //   - struct：所有 primary ctor params 都会被 resolver 注入为 value member；
    //   - class：仅 `val/var` ctor params 会被注入为 value member（裸参数会被过滤掉）。
    // - 字段类型需要在“声明处文件”的 package/import 语境里 lowering（避免跨文件 span 切片错位）。
    // NOTE: 这里需要在循环内对 `lower` 做可变借用（lowering field type ref），因此先把 constructors
    // 拷贝出来，避免同时持有 `lower.index()` 的不可变借用导致 borrow checker 冲突。
    let constructors = lower.index().constructors.clone();
    lower.with_warning_emission_suspended(|lower| {
        for (type_fqn, ctors) in &constructors {
            // T0124: skip generic types — their field types contain unresolved type params
            // that cannot be lowered without concrete instantiation arguments.
            if lower.env().type_param_count(type_fqn).unwrap_or(0) > 0 {
                continue;
            }
            if lower
                .env()
                .type_symbol(type_fqn)
                .is_some_and(|sym| sym.is_annotation_class)
            {
                continue;
            }
            for ctor in ctors {
                if ctor.kind != crate::resolve::ConstructorKind::Primary {
                    continue;
                }
                for p in &ctor.params {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_fqn = format!("{type_fqn}.{}", p.name);
                    let has_value_symbol = lower
                        .index()
                        .by_fqn
                        .get(&field_fqn)
                        .is_some_and(|syms| syms.value.is_some());
                    if !has_value_symbol {
                        continue;
                    }
                    if map.contains_key(&field_fqn) {
                        continue;
                    }
                    let field_ty = lower.lower_type_ref_in_decl_file(&ctor.decl_file, ty_ref)?;
                    map.insert(field_fqn, field_ty);
                }
            }
        }
        Ok::<(), ExprTypeError>(())
    })?;

    Ok(map)
}

fn collect_struct_field_types_in_foreign_file(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_struct_field_types_in_foreign_type_decl(
                    source,
                    source.path(),
                    ty,
                    &pkg_prefix,
                    lower,
                    out,
                )?;
            }
            ast::Item::Object(obj) => {
                collect_struct_field_types_in_foreign_object_decl(
                    source,
                    source.path(),
                    obj,
                    &pkg_prefix,
                    lower,
                    out,
                )?;
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    Ok(())
}

fn collect_struct_field_types_in_foreign_type_decl(
    source: &SourceFile,
    decl_file: &Path,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };
    let type_param_names = decl
        .type_params
        .iter()
        .map(|param| source.slice(param.name.span).to_string())
        .collect::<Vec<_>>();

    if matches!(decl.kind, ast::TypeKind::Struct) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for param in &primary_ctor.params {
                let Some(ty_ref) = &param.ty else {
                    continue;
                };
                let field_name = source.slice(param.name.span);
                insert_foreign_struct_field_type(
                    decl_file,
                    &type_fqn,
                    field_name,
                    &type_param_names,
                    decl.eff_param.as_ref(),
                    ty_ref,
                    lower,
                    out,
                )?;
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(prop) = member else {
                    continue;
                };
                let Some(ty_ref) = &prop.ty else {
                    continue;
                };
                let field_name = source.slice(prop.name.span);
                insert_foreign_struct_field_type(
                    decl_file,
                    &type_fqn,
                    field_name,
                    &type_param_names,
                    decl.eff_param.as_ref(),
                    ty_ref,
                    lower,
                    out,
                )?;
            }
        }
    }

    if matches!(decl.kind, ast::TypeKind::Class) && !is_annotation_class_decl(decl) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for param in &primary_ctor.params {
                if param.kind.is_none() {
                    continue;
                }
                let Some(ty_ref) = &param.ty else {
                    continue;
                };
                let field_name = source.slice(param.name.span);
                insert_foreign_struct_field_type(
                    decl_file,
                    &type_fqn,
                    field_name,
                    &type_param_names,
                    decl.eff_param.as_ref(),
                    ty_ref,
                    lower,
                    out,
                )?;
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(prop) = member else {
                    continue;
                };
                let Some(ty_ref) = &prop.ty else {
                    continue;
                };
                let field_name = source.slice(prop.name.span);
                insert_foreign_struct_field_type(
                    decl_file,
                    &type_fqn,
                    field_name,
                    &type_param_names,
                    decl.eff_param.as_ref(),
                    ty_ref,
                    lower,
                    out,
                )?;
            }
        }
    }

    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_struct_field_types_in_foreign_type_decl(
                        source, decl_file, nested, &type_fqn, lower, out,
                    )?;
                }
                ast::TypeMember::Object(obj) => {
                    collect_struct_field_types_in_foreign_object_decl(
                        source, decl_file, obj, &type_fqn, lower, out,
                    )?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    Ok(())
}

fn collect_struct_field_types_in_foreign_object_decl(
    source: &SourceFile,
    decl_file: &Path,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                return Ok(());
            }
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(prop) => {
                let Some(ty_ref) = &prop.ty else {
                    continue;
                };
                let field_name = source.slice(prop.name.span);
                insert_foreign_struct_field_type(
                    decl_file,
                    &obj_fqn,
                    field_name,
                    &[],
                    None,
                    ty_ref,
                    lower,
                    out,
                )?;
            }
            ast::TypeMember::Type(nested) => {
                collect_struct_field_types_in_foreign_type_decl(
                    source, decl_file, nested, &obj_fqn, lower, out,
                )?;
            }
            ast::TypeMember::Object(nested) => {
                collect_struct_field_types_in_foreign_object_decl(
                    source, decl_file, nested, &obj_fqn, lower, out,
                )?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_foreign_struct_field_type(
    decl_file: &Path,
    owner_fqn: &str,
    field_name: &str,
    type_param_names: &[String],
    eff_param: Option<&ast::EffectRowParam>,
    ty_ref: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let field_fqn = format!("{owner_fqn}.{field_name}");
    if out.contains_key(&field_fqn) {
        return Ok(());
    }

    let ty = lower.lower_type_ref_in_decl_file_with_fresh_type_params_and_eff(
        decl_file,
        type_param_names,
        eff_param,
        ty_ref,
    )?;
    out.insert(field_fqn, ty);
    Ok(())
}

fn collect_struct_field_types_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        // T0124: push type params so generic field types (e.g. `A`, `B`) resolve correctly.
        lower.push_type_params(&decl.type_params);
        let bounds =
            build_where_bound_entries(source, &decl.type_params, decl.where_clause.as_ref());
        let where_bounds_pushed = if bounds.is_empty() {
            false
        } else {
            lower.push_where_bounds(bounds);
            true
        };
        let eff_binding_pushed = if let Some(eff_param) = &decl.eff_param {
            let name = source.slice(eff_param.name.span).to_string();
            lower.push_effect_row_param_marker_binding(name, eff_param.name.span);
            true
        } else {
            false
        };

        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::Property(p) = member {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_name = source.slice(p.name.span);
                    let field_fqn = format!("{type_fqn}.{field_name}");
                    out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
                }
            }
        }

        if where_bounds_pushed {
            lower.pop_where_bounds();
        }
        if eff_binding_pushed {
            lower.pop_effect_row_param_binding();
        }
        lower.pop_type_params(&decl.type_params);
    }

    if matches!(decl.kind, ast::TypeKind::Class) && !is_annotation_class_decl(decl) {
        // T0125: push type params so generic field types (e.g. `T`) resolve correctly.
        lower.push_type_params(&decl.type_params);
        let bounds =
            build_where_bound_entries(source, &decl.type_params, decl.where_clause.as_ref());
        let where_bounds_pushed = if bounds.is_empty() {
            false
        } else {
            lower.push_where_bounds(bounds);
            true
        };
        let eff_binding_pushed = if let Some(eff_param) = &decl.eff_param {
            let name = source.slice(eff_param.name.span).to_string();
            lower.push_effect_row_param_marker_binding(name, eff_param.name.span);
            true
        } else {
            false
        };

        // class ctor `val/var` 参数声明同名字段/属性；裸参数不应进入 member 类型表。
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                if p.kind.is_none() {
                    continue;
                }
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::Property(p) = member {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_name = source.slice(p.name.span);
                    let field_fqn = format!("{type_fqn}.{field_name}");
                    out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
                }
            }
        }

        if where_bounds_pushed {
            lower.pop_where_bounds();
        }
        if eff_binding_pushed {
            lower.pop_effect_row_param_binding();
        }
        lower.pop_type_params(&decl.type_params);
    }

    // 无论外层是否 struct，都递归收集 nested type（可能存在 nested struct）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_struct_field_types_in_type_decl(source, nested, &type_fqn, lower, out)?;
                }
                ast::TypeMember::Object(obj) => {
                    collect_struct_field_types_in_object_decl(source, obj, &type_fqn, lower, out)?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    Ok(())
}

fn collect_struct_field_types_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                return Ok(());
            }
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) => {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{obj_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
            ast::TypeMember::Type(nested) => {
                collect_struct_field_types_in_type_decl(source, nested, &obj_fqn, lower, out)?;
            }
            ast::TypeMember::Object(nested) => {
                collect_struct_field_types_in_object_decl(source, nested, &obj_fqn, lower, out)?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::resolve::{ImportTable, Index};
    use crate::ty::TypeStore;
    use crate::typecheck::TypeEnv;
    use crate::typecheck::assignable::is_type_assignable;

    #[test]
    fn nothing_is_assignable_to_any_type() {
        // 该测试不依赖 sysroot 或 resolver 的完整能力；
        // 只验证 typecheck 的“赋值兼容”最小规则：`Nothing <: T`。
        let source = SourceFile::new_virtual("<mem>", "package a\nfun f(): Unit { return }");
        let file = parse_file(&source).unwrap();
        let index = Index::build(&[(&source, &file)]).unwrap();
        let imports = ImportTable::build(&source, &file, &index).unwrap();

        let env = TypeEnv::default();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let lower = TypeLowering::new(&source, &file, &index, &imports, &env, &mut types, builtins);

        assert!(is_type_assignable(
            builtins.nothing,
            builtins.any,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.unit,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.bool_,
            &lower,
            builtins
        ));

        // 反例：普通值类型不应在 v0 阶段隐式互转。
        assert!(!is_type_assignable(
            builtins.unit,
            builtins.bool_,
            &lower,
            builtins
        ));
    }

    #[test]
    fn collect_struct_field_types_includes_foreign_body_properties() {
        let defs = SourceFile::new_virtual(
            "defs.scoop",
            r#"
package fixtures.typecheck_multi.generic_value_member_access_cross_file

struct Box<T>(val value: T) {
    val bodyCopy: T = value
    val readBack: T
        get() = this.bodyCopy
}
"#,
        );
        let defs_ast = parse_file(&defs).unwrap();
        let use_source = SourceFile::new_virtual(
            "use.scoop",
            r#"
package fixtures.typecheck_multi.generic_value_member_access_cross_file

fun crossFileTotal(): Int {
    return Box(40).value + Box(1).bodyCopy + Box(1).readBack
}
"#,
        );
        let use_ast = parse_file(&use_source).unwrap();
        let index = Index::build(&[(&defs, &defs_ast), (&use_source, &use_ast)]).unwrap();
        let imports = ImportTable::build(&use_source, &use_ast, &index).unwrap();

        let mut env = TypeEnv::default();
        env.extend_from_file(&defs, &defs_ast, &index).unwrap();
        env.extend_from_file(&use_source, &use_ast, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut lower = TypeLowering::new(
            &use_source,
            &use_ast,
            &index,
            &imports,
            &env,
            &mut types,
            builtins,
        );

        let fields = collect_struct_field_types(&use_source, &use_ast, &mut lower).unwrap();
        let owner = "fixtures.typecheck_multi.generic_value_member_access_cross_file.Box";

        assert!(fields.contains_key(&format!("{owner}.value")));
        assert!(fields.contains_key(&format!("{owner}.bodyCopy")));
        assert!(fields.contains_key(&format!("{owner}.readBack")));
    }
}
