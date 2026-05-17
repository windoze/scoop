//! Cross-module helpers used by the materializer: canonical template selection, type/effect parameter substitution, family symbol rewrites, type-parameter inspection, and binding-overlap resolution.

use super::*;

pub(super) fn canonical_template_map(
    candidates: &[TemplateCatalogCandidate],
) -> HashMap<(String, String), TemplateKey> {
    let mut grouped: HashMap<(String, String), Vec<&TemplateCatalogCandidate>> = HashMap::new();
    for candidate in candidates {
        grouped
            .entry((
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            ))
            .or_default()
            .push(candidate);
    }

    let mut out = HashMap::new();
    for (key, group) in grouped {
        let chosen = choose_canonical_template(&group);
        out.insert(key, chosen.template.clone());
    }
    out
}

pub(super) fn choose_canonical_template<'a>(
    candidates: &[&'a TemplateCatalogCandidate],
) -> &'a TemplateCatalogCandidate {
    let mut preferred = candidates
        .iter()
        .copied()
        .filter(|candidate| candidate.prefers_materialized_body)
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter().copied());
    }
    preferred.sort_by(|a, b| {
        a.template
            .source_path
            .cmp(&b.template.source_path)
            .then_with(|| a.template.decl_span.start.cmp(&b.template.decl_span.start))
            .then_with(|| a.template.decl_span.end.cmp(&b.template.decl_span.end))
    });
    preferred
        .into_iter()
        .next()
        .expect("template candidate group must not be empty")
}

pub(super) fn build_template_symbol_suffixes(
    stable_template_keys: &HashMap<TemplateKey, StableTemplateKey>,
) -> HashMap<TemplateKey, String> {
    let mut templates_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
    for template in stable_template_keys.keys() {
        templates_by_fqn
            .entry(template.fqn.clone())
            .or_default()
            .push(template.clone());
    }

    let mut out = HashMap::new();
    for (_, mut templates) in templates_by_fqn {
        templates.sort_by(template_key_sort);
        let overloaded = templates.len() > 1;
        for template in templates {
            let symbol_suffix = if overloaded {
                let stable_template_key = stable_template_keys
                    .get(&template)
                    .expect("every template symbol suffix should have a stable template key");
                format!(
                    "$overload${}",
                    stable_template_symbol_suffix(stable_template_key)
                )
            } else {
                String::new()
            };
            out.insert(template, symbol_suffix);
        }
    }
    out
}

pub(super) fn template_key_sort(lhs: &TemplateKey, rhs: &TemplateKey) -> std::cmp::Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then_with(|| lhs.decl_span.start.cmp(&rhs.decl_span.start))
        .then_with(|| lhs.decl_span.end.cmp(&rhs.decl_span.end))
}

pub(super) fn belongs_to_template_family(fun: &FunDecl, root_fun: &FunDecl) -> bool {
    if fun.fqn == root_fun.fqn {
        return fun.span == root_fun.span;
    }
    fun.fqn.strip_prefix(&root_fun.fqn).is_some_and(|suffix| {
        suffix.starts_with(".$lambda") && span_contains(root_fun.span, fun.span)
    })
}

pub(super) fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

pub(super) fn rewrite_family_symbol_name(
    symbol: &str,
    root_fqn: &str,
    instance_root_fqn: &str,
) -> Option<String> {
    if symbol == root_fqn {
        return Some(instance_root_fqn.to_string());
    }
    let suffix = symbol.strip_prefix(root_fqn)?;
    suffix
        .starts_with(".$lambda")
        .then(|| format!("{instance_root_fqn}{suffix}"))
}

pub(super) fn re_intern_effect_row_from(
    types: &mut TypeStore,
    other: &TypeStore,
    row: &EffectRow,
) -> EffectRow {
    EffectRow::new(
        row.terms
            .iter()
            .map(|&term| types.re_intern_from(other, term))
            .collect(),
    )
}

pub(super) fn substitute_type_and_effect_params(
    types: &mut TypeStore,
    ty: TypeId,
    substitution: &InstanceSubstitution,
) -> TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(param) => {
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE {
                ty
            } else {
                substitution
                    .type_params
                    .get(&param.name)
                    .copied()
                    .unwrap_or(ty)
            }
        }
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_and_effect_params(types, star.read_ty, substitution);
            types.ty_star_projection(read_ty)
        }
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Ref(RefTypeKind::String) => ty,
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let receiver = fun
                .receiver
                .map(|receiver| substitute_type_and_effect_params(types, receiver, substitution));
            let params = fun
                .params
                .iter()
                .map(|&param| substitute_type_and_effect_params(types, param, substitution))
                .collect();
            let return_ty = substitute_type_and_effect_params(types, fun.return_ty, substitution);
            let effects =
                substitute_type_and_effect_params_in_effect_row(types, &fun.effects, substitution);
            types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let variants = union
                .variants
                .iter()
                .map(|&variant| substitute_type_and_effect_params(types, variant, substitution))
                .collect();
            types.ty_union(variants)
        }
        TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => ty,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let inner = substitute_type_and_effect_params(types, inner, substitution);
            types.ty_option(inner)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elements = elements
                .iter()
                .map(|&element| substitute_type_and_effect_params(types, element, substitution))
                .collect();
            types.ty_tuple(elements)
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
    }
}

pub(super) fn substitute_type_and_effect_params_in_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    substitution: &InstanceSubstitution,
) -> EffectRow {
    let mut terms = Vec::new();
    for &term in &row.terms {
        if let Some(name) = effect_row_param_marker_name(types, term)
            && let Some(bound) = substitution.effect_params.get(&name)
        {
            terms.extend(bound.terms.iter().copied().map(|bound_term| {
                substitute_type_and_effect_params(types, bound_term, substitution)
            }));
            continue;
        }
        terms.push(substitute_type_and_effect_params(types, term, substitution));
    }
    EffectRow::new(terms)
}

pub(super) fn effect_row_param_marker_name(types: &TypeStore, ty: TypeId) -> Option<String> {
    match types.kind(ty) {
        TypeKind::Param(param)
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE =>
        {
            Some(param.name.clone())
        }
        _ => None,
    }
}

pub(super) fn is_canonical_array_member_intrinsic_fqn(fqn: &str) -> bool {
    matches!(
        fqn,
        "scoop.core.size"
            | "scoop.core.get"
            | "scoop.core.set"
            | "scoop.core.Array.size"
            | "scoop.core.Array.get"
            | "scoop.core.MutableArray.size"
            | "scoop.core.MutableArray.get"
            | "scoop.core.MutableArray.set"
    )
}

pub(super) fn map_call_args_to_signature_params(
    params: &[CallableSignatureParam],
    args: &[CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    Some(out)
}

pub(super) fn lookup_overlapping_site_instance_binding<'a>(
    bindings: &'a HashMap<SourceSiteKey, SiteInstanceBinding>,
    template_source_path: &Path,
    enclosing_span: Span,
) -> Option<&'a SiteInstanceBinding> {
    let mut found: Option<(Span, &SiteInstanceBinding)> = None;
    for ((source_path, span), binding) in bindings {
        if source_path != template_source_path
            || span.start >= enclosing_span.end
            || enclosing_span.start >= span.end
        {
            continue;
        }
        let Some((found_span, found_binding)) = found else {
            found = Some((*span, binding));
            continue;
        };
        if found_binding != binding {
            return None;
        }
        if span.end - span.start < found_span.end - found_span.start {
            found = Some((*span, binding));
        }
    }
    found.map(|(_, binding)| binding)
}

pub(super) fn same_top_level_fun_call_binding(
    lhs: &ast::TopLevelFunCallBinding,
    rhs: &ast::TopLevelFunCallBinding,
) -> bool {
    lhs.fqn == rhs.fqn
        && lhs.decl_file == rhs.decl_file
        && lhs.decl_span == rhs.decl_span
        && lhs.is_intrinsic == rhs.is_intrinsic
        && lhs.intrinsic_entry_name == rhs.intrinsic_entry_name
        && lhs.type_args == rhs.type_args
        && lhs.eff_args == rhs.eff_args
}

pub(super) fn lookup_overlapping_direct_call_binding<'a>(
    bindings: &'a HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    template_source_path: &Path,
    enclosing_span: Span,
) -> Option<&'a ast::TopLevelFunCallBinding> {
    let mut found: Option<(Span, &ast::TopLevelFunCallBinding)> = None;
    for ((source_path, span), binding) in bindings {
        if source_path != template_source_path
            || span.start >= enclosing_span.end
            || enclosing_span.start >= span.end
        {
            continue;
        }
        let Some((found_span, found_binding)) = found else {
            found = Some((*span, binding));
            continue;
        };
        if !same_top_level_fun_call_binding(found_binding, binding) {
            return None;
        }
        if span.end - span.start < found_span.end - found_span.start {
            found = Some((*span, binding));
        }
    }
    found.map(|(_, binding)| binding)
}

pub(super) fn collect_materialized_local_references(body: &Body) -> HashSet<LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            collect_statement_local_references(stmt, &mut out);
        }
        collect_terminator_local_references(&block.terminator.kind, &mut out);
    }
    out
}

pub(super) fn collect_statement_local_references(stmt: &Statement, out: &mut HashSet<LocalId>) {
    match &stmt.kind {
        StatementKind::Assign { target, value } => {
            let _ = target;
            collect_rvalue_local_references(value, out);
        }
        StatementKind::StoreMember {
            receiver, value, ..
        } => {
            collect_operand_local_reference(receiver, out);
            collect_operand_local_reference(value, out);
        }
        StatementKind::StoreTopLevelVar { value, .. } => {
            collect_operand_local_reference(value, out);
        }
        StatementKind::Nop => {}
        StatementKind::Todo(_) => {}
    }
}

pub(super) fn collect_rvalue_local_references(value: &Rvalue, out: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Transport { value: operand, .. }
        | Rvalue::Unary { operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::CaptureBoxNew { value: operand, .. }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
            ..
        }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        }
        | Rvalue::MemberAccess {
            receiver: operand, ..
        }
        | Rvalue::MakeClosure { env: operand, .. } => collect_operand_local_reference(operand, out),
        Rvalue::Binary { lhs, rhs, .. } => {
            collect_operand_local_reference(lhs, out);
            collect_operand_local_reference(rhs, out);
        }
        Rvalue::CaptureBoxSet {
            box_operand, value, ..
        } => {
            collect_operand_local_reference(box_operand, out);
            collect_operand_local_reference(value, out);
        }
        Rvalue::EnumVariant { args, .. }
        | Rvalue::ClassCtor { args, .. }
        | Rvalue::Call { args, .. } => {
            if let Rvalue::Call { kind, .. } = value {
                collect_call_kind_local_references(kind, out);
            }
            for arg in args {
                collect_operand_local_reference(&arg.value, out);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_operand_local_reference(element, out);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_operand_local_reference(&field.value, out);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let InterpolatedStringPart::Expr { value, .. } = part {
                    collect_operand_local_reference(value, out);
                }
            }
        }
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. } => {}
        Rvalue::Todo(reason) => {
            let _ = reason;
        }
    }
}

pub(super) fn collect_call_kind_local_references(kind: &CallKind, out: &mut HashSet<LocalId>) {
    match kind {
        CallKind::Direct { .. } => {}
        CallKind::Closure { callee, .. }
        | CallKind::FunValue { callee }
        | CallKind::FunPtr { callee } => {
            collect_operand_local_reference(callee, out);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            collect_operand_local_reference(receiver, out);
        }
        CallKind::Resume { continuation, .. } => collect_operand_local_reference(continuation, out),
    }
}

pub(super) fn collect_terminator_local_references(
    kind: &TerminatorKind,
    out: &mut HashSet<LocalId>,
) {
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_operand_local_reference(value, out);
            }
        }
        TerminatorKind::CondBr { cond, .. } => collect_operand_local_reference(cond, out),
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_operand_local_reference(&arg.value, out);
            }
        }
        TerminatorKind::Handle { arms, .. } => {
            for arm in arms {
                out.extend(arm.binder_locals.iter().copied());
                if let Some(local) = arm.continuation_local {
                    out.insert(local);
                }
            }
        }
        TerminatorKind::ResumeUnwind
        | TerminatorKind::Goto { .. }
        | TerminatorKind::Unreachable => {}
        TerminatorKind::Todo(_) => {}
    }
}

pub(super) fn collect_operand_local_reference(operand: &Operand, out: &mut HashSet<LocalId>) {
    if let Operand::Local(local) = operand {
        out.insert(*local);
    }
}

pub(super) fn materialized_payload_tuple_ty_from_components(
    types: &mut TypeStore,
    unit_ty: TypeId,
    components: &[TypeId],
) -> Option<TypeId> {
    match components {
        [] => Some(unit_ty),
        [single] => Some(*single),
        _ => Some(types.ty_tuple(components.to_vec())),
    }
}

pub(super) fn operand_type(
    types: &TypeStore,
    builtins: BuiltinTypes,
    locals: &[LocalDecl],
    operand: &Operand,
) -> Option<TypeId> {
    match operand {
        Operand::Local(local) => locals.get(local.as_u32() as usize).map(|decl| decl.ty),
        Operand::Const(ConstValue::Bool(_)) => Some(builtins.bool_),
        Operand::Const(ConstValue::Char) => Some(builtins.char_),
        Operand::Const(ConstValue::Unit) => Some(builtins.unit),
        Operand::Const(ConstValue::Int) => Some(builtins.int),
        Operand::Const(ConstValue::SynthInt(_)) => Some(builtins.int),
        Operand::Const(ConstValue::Float64) => Some(builtins.float64),
        Operand::Const(ConstValue::Float32) => Some(builtins.float32),
        Operand::Const(ConstValue::String | ConstValue::SynthString(_)) => Some(builtins.string),
    }
    .filter(|ty| {
        !type_contains_param(types, *ty)
            && !matches!(types.kind(*ty), TypeKind::Ref(RefTypeKind::Any))
    })
}

pub(super) fn instance_request_is_concrete(
    types: &TypeStore,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> bool {
    type_args.iter().all(|&ty| !type_contains_param(types, ty))
        && eff_args
            .iter()
            .all(|row| !effect_row_contains_param(types, row))
}

pub(super) fn effect_row_contains_param(types: &TypeStore, row: &EffectRow) -> bool {
    row.terms
        .iter()
        .copied()
        .any(|term| type_contains_param(types, term))
}

pub(super) fn collect_type_param_names_in_type(
    types: &TypeStore,
    ty: TypeId,
    out: &mut Vec<String>,
) {
    match types.kind(ty) {
        TypeKind::Param(param) => {
            if param.decl_file.as_os_str() != crate::hir::EFFECT_ROW_PARAM_DECL_FILE
                && !out.contains(&param.name)
            {
                out.push(param.name.clone());
            }
        }
        TypeKind::StarProjection(star) => {
            collect_type_param_names_in_type(types, star.read_ty, out)
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            for &arg in &nominal.args {
                collect_type_param_names_in_type(types, arg, out);
            }
            if let Some(eff) = &nominal.eff {
                for &term in &eff.terms {
                    collect_type_param_names_in_type(types, term, out);
                }
            }
        }
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            collect_type_param_names_in_type(types, *inner, out)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            for &element in elements {
                collect_type_param_names_in_type(types, element, out);
            }
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            if let Some(receiver) = fun.receiver {
                collect_type_param_names_in_type(types, receiver, out);
            }
            for &param in &fun.params {
                collect_type_param_names_in_type(types, param, out);
            }
            collect_type_param_names_in_type(types, fun.return_ty, out);
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            for &variant in &union.variants {
                collect_type_param_names_in_type(types, variant, out);
            }
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

pub(super) fn function_type_has_effect_param(types: &TypeStore, fun_ty: TypeId) -> bool {
    let TypeKind::Ref(RefTypeKind::Function(fun)) = types.kind(fun_ty) else {
        return false;
    };
    fun.effects
        .terms
        .iter()
        .any(|&term| effect_row_param_marker_name(types, term).is_some())
}

pub(super) fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
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
                stack.extend(elements.iter().copied())
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
                stack.extend(union.variants.iter().copied())
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

pub(super) fn collect_type_param_bindings(
    types: &TypeStore,
    declared_ty: TypeId,
    concrete_ty: TypeId,
    bindings: &mut HashMap<String, TypeId>,
) {
    match (types.kind(declared_ty), types.kind(concrete_ty)) {
        (TypeKind::Param(param), _) => match bindings.get(&param.name).copied() {
            Some(existing) if existing == concrete_ty => {}
            Some(_) => {}
            None => {
                bindings.insert(param.name.clone(), concrete_ty);
            }
        },
        (
            TypeKind::Ref(RefTypeKind::Nominal(declared)),
            TypeKind::Ref(RefTypeKind::Nominal(concrete)),
        )
        | (
            TypeKind::Value(ValueTypeKind::Nominal(declared)),
            TypeKind::Value(ValueTypeKind::Nominal(concrete)),
        ) => {
            if declared.fqn != concrete.fqn || declared.args.len() != concrete.args.len() {
                return;
            }
            for (decl_arg, concrete_arg) in declared.args.iter().zip(concrete.args.iter()) {
                collect_type_param_bindings(types, *decl_arg, *concrete_arg, bindings);
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Option(declared_inner)),
            TypeKind::Value(ValueTypeKind::Option(concrete_inner)),
        ) => {
            collect_type_param_bindings(types, *declared_inner, *concrete_inner, bindings);
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(declared_elements)),
            TypeKind::Value(ValueTypeKind::Tuple(concrete_elements)),
        ) => {
            if declared_elements.len() != concrete_elements.len() {
                return;
            }
            for (decl_elem, concrete_elem) in declared_elements.iter().zip(concrete_elements.iter())
            {
                collect_type_param_bindings(types, *decl_elem, *concrete_elem, bindings);
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(declared_fun)),
            TypeKind::Ref(RefTypeKind::Function(concrete_fun)),
        ) => {
            match (declared_fun.receiver, concrete_fun.receiver) {
                (Some(declared_receiver), Some(concrete_receiver)) => collect_type_param_bindings(
                    types,
                    declared_receiver,
                    concrete_receiver,
                    bindings,
                ),
                (None, None) => {}
                _ => return,
            }
            if declared_fun.params.len() != concrete_fun.params.len() {
                return;
            }
            for (decl_param, concrete_param) in
                declared_fun.params.iter().zip(concrete_fun.params.iter())
            {
                collect_type_param_bindings(types, *decl_param, *concrete_param, bindings);
            }
            collect_type_param_bindings(
                types,
                declared_fun.return_ty,
                concrete_fun.return_ty,
                bindings,
            );
        }
        (
            TypeKind::Ref(RefTypeKind::Union(declared_union)),
            TypeKind::Ref(RefTypeKind::Union(concrete_union)),
        ) => {
            if declared_union.variants.len() != concrete_union.variants.len() {
                return;
            }
            for (decl_variant, concrete_variant) in declared_union
                .variants
                .iter()
                .zip(concrete_union.variants.iter())
            {
                collect_type_param_bindings(types, *decl_variant, *concrete_variant, bindings);
            }
        }
        _ => {}
    }
}
