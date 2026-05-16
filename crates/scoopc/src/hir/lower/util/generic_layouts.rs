//! Mangled FQN helpers, layout type ID resolution, generic struct/enum/class instantiation layout collection.

#![allow(dead_code)]

use super::*;

/// 为参数化名义类型构造 mangled FQN（用作 struct_layouts/enum_layouts 的 key）。
///
/// 规则：
/// - 无 type args 时返回 base FQN 本身（如 `"pkg.Point"`）
/// - 有 type args 时返回 `"pkg.Pair<Int, String>"` 格式（与 TypeStore display 格式对齐）
pub fn mangle_nominal_fqn(fqn: &str, args: &[crate::ty::TypeId], types: &TypeStore) -> String {
    if args.is_empty() {
        return fqn.to_string();
    }
    let arg_str = args
        .iter()
        .map(|id| types.display(*id).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{fqn}<{arg_str}>")
}

/// 将 TypeId 转为 layout 索引中使用的 FQN 字符串。
///
/// 用途：为泛型 struct/enum 的字段类型生成 `StructFieldLayout.ty_fqn` / `EnumVariantFieldLayout.ty_fqn`。
/// 返回 `None` 表示无法确定（例如未知类型或未支持的类型类别）。
pub(in crate::hir::lower) fn type_id_to_layout_fqn(
    types: &TypeStore,
    ty: crate::ty::TypeId,
) -> Option<String> {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Unit) => Some("scoop.core.Unit".to_string()),
        TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool".to_string()),
        TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char".to_string()),
        TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64".to_string()),
        TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32".to_string()),
        TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int".to_string()),
        TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt".to_string()),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(format!("scoop.core.Int{bits}")),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(format!("scoop.core.UInt{bits}")),
        TypeKind::Value(ValueTypeKind::Nothing) => Some("scoop.core.Nothing".to_string()),
        // builtin `Option<T>` 在类型系统里不是 nominal，但 layout 索引仍需要一个稳定 key，
        // 以便 enum payload / boxed payload object 的字段收集能恢复真实 TypeId。
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            Some(mangle_nominal_fqn("scoop.core.Option", &[*inner], types))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            Some(mangle_nominal_fqn(&nominal.fqn, &nominal.args, types))
        }
        TypeKind::Ref(RefTypeKind::Any) => Some("scoop.core.Any".to_string()),
        TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String".to_string()),
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            Some(mangle_nominal_fqn(&nominal.fqn, &nominal.args, types))
        }
        _ => None,
    }
}

pub(in crate::hir::lower) fn type_ref_to_layout_type_id(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty: &ast::TypeRef,
    types: &mut TypeStore,
) -> Option<crate::ty::TypeId> {
    lower_layout_type_ref_with_bindings(source, file, index, ty, None, &HashMap::new(), types)
}

pub(in crate::hir::lower) fn builtin_layout_alias_type_id(
    base_fqn: &str,
    types: &mut TypeStore,
) -> Option<crate::ty::TypeId> {
    match base_fqn {
        "scoop.unsafe.__AtomicInt" => Some(types.intern(TypeKind::Value(ValueTypeKind::Int))),
        "scoop.core.UIntPtr" => Some(types.intern(TypeKind::Value(ValueTypeKind::UInt))),
        _ => None,
    }
}

pub(in crate::hir::lower) fn find_layout_type_id_by_key(
    types: &TypeStore,
    layout_key: &str,
) -> Option<crate::ty::TypeId> {
    types
        .iter_ids()
        .find(|id| type_id_to_layout_fqn(types, *id).as_deref() == Some(layout_key))
}

pub(in crate::hir::lower) fn intern_layout_nominal_type(
    types: &mut TypeStore,
    type_kinds: Option<&HashMap<String, ast::TypeKind>>,
    base_fqn: &str,
    type_args: Vec<crate::ty::TypeId>,
) -> Option<crate::ty::TypeId> {
    let kind = type_kinds
        .and_then(|kinds| kinds.get(base_fqn).copied())
        .map(|kind| !matches!(kind, ast::TypeKind::Struct | ast::TypeKind::Enum))
        .or_else(|| {
            types.iter_ids().find_map(|id| match types.kind(id) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == base_fqn => {
                    Some(true)
                }
                TypeKind::Value(ValueTypeKind::Nominal(nominal)) if nominal.fqn == base_fqn => {
                    Some(false)
                }
                _ => None,
            })
        })?;

    let nominal = crate::ty::NominalType {
        fqn: base_fqn.to_string(),
        args: type_args,
        eff: None,
    };

    Some(if kind {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(nominal)))
    } else {
        types.intern(TypeKind::Value(ValueTypeKind::Nominal(nominal)))
    })
}

pub(in crate::hir::lower) fn lower_layout_type_ref_with_bindings(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty: &ast::TypeRef,
    type_kinds: Option<&HashMap<String, ast::TypeKind>>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &mut TypeStore,
) -> Option<crate::ty::TypeId> {
    match ty {
        ast::TypeRef::Path(path) => {
            if path.segments.len() == 1 && path.args.is_empty() {
                let name = path.segments[0].text(source);
                if let Some(concrete_ty) = param_map.get(name) {
                    return Some(*concrete_ty);
                }
            }

            let base_fqn = index.type_ref_to_fqn_in_file(source, file, ty)?;
            if path.args.is_empty()
                && let Some(alias_ty) = builtin_layout_alias_type_id(&base_fqn, types)
            {
                return Some(alias_ty);
            }
            let mut type_args = Vec::new();
            for arg in &path.args {
                if matches!(arg, ast::TypeRef::EffectRowArg { .. }) {
                    continue;
                }
                type_args.push(lower_layout_type_ref_with_bindings(
                    source, file, index, arg, type_kinds, param_map, types,
                )?);
            }

            let layout_key = if type_args.is_empty() {
                base_fqn.clone()
            } else {
                mangle_nominal_fqn(&base_fqn, &type_args, types)
            };
            find_layout_type_id_by_key(types, &layout_key)
                .or_else(|| intern_layout_nominal_type(types, type_kinds, &base_fqn, type_args))
        }
        ast::TypeRef::Tuple(tuple) => {
            if tuple.elements.is_empty() {
                if let Some(unit_ty) = types
                    .iter_ids()
                    .find(|id| matches!(types.kind(*id), TypeKind::Value(ValueTypeKind::Unit)))
                {
                    return Some(unit_ty);
                }
                return Some(types.intern(TypeKind::Value(ValueTypeKind::Unit)));
            }

            let elements = tuple
                .elements
                .iter()
                .map(|elem| {
                    lower_layout_type_ref_with_bindings(
                        source, file, index, elem, type_kinds, param_map, types,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            Some(types.ty_tuple(elements))
        }
        ast::TypeRef::Nullable { inner, .. } => {
            let inner_ty = lower_layout_type_ref_with_bindings(
                source, file, index, inner, type_kinds, param_map, types,
            )?;
            Some(types.ty_option(inner_ty))
        }
        ast::TypeRef::Function(fun) => {
            let receiver = match fun.receiver.as_ref() {
                Some(receiver) => Some(lower_layout_type_ref_with_bindings(
                    source, file, index, receiver, type_kinds, param_map, types,
                )?),
                None => None,
            };
            let params = fun
                .params
                .iter()
                .map(|param| {
                    lower_layout_type_ref_with_bindings(
                        source, file, index, param, type_kinds, param_map, types,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let return_ty = lower_layout_type_ref_with_bindings(
                source,
                file,
                index,
                &fun.return_ty,
                type_kinds,
                param_map,
                types,
            )?;
            let effects = lower_layout_effect_row_expr(
                source,
                file,
                index,
                fun.effects.as_ref(),
                type_kinds,
                param_map,
                types,
            )?;
            Some(types.ty_function(
                receiver,
                params,
                return_ty,
                effects,
                fun.effects.as_ref().is_some_and(|row| row.closed),
            ))
        }
        ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => None,
    }
}

pub(in crate::hir::lower) fn lower_layout_effect_row_expr(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    expr: Option<&ast::EffectRowExpr>,
    type_kinds: Option<&HashMap<String, ast::TypeKind>>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &mut TypeStore,
) -> Option<EffectRow> {
    let Some(expr) = expr else {
        return Some(EffectRow::pure());
    };
    if expr.terms.is_empty() {
        return Some(EffectRow::pure());
    }

    let terms = expr
        .terms
        .iter()
        .map(|term| {
            lower_layout_type_ref_with_bindings(
                source,
                file,
                index,
                &ast::TypeRef::Path(term.clone()),
                type_kinds,
                param_map,
                types,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(EffectRow::new(terms))
}

pub(in crate::hir::lower) fn push_struct_layout_field(
    fields: &mut Vec<StructFieldLayout>,
    owner_fqn: &str,
    span: Span,
    name: String,
    ty: Option<crate::ty::TypeId>,
    ty_fqn: Option<String>,
) {
    let field_fqn = format!("{owner_fqn}.{name}");
    fields.push(StructFieldLayout {
        span,
        name,
        fqn: field_fqn,
        ty,
        ty_fqn,
    });
}

pub(in crate::hir::lower) fn append_struct_body_property_layout_fields(
    source: &SourceFile,
    body: Option<&ast::TypeBody>,
    owner_fqn: &str,
    mut resolve_field_ty: impl FnMut(&ast::TypeRef) -> (Option<String>, Option<crate::ty::TypeId>),
    fields: &mut Vec<StructFieldLayout>,
) {
    let Some(body) = body else {
        return;
    };

    for member in &body.members {
        let ast::TypeMember::Property(property) = member else {
            continue;
        };

        // 只有真正拥有 backing field 的属性才参与 value layout。
        if property.delegate.is_some() || property.getter.is_some() || property.setter.is_some() {
            continue;
        }
        let Some(ty_ref) = &property.ty else {
            continue;
        };

        let (ty_fqn, ty) = resolve_field_ty(ty_ref);
        push_struct_layout_field(
            fields,
            owner_fqn,
            property.name.span,
            property.name.text(source).to_string(),
            ty,
            ty_fqn,
        );
    }
}

pub(in crate::hir::lower) fn type_contains_param(types: &TypeStore, ty: crate::ty::TypeId) -> bool {
    let mut stack = vec![ty];
    while let Some(id) = stack.pop() {
        match types.kind(id) {
            TypeKind::Param(_) => return true,
            TypeKind::StarProjection(star) => stack.push(star.read_ty),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                stack.extend(nominal.args.iter().copied());
                if let Some(eff) = &nominal.eff {
                    stack.extend(eff.terms.iter().copied());
                }
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                stack.extend(elements.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if let Some(receiver) = fun.receiver {
                    stack.push(receiver);
                }
                stack.extend(fun.params.iter().copied());
                stack.push(fun.return_ty);
                stack.extend(fun.effects.terms.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => {
                stack.extend(union.variants.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
            | TypeKind::Value(ValueTypeKind::Unit)
            | TypeKind::Value(ValueTypeKind::Nothing)
            | TypeKind::Value(ValueTypeKind::Bool)
            | TypeKind::Value(ValueTypeKind::Char)
            | TypeKind::Value(ValueTypeKind::Float64)
            | TypeKind::Value(ValueTypeKind::Float32)
            | TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::IntN(_))
            | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
        }
    }
    false
}

/// 收集泛型 struct 的具体实例化布局（T0124）。
///
/// 在 typecheck 之后运行：扫描 TypeStore 中所有 `ValueTypeKind::Nominal`（args 非空），
/// 匹配到编译单元中声明的泛型 struct 后，为每个具体实例化生成 StructLayout。
///
/// 布局的 key 使用 mangled FQN（如 `"pkg.Pair<Int, String>"`），
/// 字段的 ty_fqn 通过 type param 替换为具体类型。
pub(in crate::hir::lower) fn collect_generic_struct_instantiation_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    types: &mut TypeStore,
) -> StructLayoutIndex {
    let type_kinds = collect_type_decl_kinds(pairs);
    // 1) 收集泛型 struct 声明：base_fqn → (source, decl)
    let mut generic_structs: HashMap<String, (&SourceFile, &ast::File, &ast::TypeDecl)> =
        HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else { continue };
            if !matches!(ty.kind, ast::TypeKind::Struct) {
                continue;
            }
            if ty.type_params.is_empty() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            generic_structs.insert(fqn, (source, file, ty));
        }
    }

    if generic_structs.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore 中的具体实例化
    let mut out: StructLayoutIndex = HashMap::new();
    let concrete_type_ids: Vec<crate::ty::TypeId> = types.iter_ids().collect();
    for ty_id in concrete_type_ids {
        let nominal = match types.kind(ty_id) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.clone(),
            _ => continue,
        };
        if nominal.args.is_empty() {
            continue;
        }
        if nominal
            .args
            .iter()
            .any(|&arg| type_contains_param(types, arg))
        {
            continue;
        }

        let Some((source, file, decl)) = generic_structs.get(&nominal.fqn) else {
            continue;
        };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) {
            continue;
        }

        // 构建 type param name → concrete TypeId 映射
        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() {
            continue;
        }

        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        // 为每个字段解析 ty_fqn。
        let mut fields: Vec<StructFieldLayout> = Vec::new();
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                // 解析字段类型：优先检查是否为 type param，若是则替换为具体类型。
                let ty_fqn = resolve_field_type_fqn_with_type_kinds(
                    source,
                    file,
                    index,
                    Some(&type_kinds),
                    p.ty.as_ref(),
                    &param_map,
                    types,
                );
                let ty = resolve_field_type_id_with_type_kinds(
                    source,
                    file,
                    index,
                    Some(&type_kinds),
                    p.ty.as_ref(),
                    &param_map,
                    types,
                );
                push_struct_layout_field(
                    &mut fields,
                    &nominal.fqn,
                    p.name.span,
                    p.name.text(source).to_string(),
                    ty,
                    ty_fqn,
                );
            }
        }
        append_struct_body_property_layout_fields(
            source,
            decl.body.as_ref(),
            &nominal.fqn,
            |ty_ref| {
                (
                    resolve_field_type_fqn_with_type_kinds(
                        source,
                        file,
                        index,
                        Some(&type_kinds),
                        Some(ty_ref),
                        &param_map,
                        types,
                    ),
                    resolve_field_type_id_with_type_kinds(
                        source,
                        file,
                        index,
                        Some(&type_kinds),
                        Some(ty_ref),
                        &param_map,
                        types,
                    ),
                )
            },
            &mut fields,
        );

        out.insert(
            mangled.clone(),
            StructLayout {
                fqn: mangled,
                fields,
                c_layout: None,
            },
        );
    }

    out
}

/// 收集泛型 enum 的具体实例化布局（T0124）。
///
/// 与 `collect_generic_struct_instantiation_layouts` 类似，为泛型 enum 的具体实例化生成布局。
pub(in crate::hir::lower) fn collect_generic_enum_instantiation_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    types: &mut TypeStore,
) -> EnumLayoutIndex {
    let type_kinds = collect_type_decl_kinds(pairs);
    // 1) 收集泛型 enum 声明
    let mut generic_enums: HashMap<String, (&SourceFile, &ast::File, &ast::TypeDecl)> =
        HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else { continue };
            if !matches!(ty.kind, ast::TypeKind::Enum) {
                continue;
            }
            if ty.type_params.is_empty() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            generic_enums.insert(fqn, (source, file, ty));
        }
    }

    if generic_enums.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore
    let mut out: EnumLayoutIndex = HashMap::new();
    let concrete_type_ids: Vec<crate::ty::TypeId> = types.iter_ids().collect();
    for ty_id in concrete_type_ids {
        let nominal = match types.kind(ty_id) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.clone(),
            _ => continue,
        };
        if nominal.args.is_empty() {
            continue;
        }
        if nominal
            .args
            .iter()
            .any(|&arg| type_contains_param(types, arg))
        {
            continue;
        }

        let Some((source, file, decl)) = generic_enums.get(&nominal.fqn) else {
            continue;
        };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) {
            continue;
        }

        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() {
            continue;
        }

        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        let mut variants: Vec<EnumVariantLayout> = Vec::new();
        let mut next_tag: u64 = 0;

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };
                let variant_name = v.name.text(source).to_string();
                let tag = next_tag;
                next_tag = next_tag.saturating_add(1);

                let mut fields: Vec<EnumVariantFieldLayout> = Vec::new();
                for p in &v.params {
                    let field_name = p.name.text(source).to_string();
                    let ty_fqn = resolve_field_type_fqn_with_type_kinds(
                        source,
                        file,
                        index,
                        Some(&type_kinds),
                        p.ty.as_ref(),
                        &param_map,
                        types,
                    );
                    let ty = resolve_field_type_id_with_type_kinds(
                        source,
                        file,
                        index,
                        Some(&type_kinds),
                        p.ty.as_ref(),
                        &param_map,
                        types,
                    );
                    fields.push(EnumVariantFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        ty,
                        ty_fqn,
                    });
                }

                variants.push(EnumVariantLayout {
                    span: v.span,
                    name: variant_name,
                    tag,
                    fields,
                });
            }
        }

        out.insert(
            mangled.clone(),
            EnumLayout {
                fqn: mangled,
                repr: EnumRepr::TaggedUnion,
                variants,
            },
        );
    }

    out
}

/// 收集泛型 class 的具体实例化 ClassInit（T0125）。
///
/// 与 `collect_generic_struct_instantiation_layouts` 类似，为泛型 class（如 `class Box<T>`）
/// 的每个具体实例化（如 `Box<Int>`、`Box<String>`）生成独立的 ClassInit 条目。
///
/// 实例化的 ClassInit 使用 mangled FQN 作为 key（如 `"pkg.Box<Int>"`），
/// 字段的 TypeId 通过 type param 替换为具体类型（Param("T") → Int）。
pub(in crate::hir::lower) fn collect_generic_class_instantiation_inits(
    pairs: &[(&SourceFile, &ast::File)],
    types: &mut TypeStore,
    base_class_inits: &ClassInitIndex,
) -> ClassInitIndex {
    // 1) 收集泛型 class 声明：base_fqn → (source, decl)
    let mut generic_classes: HashMap<String, (&SourceFile, &ast::TypeDecl)> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        collect_generic_class_decls_in_items(
            source,
            &pkg_prefix,
            &pkg_prefix,
            &file.items,
            &mut generic_classes,
        );
    }

    if generic_classes.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore 中的具体实例化（class 是 ref type → RefTypeKind::Nominal）
    let mut out: ClassInitIndex = HashMap::new();
    let concrete_type_ids: Vec<crate::ty::TypeId> = types.iter_ids().collect();
    for ty_id in concrete_type_ids {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(ty_id) else {
            continue;
        };
        if nominal.args.is_empty() {
            continue;
        }
        if nominal
            .args
            .iter()
            .any(|&arg| type_contains_param(types, arg))
        {
            continue;
        }

        let Some((source, decl)) = generic_classes.get(&nominal.fqn) else {
            continue;
        };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) {
            continue;
        }

        // base ClassInit 必须存在
        let Some(base_init) = base_class_inits.get(&nominal.fqn) else {
            continue;
        };

        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() {
            continue;
        }

        // 构建 type param name → concrete TypeId 映射
        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        // 替换字段类型：必须递归穿透 nominal args / Option / function 等嵌套位置，
        // 否则 `__TaskState<T>`、`Option<T>` 这类字段会把 `TypeKind::Param`
        // 残留到后端。
        let fields: Vec<ClassField> = base_init
            .fields
            .iter()
            .map(|f| ClassField {
                fqn: f.fqn.clone(),
                name: f.name.clone(),
                mutable: f.mutable,
                ty: substitute_type_params(types, f.ty, &param_map),
            })
            .collect();

        let field_indices = base_init.field_indices.clone();
        // 如果 field FQN 中使用了基础 FQN 前缀，替换为 mangled 版本不需要——
        // field FQN 使用原始 class FQN 前缀（如 "pkg.Box.inner"），保持不变。
        let _ = &field_indices; // 保留原始映射

        // 替换 ctor 参数类型
        let ctors: Vec<ClassCtor> = base_init
            .ctors
            .iter()
            .map(|ctor| ClassCtor {
                kind: ctor.kind,
                span: ctor.span,
                params: ctor
                    .params
                    .iter()
                    .map(|p| ClassCtorParam {
                        id: p.id,
                        name: p.name.clone(),
                        decl_span: p.decl_span,
                        ty: substitute_type_params(types, p.ty, &param_map),
                        has_default: p.has_default,
                        default_value: p.default_value.clone(),
                        is_property: p.is_property,
                        property_field_fqn: p.property_field_fqn.clone(),
                    })
                    .collect(),
                delegation: ctor.delegation.clone(),
                body: ctor.body.clone(),
            })
            .collect();

        out.insert(
            mangled.clone(),
            ClassInit {
                fqn: mangled,
                source_path: base_init.source_path.clone(),
                super_class_fqn: base_init.super_class_fqn.clone(),
                super_ctor_args_span: base_init.super_ctor_args_span,
                super_ctor_call: base_init.super_ctor_call.clone(),
                super_ctor_args: base_init.super_ctor_args.clone(),
                this_id: base_init.this_id,
                fields,
                field_indices,
                steps: base_init.steps.clone(),
                ctors,
            },
        );
    }

    out
}

/// 递归收集泛型 class 声明（支持嵌套在 type/object 内的 class）。
pub(in crate::hir::lower) fn collect_generic_class_decls_in_items<'a>(
    source: &'a SourceFile,
    _pkg_prefix: &str,
    owner_prefix: &str,
    items: &'a [ast::Item],
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::TypeDecl)>,
) {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                let name = ty.name.text(source).to_string();
                let fqn = join_prefix(owner_prefix, &name);

                if matches!(ty.kind, ast::TypeKind::Class) && !ty.type_params.is_empty() {
                    out.insert(fqn.clone(), (source, ty));
                }

                // 嵌套声明
                if let Some(body) = &ty.body {
                    for member in &body.members {
                        if let ast::TypeMember::Type(nested) = member {
                            let nested_name = nested.name.text(source).to_string();
                            let nested_fqn = join_prefix(&fqn, &nested_name);
                            if matches!(nested.kind, ast::TypeKind::Class)
                                && !nested.type_params.is_empty()
                            {
                                out.insert(nested_fqn, (source, nested));
                            }
                        }
                    }
                }
            }
            ast::Item::Object(obj) => {
                let obj_name = obj.name.as_ref().map(|n| n.text(source).to_string());
                if let Some(obj_name) = obj_name {
                    let obj_fqn = join_prefix(owner_prefix, &obj_name);
                    if let Some(body) = &obj.body {
                        for member in &body.members {
                            if let ast::TypeMember::Type(nested) = member {
                                let nested_name = nested.name.text(source).to_string();
                                let nested_fqn = join_prefix(&obj_fqn, &nested_name);
                                if matches!(nested.kind, ast::TypeKind::Class)
                                    && !nested.type_params.is_empty()
                                {
                                    out.insert(nested_fqn, (source, nested));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
