use std::path::Path;

use crate::hir::{
    Block, CallArg, ClassCtor, ClassInit, ClassInitStep, Expr, ExprKind, FunDecl, HirStageError,
    Item, LoweredHir, ObjectInit, ObjectInitStep, Stmt, StmtKind, ValDecl,
};
use crate::span::Span;

/// Verifies that the refactor typed HIR handoff contains no placeholder HIR nodes.
pub(crate) struct RefactorHirCompletenessVerifier<'a> {
    lowered_hir: &'a LoweredHir,
    default_source_path: &'a Path,
}

impl<'a> RefactorHirCompletenessVerifier<'a> {
    pub(crate) fn new(lowered_hir: &'a LoweredHir, default_source_path: &'a Path) -> Self {
        Self {
            lowered_hir,
            default_source_path,
        }
    }

    pub(crate) fn verify(&self) -> Result<(), HirStageError> {
        self.verify_file()?;
        self.verify_member_funs()?;
        self.verify_top_level_init_roots()?;
        self.verify_object_init_roots()?;
        self.verify_class_init_roots()?;
        Ok(())
    }

    fn verify_file(&self) -> Result<(), HirStageError> {
        for item in &self.lowered_hir.file.items {
            self.verify_item(item)?;
        }
        Ok(())
    }

    fn verify_member_funs(&self) -> Result<(), HirStageError> {
        for fun in &self.lowered_hir.member_funs {
            let owner = format!("member fun {}", fun.fqn);
            self.verify_fun(&fun.source_path, fun, &owner)?;
        }
        Ok(())
    }

    fn verify_top_level_init_roots(&self) -> Result<(), HirStageError> {
        let mut vars = self.lowered_hir.top_level_vars.values().collect::<Vec<_>>();
        vars.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for var in vars {
            let owner = format!("top-level var {}", var.fqn);
            if let Some(init) = &var.init {
                self.verify_expr(&var.source_path, init, &owner)?;
            }
        }

        let mut consts = self
            .lowered_hir
            .top_level_consts
            .values()
            .collect::<Vec<_>>();
        consts.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for konst in consts {
            let owner = format!("top-level const {}", konst.fqn);
            if let Some(init) = &konst.init {
                self.verify_expr(&konst.source_path, init, &owner)?;
            }
        }

        let mut values = self
            .lowered_hir
            .top_level_immutable_values
            .values()
            .collect::<Vec<_>>();
        values.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for value in values {
            let owner = format!("top-level val {}", value.fqn);
            if let Some(init) = &value.init {
                self.verify_expr(&value.source_path, init, &owner)?;
            }
        }

        Ok(())
    }

    fn verify_object_init_roots(&self) -> Result<(), HirStageError> {
        let mut object_inits = self.lowered_hir.object_inits.values().collect::<Vec<_>>();
        object_inits.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for object_init in object_inits {
            self.verify_object_init(object_init)?;
        }
        Ok(())
    }

    fn verify_class_init_roots(&self) -> Result<(), HirStageError> {
        let mut class_inits = self.lowered_hir.class_inits.values().collect::<Vec<_>>();
        class_inits.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for class_init in class_inits {
            self.verify_class_init(class_init)?;
        }
        Ok(())
    }

    fn verify_item(&self, item: &Item) -> Result<(), HirStageError> {
        match item {
            Item::Fun(fun) => {
                let owner = format!("fun {}", fun.fqn);
                self.verify_fun(&fun.source_path, fun, &owner)
            }
            Item::Val(val) => {
                let owner = top_level_val_owner(val);
                self.verify_val(self.default_source_path, val, &owner)
            }
            Item::Todo { span, kind } => self.placeholder(
                self.default_source_path,
                *span,
                "Item::Todo",
                kind,
                "top-level item",
            ),
        }
    }

    fn verify_fun(
        &self,
        source_path: &Path,
        fun: &FunDecl,
        owner: &str,
    ) -> Result<(), HirStageError> {
        if let Some(body) = &fun.body {
            self.verify_block(source_path, body, owner)?;
        }
        Ok(())
    }

    fn verify_val(
        &self,
        source_path: &Path,
        val: &ValDecl,
        owner: &str,
    ) -> Result<(), HirStageError> {
        if let Some(init) = &val.init {
            self.verify_expr(source_path, init, owner)?;
        }
        Ok(())
    }

    fn verify_object_init(&self, object_init: &ObjectInit) -> Result<(), HirStageError> {
        let owner = format!("object {}", object_init.fqn);
        for step in &object_init.steps {
            match step {
                ObjectInitStep::PropertyInit { init, .. } => {
                    self.verify_expr(&object_init.source_path, init, &owner)?;
                }
                ObjectInitStep::InitBlock { block } => {
                    self.verify_block(&object_init.source_path, block, &owner)?;
                }
            }
        }
        Ok(())
    }

    fn verify_class_init(&self, class_init: &ClassInit) -> Result<(), HirStageError> {
        let owner = format!("class {}", class_init.fqn);
        for arg in &class_init.super_ctor_args {
            self.verify_call_arg(&class_init.source_path, arg, &owner)?;
        }
        for step in &class_init.steps {
            match step {
                ClassInitStep::PropertyInit { init, .. } => {
                    self.verify_expr(&class_init.source_path, init, &owner)?;
                }
                ClassInitStep::InitBlock { block } => {
                    self.verify_block(&class_init.source_path, block, &owner)?;
                }
            }
        }
        for ctor in &class_init.ctors {
            self.verify_class_ctor(class_init, ctor)?;
        }
        Ok(())
    }

    fn verify_class_ctor(
        &self,
        class_init: &ClassInit,
        ctor: &ClassCtor,
    ) -> Result<(), HirStageError> {
        let owner = format!("class {} {:?} ctor", class_init.fqn, ctor.kind);
        for param in &ctor.params {
            if let Some(default_value) = &param.default_value {
                self.verify_expr(&class_init.source_path, default_value, &owner)?;
            }
        }
        if let Some(delegation) = &ctor.delegation {
            for arg in &delegation.args {
                self.verify_call_arg(&class_init.source_path, arg, &owner)?;
            }
        }
        if let Some(body) = &ctor.body {
            self.verify_block(&class_init.source_path, body, &owner)?;
        }
        Ok(())
    }

    fn verify_block(
        &self,
        source_path: &Path,
        block: &Block,
        owner: &str,
    ) -> Result<(), HirStageError> {
        for stmt in &block.stmts {
            self.verify_stmt(source_path, stmt, owner)?;
        }
        Ok(())
    }

    fn verify_stmt(
        &self,
        source_path: &Path,
        stmt: &Stmt,
        owner: &str,
    ) -> Result<(), HirStageError> {
        match &stmt.kind {
            StmtKind::Empty | StmtKind::Break { .. } | StmtKind::Continue { .. } => Ok(()),
            StmtKind::Todo(kind) => {
                self.placeholder(source_path, stmt.span, "StmtKind::Todo", kind, owner)
            }
            StmtKind::Expr(expr) => self.verify_expr(source_path, expr, owner),
            StmtKind::Val(val) => self.verify_val(source_path, val, owner),
            StmtKind::Assign { lhs, rhs, .. } => {
                self.verify_expr(source_path, lhs, owner)?;
                self.verify_expr(source_path, rhs, owner)
            }
            StmtKind::While { cond, body } => {
                self.verify_expr(source_path, cond, owner)?;
                self.verify_block(source_path, body, owner)
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    self.verify_expr(source_path, value, owner)?;
                }
                Ok(())
            }
        }
    }

    fn verify_expr(
        &self,
        source_path: &Path,
        expr: &Expr,
        owner: &str,
    ) -> Result<(), HirStageError> {
        match &expr.kind {
            ExprKind::Missing => self.fail(source_path, expr.span, "ExprKind::Missing", owner),
            ExprKind::Todo(kind) => {
                self.placeholder(source_path, expr.span, "ExprKind::Todo", kind, owner)
            }
            ExprKind::Literal(_) | ExprKind::VarRef(_) | ExprKind::UnresolvedIdent { .. } => Ok(()),
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.verify_expr(source_path, &field.value, owner)?;
                }
                Ok(())
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.verify_expr(source_path, element, owner)?;
                }
                Ok(())
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        self.verify_expr(source_path, expr, owner)?;
                    }
                }
                Ok(())
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::MemberAccess { receiver: expr, .. } => {
                self.verify_expr(source_path, expr, owner)
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.verify_expr(source_path, lhs, owner)?;
                self.verify_expr(source_path, rhs, owner)
            }
            ExprKind::Block(block) => self.verify_block(source_path, block, owner),
            ExprKind::Closure(closure) => self.verify_expr(source_path, &closure.body, owner),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.verify_expr(source_path, cond, owner)?;
                self.verify_expr(source_path, then_branch, owner)?;
                if let Some(else_branch) = else_branch {
                    self.verify_expr(source_path, else_branch, owner)?;
                }
                Ok(())
            }
            ExprKind::When { subject, arms } => {
                self.verify_expr(source_path, subject, owner)?;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.verify_expr(source_path, guard, owner)?;
                    }
                    self.verify_expr(source_path, &arm.body, owner)?;
                }
                Ok(())
            }
            ExprKind::Call { callee, args } => {
                self.verify_expr(source_path, callee, owner)?;
                for arg in args {
                    self.verify_call_arg(source_path, arg, owner)?;
                }
                Ok(())
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    self.verify_call_arg(source_path, arg, owner)?;
                }
                Ok(())
            }
            ExprKind::Handle(handle) => {
                self.verify_block(source_path, &handle.body, owner)?;
                for arm in &handle.arms {
                    self.verify_expr(source_path, &arm.body, owner)?;
                }
                if let Some(finally) = &handle.finally {
                    self.verify_block(source_path, finally, owner)?;
                }
                Ok(())
            }
        }
    }

    fn verify_call_arg(
        &self,
        source_path: &Path,
        arg: &CallArg,
        owner: &str,
    ) -> Result<(), HirStageError> {
        match arg {
            CallArg::Positional(expr) => self.verify_expr(source_path, expr, owner),
            CallArg::Named { value, .. } => self.verify_expr(source_path, value, owner),
        }
    }

    fn placeholder(
        &self,
        source_path: &Path,
        span: Span,
        surface: &'static str,
        kind: &str,
        owner: &str,
    ) -> Result<(), HirStageError> {
        self.fail(source_path, span, format!("{surface}({kind})"), owner)
    }

    fn fail(
        &self,
        source_path: &Path,
        span: Span,
        reason: impl Into<String>,
        owner: &str,
    ) -> Result<(), HirStageError> {
        Err(HirStageError::new(
            source_path.to_path_buf(),
            span,
            reason,
            owner.to_string(),
        ))
    }
}

fn top_level_val_owner(val: &ValDecl) -> String {
    match &val.name {
        Some(name) => format!("top-level val {name}"),
        None => "top-level val <pattern>".to_string(),
    }
}
