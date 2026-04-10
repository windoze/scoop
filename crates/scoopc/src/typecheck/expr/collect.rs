use std::collections::HashMap;

use crate::ast;
use crate::source::SourceFile;
use crate::ty::{BuiltinTypes, EffectRow, TypeId};

use super::call::{type_ref_fn_effect_eff_base, type_ref_nominal_eff_eff_base};
use super::util::package_prefix;

use super::{EffParamSig, ExprTypeError, FunSigOwned, FunWhereConstraintInfo, TASK_FQN};

use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::eff_row_subst::{EffRowVarSubstPlan, build_eff_row_var_subst_plan};
use super::super::lower::TypeLowering;

/// 收集“当前文件内”的顶层 `val/var` 声明类型（FQN → TypeId）。
///
/// 说明：
/// - 顶层变量的类型注解由 `typecheck::check_file_headers` 强制要求，因此这里可以直接做 lowering；
/// - 该表用于处理表达式中的 `ResolvedValueRef::TopLevel`（变量引用）。
pub(super) fn collect_top_level_value_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };

        let ast::ValBinding::Name(name) = &v.binding else {
            // 顶层 pattern binding 会在 headers check 中报错；这里仅保持健壮性。
            continue;
        };

        let Some(ty_ref) = &v.ty else {
            continue;
        };

        let local_name = source.slice(name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };

        let ty = lower.lower_type_ref(ty_ref)?;
        map.insert(fqn, ty);
    }

    Ok(map)
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
            let is_inline = fun.modifiers.contains(&ast::Modifier::Inline);
            let is_const = fun.modifiers.contains(&ast::Modifier::Const);
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

            // T0623：`async fun foo(): T` 对外暴露 `Task<T>`。
            //
            // 说明：
            // - 这里的 `return_ty` 用于调用点类型与 overload resolution；
            // - 函数体内部的 `return` 类型检查仍以 AST 上的 `return_ty`（T）为准（见 `check_fun_body_exprs`）。
            let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
            let inner_return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => builtins.unit,
            };
            let return_ty = if is_async_fun {
                lower.lower_type_fqn_with_args(
                    TASK_FQN.to_string(),
                    vec![inner_return_ty],
                    fun.name.span,
                )?
            } else {
                inner_return_ty
            };

            let return_eff_row_var_subst = if let (Some(eff_param), Some(ret_ref)) =
                (eff_param_sig.as_ref(), fun.return_ty.as_ref())
            {
                if is_async_fun {
                    // 对 eff var substitution：在签名视图下，返回类型是 `Task<ret_ref>`。
                    let synth_span = ret_ref.span();
                    let synth_ret_ref = ast::TypeRef::Path(ast::TypePath {
                        span: synth_span,
                        segments: vec![
                            ast::Ident::synthetic(synth_span, "scoop"),
                            ast::Ident::synthetic(synth_span, "core"),
                            ast::Ident::synthetic(synth_span, "Task"),
                        ],
                        args: vec![ret_ref.clone()],
                    });
                    build_eff_row_var_subst_plan(
                        &synth_ret_ref,
                        return_ty,
                        &eff_param.name,
                        source,
                        lower,
                    )?
                } else {
                    build_eff_row_var_subst_plan(
                        ret_ref,
                        return_ty,
                        &eff_param.name,
                        source,
                        lower,
                    )?
                }
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
                is_inline,
                is_const,
                is_unsafe: builtin_flags.is_unsafe,
                is_nogc: builtin_flags.is_nogc,
                is_extern: builtin_flags.is_extern,
                is_intrinsic: builtin_flags.is_intrinsic,
                param_names,
                param_has_defaults,
                param_is_vararg,
                type_params,
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
                is_inline: false,
                is_const: false,
                is_unsafe: false,
                is_nogc: false,
                is_extern: false,
                is_intrinsic: false,
                param_names: vec!["<receiver>".to_string()],
                param_has_defaults: vec![false],
                param_is_vararg: vec![false],
                type_params: Vec::new(),
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
    let Some(wc) = where_clause else {
        return Vec::new();
    };
    let param_names: Vec<String> = type_params
        .iter()
        .map(|p| p.name.text(source).to_string())
        .collect();
    let mut out = Vec::new();
    for c in &wc.constraints {
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
    let Some(wc) = where_clause else {
        return Vec::new();
    };
    let param_names: Vec<&str> = type_params.iter().map(|p| p.name.as_str()).collect();
    let mut out = Vec::new();
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
    out
}

pub(super) fn collect_member_mutabilities(
    source: &SourceFile,
    file: &ast::File,
) -> HashMap<String, bool> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, bool> = HashMap::new();

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_member_mutabilities_in_type_decl(source, ty, &pkg_prefix, &mut map);
            }
            ast::Item::Object(obj) => {
                collect_member_mutabilities_in_object_decl(source, obj, &pkg_prefix, &mut map);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    map
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
                out.insert(field_fqn, matches!(p.kind, Some(ast::ValKind::Var)));
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

    if matches!(decl.kind, ast::TypeKind::Class) {
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

/// 收集当前文件内“可通过成员访问读取”的 value members 声明类型（member FQN → TypeId）。
///
/// 说明：
/// - 初始版本（T0408）仅收集 `struct`（值类型）的字段；
/// - T0438 起额外收集 class 的 ctor `val/var` 参数与 type body 属性，用于最小 member access typecheck；
/// - 字段来源：
///   - 主构造参数（`struct Point(val x: Int)`）：在语义上等价于字段
///   - type body 内的 `val/var` property（`struct Point { val x: Int }`）
/// - 当前阶段默认只扫描“当前文件”的 AST；但为支持 stdlib 注入后的跨文件 member access/struct literal，
///   会额外从 `Index` 的 primary ctor 信息补全“跨文件 ctor 字段”（type body property 仍以当前文件为准）。
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
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    // 补全“跨文件 ctor 字段”的 member 类型表：
    //
    // 背景：
    // - `check_file_exprs` 逐文件执行，但 driver 可能注入 stdlib（多文件编译单元）；
    // - stdlib/用户代码可能会构造或访问 sysroot/其它文件声明的 struct/class 字段；
    // - 若只扫描当前文件，会在 struct literal / member access 处产生
    //   `struct_lit_unknown_field` / `unsupported_member_access` 的假错误。
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
    for (type_fqn, ctors) in &constructors {
        // T0124: skip generic types — their field types contain unresolved type params
        // that cannot be lowered without concrete instantiation arguments.
        if lower.env().type_param_count(type_fqn).unwrap_or(0) > 0 {
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

    Ok(map)
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

        lower.pop_type_params(&decl.type_params);
    }

    if matches!(decl.kind, ast::TypeKind::Class) {
        // T0125: push type params so generic field types (e.g. `T`) resolve correctly.
        lower.push_type_params(&decl.type_params);

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
}
