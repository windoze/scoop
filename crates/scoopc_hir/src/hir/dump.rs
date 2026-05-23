//! Stable HIR dump renderer used by CLI and fixture baselines.

use std::collections::HashMap;
use std::path::Path;

use crate::dump_support::{IndentWriter, LocalEntityKey, format_debug, format_type};
use crate::ty::TypeStore;

use super::{
    AccessorContract, Block, CallArg, Capture, ClassLiteralExpr, ClosureExpr, CtorDecl,
    CtorParamDecl, Decl, DeclMember, DeclTypeParam, EffectOpRef, EnumVariantDecl, Expr, ExprKind,
    ExtensionPropertyDecl, FieldDecl, File, FunDecl, HandleArm, HandleArmKind, HandleBinder,
    HandleExpr, HandleOp, InterpolatedStringPart, Item, LiteralKind, MemberAccess, MemberFunDecl,
    MemberRef, NominalDecl, ObjectDecl, Param, PropertyDecl, Stmt, StmtKind, StructLitField,
    SupertypeDecl, SymbolId, TypeAliasDecl, ValDecl, ValueRef, WhenArm, WhenPat,
};

pub fn stable_dump_file(file: &File, types: &TypeStore, source_path: &Path) -> String {
    let mut renderer = HirDumpRenderer::new(types, source_path, collect_symbol_decl_spans(file));
    renderer.render_file(file);
    renderer.finish()
}

struct HirDumpRenderer<'a> {
    types: &'a TypeStore,
    source_path: &'a Path,
    symbol_spans: HashMap<SymbolId, crate::span::Span>,
    out: IndentWriter,
}

impl<'a> HirDumpRenderer<'a> {
    fn new(
        types: &'a TypeStore,
        source_path: &'a Path,
        symbol_spans: HashMap<SymbolId, crate::span::Span>,
    ) -> Self {
        Self {
            types,
            source_path,
            symbol_spans,
            out: IndentWriter::new(),
        }
    }

    fn finish(self) -> String {
        self.out.finish()
    }

    fn render_file(&mut self, file: &File) {
        self.open_struct("File");
        if !file.decls.is_empty() {
            self.open_list_field("decls");
            for decl in &file.decls {
                self.render_decl(decl);
            }
            self.close_list_field();
        }
        self.open_list_field("items");
        for item in &file.items {
            self.render_item(item);
        }
        self.close_list_field();
        self.close_struct("");
    }

    fn render_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::TypeAlias(alias) => self.render_variant("TypeAlias", |this| {
                this.render_type_alias_decl(alias);
            }),
            Decl::Nominal(nominal) => self.render_variant("Nominal", |this| {
                this.render_nominal_decl(nominal);
            }),
            Decl::Object(object) => self.render_variant("Object", |this| {
                this.render_object_decl(object);
            }),
            Decl::ExtensionProperty(prop) => self.render_variant("ExtensionProperty", |this| {
                this.render_extension_property_decl(prop);
            }),
        }
    }

    fn render_item(&mut self, item: &Item) {
        match item {
            Item::Fun(fun) => self.render_variant("Fun", |this| {
                this.render_fun_decl(fun);
            }),
            Item::Val(val) => {
                let owner = self.top_level_owner(val);
                self.render_variant("Val", |this| {
                    this.render_val_decl(&owner, val);
                })
            }
            Item::Todo { span, kind } => {
                self.open_struct("Todo");
                self.field_debug("span", span);
                self.field_debug("kind", kind);
                self.close_struct(",");
            }
        }
    }

    fn render_type_alias_decl(&mut self, alias: &TypeAliasDecl) {
        self.open_struct("TypeAliasDecl");
        self.field_debug("span", &alias.span);
        self.field_debug("fqn", &alias.fqn);
        self.field_debug("name", &alias.name);
        self.render_decl_type_params("type_params", &alias.type_params);
        self.field_type("ty", alias.ty);
        self.close_struct(",");
    }

    fn render_nominal_decl(&mut self, nominal: &NominalDecl) {
        self.open_struct("NominalDecl");
        self.field_debug("span", &nominal.span);
        self.field_debug("fqn", &nominal.fqn);
        self.field_debug("name", &nominal.name);
        self.field_debug("kind", &nominal.kind);
        self.render_decl_type_params("type_params", &nominal.type_params);
        self.open_list_field("supertypes");
        for supertype in &nominal.supertypes {
            self.render_supertype_decl(supertype);
        }
        self.close_list_field();
        self.render_debug_string_list("interfaces", &nominal.interfaces);
        self.open_list_field("constructors");
        for ctor in &nominal.constructors {
            self.render_ctor_decl(ctor);
        }
        self.close_list_field();
        self.open_list_field("members");
        for member in &nominal.members {
            self.render_decl_member(member);
        }
        self.close_list_field();
        self.close_struct(",");
    }

    fn render_object_decl(&mut self, object: &ObjectDecl) {
        self.open_struct("ObjectDecl");
        self.field_debug("span", &object.span);
        self.field_debug("fqn", &object.fqn);
        self.field_debug("name", &object.name);
        self.field_debug("kind", &object.kind);
        self.open_list_field("supertypes");
        for supertype in &object.supertypes {
            self.render_supertype_decl(supertype);
        }
        self.close_list_field();
        self.render_debug_string_list("interfaces", &object.interfaces);
        self.field_debug("initializer_root", &object.initializer_root);
        self.open_list_field("members");
        for member in &object.members {
            self.render_decl_member(member);
        }
        self.close_list_field();
        self.close_struct(",");
    }

    fn render_extension_property_decl(&mut self, prop: &ExtensionPropertyDecl) {
        self.open_struct("ExtensionPropertyDecl");
        self.field_debug("span", &prop.span);
        self.field_debug("fqn", &prop.fqn);
        self.field_debug("name", &prop.name);
        self.field_bool("mutable", prop.mutable);
        self.render_decl_type_params("type_params", &prop.type_params);
        self.field_type("receiver_ty", prop.receiver_ty);
        self.field_type("ty", prop.ty);
        self.render_accessor_contract_field("getter", prop.getter.as_ref());
        self.render_accessor_contract_field("setter", prop.setter.as_ref());
        self.close_struct(",");
    }

    fn render_decl_member(&mut self, member: &DeclMember) {
        match member {
            DeclMember::Field(field) => self.render_variant("Field", |this| {
                this.render_field_decl(field);
            }),
            DeclMember::Property(prop) => self.render_variant("Property", |this| {
                this.render_property_decl(prop);
            }),
            DeclMember::Fun(fun) => self.render_variant("Fun", |this| {
                this.render_member_fun_decl(fun);
            }),
            DeclMember::EnumVariant(variant) => self.render_variant("EnumVariant", |this| {
                this.render_enum_variant_decl(variant);
            }),
            DeclMember::InitBlock { span } => {
                self.open_struct("InitBlock");
                self.field_debug("span", span);
                self.close_struct(",");
            }
            DeclMember::Nested(decl) => self.render_variant("Nested", |this| {
                this.render_decl(decl);
            }),
        }
    }

    fn render_field_decl(&mut self, field: &FieldDecl) {
        self.open_struct("FieldDecl");
        self.field_debug("span", &field.span);
        self.field_debug("fqn", &field.fqn);
        self.field_debug("name", &field.name);
        self.field_bool("mutable", field.mutable);
        self.field_type("ty", field.ty);
        self.field_debug("origin", &field.origin);
        self.close_struct(",");
    }

    fn render_property_decl(&mut self, prop: &PropertyDecl) {
        self.open_struct("PropertyDecl");
        self.field_debug("span", &prop.span);
        self.field_debug("fqn", &prop.fqn);
        self.field_debug("name", &prop.name);
        self.field_bool("mutable", prop.mutable);
        self.field_type("ty", prop.ty);
        self.field_bool("has_backing_field", prop.has_backing_field);
        self.render_accessor_contract_field("getter", prop.getter.as_ref());
        self.render_accessor_contract_field("setter", prop.setter.as_ref());
        self.close_struct(",");
    }

    fn render_member_fun_decl(&mut self, fun: &MemberFunDecl) {
        self.open_struct("MemberFunDecl");
        self.field_debug("span", &fun.span);
        self.field_debug("fqn", &fun.fqn);
        self.field_debug("name", &fun.name);
        self.render_decl_type_params("type_params", &fun.type_params);
        self.open_list_field("params");
        for param in &fun.params {
            self.render_ctor_param_decl(param);
        }
        self.close_list_field();
        self.field_type("return_ty", fun.return_ty);
        self.close_struct(",");
    }

    fn render_enum_variant_decl(&mut self, variant: &EnumVariantDecl) {
        self.open_struct("EnumVariantDecl");
        self.field_debug("span", &variant.span);
        self.field_debug("fqn", &variant.fqn);
        self.field_debug("name", &variant.name);
        self.open_list_field("fields");
        for field in &variant.fields {
            self.render_field_decl(field);
        }
        self.close_list_field();
        self.close_struct(",");
    }

    fn render_ctor_decl(&mut self, ctor: &CtorDecl) {
        self.open_struct("CtorDecl");
        self.field_debug("span", &ctor.span);
        self.field_debug("kind", &ctor.kind);
        self.open_list_field("params");
        for param in &ctor.params {
            self.render_ctor_param_decl(param);
        }
        self.close_list_field();
        self.render_option_debug_field("delegation", ctor.delegation.as_ref());
        self.close_struct(",");
    }

    fn render_ctor_param_decl(&mut self, param: &CtorParamDecl) {
        self.open_struct("CtorParamDecl");
        self.field_debug("span", &param.span);
        self.field_debug("name", &param.name);
        self.field_type("ty", param.ty);
        self.field_bool("has_default", param.has_default);
        self.render_option_debug_field("property", param.property.as_ref());
        self.close_struct(",");
    }

    fn render_supertype_decl(&mut self, supertype: &SupertypeDecl) {
        self.open_struct("SupertypeDecl");
        self.field_debug("span", &supertype.span);
        self.render_option_debug_field("fqn", supertype.fqn.as_ref());
        self.field_type("ty", supertype.ty);
        self.field_usize("ctor_arg_count", supertype.ctor_arg_count);
        self.close_struct(",");
    }

    fn render_fun_decl(&mut self, fun: &FunDecl) {
        self.open_struct("FunDecl");
        self.field_debug("span", &fun.span);
        self.field_debug("fqn", &fun.fqn);
        self.field_debug("name", &fun.name);
        self.field_type("ty", fun.ty);
        self.open_list_field("params");
        for param in &fun.params {
            self.render_param(param);
        }
        self.close_list_field();
        self.field_type("return_ty", fun.return_ty);
        self.render_option_block_field(&fun.fqn, "body", fun.body.as_ref());
        self.close_struct(",");
    }

    fn render_param(&mut self, param: &Param) {
        self.open_struct("Param");
        self.field_debug("span", &param.span);
        self.field_label("label", self.symbol_label(&param.name, param.span));
        self.field_debug("name", &param.name);
        self.field_type("ty", param.ty);
        self.close_struct(",");
    }

    fn render_val_decl(&mut self, owner: &str, val: &ValDecl) {
        self.open_struct("ValDecl");
        self.field_debug("span", &val.span);
        match val.name.as_deref() {
            Some(name) => {
                self.field_label(
                    "label",
                    self.symbol_label(name, self.resolved_symbol_span(val.id, val.span)),
                );
                self.field_debug("name", name);
            }
            None => {
                self.field_label("label", self.synthetic_label("val", val.span));
                self.field_debug("name", &Option::<String>::None);
            }
        }
        self.field_bool("mutable", val.mutable);
        self.field_type("ty", val.ty);
        self.render_option_expr_field(owner, "init", val.init.as_ref());
        self.close_struct(",");
    }

    fn render_block(&mut self, owner: &str, block: &Block) {
        self.open_struct("Block");
        self.field_debug("span", &block.span);
        self.field_type("ty", block.ty);
        self.open_list_field("stmts");
        for stmt in &block.stmts {
            self.render_stmt(owner, stmt);
        }
        self.close_list_field();
        self.close_struct("");
    }

    fn render_stmt(&mut self, owner: &str, stmt: &Stmt) {
        self.open_struct("Stmt");
        self.field_debug("span", &stmt.span);
        self.field_type("ty", stmt.ty);
        self.render_stmt_kind(owner, &stmt.kind);
        self.close_struct(",");
    }

    fn render_stmt_kind(&mut self, owner: &str, kind: &StmtKind) {
        match kind {
            StmtKind::Empty => self.field_raw("kind", "Empty"),
            StmtKind::Expr(expr) => {
                self.line("kind: Expr(");
                self.out.push_indent();
                self.render_expr(owner, expr);
                self.out.pop_indent();
                self.line("),");
            }
            StmtKind::Val(val) => {
                self.line("kind: Val(");
                self.out.push_indent();
                self.render_val_decl(owner, val);
                self.out.pop_indent();
                self.line("),");
            }
            StmtKind::Assign { lhs, eq_span, rhs } => {
                self.line("kind: Assign {");
                self.out.push_indent();
                self.render_expr_field(owner, "lhs", lhs);
                self.field_debug("eq_span", eq_span);
                self.render_expr_field(owner, "rhs", rhs);
                self.out.pop_indent();
                self.line("},");
            }
            StmtKind::While { cond, body } => {
                self.line("kind: While {");
                self.out.push_indent();
                self.render_expr_field(owner, "cond", cond);
                self.render_block_field(owner, "body", body);
                self.out.pop_indent();
                self.line("},");
            }
            StmtKind::Break { break_span } => {
                self.line("kind: Break {");
                self.out.push_indent();
                self.field_debug("break_span", break_span);
                self.out.pop_indent();
                self.line("},");
            }
            StmtKind::Continue { continue_span } => {
                self.line("kind: Continue {");
                self.out.push_indent();
                self.field_debug("continue_span", continue_span);
                self.out.pop_indent();
                self.line("},");
            }
            StmtKind::Return { value } => {
                self.line("kind: Return {");
                self.out.push_indent();
                self.render_option_expr_field(owner, "value", value.as_ref());
                self.out.pop_indent();
                self.line("},");
            }
            StmtKind::Todo(reason) => self.field_raw("kind", &format!("Todo({reason:?})")),
        }
    }

    fn render_expr(&mut self, owner: &str, expr: &Expr) {
        self.open_struct("Expr");
        self.field_debug("span", &expr.span);
        self.field_type("ty", expr.ty);
        self.render_expr_kind(owner, expr);
        self.close_struct("");
    }

    fn render_expr_kind(&mut self, owner: &str, expr: &Expr) {
        match &expr.kind {
            ExprKind::Missing => self.field_raw("kind", "Missing"),
            ExprKind::Literal(literal) => {
                self.field_raw("kind", &format!("Literal({})", self.literal_text(literal)));
            }
            ExprKind::VarRef(value) => {
                self.line("kind: VarRef(");
                self.out.push_indent();
                self.render_value_ref(value);
                self.out.pop_indent();
                self.line("),");
            }
            ExprKind::UnresolvedIdent { name } => {
                self.line("kind: UnresolvedIdent {");
                self.out.push_indent();
                self.field_debug("name", name);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::StructLit { ty, fields } => {
                self.line("kind: StructLit {");
                self.out.push_indent();
                self.field_type("ty", *ty);
                self.open_list_field("fields");
                for field in fields {
                    self.render_struct_lit_field(owner, field);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::ClassLiteral(class_lit) => {
                self.line("kind: ClassLiteral(");
                self.out.push_indent();
                self.render_class_literal_expr(class_lit);
                self.out.pop_indent();
                self.line("),");
            }
            ExprKind::TupleLit { elements } => {
                self.line("kind: TupleLit {");
                self.out.push_indent();
                self.open_list_field("elements");
                for element in elements {
                    self.render_expr(owner, element);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::InterpolatedString { raw, parts } => {
                self.line("kind: InterpolatedString {");
                self.out.push_indent();
                self.field_bool("raw", *raw);
                self.open_list_field("parts");
                for part in parts {
                    self.render_interpolated_string_part(owner, part);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Unary { op, op_span, expr } => {
                self.line("kind: Unary {");
                self.out.push_indent();
                self.field_debug("op", op);
                self.field_debug("op_span", op_span);
                self.render_expr_field(owner, "expr", expr);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Binary {
                lhs,
                op,
                op_span,
                rhs,
            } => {
                self.line("kind: Binary {");
                self.out.push_indent();
                self.render_expr_field(owner, "lhs", lhs);
                self.field_debug("op", op);
                self.field_debug("op_span", op_span);
                self.render_expr_field(owner, "rhs", rhs);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::TypeCheck {
                expr,
                op,
                op_span,
                target_ty,
            } => {
                self.line("kind: TypeCheck {");
                self.out.push_indent();
                self.render_expr_field(owner, "expr", expr);
                self.field_debug("op", op);
                self.field_debug("op_span", op_span);
                self.field_type("target_ty", *target_ty);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Cast {
                expr,
                op,
                op_span,
                target_ty,
            } => {
                self.line("kind: Cast {");
                self.out.push_indent();
                self.render_expr_field(owner, "expr", expr);
                self.field_debug("op", op);
                self.field_debug("op_span", op_span);
                self.field_type("target_ty", *target_ty);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Block(block) => {
                self.line("kind: Block(");
                self.out.push_indent();
                self.render_block(owner, block);
                self.out.pop_indent();
                self.line("),");
            }
            ExprKind::Closure(closure) => {
                self.line("kind: Closure(");
                self.out.push_indent();
                self.render_closure_expr(owner, closure);
                self.out.pop_indent();
                self.line("),");
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.line("kind: If {");
                self.out.push_indent();
                self.render_expr_field(owner, "cond", cond);
                self.render_expr_field(owner, "then_branch", then_branch);
                self.render_option_expr_field(owner, "else_branch", else_branch.as_deref());
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::When { subject, arms } => {
                self.line("kind: When {");
                self.out.push_indent();
                self.render_expr_field(owner, "subject", subject);
                self.open_list_field("arms");
                for arm in arms {
                    self.render_when_arm(owner, arm);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::MemberAccess { receiver, member } => {
                self.line("kind: MemberAccess {");
                self.out.push_indent();
                self.render_expr_field(owner, "receiver", receiver);
                self.render_member_access_field("member", member);
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Call { callee, args } => {
                self.line("kind: Call {");
                self.out.push_indent();
                self.render_expr_field(owner, "callee", callee);
                self.open_list_field("args");
                for arg in args {
                    self.render_call_arg(owner, arg);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => {
                self.line("kind: Perform {");
                self.out.push_indent();
                self.field_type("effect_ty", *effect_ty);
                self.render_effect_op_ref_field("op", op);
                self.open_list_field("args");
                for arg in args {
                    self.render_call_arg(owner, arg);
                }
                self.close_list_field();
                self.out.pop_indent();
                self.line("},");
            }
            ExprKind::Handle(handle) => {
                self.line("kind: Handle(");
                self.out.push_indent();
                self.render_handle_expr(owner, handle);
                self.out.pop_indent();
                self.line("),");
            }
            ExprKind::Todo(reason) => self.field_raw("kind", &format!("Todo({reason:?})")),
        }
    }

    fn render_struct_lit_field(&mut self, owner: &str, field: &StructLitField) {
        self.open_struct("StructLitField");
        self.field_debug("span", &field.span);
        self.field_debug("name", &field.name);
        self.field_debug("name_span", &field.name_span);
        self.field_debug("colon_span", &field.colon_span);
        self.render_expr_field(owner, "value", &field.value);
        self.close_struct(",");
    }

    fn render_interpolated_string_part(&mut self, owner: &str, part: &InterpolatedStringPart) {
        match part {
            InterpolatedStringPart::Text { span } => {
                self.line("Text {");
                self.out.push_indent();
                self.field_debug("span", span);
                self.out.pop_indent();
                self.line("},");
            }
            InterpolatedStringPart::Expr { expr } => {
                self.line("Expr {");
                self.out.push_indent();
                self.render_expr_field(owner, "expr", expr);
                self.out.pop_indent();
                self.line("},");
            }
        }
    }

    fn render_class_literal_expr(&mut self, class_lit: &ClassLiteralExpr) {
        self.open_struct("ClassLiteralExpr");
        self.field_type("source_ty", class_lit.source_ty);
        self.render_option_debug_field("source_fqn", class_lit.source_fqn.as_ref());
        self.field_debug("metadata_kind", &class_lit.metadata_kind);
        self.field_type("result_ty", class_lit.result_ty);
        self.close_struct("");
    }

    fn render_closure_expr(&mut self, owner: &str, closure: &ClosureExpr) {
        let closure_owner = format!(
            "{owner}/closure:{}..{}",
            closure.span.start, closure.span.end
        );
        self.open_struct("ClosureExpr");
        self.field_debug("span", &closure.span);
        self.field_label("label", self.synthetic_label("closure", closure.span));
        self.render_option_debug_field("at_safe_span", closure.at_safe_span.as_ref());
        self.open_list_field("captures");
        for capture in &closure.captures {
            self.render_capture(capture);
        }
        self.close_list_field();
        self.open_list_field("params");
        for param in &closure.params {
            self.render_param(param);
        }
        self.close_list_field();
        self.render_expr_field(&closure_owner, "body", &closure.body);
        self.close_struct("");
    }

    fn render_capture(&mut self, capture: &Capture) {
        self.open_struct("Capture");
        self.field_label("label", self.symbol_label(&capture.name, capture.decl_span));
        self.field_debug("name", &capture.name);
        self.field_debug("decl_span", &capture.decl_span);
        self.field_bool("mutable", capture.mutable);
        self.close_struct(",");
    }

    fn render_value_ref(&mut self, value: &ValueRef) {
        match value {
            ValueRef::Local {
                name, decl_span, ..
            } => {
                self.open_struct("Local");
                self.field_label("label", self.symbol_label(name, *decl_span));
                self.field_debug("name", name);
                self.field_debug("decl_span", decl_span);
                self.close_struct("");
            }
            ValueRef::TopLevel { fqn, .. } => {
                self.open_struct("TopLevel");
                self.field_debug("fqn", fqn);
                self.close_struct("");
            }
        }
    }

    fn render_member_access_field(&mut self, name: &str, member: &MemberAccess) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_member_access(member);
        self.out.pop_indent();
    }

    fn render_member_access(&mut self, member: &MemberAccess) {
        self.open_struct("MemberAccess");
        self.field_debug("span", &member.span);
        self.field_debug("name", &member.name);
        match member.resolved.as_ref() {
            Some(resolved) => {
                self.line("resolved: Some(");
                self.out.push_indent();
                self.render_member_ref(resolved);
                self.out.pop_indent();
                self.line("),");
            }
            None => self.field_debug("resolved", &Option::<MemberRef>::None),
        }
        self.close_struct("");
    }

    fn render_member_ref(&mut self, member: &MemberRef) {
        match member {
            MemberRef::Value { fqn, .. } => self.render_simple_member_ref("Value", fqn),
            MemberRef::Fun { fqn, .. } => self.render_simple_member_ref("Fun", fqn),
            MemberRef::ExtensionValue { fqn, .. } => {
                self.render_simple_member_ref("ExtensionValue", fqn)
            }
            MemberRef::ExtensionFun { fqn, .. } => {
                self.render_simple_member_ref("ExtensionFun", fqn)
            }
        }
    }

    fn render_simple_member_ref(&mut self, variant: &str, fqn: &str) {
        self.open_struct(variant);
        self.field_debug("fqn", &fqn);
        self.close_struct("");
    }

    fn render_call_arg(&mut self, owner: &str, arg: &CallArg) {
        match arg {
            CallArg::Positional(expr) => {
                self.line("Positional(");
                self.out.push_indent();
                self.render_expr(owner, expr);
                self.out.pop_indent();
                self.line("),");
            }
            CallArg::Named {
                name,
                name_span,
                value,
            } => {
                self.line("Named {");
                self.out.push_indent();
                self.field_debug("name", name);
                self.field_debug("name_span", name_span);
                self.render_expr_field(owner, "value", value);
                self.out.pop_indent();
                self.line("},");
            }
        }
    }

    fn render_effect_op_ref_field(&mut self, name: &str, op: &EffectOpRef) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_effect_op_ref(op);
        self.out.pop_indent();
    }

    fn render_effect_op_ref(&mut self, op: &EffectOpRef) {
        self.open_struct("EffectOpRef");
        self.field_debug("span", &op.span);
        self.field_debug("fqn", &op.fqn);
        if !op.type_args.is_empty() {
            self.open_list_field("type_args");
            for &ty in &op.type_args {
                self.line(&format!("{},", self.type_text(ty)));
            }
            self.close_list_field();
        }
        self.close_struct("");
    }

    fn render_when_arm(&mut self, owner: &str, arm: &WhenArm) {
        self.open_struct("WhenArm");
        self.field_debug("span", &arm.span);
        self.render_when_pat_field("pat", &arm.pat);
        self.render_option_expr_field(owner, "guard", arm.guard.as_ref());
        self.field_debug("arrow_span", &arm.arrow_span);
        self.render_expr_field(owner, "body", &arm.body);
        self.close_struct(",");
    }

    fn render_when_pat_field(&mut self, name: &str, pat: &WhenPat) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_when_pat(pat);
        self.out.pop_indent();
    }

    fn render_when_pat(&mut self, pat: &WhenPat) {
        match pat {
            WhenPat::Else { span } => {
                self.open_struct("Else");
                self.field_debug("span", span);
                self.close_struct(",");
            }
            WhenPat::Or { span, pats } => {
                self.open_struct("Or");
                self.field_debug("span", span);
                self.open_list_field("pats");
                for pat in pats {
                    self.render_when_pat(pat);
                }
                self.close_list_field();
                self.close_struct(",");
            }
            WhenPat::Wildcard { span } => {
                self.open_struct("Wildcard");
                self.field_debug("span", span);
                self.close_struct(",");
            }
            WhenPat::Rest { span } => {
                self.open_struct("Rest");
                self.field_debug("span", span);
                self.close_struct(",");
            }
            WhenPat::Is { span, ty } => {
                self.open_struct("Is");
                self.field_debug("span", span);
                self.field_type("ty", *ty);
                self.close_struct(",");
            }
            WhenPat::Bind { span, name, .. } => {
                self.open_struct("Bind");
                self.field_debug("span", span);
                self.field_label("label", self.symbol_label(name, *span));
                self.field_debug("name", name);
                self.close_struct(",");
            }
            WhenPat::Tuple { span, elements } => {
                self.open_struct("Tuple");
                self.field_debug("span", span);
                self.open_list_field("elements");
                for pat in elements {
                    self.render_when_pat(pat);
                }
                self.close_list_field();
                self.close_struct(",");
            }
            WhenPat::Variant {
                span,
                name_span,
                name,
                args,
            } => {
                self.open_struct("Variant");
                self.field_debug("span", span);
                self.field_debug("name_span", name_span);
                self.field_debug("name", name);
                self.open_list_field("args");
                for pat in args {
                    self.render_when_pat(pat);
                }
                self.close_list_field();
                self.close_struct(",");
            }
            WhenPat::IntLit { span, raw } => {
                self.open_struct("IntLit");
                self.field_debug("span", span);
                self.field_debug("raw", raw);
                self.close_struct(",");
            }
            WhenPat::CharLit { span, value } => {
                self.open_struct("CharLit");
                self.field_debug("span", span);
                self.field_debug("value", value);
                self.close_struct(",");
            }
            WhenPat::StringLit { span, value } => {
                self.open_struct("StringLit");
                self.field_debug("span", span);
                self.field_debug("value", value);
                self.close_struct(",");
            }
            WhenPat::BoolLit { span, value } => {
                self.open_struct("BoolLit");
                self.field_debug("span", span);
                self.field_bool("value", *value);
                self.close_struct(",");
            }
        }
    }

    fn render_handle_expr(&mut self, owner: &str, handle: &HandleExpr) {
        self.open_struct("HandleExpr");
        self.render_block_field(owner, "body", &handle.body);
        self.open_list_field("arms");
        for arm in &handle.arms {
            self.render_handle_arm(arm);
        }
        self.close_list_field();
        self.render_option_block_field(owner, "finally", handle.finally.as_ref());
        self.close_struct("");
    }

    fn render_handle_arm(&mut self, arm: &HandleArm) {
        self.open_struct("HandleArm");
        self.field_debug("span", &arm.span);
        self.render_handle_op_field("op", &arm.op);
        if let HandleArmKind::EscapeContinuation { .. } = arm.kind {
            self.field_label(
                "continuation",
                self.synthetic_label("continuation", arm.span),
            );
        }
        self.render_expr_field("handle_arm", "body", &arm.body);
        self.close_struct("");
    }

    fn render_handle_op_field(&mut self, name: &str, op: &HandleOp) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_handle_op(op);
        self.out.pop_indent();
    }

    fn render_handle_op(&mut self, op: &HandleOp) {
        self.open_struct("HandleOp");
        self.field_debug("span", &op.span);
        self.field_type("effect_ty", op.effect_ty);
        self.render_effect_op_ref_field("op", &op.op);
        self.open_list_field("binders");
        for binder in &op.binders {
            self.render_handle_binder(binder);
        }
        self.close_list_field();
        self.close_struct("");
    }

    fn render_handle_binder(&mut self, binder: &HandleBinder) {
        self.open_struct("HandleBinder");
        self.field_debug("span", &binder.span);
        self.field_label("label", self.symbol_label(&binder.name, binder.span));
        self.field_debug("name", &binder.name);
        self.field_type("ty", binder.ty);
        self.close_struct(",");
    }

    fn render_decl_type_params(&mut self, name: &str, params: &[DeclTypeParam]) {
        self.open_list_field(name);
        for param in params {
            self.open_struct("DeclTypeParam");
            self.field_debug("span", &param.span);
            self.field_debug("name", &param.name);
            self.render_option_debug_field("variance", param.variance.as_ref());
            self.field_type("ty", param.ty);
            self.close_struct(",");
        }
        self.close_list_field();
    }

    fn render_accessor_contract_field(&mut self, name: &str, accessor: Option<&AccessorContract>) {
        match accessor {
            Some(accessor) => {
                self.line(&format!("{name}: Some("));
                self.out.push_indent();
                self.open_struct("AccessorContract");
                self.field_debug("span", &accessor.span);
                self.field_debug("fqn", &accessor.fqn);
                self.close_struct("");
                self.out.pop_indent();
                self.line("),");
            }
            None => self.field_debug(name, &Option::<AccessorContract>::None),
        }
    }

    fn render_expr_field(&mut self, owner: &str, name: &str, expr: &Expr) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_expr(owner, expr);
        self.out.pop_indent();
    }

    fn render_block_field(&mut self, owner: &str, name: &str, block: &Block) {
        self.line(&format!("{name}:"));
        self.out.push_indent();
        self.render_block(owner, block);
        self.out.pop_indent();
    }

    fn render_option_expr_field(&mut self, owner: &str, name: &str, expr: Option<&Expr>) {
        match expr {
            Some(expr) => {
                self.line(&format!("{name}: Some("));
                self.out.push_indent();
                self.render_expr(owner, expr);
                self.out.pop_indent();
                self.line("),");
            }
            None => self.field_debug(name, &Option::<Expr>::None),
        }
    }

    fn render_option_block_field(&mut self, owner: &str, name: &str, block: Option<&Block>) {
        match block {
            Some(block) => {
                self.line(&format!("{name}: Some("));
                self.out.push_indent();
                self.render_block(owner, block);
                self.out.pop_indent();
                self.line("),");
            }
            None => self.field_debug(name, &Option::<Block>::None),
        }
    }

    fn render_option_debug_field<T>(&mut self, name: &str, value: Option<&T>)
    where
        T: std::fmt::Debug + ?Sized,
    {
        match value {
            Some(value) => self.line(&format!("{name}: Some({}),", format_debug(value))),
            None => self.field_raw(name, "None"),
        }
    }

    fn render_debug_string_list(&mut self, name: &str, values: &[String]) {
        self.open_list_field(name);
        for value in values {
            self.line(&format!("{},", format_debug(value)));
        }
        self.close_list_field();
    }

    fn top_level_owner(&self, val: &ValDecl) -> String {
        match val.name.as_deref() {
            Some(name) => format!("top_level:{name}"),
            None => format!("top_level:{}..{}", val.span.start, val.span.end),
        }
    }

    fn type_text(&self, ty: crate::ty::TypeId) -> String {
        format_type(self.types, ty)
    }

    fn literal_text(&self, literal: &LiteralKind) -> String {
        match literal {
            LiteralKind::Int => "Int".to_string(),
            LiteralKind::Float64(value) => format!("Float64({})", format_debug(value)),
            LiteralKind::Float32(value) => format!("Float32({})", format_debug(value)),
            LiteralKind::Char(value) => format!("Char({})", format_debug(value)),
            LiteralKind::String => "String".to_string(),
            LiteralKind::SynthString(value) => format!("SynthString({})", format_debug(value)),
            LiteralKind::Unit => "Unit".to_string(),
            LiteralKind::Bool(value) => format!("Bool({value})"),
            LiteralKind::SynthInt(value) => format!("SynthInt({value})"),
        }
    }

    fn symbol_label(&self, name: &str, span: crate::span::Span) -> String {
        LocalEntityKey::new("hir", self.source_path, span, "symbol", name, 0).label("sym")
    }

    fn resolved_symbol_span(
        &self,
        id: Option<SymbolId>,
        fallback: crate::span::Span,
    ) -> crate::span::Span {
        id.and_then(|id| self.symbol_spans.get(&id).copied())
            .unwrap_or(fallback)
    }

    fn synthetic_label(&self, kind: &str, span: crate::span::Span) -> String {
        LocalEntityKey::new("hir", self.source_path, span, kind, "", 0).label(kind)
    }

    fn open_struct(&mut self, name: &str) {
        self.line(&format!("{name} {{"));
        self.out.push_indent();
    }

    fn close_struct(&mut self, suffix: &str) {
        self.out.pop_indent();
        self.line(&format!("}}{suffix}"));
    }

    fn open_list_field(&mut self, name: &str) {
        self.line(&format!("{name}: ["));
        self.out.push_indent();
    }

    fn close_list_field(&mut self) {
        self.out.pop_indent();
        self.line("],");
    }

    fn render_variant(&mut self, name: &str, render: impl FnOnce(&mut Self)) {
        self.line(&format!("{name}("));
        self.out.push_indent();
        render(self);
        self.out.pop_indent();
        self.line("),");
    }

    fn field_bool(&mut self, name: &str, value: bool) {
        self.line(&format!("{name}: {value},"));
    }

    fn field_usize(&mut self, name: &str, value: usize) {
        self.line(&format!("{name}: {value},"));
    }

    fn field_type(&mut self, name: &str, ty: crate::ty::TypeId) {
        self.line(&format!("{name}: {},", self.type_text(ty)));
    }

    fn field_debug<T>(&mut self, name: &str, value: &T)
    where
        T: std::fmt::Debug + ?Sized,
    {
        self.line(&format!("{name}: {},", format_debug(value)));
    }

    fn field_label(&mut self, name: &str, value: String) {
        self.line(&format!("{name}: {value},"));
    }

    fn field_raw(&mut self, name: &str, value: &str) {
        self.line(&format!("{name}: {value},"));
    }

    fn line(&mut self, text: &str) {
        self.out.line(text);
    }
}

fn collect_symbol_decl_spans(file: &File) -> HashMap<SymbolId, crate::span::Span> {
    let mut spans = HashMap::new();
    for item in &file.items {
        collect_item_symbol_spans(item, &mut spans);
    }
    spans
}

fn collect_item_symbol_spans(item: &Item, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    match item {
        Item::Fun(fun) => {
            for param in &fun.params {
                spans.entry(param.id).or_insert(param.span);
            }
            if let Some(body) = &fun.body {
                collect_block_symbol_spans(body, spans);
            }
        }
        Item::Val(val) => {
            if let Some(id) = val.id {
                spans.entry(id).or_insert(val.span);
            }
            if let Some(init) = &val.init {
                collect_expr_symbol_spans(init, spans);
            }
        }
        Item::Todo { .. } => {}
    }
}

fn collect_block_symbol_spans(block: &Block, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    for stmt in &block.stmts {
        collect_stmt_symbol_spans(stmt, spans);
    }
}

fn collect_stmt_symbol_spans(stmt: &Stmt, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => collect_expr_symbol_spans(expr, spans),
        StmtKind::Val(val) => {
            if let Some(id) = val.id {
                spans.entry(id).or_insert(val.span);
            }
            if let Some(init) = &val.init {
                collect_expr_symbol_spans(init, spans);
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_expr_symbol_spans(lhs, spans);
            collect_expr_symbol_spans(rhs, spans);
        }
        StmtKind::While { cond, body } => {
            collect_expr_symbol_spans(cond, spans);
            collect_block_symbol_spans(body, spans);
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_expr_symbol_spans(value, spans);
            }
        }
    }
}

fn collect_expr_symbol_spans(expr: &Expr, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::Todo(_) => {}
        ExprKind::VarRef(ValueRef::Local { id, decl_span, .. }) => {
            spans.insert(*id, *decl_span);
        }
        ExprKind::VarRef(ValueRef::TopLevel { .. }) => {}
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_expr_symbol_spans(&field.value, spans);
            }
        }
        ExprKind::ClassLiteral(_) => {}
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_expr_symbol_spans(element, spans);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let InterpolatedStringPart::Expr { expr } = part {
                    collect_expr_symbol_spans(expr, spans);
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => collect_expr_symbol_spans(expr, spans),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_symbol_spans(lhs, spans);
            collect_expr_symbol_spans(rhs, spans);
        }
        ExprKind::Block(block) => collect_block_symbol_spans(block, spans),
        ExprKind::Closure(closure) => {
            for capture in &closure.captures {
                spans.insert(capture.id, capture.decl_span);
            }
            for param in &closure.params {
                spans.entry(param.id).or_insert(param.span);
            }
            collect_expr_symbol_spans(&closure.body, spans);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_symbol_spans(cond, spans);
            collect_expr_symbol_spans(then_branch, spans);
            if let Some(else_branch) = else_branch {
                collect_expr_symbol_spans(else_branch, spans);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_expr_symbol_spans(subject, spans);
            for arm in arms {
                collect_when_pat_symbol_spans(&arm.pat, spans);
                if let Some(guard) = &arm.guard {
                    collect_expr_symbol_spans(guard, spans);
                }
                collect_expr_symbol_spans(&arm.body, spans);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => collect_expr_symbol_spans(receiver, spans),
        ExprKind::Call { callee, args } => {
            collect_expr_symbol_spans(callee, spans);
            for arg in args {
                collect_call_arg_symbol_spans(arg, spans);
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                collect_call_arg_symbol_spans(arg, spans);
            }
        }
        ExprKind::Handle(handle) => {
            collect_block_symbol_spans(&handle.body, spans);
            for arm in &handle.arms {
                for binder in &arm.op.binders {
                    spans.entry(binder.id).or_insert(binder.span);
                }
                collect_expr_symbol_spans(&arm.body, spans);
            }
            if let Some(finally) = &handle.finally {
                collect_block_symbol_spans(finally, spans);
            }
        }
    }
}

fn collect_call_arg_symbol_spans(arg: &CallArg, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    match arg {
        CallArg::Positional(expr) => collect_expr_symbol_spans(expr, spans),
        CallArg::Named { value, .. } => collect_expr_symbol_spans(value, spans),
    }
}

fn collect_when_pat_symbol_spans(pat: &WhenPat, spans: &mut HashMap<SymbolId, crate::span::Span>) {
    match pat {
        WhenPat::Else { .. }
        | WhenPat::Wildcard { .. }
        | WhenPat::Rest { .. }
        | WhenPat::Is { .. }
        | WhenPat::IntLit { .. }
        | WhenPat::CharLit { .. }
        | WhenPat::StringLit { .. }
        | WhenPat::BoolLit { .. } => {}
        WhenPat::Or { pats, .. } | WhenPat::Tuple { elements: pats, .. } => {
            for pat in pats {
                collect_when_pat_symbol_spans(pat, spans);
            }
        }
        WhenPat::Bind { id, span, .. } => {
            spans.entry(*id).or_insert(*span);
        }
        WhenPat::Variant { args, .. } => {
            for pat in args {
                collect_when_pat_symbol_spans(pat, spans);
            }
        }
    }
}
