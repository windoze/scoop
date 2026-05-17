//! Reachability analysis: walks materialized bodies (including statics, top-level inits, dispatch candidates, and rvalues) to enumerate every callee instance the materializer must produce.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn scan_materialized_family_reachable_calls(
        &mut self,
        instance: &InstanceKey,
        family: &[FunDecl],
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        let substitution = InstanceSubstitution::default();
        for fun in family {
            let Some(body) = &fun.body else {
                continue;
            };
            let locals = &body.locals;
            for block_idx in reachable_body_block_indices(body) {
                let Some(block) = body.blocks.get(block_idx) else {
                    continue;
                };
                for stmt in &block.stmts {
                    let StatementKind::Assign { target, value } = &stmt.kind else {
                        continue;
                    };
                    let result_ty = locals.get(target.as_u32() as usize).map(|local| local.ty);
                    self.collect_reachable_instances_from_rvalue(
                        value,
                        ReachableRvalueScanContext {
                            span: stmt.span,
                            result_ty,
                            template_source_path: instance.template.source_path.as_path(),
                            locals,
                            substitution: &substitution,
                        },
                        out,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn scan_reachable_non_generic_fun(
        &mut self,
        reachable_fun: &ReachableMirFun,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if self.generic_family_fqns.contains(&reachable_fun.fun.fqn) {
            return Ok(());
        }
        let scan_key = (reachable_fun.source_path.clone(), reachable_fun.fun.span);
        if !self.scanned_non_generic_funs.insert(scan_key.clone()) {
            return Ok(());
        }
        let Some(body) = &reachable_fun.fun.body else {
            return Ok(());
        };
        if let Some(hir_direct_instances) =
            self.hir_direct_instance_keys_by_fun.get(&scan_key).cloned()
        {
            out.extend(hir_direct_instances);
        }
        let substitution = InstanceSubstitution::default();
        let locals = &body.locals;
        for block_idx in reachable_body_block_indices(body) {
            let Some(block) = body.blocks.get(block_idx) else {
                continue;
            };
            for stmt in &block.stmts {
                self.reachable_request_stmt_spans
                    .push((reachable_fun.source_path.clone(), stmt.span));
                if let StatementKind::Assign { target, value } = &stmt.kind {
                    let result_ty = locals.get(target.as_u32() as usize).map(|local| local.ty);
                    self.collect_reachable_instances_from_rvalue(
                        value,
                        ReachableRvalueScanContext {
                            span: stmt.span,
                            result_ty,
                            template_source_path: &reachable_fun.source_path,
                            locals,
                            substitution: &substitution,
                        },
                        out,
                    )?;
                }
            }
        }

        let mut candidate_fun = reachable_fun.fun.clone();
        let template_root_fqn = candidate_fun.fqn.clone();
        let candidate_root_fqn = self.pass_visible_non_generic_callable_fqn(
            reachable_fun.source_path.as_path(),
            &candidate_fun,
        );
        candidate_fun.fqn = candidate_root_fqn.clone();
        if let Some(candidate_body) = candidate_fun.body.as_mut() {
            self.rewrite_reachable_body(
                candidate_body,
                &substitution,
                reachable_fun.source_path.as_path(),
                &template_root_fqn,
                &candidate_root_fqn,
            )?;
        }
        self.caller_side_pass_candidates.push(candidate_fun.clone());
        self.pass_published_ordinary_callables
            .push(PassPublishedOrdinaryCallable {
                source_path: reachable_fun.source_path.clone(),
                fun: candidate_fun,
            });
        Ok(())
    }

    pub(super) fn collect_reachable_instances_from_rvalue(
        &mut self,
        value: &Rvalue,
        scan: ReachableRvalueScanContext<'_>,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Call {
                kind: CallKind::Direct { callee_fqn },
                args,
                ..
            } => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                if let Some(instance_key) =
                    self.infer_direct_call_instance(DirectCallInferenceInput {
                        template_source_path: scan.template_source_path,
                        call_span: scan.span,
                        callee_fqn,
                        args,
                        result_ty: scan.result_ty,
                        locals: scan.locals,
                        substitution: scan.substitution,
                    })
                {
                    out.push(instance_key);
                    return Ok(());
                }
                if let Some(reachable_callee) = self.resolve_non_generic_direct_callee(
                    scan.template_source_path,
                    scan.span,
                    callee_fqn,
                    args,
                    scan.locals,
                ) {
                    self.scan_reachable_non_generic_fun(&reachable_callee, out)?;
                }
            }
            Rvalue::Call {
                kind: CallKind::Virtual { dispatch, .. },
                args,
                ..
            } => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                let receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    scan.substitution,
                );
                let candidates = self.virtual_dispatch_candidate_fqns(
                    receiver_ty,
                    &dispatch.member_name,
                    args.len(),
                );
                self.scan_reachable_dispatch_candidates(
                    scan.template_source_path,
                    &candidates,
                    out,
                )?;
            }
            Rvalue::Call {
                kind: CallKind::Interface { dispatch, .. },
                args,
                ..
            } => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                let receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    scan.substitution,
                );
                let candidates = self.interface_dispatch_candidate_fqns(
                    receiver_ty,
                    &dispatch.owner_fqn,
                    &dispatch.member_name,
                    args.len(),
                );
                self.scan_reachable_dispatch_candidates(
                    scan.template_source_path,
                    &candidates,
                    out,
                )?;
            }
            Rvalue::ClassCtor {
                class_fqn, ctor, ..
            } => {
                self.scan_reachable_class_ctor(class_fqn, ctor.selected_ctor_span, out)?;
            }
            Rvalue::MakeClosure { fn_ptr, .. } => {
                if let Some(reachable_closure) =
                    self.resolve_non_generic_fun_body_by_fqn(scan.template_source_path, fn_ptr)
                {
                    self.scan_reachable_non_generic_fun(&reachable_closure, out)?;
                }
            }
            Rvalue::TopLevelRef(TopLevelRef { fqn, .. }) => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                self.scan_reachable_top_level_ref_fqn(
                    scan.template_source_path,
                    scan.span,
                    fqn,
                    out,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn scan_reachable_class_ctor(
        &mut self,
        class_fqn: &str,
        ctor_span: Option<Span>,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        let scan_key = format!("{}:{ctor_span:?}", class_fqn);
        if !self.scanned_class_inits.insert(scan_key) {
            return Ok(());
        }
        let Some(class) = self.class_inits.get(class_fqn).cloned() else {
            return Ok(());
        };

        if let Some(super_call) = class.super_ctor_call.as_ref() {
            self.scan_reachable_class_ctor(&super_call.class_fqn, super_call.ctor_span, out)?;
        } else if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            self.scan_reachable_class_ctor(super_fqn, None, out)?;
        }
        self.scan_reachable_static_init_call_args(
            class.source_path.as_path(),
            &class.super_ctor_args,
            out,
        )?;

        let selected_ctor = ctor_span
            .and_then(|span| class.ctors.iter().find(|ctor| ctor.span == span))
            .or_else(|| {
                if ctor_span.is_none() && class.ctors.len() == 1 {
                    class.ctors.first()
                } else {
                    None
                }
            });

        if let Some(ctor) = selected_ctor {
            for param in &ctor.params {
                if let Some(default_value) = param.default_value.as_ref() {
                    self.reachable_request_stmt_spans
                        .push((class.source_path.clone(), default_value.span));
                    self.scan_reachable_static_init_expr(
                        class.source_path.as_path(),
                        default_value,
                        out,
                    )?;
                }
            }
            if let Some(delegation) = ctor.delegation.as_ref() {
                if let Some(call) = delegation.call.as_ref() {
                    self.scan_reachable_class_ctor(&call.class_fqn, call.ctor_span, out)?;
                }
                self.scan_reachable_static_init_call_args(
                    class.source_path.as_path(),
                    &delegation.args,
                    out,
                )?;
            }
        }

        for step in &class.steps {
            match step {
                crate::hir::ClassInitStep::PropertyInit { init, .. } => {
                    self.reachable_request_stmt_spans
                        .push((class.source_path.clone(), init.span));
                    self.scan_reachable_static_init_expr(class.source_path.as_path(), init, out)?;
                }
                crate::hir::ClassInitStep::InitBlock { block } => {
                    self.reachable_request_stmt_spans
                        .push((class.source_path.clone(), block.span));
                    self.scan_reachable_static_init_block(class.source_path.as_path(), block, out)?;
                }
            }
        }

        if let Some(ctor) = selected_ctor
            && let Some(body) = ctor.body.as_ref()
        {
            self.reachable_request_stmt_spans
                .push((class.source_path.clone(), body.span));
            self.scan_reachable_static_init_block(class.source_path.as_path(), body, out)?;
        }

        Ok(())
    }

    pub(super) fn scan_reachable_top_level_immutable_value_inner(
        &mut self,
        fqn: &str,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if !self
            .scanned_top_level_immutable_values
            .insert(fqn.to_string())
        {
            return Ok(());
        }
        let Some(value) = self.top_level_immutable_values.get(fqn).cloned() else {
            return Ok(());
        };
        let Some(init) = value.init else {
            return Ok(());
        };

        self.reachable_request_stmt_spans
            .push((value.source_path.clone(), init.span));
        self.scan_reachable_static_init_expr(value.source_path.as_path(), &init, out)
    }

    pub(super) fn scan_reachable_top_level_var(
        &mut self,
        fqn: &str,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if !self.scanned_top_level_vars.insert(fqn.to_string()) {
            return Ok(());
        }
        let Some(var) = self.top_level_vars.get(fqn).cloned() else {
            return Ok(());
        };
        let Some(init) = var.init else {
            return Ok(());
        };
        self.reachable_request_stmt_spans
            .push((var.source_path.clone(), init.span));
        self.scan_reachable_static_init_expr(var.source_path.as_path(), &init, out)
    }

    pub(super) fn scan_reachable_top_level_const(
        &mut self,
        fqn: &str,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if !self.scanned_top_level_consts.insert(fqn.to_string()) {
            return Ok(());
        }
        let Some(konst) = self.top_level_consts.get(fqn).cloned() else {
            return Ok(());
        };
        let Some(init) = konst.init else {
            return Ok(());
        };
        self.reachable_request_stmt_spans
            .push((konst.source_path.clone(), init.span));
        self.scan_reachable_static_init_expr(konst.source_path.as_path(), &init, out)
    }

    pub(super) fn scan_reachable_object_init(
        &mut self,
        fqn: &str,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if !self.scanned_object_inits.insert(fqn.to_string()) {
            return Ok(());
        }
        let Some(object) = self.object_inits.get(fqn).cloned() else {
            return Ok(());
        };
        for step in &object.steps {
            match step {
                crate::hir::ObjectInitStep::PropertyInit { init, .. } => {
                    self.reachable_request_stmt_spans
                        .push((object.source_path.clone(), init.span));
                    self.scan_reachable_static_init_expr(object.source_path.as_path(), init, out)?;
                }
                crate::hir::ObjectInitStep::InitBlock { block } => {
                    self.reachable_request_stmt_spans
                        .push((object.source_path.clone(), block.span));
                    self.scan_reachable_static_init_block(
                        object.source_path.as_path(),
                        block,
                        out,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn scan_reachable_top_level_ref_fqn(
        &mut self,
        source_path: &Path,
        span: Span,
        fqn: &str,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if let Some(binding) = self.site_instance_binding_for_callee(source_path, span, fqn)
            && let Some(instance_key) =
                self.instantiate_site_binding(&binding, &InstanceSubstitution::default())
        {
            out.push(instance_key);
            return Ok(());
        }
        if let Some(reachable_fun) = self.resolve_non_generic_fun_body_by_fqn(source_path, fqn) {
            self.scan_reachable_non_generic_fun(&reachable_fun, out)?;
        }
        self.scan_reachable_top_level_const(fqn, out)?;
        self.scan_reachable_top_level_var(fqn, out)?;
        self.scan_reachable_top_level_immutable_value_inner(fqn, out)?;
        self.scan_reachable_object_init(fqn, out)
    }

    pub(super) fn scan_reachable_static_init_block(
        &mut self,
        source_path: &Path,
        block: &crate::hir::Block,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        for stmt in &block.stmts {
            match &stmt.kind {
                crate::hir::StmtKind::Expr(expr) => {
                    self.scan_reachable_static_init_expr(source_path, expr, out)?;
                }
                crate::hir::StmtKind::Val(decl) => {
                    if let Some(init) = &decl.init {
                        self.scan_reachable_static_init_expr(source_path, init, out)?;
                    }
                }
                crate::hir::StmtKind::Assign { lhs, rhs, .. } => {
                    self.scan_reachable_static_init_expr(source_path, lhs, out)?;
                    self.scan_reachable_static_init_expr(source_path, rhs, out)?;
                }
                crate::hir::StmtKind::While { cond, body } => {
                    self.scan_reachable_static_init_expr(source_path, cond, out)?;
                    self.scan_reachable_static_init_block(source_path, body, out)?;
                }
                crate::hir::StmtKind::Return { value } => {
                    if let Some(value) = value {
                        self.scan_reachable_static_init_expr(source_path, value, out)?;
                    }
                }
                crate::hir::StmtKind::Empty
                | crate::hir::StmtKind::Break { .. }
                | crate::hir::StmtKind::Continue { .. }
                | crate::hir::StmtKind::Todo(_) => {}
            }
        }
        Ok(())
    }

    pub(super) fn scan_reachable_static_init_call_args(
        &mut self,
        source_path: &Path,
        args: &[crate::hir::CallArg],
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        for arg in args {
            match arg {
                crate::hir::CallArg::Positional(value)
                | crate::hir::CallArg::Named { value, .. } => {
                    self.scan_reachable_static_init_expr(source_path, value, out)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn scan_reachable_static_init_expr(
        &mut self,
        source_path: &Path,
        expr: &crate::hir::Expr,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        match &expr.kind {
            crate::hir::ExprKind::VarRef(crate::hir::ValueRef::TopLevel { fqn, .. }) => {
                self.scan_reachable_top_level_ref_fqn(source_path, expr.span, fqn, out)?;
            }
            crate::hir::ExprKind::Call { callee, args } => {
                if let Some(call) = self
                    .ctor_call_sites
                    .get(&crate::hir::CallSite::new(
                        source_path.to_path_buf(),
                        expr.span,
                    ))
                    .cloned()
                {
                    self.scan_reachable_class_ctor(&call.class_fqn, call.ctor_span, out)?;
                }

                if let crate::hir::ExprKind::VarRef(crate::hir::ValueRef::TopLevel {
                    fqn, ..
                }) = &callee.kind
                {
                    self.scan_reachable_top_level_ref_fqn(source_path, expr.span, fqn, out)?;
                } else {
                    self.scan_reachable_static_init_expr(source_path, callee, out)?;
                }
                for arg in args {
                    match arg {
                        crate::hir::CallArg::Positional(value) => {
                            self.scan_reachable_static_init_expr(source_path, value, out)?;
                        }
                        crate::hir::CallArg::Named { value, .. } => {
                            self.scan_reachable_static_init_expr(source_path, value, out)?;
                        }
                    }
                }
            }
            crate::hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.scan_reachable_static_init_expr(source_path, &field.value, out)?;
                }
            }
            crate::hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.scan_reachable_static_init_expr(source_path, element, out)?;
                }
            }
            crate::hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        self.scan_reachable_static_init_expr(source_path, expr, out)?;
                    }
                }
            }
            crate::hir::ExprKind::Unary { expr, .. }
            | crate::hir::ExprKind::TypeCheck { expr, .. }
            | crate::hir::ExprKind::Cast { expr, .. }
            | crate::hir::ExprKind::MemberAccess { receiver: expr, .. } => {
                self.scan_reachable_static_init_expr(source_path, expr, out)?;
            }
            crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.scan_reachable_static_init_expr(source_path, lhs, out)?;
                self.scan_reachable_static_init_expr(source_path, rhs, out)?;
            }
            crate::hir::ExprKind::Block(block) => {
                self.scan_reachable_static_init_block(source_path, block, out)?;
            }
            crate::hir::ExprKind::Closure(closure) => {
                self.scan_reachable_static_init_expr(source_path, &closure.body, out)?;
            }
            crate::hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.scan_reachable_static_init_expr(source_path, cond, out)?;
                self.scan_reachable_static_init_expr(source_path, then_branch, out)?;
                if let Some(else_branch) = else_branch {
                    self.scan_reachable_static_init_expr(source_path, else_branch, out)?;
                }
            }
            crate::hir::ExprKind::When { subject, arms } => {
                self.scan_reachable_static_init_expr(source_path, subject, out)?;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.scan_reachable_static_init_expr(source_path, guard, out)?;
                    }
                    self.scan_reachable_static_init_expr(source_path, &arm.body, out)?;
                }
            }
            crate::hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        crate::hir::CallArg::Positional(value) => {
                            self.scan_reachable_static_init_expr(source_path, value, out)?;
                        }
                        crate::hir::CallArg::Named { value, .. } => {
                            self.scan_reachable_static_init_expr(source_path, value, out)?;
                        }
                    }
                }
            }
            crate::hir::ExprKind::Handle(handle) => {
                self.scan_reachable_static_init_block(source_path, &handle.body, out)?;
                for arm in &handle.arms {
                    self.scan_reachable_static_init_expr(source_path, &arm.body, out)?;
                }
                if let Some(finally) = &handle.finally {
                    self.scan_reachable_static_init_block(source_path, finally, out)?;
                }
            }
            crate::hir::ExprKind::Missing
            | crate::hir::ExprKind::Literal(_)
            | crate::hir::ExprKind::VarRef(_)
            | crate::hir::ExprKind::UnresolvedIdent { .. }
            | crate::hir::ExprKind::ClassLiteral(_)
            | crate::hir::ExprKind::Todo(_) => {}
        }
        Ok(())
    }

    pub(super) fn scan_reachable_dispatch_candidates(
        &mut self,
        default_source_path: &Path,
        candidate_fqns: &[String],
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        for candidate_fqn in candidate_fqns {
            if let Some(instance_key) = self.explicit_dispatch_candidate_instance(candidate_fqn) {
                out.push(instance_key);
                continue;
            }
            if let Some(reachable_fun) =
                self.resolve_non_generic_fun_body_by_fqn(default_source_path, candidate_fqn)
            {
                self.scan_reachable_non_generic_fun(&reachable_fun, out)?;
            }
        }
        Ok(())
    }

    pub(super) fn collect_explicit_dispatch_candidate_instances(
        &mut self,
        typecheck_types: &TypeStore,
    ) -> HashMap<String, Vec<InstanceKey>> {
        let mut out = HashMap::<String, Vec<InstanceKey>>::new();
        let all_type_ids = typecheck_types.iter_ids().collect::<Vec<_>>();

        for templates in self.roots_by_fqn.values() {
            for template in templates {
                let Some(signature) = self.template_signatures.get(template) else {
                    continue;
                };
                if signature.eff_param_name.is_some() {
                    continue;
                }
                let Some((owner_fqn, _)) = template.fqn.rsplit_once('.') else {
                    continue;
                };
                let Some(receiver_param) = signature.params.first() else {
                    continue;
                };
                if nominal_type_fqn(&self.types, receiver_param.ty) != Some(owner_fqn) {
                    continue;
                }

                for ty_id in &all_type_ids {
                    let nominal = match typecheck_types.kind(*ty_id) {
                        TypeKind::Ref(RefTypeKind::Nominal(nominal))
                        | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                            if nominal.fqn == owner_fqn =>
                        {
                            nominal
                        }
                        _ => continue,
                    };
                    if nominal.args.is_empty()
                        || nominal
                            .args
                            .iter()
                            .any(|ty| type_contains_param(typecheck_types, *ty))
                        || nominal.args.len() != signature.type_param_names.len()
                    {
                        continue;
                    }

                    let instance = InstanceKey {
                        template: template.clone(),
                        type_args: nominal
                            .args
                            .iter()
                            .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                            .collect(),
                        eff_args: Vec::new(),
                    };
                    let display_fqn = self.instance_display_fqn(&instance);
                    let entry = out.entry(display_fqn).or_default();
                    if !entry.contains(&instance) {
                        entry.push(instance);
                    }
                }
            }
        }

        out
    }

    pub(super) fn explicit_dispatch_candidate_instance(
        &self,
        candidate_fqn: &str,
    ) -> Option<InstanceKey> {
        match self
            .explicit_dispatch_candidate_instances
            .get(candidate_fqn)
            .map(Vec::as_slice)
        {
            Some([instance]) => Some(instance.clone()),
            _ => None,
        }
    }

    pub(super) fn virtual_dispatch_candidate_fqns(
        &self,
        receiver_ty: TypeId,
        member_name: &str,
        explicit_arg_count: usize,
    ) -> Vec<String> {
        let Some(receiver_fqn) = nominal_type_fqn(&self.types, receiver_ty) else {
            return Vec::new();
        };
        let mut targets = BTreeSet::new();
        for class_fqn in self.descendants_and_self(receiver_fqn) {
            if let Some(slot) = self
                .class_vtables
                .get(class_fqn.as_str())
                .and_then(|slots| {
                    slots.iter().find(|slot| {
                        slot.name == member_name && slot.params_len == explicit_arg_count as u32
                    })
                })
            {
                targets.insert(slot.impl_member_fqn.clone());
            } else if class_fqn == receiver_fqn {
                targets.insert(format!("{class_fqn}.{member_name}"));
            }
        }
        targets.into_iter().collect()
    }

    pub(super) fn interface_dispatch_candidate_fqns(
        &self,
        receiver_ty: TypeId,
        owner_fqn: &str,
        member_name: &str,
        explicit_arg_count: usize,
    ) -> Vec<String> {
        let Some(interface) = self.interfaces.get(owner_fqn) else {
            return Vec::new();
        };
        let mut matching_slots = interface.method_slots.iter().filter(|slot| {
            slot.name == member_name && slot.params_len == explicit_arg_count as u32
        });
        let Some(slot) = matching_slots.next() else {
            return Vec::new();
        };
        if matching_slots.next().is_some() {
            return Vec::new();
        }

        let mut targets = BTreeSet::new();
        if let Some(receiver_fqn) = nominal_type_fqn(&self.types, receiver_ty)
            && let Some(entries) = self.class_itables.get(receiver_fqn)
        {
            collect_interface_slot_targets(entries, owner_fqn, slot.slot as usize, &mut targets);
        }
        if targets.is_empty() {
            for entries in self.class_itables.values() {
                collect_interface_slot_targets(
                    entries,
                    owner_fqn,
                    slot.slot as usize,
                    &mut targets,
                );
            }
        }
        if is_builtin_scalar_or_string_type(&self.types, receiver_ty) {
            targets.remove(&format!("{owner_fqn}.{member_name}"));
        }
        targets.into_iter().collect()
    }

    pub(super) fn descendants_and_self(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::from([root.to_string()]);
        let mut stack = vec![root.to_string()];
        while let Some(current) = stack.pop() {
            if let Some(children) = self.direct_subclasses.get(&current) {
                for child in children {
                    if seen.insert(child.clone()) {
                        stack.push(child.clone());
                    }
                }
            }
        }
        seen
    }
}
