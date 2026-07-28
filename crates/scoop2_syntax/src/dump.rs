//! 稳定 AST 文本渲染器（`dump-ast` 的 golden 格式）。
//!
//! 格式约定（缩进树，每级两个空格）：
//!
//! - **节点行**：`Kind start..end key=value ...`（span 为字节偏移区间；
//!   `NodeId` 不输出——它是实现细节，span 已携带位置信息）。
//! - **section 行**：`name:` 单独一行，子节点缩进展示。规则：
//!   节点只有一个结构性子字段时子节点直接缩进展示（不带标签）；
//!   有多个结构性字段时每个字段用 section 标签。缺失的 `Option` 字段
//!   整个省略，空 `Vec` 字段也省略。
//! - **内联属性**：标识符 `name=foo`（经 [`Interner`] 解析）、路径
//!   `path=a.b.C`、修饰符 `mods=[public, open]`（非空才输出）、标志位
//!   直接写裸词（`vararg`、`spread`、`raw`、`closed`）、字面量值
//!   `value=...`（字符串/字符用 Rust debug 转义，保证确定性）。
//! - 渲染是**穷尽式**的：不使用会静默跳过数据的通配分支，新增 AST 变体
//!   会导致编译错误，强制同步更新本渲染器。
//!
//! 示例：
//!
//! ```text
//! File 0..40
//!   items:
//!     FunDecl 0..40 name=main mods=[public]
//!       body:
//!         Block 16..40
//!           ExprStmt 18..38
//!             Call 18..38
//!               callee:
//!                 Ident 18..25 name=println
//!               args:
//!                 CallArg 26..37
//!                   StringLit 26..37 value="hello"
//! ```

use scoop2_base::{Interner, Span, Symbol};

use crate::ast::decl::*;
use crate::ast::expr::*;
use crate::ast::pattern::*;
use crate::ast::types::*;
use crate::ast::{self, File, Ident, TypePath};

/// 渲染整个源文件为稳定的缩进树文本。
pub fn dump_file(file: &File, interner: &Interner) -> String {
    let mut dumper = Dumper {
        interner,
        out: String::new(),
        indent: 0,
        type_of: None,
    };
    dumper.file(file);
    dumper.out
}

/// 渲染整个源文件为稳定的缩进树文本，并为每个表达式节点追加 `ty=<type>`。
///
/// `type_of` 把表达式 [`scoop2_base::NodeId`] 映射为其推断类型的可读文本（如
/// `"Int"`、`"() -> String / Pure"`）。返回 `None` 的节点不追加 `ty=`（例如
/// typecheck 未覆盖或被跳过的节点）。
///
/// 其余格式与 [`dump_file`] 完全一致；本函数供 `dump-hir` 使用。
pub fn dump_file_typed(
    file: &File,
    interner: &Interner,
    type_of: &dyn Fn(scoop2_base::NodeId) -> Option<String>,
) -> String {
    let mut dumper = Dumper {
        interner,
        out: String::new(),
        indent: 0,
        type_of: Some(type_of),
    };
    dumper.file(file);
    dumper.out
}

struct Dumper<'a> {
    interner: &'a Interner,
    out: String,
    indent: usize,
    /// 可选：表达式 NodeId → 类型文本（dump-hir 用；普通 dump-ast 为 None）。
    type_of: Option<&'a dyn Fn(scoop2_base::NodeId) -> Option<String>>,
}

impl Dumper<'_> {
    // ------------------------------------------------------------------
    // 基础设施
    // ------------------------------------------------------------------

    fn line(&mut self, text: String) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(&text);
        self.out.push('\n');
    }

    /// 渲染一个表达式节点行：在 `text` 后追加 ` ty=<type>`（若 `type_of` 提供）。
    /// 仅 dump-hir 路径会带 `type_of`；普通 dump-ast 等价于直接 `line`。
    fn expr_line(&mut self, id: scoop2_base::NodeId, text: String) {
        let mut text = text;
        if let Some(type_of) = self.type_of
            && let Some(ty) = type_of(id)
        {
            text.push_str(" ty=");
            text.push_str(&ty);
        }
        self.line(text);
    }

    /// 以加一缩进级别渲染子节点（无标签）。
    fn child(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    /// 渲染一个 section：`name:` 行 + 缩进的子节点。
    fn section(&mut self, name: &str, f: impl FnOnce(&mut Self)) {
        self.line(format!("{name}:"));
        self.child(f);
    }

    fn sym(&self, symbol: Symbol) -> &str {
        self.interner.resolve(symbol)
    }

    fn path_text(&self, path: &TypePath) -> String {
        path.segments
            .iter()
            .map(|seg| self.sym(seg.symbol))
            .collect::<Vec<_>>()
            .join(".")
    }

    fn mods_attr(&self, mods: &[ast::Modifier]) -> String {
        if mods.is_empty() {
            return String::new();
        }
        let names: Vec<&str> = mods
            .iter()
            .map(|m| match m.kind {
                ast::ModifierKind::Public => "public",
                ast::ModifierKind::Internal => "internal",
                ast::ModifierKind::Private => "private",
                ast::ModifierKind::Open => "open",
                ast::ModifierKind::Abstract => "abstract",
                ast::ModifierKind::Sealed => "sealed",
                ast::ModifierKind::Override => "override",
                ast::ModifierKind::Operator => "operator",
                ast::ModifierKind::Annotation => "annotation",
            })
            .collect();
        format!(" mods=[{}]", names.join(", "))
    }

    fn member_name_text(&self, member: &MemberName) -> String {
        match member {
            MemberName::Named(ident) => self.sym(ident.symbol).to_string(),
            MemberName::TupleIndex { value, .. } => value.to_string(),
        }
    }

    fn field_path_text(&self, path: &FieldPath) -> String {
        path.segments
            .iter()
            .map(|seg| self.member_name_text(seg))
            .collect::<Vec<_>>()
            .join(".")
    }

    fn annotations_field(&mut self, annotations: &[ast::AnnotationUse]) {
        if annotations.is_empty() {
            return;
        }
        self.section("annotations", |d| {
            for a in annotations {
                d.annotation(a);
            }
        });
    }

    // ------------------------------------------------------------------
    // 文件 / 包 / 导入 / 注解
    // ------------------------------------------------------------------

    fn file(&mut self, file: &File) {
        self.line(format!("File {}", file.span));
        self.child(|d| {
            if !file.file_annotations.is_empty() {
                d.section("file_annotations", |d| {
                    for a in &file.file_annotations {
                        d.annotation(a);
                    }
                });
            }
            if let Some(package) = &file.package {
                d.section("package", |d| {
                    d.line(format!(
                        "Package {} path={}",
                        package.span,
                        d.path_text(&package.path)
                    ));
                });
            }
            if !file.imports.is_empty() {
                d.section("imports", |d| {
                    for import in &file.imports {
                        d.import(import);
                    }
                });
            }
            if !file.items.is_empty() {
                d.section("items", |d| {
                    for item in &file.items {
                        d.item(item);
                    }
                });
            }
        });
    }

    fn import(&mut self, import: &ast::ImportDecl) {
        let mut text = format!(
            "Import {} path={}",
            import.span,
            self.path_text(&import.path)
        );
        if import.wildcard.is_some() {
            text.push_str(" wildcard");
        }
        if let Some(alias) = import.alias {
            text.push_str(&format!(" alias={}", self.sym(alias.symbol)));
        }
        self.line(text);
    }

    fn annotation(&mut self, ann: &ast::AnnotationUse) {
        let mut text = format!("Annotation {} path={}", ann.span, self.path_text(&ann.path));
        if let Some(target) = ann.target {
            text.push_str(&format!(" target={}", self.sym(target.symbol)));
        }
        self.line(text);
        if !ann.args.is_empty() {
            self.child(|d| {
                d.section("args", |d| {
                    for arg in &ann.args {
                        d.annotation_arg(arg);
                    }
                });
            });
        }
    }

    fn annotation_arg(&mut self, arg: &ast::AnnotationArg) {
        let mut text = format!("AnnotationArg {}", arg.span);
        if let Some(name) = arg.name {
            text.push_str(&format!(" name={}", self.sym(name.symbol)));
        }
        self.line(text);
        self.child(|d| d.expr(&arg.value));
    }

    // ------------------------------------------------------------------
    // Items / 声明
    // ------------------------------------------------------------------

    fn item(&mut self, item: &ast::Item) {
        match &item.kind {
            ItemKind::TypeAlias(decl) => self.type_alias(item.span, decl),
            ItemKind::Fun(decl) => self.fun_decl(item.span, decl),
            ItemKind::Val(decl) => self.val_decl(item.span, decl),
            ItemKind::ExtensionProperty(decl) => self.extension_property(item.span, decl),
            ItemKind::Object(decl) => self.object_decl(item.span, decl),
            ItemKind::Type(decl) => self.type_decl(item.span, decl),
        }
    }

    fn type_alias(&mut self, span: Span, decl: &TypeAliasDecl) {
        self.line(format!(
            "TypeAliasDecl {span} name={}{}",
            self.sym(decl.name.symbol),
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(tpl) = &decl.type_params {
                d.section("type_params", |d| d.type_param_list(tpl));
            }
            d.section("ty", |d| d.type_ref(&decl.ty));
        });
    }

    fn fun_decl(&mut self, span: Span, decl: &FunDecl) {
        self.line(format!(
            "FunDecl {span} name={}{}",
            self.sym(decl.name.symbol),
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(tpl) = &decl.type_params {
                d.section("type_params", |d| d.type_param_list(tpl));
            }
            if let Some(receiver) = &decl.receiver {
                d.section("receiver", |d| d.type_ref(receiver));
            }
            if !decl.params.is_empty() {
                d.section("params", |d| {
                    for param in &decl.params {
                        d.param(param);
                    }
                });
            }
            if let Some(ret) = &decl.return_ty {
                d.section("return_ty", |d| d.type_ref(ret));
            }
            if let Some(effect) = &decl.effect {
                d.section("effect", |d| d.effect_row(effect));
            }
            if let Some(where_clause) = &decl.where_clause {
                d.section("where", |d| d.where_clause(where_clause));
            }
            match &decl.body {
                Some(FunBody::Block(block)) => d.section("body", |d| d.block(block)),
                Some(FunBody::Expr(expr)) => d.section("body", |d| d.expr(expr)),
                None => {}
            }
        });
    }

    fn param(&mut self, param: &Param) {
        let mut text = format!("Param {} name={}", param.span, self.sym(param.name.symbol));
        if param.is_vararg {
            text.push_str(" vararg");
        }
        self.line(text);
        self.child(|d| {
            d.annotations_field(&param.annotations);
            if let Some(ty) = &param.ty {
                d.section("ty", |d| d.type_ref(ty));
            }
            if let Some(default) = &param.default {
                d.section("default", |d| d.expr(default));
            }
        });
    }

    fn val_decl(&mut self, span: Span, decl: &ValDecl) {
        let kind = match decl.kind {
            ValKind::Val => "val",
            ValKind::Var => "var",
        };
        let mut text = format!("ValDecl {span} kind={kind}");
        if let ValBinding::Name(name) = &decl.binding {
            text.push_str(&format!(" name={}", self.sym(name.symbol)));
        }
        text.push_str(&self.mods_attr(&decl.modifiers));
        self.line(text);
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let ValBinding::Pattern(pat) = &decl.binding {
                d.section("pattern", |d| d.pattern(pat));
            }
            if let Some(ty) = &decl.ty {
                d.section("ty", |d| d.type_ref(ty));
            }
            if let Some(init) = &decl.init {
                d.section("init", |d| d.expr(init));
            }
        });
    }

    fn type_decl(&mut self, span: Span, decl: &TypeDecl) {
        let kind = match decl.kind {
            TypeKind::Class => "ClassDecl",
            TypeKind::Interface => "InterfaceDecl",
            TypeKind::Struct => "StructDecl",
            TypeKind::Enum => "EnumDecl",
            TypeKind::Effect => "EffectDecl",
        };
        self.line(format!(
            "{kind} {span} name={}{}",
            self.sym(decl.name.symbol),
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(tpl) = &decl.type_params {
                d.section("type_params", |d| d.type_param_list(tpl));
            }
            if let Some(ctor) = &decl.primary_ctor {
                d.section("primary_ctor", |d| d.primary_ctor(ctor));
            }
            if !decl.supertypes.is_empty() {
                d.section("supertypes", |d| {
                    for st in &decl.supertypes {
                        d.super_type(st);
                    }
                });
            }
            if let Some(where_clause) = &decl.where_clause {
                d.section("where", |d| d.where_clause(where_clause));
            }
            if let Some(body) = &decl.body {
                d.section("body", |d| d.type_body(body));
            }
        });
    }

    fn primary_ctor(&mut self, ctor: &PrimaryCtorDecl) {
        self.line(format!("PrimaryCtor {}", ctor.span));
        self.child(|d| {
            for param in &ctor.params {
                d.ctor_param(param);
            }
        });
    }

    fn ctor_param(&mut self, param: &CtorParam) {
        let mut text = format!(
            "CtorParam {} name={}",
            param.span,
            self.sym(param.name.symbol)
        );
        if let Some(kind) = param.property {
            let kind = match kind {
                ValKind::Val => "val",
                ValKind::Var => "var",
            };
            text.push_str(&format!(" property={kind}"));
        }
        if param.is_vararg {
            text.push_str(" vararg");
        }
        self.line(text);
        self.child(|d| {
            d.annotations_field(&param.annotations);
            if let Some(ty) = &param.ty {
                d.section("ty", |d| d.type_ref(ty));
            }
            if let Some(default) = &param.default {
                d.section("default", |d| d.expr(default));
            }
        });
    }

    fn super_type(&mut self, st: &SuperType) {
        self.line(format!("SuperType {}", st.span));
        self.child(|d| {
            d.section("ty", |d| d.type_ref(&st.ty));
            if !st.args.is_empty() {
                d.section("args", |d| {
                    for arg in &st.args {
                        d.call_arg(arg);
                    }
                });
            }
        });
    }

    fn type_body(&mut self, body: &TypeBody) {
        self.line(format!("TypeBody {}", body.span));
        self.child(|d| {
            for member in &body.members {
                d.type_member(member);
            }
        });
    }

    fn type_member(&mut self, member: &TypeMember) {
        match &member.kind {
            TypeMemberKind::InitBlock(decl) => self.init_block(member.span, decl),
            TypeMemberKind::SecondaryCtor(decl) => self.secondary_ctor(member.span, decl),
            TypeMemberKind::EnumVariant(decl) => self.enum_variant(member.span, decl),
            TypeMemberKind::Object(decl) => self.object_decl(member.span, decl),
            TypeMemberKind::Property(decl) => self.property_decl(member.span, decl),
            TypeMemberKind::Fun(decl) => self.fun_decl(member.span, decl),
            TypeMemberKind::Type(decl) => self.type_decl(member.span, decl),
        }
    }

    fn init_block(&mut self, span: Span, decl: &InitBlockDecl) {
        self.line(format!(
            "InitBlock {span}{}",
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            d.block(&decl.body);
        });
    }

    fn secondary_ctor(&mut self, span: Span, decl: &SecondaryCtorDecl) {
        self.line(format!(
            "SecondaryCtor {span}{}",
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(tpl) = &decl.type_params {
                d.section("type_params", |d| d.type_param_list(tpl));
            }
            if !decl.params.is_empty() {
                d.section("params", |d| {
                    for param in &decl.params {
                        d.param(param);
                    }
                });
            }
            if let Some(where_clause) = &decl.where_clause {
                d.section("where", |d| d.where_clause(where_clause));
            }
            if let Some(delegation) = &decl.delegation {
                d.section("delegation", |d| {
                    let kind = match delegation.kind {
                        CtorDelegationKind::This => "ThisDelegation",
                        CtorDelegationKind::Super => "SuperDelegation",
                    };
                    d.line(format!("{kind} {}", delegation.span));
                    d.child(|d| {
                        for arg in &delegation.args {
                            d.call_arg(arg);
                        }
                    });
                });
            }
            d.section("body", |d| d.block(&decl.body));
        });
    }

    fn enum_variant(&mut self, span: Span, decl: &EnumVariantDecl) {
        self.line(format!(
            "EnumVariant {span} name={}",
            self.sym(decl.name.symbol)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if !decl.fields.is_empty() {
                d.section("fields", |d| {
                    for field in &decl.fields {
                        d.line(format!(
                            "EnumVariantField {} name={}",
                            field.span,
                            d.sym(field.name.symbol)
                        ));
                        d.child(|d| d.type_ref(&field.ty));
                    }
                });
            }
            if let Some(discriminant) = &decl.discriminant {
                d.section("discriminant", |d| d.expr(discriminant));
            }
        });
    }

    fn object_decl(&mut self, span: Span, decl: &ObjectDecl) {
        let kind = if decl.companion {
            "CompanionObject"
        } else {
            "ObjectDecl"
        };
        let mut text = format!("{kind} {span}");
        if let Some(name) = decl.name {
            text.push_str(&format!(" name={}", self.sym(name.symbol)));
        }
        text.push_str(&self.mods_attr(&decl.modifiers));
        self.line(text);
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if !decl.supertypes.is_empty() {
                d.section("supertypes", |d| {
                    for st in &decl.supertypes {
                        d.super_type(st);
                    }
                });
            }
            if let Some(body) = &decl.body {
                d.section("body", |d| d.type_body(body));
            }
        });
    }

    fn property_decl(&mut self, span: Span, decl: &PropertyDecl) {
        let kind = match decl.kind {
            ValKind::Val => "val",
            ValKind::Var => "var",
        };
        self.line(format!(
            "PropertyDecl {span} kind={kind} name={}{}",
            self.sym(decl.name.symbol),
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(ty) = &decl.ty {
                d.section("ty", |d| d.type_ref(ty));
            }
            if let Some(delegate) = &decl.delegate {
                d.section("delegate", |d| d.expr(delegate));
            }
            if let Some(init) = &decl.init {
                d.section("init", |d| d.expr(init));
            }
            if !decl.accessors.is_empty() {
                d.section("accessors", |d| {
                    for accessor in &decl.accessors {
                        d.accessor(accessor);
                    }
                });
            }
        });
    }

    fn accessor(&mut self, accessor: &AccessorDecl) {
        let mut text = match accessor.kind {
            AccessorKind::Get => format!("Getter {}", accessor.span),
            AccessorKind::Set => format!("Setter {}", accessor.span),
        };
        if let Some(param) = accessor.param {
            text.push_str(&format!(" param={}", self.sym(param.symbol)));
        }
        self.line(text);
        self.child(|d| {
            if let Some(param_ty) = &accessor.param_ty {
                d.section("param_ty", |d| d.type_ref(param_ty));
            }
            match &accessor.body {
                AccessorBody::Block(block) => d.section("body", |d| d.block(block)),
                AccessorBody::Expr(expr) => d.section("body", |d| d.expr(expr)),
            }
        });
    }

    fn extension_property(&mut self, span: Span, decl: &ExtensionPropertyDecl) {
        let kind = match decl.kind {
            ValKind::Val => "val",
            ValKind::Var => "var",
        };
        self.line(format!(
            "ExtensionPropertyDecl {span} kind={kind} name={}{}",
            self.sym(decl.name.symbol),
            self.mods_attr(&decl.modifiers)
        ));
        self.child(|d| {
            d.annotations_field(&decl.annotations);
            if let Some(tpl) = &decl.type_params {
                d.section("type_params", |d| d.type_param_list(tpl));
            }
            d.section("receiver", |d| d.type_ref(&decl.receiver));
            d.section("ty", |d| d.type_ref(&decl.ty));
            if let Some(init) = &decl.init {
                d.section("init", |d| d.expr(init));
            }
            if !decl.accessors.is_empty() {
                d.section("accessors", |d| {
                    for accessor in &decl.accessors {
                        d.accessor(accessor);
                    }
                });
            }
        });
    }

    // ------------------------------------------------------------------
    // Generics（声明位）
    // ------------------------------------------------------------------

    fn type_param_list(&mut self, tpl: &TypeParamList) {
        self.line(format!("TypeParams {}", tpl.span));
        self.child(|d| {
            for param in &tpl.params {
                d.type_param(param);
            }
            if let Some(effect_row) = &tpl.effect_row {
                d.section("effect_row", |d| d.effect_row_param(effect_row));
            }
        });
    }

    fn type_param(&mut self, param: &TypeParam) {
        let mut text = format!(
            "TypeParam {} name={}",
            param.span,
            self.sym(param.name.symbol)
        );
        if let Some(variance) = param.variance {
            let variance = match variance {
                Variance::In => "in",
                Variance::Out => "out",
            };
            text.push_str(&format!(" variance={variance}"));
        }
        self.line(text);
        if let Some(bound) = &param.bound {
            self.child(|d| d.generic_bound(bound));
        }
    }

    fn generic_bound(&mut self, bound: &GenericBound) {
        match bound {
            GenericBound::Ref(span) => self.line(format!("RefBound {span}")),
            GenericBound::Value(span) => self.line(format!("ValueBound {span}")),
            GenericBound::Type(ty) => self.type_ref(ty),
        }
    }

    fn effect_row_param(&mut self, param: &EffectRowParam) {
        self.line(format!(
            "EffectRowParam {} name={}",
            param.span,
            self.sym(param.name.symbol)
        ));
        if let Some(default) = &param.default {
            self.child(|d| d.effect_row(default));
        }
    }

    fn where_clause(&mut self, where_clause: &WhereClause) {
        self.line(format!("Where {}", where_clause.span));
        self.child(|d| {
            for constraint in &where_clause.constraints {
                d.line(format!(
                    "WhereConstraint {} name={}",
                    constraint.span,
                    d.sym(constraint.name.symbol)
                ));
                d.child(|d| d.generic_bound(&constraint.bound));
            }
        });
    }

    // ------------------------------------------------------------------
    // 类型 / effect 行
    // ------------------------------------------------------------------

    fn type_ref(&mut self, ty: &TypeRef) {
        match &ty.kind {
            TypeRefKind::Path { path, args } => {
                self.line(format!(
                    "PathType {} path={}",
                    ty.span,
                    self.path_text(path)
                ));
                if !args.is_empty() {
                    self.child(|d| {
                        d.section("args", |d| {
                            for arg in args {
                                d.type_arg(arg);
                            }
                        });
                    });
                }
            }
            TypeRefKind::Unit => self.line(format!("UnitType {}", ty.span)),
            TypeRefKind::Tuple(elements) => {
                self.line(format!("TupleType {}", ty.span));
                self.child(|d| {
                    for element in elements {
                        d.type_ref(element);
                    }
                });
            }
            TypeRefKind::Function {
                params,
                ret,
                effect,
            } => {
                self.line(format!("FunctionType {}", ty.span));
                self.child(|d| {
                    if !params.is_empty() {
                        d.section("params", |d| {
                            for param in params {
                                d.type_ref(param);
                            }
                        });
                    }
                    d.section("ret", |d| d.type_ref(ret));
                    if let Some(effect) = effect {
                        d.section("effect", |d| d.effect_row(effect));
                    }
                });
            }
            TypeRefKind::ReceiverFunction {
                receiver,
                params,
                ret,
                effect,
            } => {
                self.line(format!("ReceiverFunctionType {}", ty.span));
                self.child(|d| {
                    d.section("receiver", |d| d.type_ref(receiver));
                    if !params.is_empty() {
                        d.section("params", |d| {
                            for param in params {
                                d.type_ref(param);
                            }
                        });
                    }
                    d.section("ret", |d| d.type_ref(ret));
                    if let Some(effect) = effect {
                        d.section("effect", |d| d.effect_row(effect));
                    }
                });
            }
            TypeRefKind::Nullable(inner) => {
                self.line(format!("NullableType {}", ty.span));
                self.child(|d| d.type_ref(inner));
            }
        }
    }

    fn type_arg(&mut self, arg: &TypeArg) {
        match &arg.kind {
            TypeArgKind::Type(ty) => {
                self.line(format!("TypeArg {}", arg.span));
                self.child(|d| d.type_ref(ty));
            }
            TypeArgKind::Star => self.line(format!("StarProjection {}", arg.span)),
            TypeArgKind::Effect(row) => {
                self.line(format!("EffectRowArg {}", arg.span));
                self.child(|d| d.effect_row(row));
            }
        }
    }

    fn effect_row(&mut self, row: &EffectRowExpr) {
        let mut text = format!("EffectRow {}", row.span);
        if row.closed.is_some() {
            text.push_str(" closed");
        }
        self.line(text);
        self.child(|d| {
            for term in &row.terms {
                d.effect_row_term(term);
            }
        });
    }

    fn effect_row_term(&mut self, term: &EffectRowTerm) {
        self.line(format!(
            "EffectRowTerm {} path={}",
            term.span,
            self.path_text(&term.path)
        ));
        if !term.args.is_empty() {
            self.child(|d| {
                d.section("args", |d| {
                    for arg in &term.args {
                        d.type_arg(arg);
                    }
                });
            });
        }
    }

    // ------------------------------------------------------------------
    // 语句 / 块
    // ------------------------------------------------------------------

    fn block(&mut self, block: &Block) {
        self.line(format!("Block {}", block.span));
        self.child(|d| {
            for stmt in &block.stmts {
                d.stmt(stmt);
            }
        });
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Empty => self.line(format!("EmptyStmt {}", stmt.span)),
            StmtKind::Expr(expr) => {
                self.line(format!("ExprStmt {}", stmt.span));
                self.child(|d| d.expr(expr));
            }
            StmtKind::Assign { target, value } => {
                self.line(format!("AssignStmt {}", stmt.span));
                self.child(|d| {
                    d.section("target", |d| d.assign_target(target));
                    d.section("value", |d| d.expr(value));
                });
            }
            StmtKind::LocalVal(decl) => {
                self.line(format!("LocalValStmt {}", stmt.span));
                self.child(|d| d.val_decl(stmt.span, decl));
            }
            StmtKind::Return { value } => {
                self.line(format!("ReturnStmt {}", stmt.span));
                if let Some(value) = value {
                    self.child(|d| d.expr(value));
                }
            }
            StmtKind::While { cond, body } => {
                self.line(format!("WhileStmt {}", stmt.span));
                self.child(|d| {
                    d.section("cond", |d| d.expr(cond));
                    d.section("body", |d| d.block(body));
                });
            }
            StmtKind::For { binder, iter, body } => {
                self.line(format!(
                    "ForStmt {} binder={}",
                    stmt.span,
                    self.sym(binder.symbol)
                ));
                self.child(|d| {
                    d.section("iter", |d| d.expr(iter));
                    d.section("body", |d| d.block(body));
                });
            }
            StmtKind::Break => self.line(format!("BreakStmt {}", stmt.span)),
            StmtKind::Continue => self.line(format!("ContinueStmt {}", stmt.span)),
        }
    }

    fn assign_target(&mut self, target: &AssignTarget) {
        match &target.kind {
            AssignTargetKind::Ident(ident) => {
                self.line(format!(
                    "IdentTarget {} name={}",
                    target.span,
                    self.sym(ident.symbol)
                ));
            }
            AssignTargetKind::Member { receiver, member } => {
                self.line(format!(
                    "MemberTarget {} member={}",
                    target.span,
                    self.member_name_text(member)
                ));
                self.child(|d| d.expr(receiver));
            }
            AssignTargetKind::Index { receiver, indices } => {
                self.line(format!("IndexTarget {}", target.span));
                self.child(|d| {
                    d.section("receiver", |d| d.expr(receiver));
                    d.section("indices", |d| {
                        for index in indices {
                            d.expr(index);
                        }
                    });
                });
            }
        }
    }

    // ------------------------------------------------------------------
    // 表达式
    // ------------------------------------------------------------------

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(ident) => self.ident_expr(expr.id, expr.span, *ident),
            ExprKind::IntLit(lit) => {
                let mut text = format!("IntLit {} value={}", expr.span, lit.value);
                if let Some(suffix) = lit.suffix {
                    let suffix = match suffix {
                        ast::IntSuffix::U => "U",
                        ast::IntSuffix::L => "L",
                        ast::IntSuffix::UL => "UL",
                    };
                    text.push_str(&format!(" suffix={suffix}"));
                }
                self.expr_line(expr.id, text);
            }
            ExprKind::FloatLit(lit) => {
                let mut text = format!("FloatLit {} value={}", expr.span, lit.value);
                if lit.suffix.is_some() {
                    text.push_str(" suffix=F32");
                }
                self.expr_line(expr.id, text);
            }
            ExprKind::CharLit(lit) => {
                self.expr_line(
                    expr.id,
                    format!("CharLit {} value={:?}", expr.span, lit.value),
                );
            }
            ExprKind::StringLit(lit) => {
                self.expr_line(
                    expr.id,
                    format!("StringLit {} value={:?}", expr.span, lit.value),
                );
            }
            ExprKind::InterpolatedString { raw, parts } => {
                let mut text = format!("InterpolatedString {}", expr.span);
                if *raw {
                    text.push_str(" raw");
                }
                self.expr_line(expr.id, text);
                self.child(|d| {
                    for part in parts {
                        match part {
                            StringPart::Text(text) => d.line(format!("TextPart {text:?}")),
                            StringPart::Expr(expr) => {
                                d.line("ExprPart".to_string());
                                d.child(|d| d.expr(expr));
                            }
                        }
                    }
                });
            }
            ExprKind::UnitLit => self.expr_line(expr.id, format!("UnitLit {}", expr.span)),
            ExprKind::TupleLit(elements) => {
                self.expr_line(expr.id, format!("TupleLit {}", expr.span));
                self.child(|d| {
                    for element in elements {
                        d.expr(element);
                    }
                });
            }
            ExprKind::ArrayLit(elements) => {
                self.expr_line(expr.id, format!("ArrayLit {}", expr.span));
                self.child(|d| {
                    for element in elements {
                        d.expr(element);
                    }
                });
            }
            ExprKind::StructLit { name, fields } => {
                self.expr_line(
                    expr.id,
                    format!("StructLit {} name={}", expr.span, self.sym(name.symbol)),
                );
                self.child(|d| {
                    for field in fields {
                        d.line(format!(
                            "StructField {} name={}",
                            field.span,
                            d.sym(field.name.symbol)
                        ));
                        d.child(|d| d.expr(&field.value));
                    }
                });
            }
            ExprKind::Block(block) => self.block(block),
            ExprKind::DoBlock(block) => {
                self.expr_line(expr.id, format!("DoBlock {}", expr.span));
                self.child(|d| d.block(block));
            }
            ExprKind::UnsafeBlock(block) => {
                self.expr_line(expr.id, format!("UnsafeBlock {}", expr.span));
                self.child(|d| d.block(block));
            }
            ExprKind::SafeBlock(block) => {
                self.expr_line(expr.id, format!("SafeBlock {}", expr.span));
                self.child(|d| d.block(block));
            }
            ExprKind::Lambda(lambda) => {
                let mut text = format!("Lambda {}", expr.span);
                if lambda.is_safe {
                    text.push_str(" safe");
                }
                self.expr_line(expr.id, text);
                self.child(|d| {
                    if !lambda.params.is_empty() {
                        d.section("params", |d| {
                            for param in &lambda.params {
                                d.line(format!(
                                    "LambdaParam {} name={}",
                                    param.span,
                                    d.sym(param.name.symbol)
                                ));
                                if let Some(ty) = &param.ty {
                                    d.child(|d| d.type_ref(ty));
                                }
                            }
                        });
                    }
                    match &lambda.body {
                        LambdaBody::Block(block) => d.section("body", |d| d.block(block)),
                        LambdaBody::Expr(expr) => d.section("body", |d| d.expr(expr)),
                    }
                });
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_line(expr.id, format!("If {}", expr.span));
                self.child(|d| {
                    d.section("cond", |d| d.expr(cond));
                    d.section("then", |d| d.expr(then_branch));
                    if let Some(else_branch) = else_branch {
                        d.section("else", |d| d.expr(else_branch));
                    }
                });
            }
            ExprKind::When { subject, arms } => {
                self.expr_line(expr.id, format!("When {}", expr.span));
                self.child(|d| {
                    d.section("subject", |d| d.expr(subject));
                    d.section("arms", |d| {
                        for arm in arms {
                            d.when_arm(arm);
                        }
                    });
                });
            }
            ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                self.expr_line(expr.id, format!("Handle {}", expr.span));
                self.child(|d| {
                    d.section("body", |d| d.block(body));
                    d.section("arms", |d| {
                        for arm in arms {
                            d.handle_arm(arm);
                        }
                    });
                    if let Some(finally) = finally {
                        d.section("finally", |d| d.block(finally));
                    }
                });
            }
            ExprKind::MemberAccess { receiver, member } => {
                self.expr_line(
                    expr.id,
                    format!(
                        "MemberAccess {} member={}",
                        expr.span,
                        self.member_name_text(member)
                    ),
                );
                self.child(|d| d.expr(receiver));
            }
            ExprKind::SafeMemberAccess { receiver, member } => {
                self.expr_line(
                    expr.id,
                    format!(
                        "SafeMemberAccess {} member={}",
                        expr.span,
                        self.member_name_text(member)
                    ),
                );
                self.child(|d| d.expr(receiver));
            }
            ExprKind::SpliceField { receiver, field } => {
                self.expr_line(expr.id, format!("SpliceField {}", expr.span));
                self.child(|d| {
                    d.section("receiver", |d| d.expr(receiver));
                    d.section("field", |d| d.expr(field));
                });
            }
            ExprKind::Index { receiver, indices } => {
                self.expr_line(expr.id, format!("Index {}", expr.span));
                self.child(|d| {
                    d.section("receiver", |d| d.expr(receiver));
                    d.section("indices", |d| {
                        for index in indices {
                            d.expr(index);
                        }
                    });
                });
            }
            ExprKind::NotNullAssert { expr: inner } => {
                self.expr_line(expr.id, format!("NotNullAssert {}", expr.span));
                self.child(|d| d.expr(inner));
            }
            ExprKind::TypeApply { callee, args } => {
                self.expr_line(expr.id, format!("TypeApply {}", expr.span));
                self.child(|d| {
                    d.section("callee", |d| d.expr(callee));
                    d.section("args", |d| {
                        for arg in args {
                            d.type_arg(arg);
                        }
                    });
                });
            }
            ExprKind::Call { callee, args } => {
                self.expr_line(expr.id, format!("Call {}", expr.span));
                self.child(|d| {
                    d.section("callee", |d| d.expr(callee));
                    if !args.is_empty() {
                        d.section("args", |d| {
                            for arg in args {
                                d.call_arg(arg);
                            }
                        });
                    }
                });
            }
            ExprKind::ClassLit { path } => {
                self.expr_line(
                    expr.id,
                    format!("ClassLit {} path={}", expr.span, self.path_text(path)),
                );
            }
            ExprKind::Unary { op, expr: inner } => {
                let op = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::Neg => "-",
                    UnaryOp::BitNot => "~",
                };
                self.expr_line(expr.id, format!("Unary {} op={op}", expr.span));
                self.child(|d| d.expr(inner));
            }
            ExprKind::Binary { lhs, op, rhs } => {
                let op = match op {
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Rem => "%",
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Shr => ">>",
                    BinaryOp::Range => "..",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::Eq => "==",
                    BinaryOp::Ne => "!=",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitXor => "^",
                    BinaryOp::BitOr => "|",
                    BinaryOp::LogAnd => "&&",
                    BinaryOp::LogOr => "||",
                    BinaryOp::Elvis => "?:",
                };
                self.expr_line(expr.id, format!("Binary {} op={op}", expr.span));
                self.child(|d| {
                    d.section("lhs", |d| d.expr(lhs));
                    d.section("rhs", |d| d.expr(rhs));
                });
            }
            ExprKind::InfixCall {
                receiver,
                name,
                arg,
            } => {
                self.expr_line(
                    expr.id,
                    format!("InfixCall {} name={}", expr.span, self.sym(name.symbol)),
                );
                self.child(|d| {
                    d.section("receiver", |d| d.expr(receiver));
                    d.section("arg", |d| d.expr(arg));
                });
            }
            ExprKind::TypeCheck {
                expr: inner,
                op,
                ty,
            } => {
                let op = match op {
                    TypeCheckOp::Is => "is",
                    TypeCheckOp::NotIs => "!is",
                };
                self.expr_line(expr.id, format!("TypeCheck {} op={op}", expr.span));
                self.child(|d| {
                    d.section("expr", |d| d.expr(inner));
                    d.section("ty", |d| d.type_ref(ty));
                });
            }
            ExprKind::Cast {
                expr: inner,
                op,
                ty,
            } => {
                let op = match op {
                    CastOp::As => "as",
                    CastOp::AsSafe => "as?",
                };
                self.expr_line(expr.id, format!("Cast {} op={op}", expr.span));
                self.child(|d| {
                    d.section("expr", |d| d.expr(inner));
                    d.section("ty", |d| d.type_ref(ty));
                });
            }
            ExprKind::WithUpdate { base, updates } => {
                self.expr_line(expr.id, format!("WithUpdate {}", expr.span));
                self.child(|d| {
                    d.section("base", |d| d.expr(base));
                    d.section("updates", |d| {
                        for update in updates {
                            d.line(format!(
                                "WithUpdateField {} path={}",
                                update.span,
                                d.field_path_text(&update.path)
                            ));
                            d.child(|d| d.expr(&update.value));
                        }
                    });
                });
            }
            ExprKind::Annotated {
                annotations,
                expr: inner,
            } => {
                self.expr_line(expr.id, format!("Annotated {}", expr.span));
                self.child(|d| {
                    d.section("annotations", |d| {
                        for ann in annotations {
                            d.annotation(ann);
                        }
                    });
                    d.section("expr", |d| d.expr(inner));
                });
            }
        }
    }

    fn ident_expr(&mut self, id: scoop2_base::NodeId, span: Span, ident: Ident) {
        self.expr_line(id, format!("Ident {span} name={}", self.sym(ident.symbol)));
    }

    fn call_arg(&mut self, arg: &CallArg) {
        let mut text = format!("CallArg {}", arg.span);
        if let Some(name) = arg.name {
            text.push_str(&format!(" name={}", self.sym(name.symbol)));
        }
        if arg.is_spread {
            text.push_str(" spread");
        }
        self.line(text);
        self.child(|d| d.expr(&arg.value));
    }

    fn when_arm(&mut self, arm: &WhenArm) {
        self.line(format!("WhenArm {}", arm.span));
        self.child(|d| {
            d.section("pat", |d| d.pattern(&arm.pat));
            if let Some(guard) = &arm.guard {
                d.section("guard", |d| d.expr(guard));
            }
            d.section("body", |d| d.expr(&arm.body));
        });
    }

    fn handle_arm(&mut self, arm: &HandleArm) {
        let mut text = format!("HandleArm {}", arm.span);
        if let Some(k) = arm.escape_continuation {
            text.push_str(&format!(" escape={}", self.sym(k.symbol)));
        }
        self.line(text);
        self.child(|d| {
            d.section("op", |d| d.handle_op(&arm.op));
            d.section("body", |d| d.expr(&arm.body));
        });
    }

    fn handle_op(&mut self, op: &HandleOp) {
        self.line(format!(
            "HandleOp {} path={} op={}",
            op.span,
            self.path_text(&op.effect_path),
            self.sym(op.op.symbol)
        ));
        self.child(|d| {
            if !op.effect_args.is_empty() {
                d.section("effect_args", |d| {
                    for arg in &op.effect_args {
                        d.type_arg(arg);
                    }
                });
            }
            if !op.op_type_args.is_empty() {
                d.section("op_type_args", |d| {
                    for arg in &op.op_type_args {
                        d.type_arg(arg);
                    }
                });
            }
            if !op.binders.is_empty() {
                d.section("binders", |d| {
                    for binder in &op.binders {
                        d.line(format!(
                            "HandleBinder {} name={}",
                            binder.span,
                            d.sym(binder.name.symbol)
                        ));
                        if let Some(ty) = &binder.ty {
                            d.child(|d| d.type_ref(ty));
                        }
                    }
                });
            }
        });
    }

    // ------------------------------------------------------------------
    // 模式
    // ------------------------------------------------------------------

    fn pattern(&mut self, pat: &Pattern) {
        match &pat.kind {
            PatternKind::Wildcard => self.line(format!("WildcardPat {}", pat.span)),
            PatternKind::Bind(ident) => {
                self.line(format!(
                    "BindPat {} name={}",
                    pat.span,
                    self.sym(ident.symbol)
                ));
            }
            PatternKind::Literal(lit) => match lit {
                PatternLiteral::Int(lit) => {
                    let mut text = format!("IntPat {} value={}", pat.span, lit.value);
                    if let Some(suffix) = lit.suffix {
                        let suffix = match suffix {
                            ast::IntSuffix::U => "U",
                            ast::IntSuffix::L => "L",
                            ast::IntSuffix::UL => "UL",
                        };
                        text.push_str(&format!(" suffix={suffix}"));
                    }
                    self.line(text);
                }
                PatternLiteral::Char(lit) => {
                    self.line(format!("CharPat {} value={:?}", pat.span, lit.value));
                }
                PatternLiteral::String(lit) => {
                    self.line(format!("StringPat {} value={:?}", pat.span, lit.value));
                }
                PatternLiteral::Bool { value, .. } => {
                    self.line(format!("BoolPat {} value={value}", pat.span));
                }
            },
            PatternKind::Tuple(elements) => {
                self.line(format!("TuplePat {}", pat.span));
                self.child(|d| {
                    for element in elements {
                        d.pattern(element);
                    }
                });
            }
            PatternKind::Struct { path, fields, rest } => {
                let mut text = format!("StructPat {} path={}", pat.span, self.path_text(path));
                if rest.is_some() {
                    text.push_str(" rest");
                }
                self.line(text);
                self.child(|d| {
                    for field in fields {
                        let mut text = format!(
                            "StructPatField {} name={}",
                            field.span,
                            d.sym(field.name.symbol)
                        );
                        if field.pattern.is_none() {
                            text.push_str(" shorthand");
                        }
                        d.line(text);
                        if let Some(pat) = &field.pattern {
                            d.child(|d| d.pattern(pat));
                        }
                    }
                });
            }
            PatternKind::Variant { path, args } => {
                self.line(format!(
                    "VariantPat {} path={}",
                    pat.span,
                    self.path_text(path)
                ));
                if let Some(args) = args {
                    self.child(|d| {
                        for arg in args {
                            d.pattern(arg);
                        }
                    });
                }
            }
            PatternKind::Rest => self.line(format!("RestPat {}", pat.span)),
            PatternKind::Is(ty) => {
                self.line(format!("IsPat {}", pat.span));
                self.child(|d| d.type_ref(ty));
            }
            PatternKind::Else => self.line(format!("ElsePat {}", pat.span)),
            PatternKind::Or(alternatives) => {
                self.line(format!("OrPat {}", pat.span));
                self.child(|d| {
                    for alternative in alternatives {
                        d.pattern(alternative);
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop2_base::{NodeId, Span};

    use super::dump_file;
    use crate::ast::testutil::TestBuilder;
    use crate::ast::{self, *};

    fn sp(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    /// 推进测试游标并返回 (start, end)。
    fn advance(pos: &mut usize, n: usize) -> (usize, usize) {
        let s = *pos;
        *pos += n;
        (s, *pos)
    }

    /// 测试构建器：包装 NodeId 分配 + interning 的常见节点快捷方式。
    struct B {
        t: TestBuilder,
    }

    impl B {
        fn new() -> B {
            B {
                t: TestBuilder::new(),
            }
        }

        fn nid(&self) -> NodeId {
            self.t.id()
        }

        fn ident(&self, name: &str, s: usize, e: usize) -> Ident {
            self.t.ident(name, sp(s, e))
        }

        fn path(&self, s: usize, names: &[&str]) -> TypePath {
            self.t.path(s, names)
        }

        fn expr(&self, s: usize, e: usize, kind: ExprKind) -> Expr {
            Expr {
                id: self.nid(),
                span: sp(s, e),
                kind,
            }
        }

        fn int(&self, s: usize, e: usize, value: u128) -> Expr {
            self.expr(
                s,
                e,
                ExprKind::IntLit(IntLit {
                    value,
                    suffix: None,
                    span: sp(s, e),
                }),
            )
        }

        fn str_lit(&self, s: usize, e: usize, value: &str) -> Expr {
            self.expr(
                s,
                e,
                ExprKind::StringLit(StringLit {
                    value: value.to_string(),
                    span: sp(s, e),
                }),
            )
        }

        fn ident_expr(&self, s: usize, e: usize, name: &str) -> Expr {
            let ident = self.ident(name, s, e);
            self.expr(s, e, ExprKind::Ident(ident))
        }

        fn call(&self, s: usize, e: usize, callee: Expr, args: Vec<CallArg>) -> Expr {
            self.expr(
                s,
                e,
                ExprKind::Call {
                    callee: Box::new(callee),
                    args,
                },
            )
        }

        fn call_arg(&self, value: Expr) -> CallArg {
            let span = value.span;
            CallArg {
                id: self.nid(),
                span,
                name: None,
                is_spread: false,
                value,
            }
        }

        fn path_ty(&self, s: usize, e: usize, path: TypePath) -> TypeRef {
            TypeRef {
                id: self.nid(),
                span: sp(s, e),
                kind: TypeRefKind::Path { path, args: vec![] },
            }
        }

        fn named_ty(&self, s: usize, name: &str) -> TypeRef {
            let path = self.path(s, &[name]);
            let e = s + name.len();
            self.path_ty(s, e, path)
        }

        fn unit_ty(&self, s: usize, e: usize) -> TypeRef {
            TypeRef {
                id: self.nid(),
                span: sp(s, e),
                kind: TypeRefKind::Unit,
            }
        }

        fn block(&self, s: usize, e: usize, stmts: Vec<Stmt>) -> Block {
            Block {
                id: self.nid(),
                span: sp(s, e),
                stmts,
                last_trailing_semi: false,
            }
        }

        fn block_expr(&self, s: usize, e: usize, stmts: Vec<Stmt>) -> Expr {
            let block = self.block(s, e, stmts);
            self.expr(s, e, ExprKind::Block(block))
        }

        fn expr_stmt(&self, expr: Expr) -> Stmt {
            let span = expr.span;
            Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::Expr(expr),
            }
        }

        fn tyarg(&self, ty: TypeRef) -> TypeArg {
            let span = ty.span;
            TypeArg {
                id: self.nid(),
                span,
                kind: TypeArgKind::Type(ty),
            }
        }

        fn pattern(&self, s: usize, e: usize, kind: PatternKind) -> Pattern {
            Pattern {
                id: self.nid(),
                span: sp(s, e),
                kind,
            }
        }

        fn bind_pat(&self, s: usize, e: usize, name: &str) -> Pattern {
            let ident = self.ident(name, s, e);
            self.pattern(s, e, PatternKind::Bind(ident))
        }

        fn int_pat(&self, s: usize, e: usize, value: u128) -> Pattern {
            self.pattern(
                s,
                e,
                PatternKind::Literal(PatternLiteral::Int(IntLit {
                    value,
                    suffix: None,
                    span: sp(s, e),
                })),
            )
        }

        fn item(&self, s: usize, e: usize, kind: ItemKind) -> Item {
            Item {
                id: self.nid(),
                span: sp(s, e),
                kind,
            }
        }

        fn member(&self, s: usize, e: usize, kind: TypeMemberKind) -> TypeMember {
            TypeMember {
                id: self.nid(),
                span: sp(s, e),
                kind,
            }
        }

        fn file(&self, e: usize, items: Vec<Item>) -> File {
            File {
                id: self.nid(),
                span: sp(0, e),
                file_annotations: vec![],
                package: None,
                imports: vec![],
                items,
            }
        }

        fn fun_item(&self, s: usize, e: usize, decl: FunDecl) -> Item {
            self.item(s, e, ItemKind::Fun(decl))
        }

        fn nofun(&self) -> FunDecl {
            // 空骨架；调用方逐字段覆盖。
            FunDecl {
                annotations: vec![],
                modifiers: vec![],
                type_params: None,
                receiver: None,
                name: self.ident("f", 0, 1),
                params: vec![],
                return_ty: None,
                effect: None,
                where_clause: None,
                body: None,
            }
        }
    }

    fn check(actual: String, expected: &str) {
        if actual != expected {
            panic!("dump mismatch\n===== actual =====\n{actual}====================\n");
        }
    }

    // ------------------------------------------------------------------
    // 1. 文件头：@file 注解（use-site target + args）、package、imports
    //    （wildcard / alias）、带命名与位置参数的 item 注解
    // ------------------------------------------------------------------
    #[test]
    fn file_header() {
        let b = B::new();

        let file_ann_arg = {
            let value = b.str_lit(14, 21, "Demo");
            ast::AnnotationArg {
                id: b.nid(),
                span: sp(14, 21),
                name: None,
                value,
            }
        };
        let file_ann = ast::AnnotationUse {
            id: b.nid(),
            span: sp(0, 22),
            target: Some(b.ident("file", 1, 5)),
            path: b.path(6, &["JvmName"]),
            args: vec![file_ann_arg],
        };

        let package = PackageDecl {
            id: b.nid(),
            span: sp(23, 39),
            path: b.path(31, &["scoop", "demo"]),
        };

        let import_wild = ImportDecl {
            id: b.nid(),
            span: sp(40, 55),
            path: b.path(47, &["scoop", "core"]),
            wildcard: Some(sp(54, 55)),
            alias: None,
        };
        let import_alias = ImportDecl {
            id: b.nid(),
            span: sp(56, 74),
            path: b.path(63, &["lib", "util"]),
            wildcard: None,
            alias: Some(b.ident("u", 72, 73)),
        };

        // @Deprecated("old", level = 2) fun f(): Unit {}
        let ann_pos = {
            let value = b.str_lit(87, 92, "old");
            ast::AnnotationArg {
                id: b.nid(),
                span: sp(87, 92),
                name: None,
                value,
            }
        };
        let ann_named = {
            let value = b.int(103, 104, 2);
            ast::AnnotationArg {
                id: b.nid(),
                span: sp(94, 104),
                name: Some(b.ident("level", 94, 99)),
                value,
            }
        };
        let ann = ast::AnnotationUse {
            id: b.nid(),
            span: sp(76, 105),
            target: None,
            path: b.path(77, &["Deprecated"]),
            args: vec![ann_pos, ann_named],
        };
        let unit_ty = b.named_ty(113, "Unit");
        let body = b.block(120, 122, vec![]);
        let fun = b.fun_item(
            76,
            122,
            FunDecl {
                annotations: vec![ann],
                name: b.ident("f", 110, 111),
                return_ty: Some(unit_ty),
                body: Some(FunBody::Block(body)),
                ..b.nofun()
            },
        );

        let mut file = b.file(122, vec![fun]);
        file.file_annotations = vec![file_ann];
        file.package = Some(package);
        file.imports = vec![import_wild, import_alias];

        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..122
  file_annotations:
    Annotation 0..22 path=JvmName target=file
      args:
        AnnotationArg 14..21
          StringLit 14..21 value="Demo"
  package:
    Package 23..39 path=scoop.demo
  imports:
    Import 40..55 path=scoop.core wildcard
    Import 56..74 path=lib.util alias=u
  items:
    FunDecl 76..122 name=f
      annotations:
        Annotation 76..105 path=Deprecated
          args:
            AnnotationArg 87..92
              StringLit 87..92 value="old"
            AnnotationArg 94..104 name=level
              IntLit 103..104 value=2
      return_ty:
        PathType 113..117 path=Unit
      body:
        Block 120..122
"#,
        );
    }

    // ------------------------------------------------------------------
    // 2. 完整 class：主构造（val/普通参数）、带实参的超类型、属性
    //    （get/set 访问器）、方法（块体 + 赋值）、次构造（this 委托）
    // ------------------------------------------------------------------
    #[test]
    fn class_full() {
        let b = B::new();

        // class Point(x: Int, val y: Int) : Base(x), Drawable { ... }
        let ctor_param_x = {
            let ty = b.named_ty(15, "Int");
            CtorParam {
                id: b.nid(),
                span: sp(12, 18),
                annotations: vec![],
                property: None,
                is_vararg: false,
                name: b.ident("x", 12, 13),
                ty: Some(ty),
                default: None,
            }
        };
        let ctor_param_y = {
            let ty = b.named_ty(28, "Int");
            CtorParam {
                id: b.nid(),
                span: sp(20, 31),
                annotations: vec![],
                property: Some(ValKind::Val),
                is_vararg: false,
                name: b.ident("y", 24, 25),
                ty: Some(ty),
                default: None,
            }
        };
        let primary_ctor = PrimaryCtorDecl {
            id: b.nid(),
            span: sp(11, 32),
            params: vec![ctor_param_x, ctor_param_y],
        };
        let super_base = {
            let ty = b.named_ty(35, "Base");
            let arg = {
                let value = b.ident_expr(40, 41, "x");
                b.call_arg(value)
            };
            SuperType {
                id: b.nid(),
                span: sp(35, 42),
                ty,
                args: vec![arg],
            }
        };
        let super_drawable = {
            let ty = b.named_ty(44, "Drawable");
            SuperType {
                id: b.nid(),
                span: sp(44, 52),
                ty,
                args: vec![],
            }
        };

        // var sum: Int
        //   get() = x + y
        //   set(v) { sum = v }
        let prop_ty = b.named_ty(70, "Int");
        let getter = {
            let x = b.ident_expr(88, 89, "x");
            let y = b.ident_expr(92, 93, "y");
            let body = b.expr(
                88,
                93,
                ExprKind::Binary {
                    lhs: Box::new(x),
                    op: BinaryOp::Add,
                    rhs: Box::new(y),
                },
            );
            AccessorDecl {
                id: b.nid(),
                span: sp(80, 93),
                kind: AccessorKind::Get,
                param: None,
                param_ty: None,
                body: AccessorBody::Expr(Box::new(body)),
            }
        };
        let setter = {
            let sum = b.ident("sum", 108, 111);
            let target = AssignTarget {
                id: b.nid(),
                span: sp(108, 111),
                kind: AssignTargetKind::Ident(sum),
            };
            let value = b.ident_expr(114, 115, "v");
            let assign = Stmt {
                id: b.nid(),
                span: sp(108, 115),
                kind: StmtKind::Assign { target, value },
            };
            let body = b.block(106, 116, vec![assign]);
            AccessorDecl {
                id: b.nid(),
                span: sp(96, 116),
                kind: AccessorKind::Set,
                param: Some(b.ident("v", 100, 101)),
                param_ty: None,
                body: AccessorBody::Block(body),
            }
        };
        let property = b.member(
            62,
            116,
            TypeMemberKind::Property(PropertyDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Var,
                name: b.ident("sum", 66, 69),
                ty: Some(prop_ty),
                delegate: None,
                init: None,
                accessors: vec![getter, setter],
            }),
        );

        // constructor() : this(0, 0) {}
        let secondary = {
            let zero_a = {
                let v = b.int(146, 147, 0);
                b.call_arg(v)
            };
            let zero_b = {
                let v = b.int(149, 150, 0);
                b.call_arg(v)
            };
            let delegation = CtorDelegation {
                span: sp(140, 151),
                kind: CtorDelegationKind::This,
                args: vec![zero_a, zero_b],
            };
            let body = b.block(152, 154, vec![]);
            b.member(
                124,
                154,
                TypeMemberKind::SecondaryCtor(SecondaryCtorDecl {
                    annotations: vec![],
                    span: Span::new(124, 134),
                    modifiers: vec![],
                    type_params: None,
                    params: vec![],
                    where_clause: None,
                    delegation: Some(delegation),
                    body,
                }),
            )
        };

        // fun move(dx: Int): Unit { return }
        let method = {
            let param = {
                let ty = b.named_ty(171, "Int");
                Param {
                    id: b.nid(),
                    span: sp(168, 174),
                    annotations: vec![],
                    is_vararg: false,
                    name: b.ident("dx", 168, 170),
                    ty: Some(ty),
                    default: None,
                }
            };
            let ret = b.unit_ty(177, 179);
            let ret_stmt = Stmt {
                id: b.nid(),
                span: sp(182, 188),
                kind: StmtKind::Return { value: None },
            };
            let body = b.block(180, 190, vec![ret_stmt]);
            b.member(
                158,
                190,
                TypeMemberKind::Fun(FunDecl {
                    name: b.ident("move", 162, 166),
                    params: vec![param],
                    return_ty: Some(ret),
                    body: Some(FunBody::Block(body)),
                    ..b.nofun()
                }),
            )
        };

        let type_body = TypeBody {
            id: b.nid(),
            span: sp(53, 192),
            members: vec![property, secondary, method],
        };
        let class = b.item(
            0,
            192,
            ItemKind::Type(TypeDecl {
                annotations: vec![],
                modifiers: vec![ast::Modifier {
                    kind: ast::ModifierKind::Open,
                    span: sp(0, 4),
                }],
                kind: TypeKind::Class,
                name: b.ident("Point", 6, 11),
                type_params: None,
                primary_ctor: Some(primary_ctor),
                supertypes: vec![super_base, super_drawable],
                where_clause: None,
                body: Some(type_body),
            }),
        );

        let file = b.file(192, vec![class]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..192
  items:
    ClassDecl 0..192 name=Point mods=[open]
      primary_ctor:
        PrimaryCtor 11..32
          CtorParam 12..18 name=x
            ty:
              PathType 15..18 path=Int
          CtorParam 20..31 name=y property=val
            ty:
              PathType 28..31 path=Int
      supertypes:
        SuperType 35..42
          ty:
            PathType 35..39 path=Base
          args:
            CallArg 40..41
              Ident 40..41 name=x
        SuperType 44..52
          ty:
            PathType 44..52 path=Drawable
      body:
        TypeBody 53..192
          PropertyDecl 62..116 kind=var name=sum
            ty:
              PathType 70..73 path=Int
            accessors:
              Getter 80..93
                body:
                  Binary 88..93 op=+
                    lhs:
                      Ident 88..89 name=x
                    rhs:
                      Ident 92..93 name=y
              Setter 96..116 param=v
                body:
                  Block 106..116
                    AssignStmt 108..115
                      target:
                        IdentTarget 108..111 name=sum
                      value:
                        Ident 114..115 name=v
          SecondaryCtor 124..154
            delegation:
              ThisDelegation 140..151
                CallArg 146..147
                  IntLit 146..147 value=0
                CallArg 149..150
                  IntLit 149..150 value=0
            body:
              Block 152..154
          FunDecl 158..190 name=move
            params:
              Param 168..174 name=dx
                ty:
                  PathType 171..174 path=Int
            return_ty:
              UnitType 177..179
            body:
              Block 180..190
                ReturnStmt 182..188
"#,
        );
    }

    // ------------------------------------------------------------------
    // 3. 表达式体函数：public fun add(a: Int, b: Int): Int = a + b
    // ------------------------------------------------------------------
    #[test]
    fn expr_body_fun() {
        let b = B::new();
        let param_a = Param {
            id: b.nid(),
            span: sp(17, 23),
            annotations: vec![],
            is_vararg: false,
            name: b.ident("a", 17, 18),
            ty: Some(b.named_ty(20, "Int")),
            default: None,
        };
        let param_b = Param {
            id: b.nid(),
            span: sp(25, 31),
            annotations: vec![],
            is_vararg: false,
            name: b.ident("b", 25, 26),
            ty: Some(b.named_ty(28, "Int")),
            default: None,
        };
        let lhs = b.ident_expr(41, 42, "a");
        let rhs = b.ident_expr(45, 46, "b");
        let body_expr = b.expr(
            41,
            46,
            ExprKind::Binary {
                lhs: Box::new(lhs),
                op: BinaryOp::Add,
                rhs: Box::new(rhs),
            },
        );
        let fun = b.fun_item(
            0,
            46,
            FunDecl {
                modifiers: vec![ast::Modifier {
                    kind: ast::ModifierKind::Public,
                    span: sp(0, 6),
                }],
                name: b.ident("add", 11, 14),
                params: vec![param_a, param_b],
                return_ty: Some(b.named_ty(35, "Int")),
                body: Some(FunBody::Expr(Box::new(body_expr))),
                ..b.nofun()
            },
        );
        let file = b.file(46, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..46
  items:
    FunDecl 0..46 name=add mods=[public]
      params:
        Param 17..23 name=a
          ty:
            PathType 20..23 path=Int
        Param 25..31 name=b
          ty:
            PathType 28..31 path=Int
      return_ty:
        PathType 35..38 path=Int
      body:
        Binary 41..46 op=+
          lhs:
            Ident 41..42 name=a
          rhs:
            Ident 45..46 name=b
"#,
        );
    }

    // ------------------------------------------------------------------
    // 4. 泛型：bound(ref/value)、eff 行参数（含默认值）、effect 注解、
    //    where 子句、带 effect 的函数类型参数
    //    fun <T: ref, eff E = Pure> run(f: () -> Unit / E): Unit / E
    //        where T: value { return }
    // ------------------------------------------------------------------
    #[test]
    fn generics_eff_where() {
        let b = B::new();

        let bound_ref = GenericBound::Ref(sp(11, 14));
        let param_t = TypeParam {
            id: b.nid(),
            span: sp(9, 14),
            variance: None,
            name: b.ident("T", 9, 10),
            bound: Some(bound_ref),
        };
        let pure_row = EffectRowExpr {
            id: b.nid(),
            span: sp(26, 30),
            terms: vec![EffectRowTerm {
                id: b.nid(),
                span: sp(26, 30),
                path: b.path(26, &["Pure"]),
                args: vec![],
            }],
            closed: None,
        };
        let eff_e = EffectRowParam {
            id: b.nid(),
            span: sp(20, 30),
            name: b.ident("E", 24, 25),
            default: Some(pure_row),
        };
        let type_params = TypeParamList {
            id: b.nid(),
            span: sp(8, 31),
            params: vec![param_t],
            effect_row: Some(eff_e),
        };

        // f: () -> Unit / E
        let e_row = EffectRowExpr {
            id: b.nid(),
            span: sp(55, 56),
            terms: vec![EffectRowTerm {
                id: b.nid(),
                span: sp(55, 56),
                path: b.path(55, &["E"]),
                args: vec![],
            }],
            closed: None,
        };
        let fn_ty = TypeRef {
            id: b.nid(),
            span: sp(41, 56),
            kind: TypeRefKind::Function {
                params: vec![],
                ret: Box::new(b.unit_ty(47, 51)),
                effect: Some(e_row),
            },
        };
        let param_f = Param {
            id: b.nid(),
            span: sp(39, 56),
            annotations: vec![],
            is_vararg: false,
            name: b.ident("f", 39, 40),
            ty: Some(fn_ty),
            default: None,
        };

        // / E（返回 effect 注解，复用形状）
        let ret_effect = EffectRowExpr {
            id: b.nid(),
            span: sp(67, 68),
            terms: vec![EffectRowTerm {
                id: b.nid(),
                span: sp(67, 68),
                path: b.path(67, &["E"]),
                args: vec![],
            }],
            closed: None,
        };

        let where_clause = WhereClause {
            id: b.nid(),
            span: sp(69, 84),
            constraints: vec![WhereConstraint {
                id: b.nid(),
                span: sp(75, 84),
                name: b.ident("T", 75, 76),
                bound: GenericBound::Value(sp(79, 84)),
            }],
        };

        let ret_stmt = Stmt {
            id: b.nid(),
            span: sp(87, 93),
            kind: StmtKind::Return { value: None },
        };
        let fun = b.fun_item(
            0,
            95,
            FunDecl {
                type_params: Some(type_params),
                name: b.ident("run", 32, 35),
                params: vec![param_f],
                return_ty: Some(b.unit_ty(59, 63)),
                effect: Some(ret_effect),
                where_clause: Some(where_clause),
                body: Some(FunBody::Block(b.block(85, 95, vec![ret_stmt]))),
                ..b.nofun()
            },
        );
        let file = b.file(95, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..95
  items:
    FunDecl 0..95 name=run
      type_params:
        TypeParams 8..31
          TypeParam 9..14 name=T
            RefBound 11..14
          effect_row:
            EffectRowParam 20..30 name=E
              EffectRow 26..30
                EffectRowTerm 26..30 path=Pure
      params:
        Param 39..56 name=f
          ty:
            FunctionType 41..56
              ret:
                UnitType 47..51
              effect:
                EffectRow 55..56
                  EffectRowTerm 55..56 path=E
      return_ty:
        UnitType 59..63
      effect:
        EffectRow 67..68
          EffectRowTerm 67..68 path=E
      where:
        Where 69..84
          WhereConstraint 75..84 name=T
            ValueBound 79..84
      body:
        Block 85..95
          ReturnStmt 87..93
"#,
        );
    }

    // ------------------------------------------------------------------
    // 5. when：or-pattern、guard、is 模式、else
    // ------------------------------------------------------------------
    #[test]
    fn when_or_guard() {
        let b = B::new();
        let subject = b.ident_expr(6, 7, "x");

        // 1 | 2 -> 10
        let or_pat = b.pattern(
            12,
            17,
            PatternKind::Or(vec![b.int_pat(12, 13, 1), b.int_pat(16, 17, 2)]),
        );
        let arm1 = WhenArm {
            id: b.nid(),
            span: sp(12, 23),
            pat: or_pat,
            guard: None,
            body: b.int(21, 23, 10),
        };

        // n if n > 0 -> n
        let guard_lhs = b.ident_expr(33, 34, "n");
        let guard_rhs = b.int(37, 38, 0);
        let guard = b.expr(
            33,
            38,
            ExprKind::Binary {
                lhs: Box::new(guard_lhs),
                op: BinaryOp::Gt,
                rhs: Box::new(guard_rhs),
            },
        );
        let arm2 = WhenArm {
            id: b.nid(),
            span: sp(26, 43),
            pat: b.bind_pat(26, 27, "n"),
            guard: Some(guard),
            body: b.ident_expr(42, 43, "n"),
        };

        // is Foo -> "foo"
        let is_pat = b.pattern(46, 52, PatternKind::Is(b.named_ty(49, "Foo")));
        let arm3 = WhenArm {
            id: b.nid(),
            span: sp(46, 61),
            pat: is_pat,
            guard: None,
            body: b.str_lit(56, 61, "foo"),
        };

        // else -> 0
        let arm4 = WhenArm {
            id: b.nid(),
            span: sp(64, 73),
            pat: b.pattern(64, 68, PatternKind::Else),
            guard: None,
            body: b.int(72, 73, 0),
        };

        let when = b.expr(
            0,
            75,
            ExprKind::When {
                subject: Box::new(subject),
                arms: vec![arm1, arm2, arm3, arm4],
            },
        );
        let fun = b.fun_item(
            0,
            75,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Expr(Box::new(when))),
                ..b.nofun()
            },
        );
        let file = b.file(75, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..75
  items:
    FunDecl 0..75 name=f
      body:
        When 0..75
          subject:
            Ident 6..7 name=x
          arms:
            WhenArm 12..23
              pat:
                OrPat 12..17
                  IntPat 12..13 value=1
                  IntPat 16..17 value=2
              body:
                IntLit 21..23 value=10
            WhenArm 26..43
              pat:
                BindPat 26..27 name=n
              guard:
                Binary 33..38 op=>
                  lhs:
                    Ident 33..34 name=n
                  rhs:
                    IntLit 37..38 value=0
              body:
                Ident 42..43 name=n
            WhenArm 46..61
              pat:
                IsPat 46..52
                  PathType 49..52 path=Foo
              body:
                StringLit 56..61 value="foo"
            WhenArm 64..73
              pat:
                ElsePat 64..68
              body:
                IntLit 72..73 value=0
"#,
        );
    }

    // ------------------------------------------------------------------
    // 6. handle：逃逸 continuation arm（`, k`）+ op 类型实参 + finally
    //    handle { work() } on { Query.ask<Int>(q: String), k -> k }
    //    finally { done() }
    // ------------------------------------------------------------------
    #[test]
    fn handle_escape() {
        let b = B::new();

        let work_call = {
            let callee = b.ident_expr(9, 13, "work");
            b.call(9, 15, callee, vec![])
        };
        let body = b.block(7, 17, vec![b.expr_stmt(work_call)]);

        let int_arg = b.tyarg(b.named_ty(32, "Int"));
        let binder = HandleBinder {
            id: b.nid(),
            span: sp(37, 46),
            name: b.ident("q", 37, 38),
            ty: Some(b.named_ty(40, "String")),
        };
        let op = HandleOp {
            id: b.nid(),
            span: sp(26, 47),
            effect_path: b.path(26, &["Query"]),
            effect_args: vec![],
            op: b.ident("ask", 32, 35),
            op_type_args: vec![int_arg],
            binders: vec![binder],
        };
        let arm = HandleArm {
            id: b.nid(),
            span: sp(26, 56),
            op,
            escape_continuation: Some(b.ident("k", 49, 50)),
            arrow_span: sp(51, 53),
            body: b.ident_expr(54, 55, "k"),
        };

        let done_call = {
            let callee = b.ident_expr(70, 74, "done");
            b.call(70, 76, callee, vec![])
        };
        let finally = b.block(68, 78, vec![b.expr_stmt(done_call)]);

        let handle = b.expr(
            0,
            78,
            ExprKind::Handle {
                body,
                arms: vec![arm],
                finally: Some(finally),
            },
        );
        let fun = b.fun_item(
            0,
            78,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Expr(Box::new(handle))),
                ..b.nofun()
            },
        );
        let file = b.file(78, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..78
  items:
    FunDecl 0..78 name=f
      body:
        Handle 0..78
          body:
            Block 7..17
              ExprStmt 9..15
                Call 9..15
                  callee:
                    Ident 9..13 name=work
          arms:
            HandleArm 26..56 escape=k
              op:
                HandleOp 26..47 path=Query op=ask
                  op_type_args:
                    TypeArg 32..35
                      PathType 32..35 path=Int
                  binders:
                    HandleBinder 37..46 name=q
                      PathType 40..46 path=String
              body:
                Ident 54..55 name=k
          finally:
            Block 68..78
              ExprStmt 70..76
                Call 70..76
                  callee:
                    Ident 70..74 name=done
"#,
        );
    }

    // ------------------------------------------------------------------
    // 7. try/catch 脱糖形态：AST 没有 Try 节点；parser 将
    //    `try { work() } catch (e: IOError) { report(e) } finally { done() }`
    //    直接构建为 handle over `scoop.core.Raise.raise`（合成标识符取
    //    catch 关键字 span，这里用 18..23）。本测试手工构建该脱糖结果，
    //    作为 parser（M3）必须产出的目标形态。
    // ------------------------------------------------------------------
    #[test]
    fn try_catch_desugars_to_handle_over_raise() {
        let b = B::new();

        let work_call = {
            let callee = b.ident_expr(6, 10, "work");
            b.call(6, 12, callee, vec![])
        };
        let body = b.block(4, 14, vec![b.expr_stmt(work_call)]);

        // 合成路径 scoop.core.Raise 与 op raise：span 取 catch 关键字。
        let catch_kw = 18;
        let op = HandleOp {
            id: b.nid(),
            span: sp(catch_kw, 47),
            effect_path: TypePath {
                segments: vec![
                    b.ident("scoop", catch_kw, catch_kw + 5),
                    b.ident("core", catch_kw, catch_kw + 4),
                    b.ident("Raise", catch_kw, catch_kw + 5),
                ],
                span: sp(catch_kw, catch_kw + 5),
            },
            effect_args: vec![],
            op: b.ident("raise", catch_kw, catch_kw + 5),
            op_type_args: vec![],
            binders: vec![HandleBinder {
                id: b.nid(),
                span: sp(26, 37),
                name: b.ident("e", 26, 27),
                ty: Some(b.named_ty(29, "IOError")),
            }],
        };
        let report_call = {
            let callee = b.ident_expr(41, 47, "report");
            let arg = b.call_arg(b.ident_expr(48, 49, "e"));
            b.call(41, 50, callee, vec![arg])
        };
        let arm = HandleArm {
            id: b.nid(),
            span: sp(catch_kw, 52),
            op,
            escape_continuation: None,
            arrow_span: sp(39, 52),
            body: b.block_expr(39, 52, vec![b.expr_stmt(report_call)]),
        };

        let done_call = {
            let callee = b.ident_expr(64, 68, "done");
            b.call(64, 70, callee, vec![])
        };
        let finally = b.block(62, 72, vec![b.expr_stmt(done_call)]);

        let handle = b.expr(
            0,
            72,
            ExprKind::Handle {
                body,
                arms: vec![arm],
                finally: Some(finally),
            },
        );
        let fun = b.fun_item(
            0,
            72,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Expr(Box::new(handle))),
                ..b.nofun()
            },
        );
        let file = b.file(72, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        assert!(dump.contains("path=scoop.core.Raise op=raise"));
        check(
            dump,
            r#"File 0..72
  items:
    FunDecl 0..72 name=f
      body:
        Handle 0..72
          body:
            Block 4..14
              ExprStmt 6..12
                Call 6..12
                  callee:
                    Ident 6..10 name=work
          arms:
            HandleArm 18..52
              op:
                HandleOp 18..47 path=scoop.core.Raise op=raise
                  binders:
                    HandleBinder 26..37 name=e
                      PathType 29..36 path=IOError
              body:
                Block 39..52
                  ExprStmt 41..50
                    Call 41..50
                      callee:
                        Ident 41..47 name=report
                      args:
                        CallArg 48..49
                          Ident 48..49 name=e
          finally:
            Block 62..72
              ExprStmt 64..70
                Call 64..70
                  callee:
                    Ident 64..68 name=done
"#,
        );
    }

    // ------------------------------------------------------------------
    // 8. with 更新：字段路径（含元组整数段）
    //    p with { pos.x: 1, 0.1: 2 }
    // ------------------------------------------------------------------
    #[test]
    fn with_update() {
        let b = B::new();
        let base = b.ident_expr(0, 1, "p");
        let field_a = WithUpdateField {
            id: b.nid(),
            span: sp(9, 17),
            path: FieldPath {
                span: sp(9, 14),
                segments: vec![
                    MemberName::Named(b.ident("pos", 9, 12)),
                    MemberName::Named(b.ident("x", 13, 14)),
                ],
            },
            value: b.int(16, 17, 1),
        };
        // `0.1` float token 已由 parser 拆成两个整数段。
        let field_b = WithUpdateField {
            id: b.nid(),
            span: sp(19, 24),
            path: FieldPath {
                span: sp(19, 22),
                segments: vec![
                    MemberName::TupleIndex {
                        value: 0,
                        span: sp(19, 20),
                    },
                    MemberName::TupleIndex {
                        value: 1,
                        span: sp(21, 22),
                    },
                ],
            },
            value: b.int(23, 24, 2),
        };
        let with = b.expr(
            0,
            26,
            ExprKind::WithUpdate {
                base: Box::new(base),
                updates: vec![field_a, field_b],
            },
        );
        let fun = b.fun_item(
            0,
            26,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Expr(Box::new(with))),
                ..b.nofun()
            },
        );
        let file = b.file(26, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..26
  items:
    FunDecl 0..26 name=f
      body:
        WithUpdate 0..26
          base:
            Ident 0..1 name=p
          updates:
            WithUpdateField 9..17 path=pos.x
              IntLit 16..17 value=1
            WithUpdateField 19..24 path=0.1
              IntLit 23..24 value=2
"#,
        );
    }

    // ------------------------------------------------------------------
    // 9. 下标读取 + 下标赋值（IndexAssign）：a[i, j] 与 a[i, j] = 42
    // ------------------------------------------------------------------
    #[test]
    fn index_and_index_assign() {
        let b = B::new();

        let mk_index = |b: &B| {
            let receiver = b.ident_expr(0, 1, "a");
            let i = b.ident_expr(2, 3, "i");
            let j = b.ident_expr(5, 6, "j");
            (receiver, vec![i, j])
        };

        // 读取：a[i, j]
        let (receiver, indices) = mk_index(&b);
        let read = b.expr(
            0,
            7,
            ExprKind::Index {
                receiver: Box::new(receiver),
                indices,
            },
        );
        let read_stmt = b.expr_stmt(read);

        // 赋值：a[i, j] = 42
        let (receiver, indices) = mk_index(&b);
        let target = AssignTarget {
            id: b.nid(),
            span: sp(0, 7),
            kind: AssignTargetKind::Index {
                receiver: Box::new(receiver),
                indices,
            },
        };
        let assign_stmt = Stmt {
            id: b.nid(),
            span: sp(0, 12),
            kind: StmtKind::Assign {
                target,
                value: b.int(10, 12, 42),
            },
        };

        let block = b.block(0, 13, vec![read_stmt, assign_stmt]);
        let fun = b.fun_item(
            0,
            13,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Block(block)),
                ..b.nofun()
            },
        );
        let file = b.file(13, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..13
  items:
    FunDecl 0..13 name=f
      body:
        Block 0..13
          ExprStmt 0..7
            Index 0..7
              receiver:
                Ident 0..1 name=a
              indices:
                Ident 2..3 name=i
                Ident 5..6 name=j
          AssignStmt 0..12
            target:
              IndexTarget 0..7
                receiver:
                  Ident 0..1 name=a
                indices:
                  Ident 2..3 name=i
                  Ident 5..6 name=j
            value:
              IntLit 10..12 value=42
"#,
        );
    }

    // ------------------------------------------------------------------
    // 10. 上下文中缀调用：1 until 10 step 2（左结合嵌套）与 n downTo 0
    // ------------------------------------------------------------------
    #[test]
    fn infix_until_downto_step() {
        let b = B::new();

        // (1 until 10) step 2
        let one = b.int(0, 1, 1);
        let ten = b.int(8, 10, 10);
        let inner = b.expr(
            0,
            10,
            ExprKind::InfixCall {
                receiver: Box::new(one),
                name: b.ident("until", 2, 7),
                arg: Box::new(ten),
            },
        );
        let two = b.int(16, 17, 2);
        let outer = b.expr(
            0,
            17,
            ExprKind::InfixCall {
                receiver: Box::new(inner),
                name: b.ident("step", 11, 15),
                arg: Box::new(two),
            },
        );
        let stmt1 = b.expr_stmt(outer);

        // n downTo 0
        let n = b.ident_expr(18, 19, "n");
        let zero = b.int(27, 28, 0);
        let downto = b.expr(
            18,
            28,
            ExprKind::InfixCall {
                receiver: Box::new(n),
                name: b.ident("downTo", 20, 26),
                arg: Box::new(zero),
            },
        );
        let stmt2 = b.expr_stmt(downto);

        let block = b.block(0, 29, vec![stmt1, stmt2]);
        let fun = b.fun_item(
            0,
            29,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Block(block)),
                ..b.nofun()
            },
        );
        let file = b.file(29, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..29
  items:
    FunDecl 0..29 name=f
      body:
        Block 0..29
          ExprStmt 0..17
            InfixCall 0..17 name=step
              receiver:
                InfixCall 0..10 name=until
                  receiver:
                    IntLit 0..1 value=1
                  arg:
                    IntLit 8..10 value=10
              arg:
                IntLit 16..17 value=2
          ExprStmt 18..28
            InfixCall 18..28 name=downTo
              receiver:
                Ident 18..19 name=n
              arg:
                IntLit 27..28 value=0
"#,
        );
    }

    // ------------------------------------------------------------------
    // 11. 嵌套可空类型：val x: List<Int>??（两层 Nullable，不拍平）
    //     以及可空接收者函数类型 T?.() -> Unit
    // ------------------------------------------------------------------
    #[test]
    fn nested_nullable_type() {
        let b = B::new();

        let list_ty = TypeRef {
            id: b.nid(),
            span: sp(7, 17),
            kind: TypeRefKind::Path {
                path: b.path(7, &["List"]),
                args: vec![b.tyarg(b.named_ty(12, "Int"))],
            },
        };
        let nullable_inner = TypeRef {
            id: b.nid(),
            span: sp(7, 18),
            kind: TypeRefKind::Nullable(Box::new(list_ty)),
        };
        let nullable_outer = TypeRef {
            id: b.nid(),
            span: sp(7, 19),
            kind: TypeRefKind::Nullable(Box::new(nullable_inner)),
        };
        let val_x = b.item(
            0,
            19,
            ItemKind::Val(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Name(b.ident("x", 4, 5)),
                ty: Some(nullable_outer),
                init: None,
            }),
        );

        // val g: T?.() -> Unit
        let t_nullable = TypeRef {
            id: b.nid(),
            span: sp(28, 30),
            kind: TypeRefKind::Nullable(Box::new(b.named_ty(28, "T"))),
        };
        let receiver_fn = TypeRef {
            id: b.nid(),
            span: sp(28, 41),
            kind: TypeRefKind::ReceiverFunction {
                receiver: Box::new(t_nullable),
                params: vec![],
                ret: Box::new(b.unit_ty(37, 41)),
                effect: None,
            },
        };
        let val_g = b.item(
            20,
            41,
            ItemKind::Val(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Name(b.ident("g", 24, 25)),
                ty: Some(receiver_fn),
                init: None,
            }),
        );

        let file = b.file(41, vec![val_x, val_g]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..41
  items:
    ValDecl 0..19 kind=val name=x
      ty:
        NullableType 7..19
          NullableType 7..18
            PathType 7..17 path=List
              args:
                TypeArg 12..15
                  PathType 12..15 path=Int
    ValDecl 20..41 kind=val name=g
      ty:
        ReceiverFunctionType 28..41
          receiver:
            NullableType 28..30
              PathType 28..29 path=T
          ret:
            UnitType 37..41
"#,
        );
    }

    // ------------------------------------------------------------------
    // 12. delegated property：class C { val x: Int by lazy(0) }
    // ------------------------------------------------------------------
    #[test]
    fn delegated_property() {
        let b = B::new();
        let delegate = {
            let callee = b.ident_expr(31, 35, "lazy");
            let arg = b.call_arg(b.int(36, 37, 0));
            b.call(31, 38, callee, vec![arg])
        };
        let property = b.member(
            12,
            38,
            TypeMemberKind::Property(PropertyDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                name: b.ident("x", 16, 17),
                ty: Some(b.named_ty(20, "Int")),
                delegate: Some(delegate),
                init: None,
                accessors: vec![],
            }),
        );
        let class = b.item(
            0,
            40,
            ItemKind::Type(TypeDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: TypeKind::Class,
                name: b.ident("C", 6, 7),
                type_params: None,
                primary_ctor: None,
                supertypes: vec![],
                where_clause: None,
                body: Some(TypeBody {
                    id: b.nid(),
                    span: sp(10, 40),
                    members: vec![property],
                }),
            }),
        );
        let file = b.file(40, vec![class]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..40
  items:
    ClassDecl 0..40 name=C
      body:
        TypeBody 10..40
          PropertyDecl 12..38 kind=val name=x
            ty:
              PathType 20..23 path=Int
            delegate:
              Call 31..38
                callee:
                  Ident 31..35 name=lazy
                args:
                  CallArg 36..37
                    IntLit 36..37 value=0
"#,
        );
    }

    // ------------------------------------------------------------------
    // 13. enum：带字段的 variant + 判别值 + 底层类型超类型
    //     enum Color : Int { Red(val r: Int) = 1, Blue = 2 }
    // ------------------------------------------------------------------
    #[test]
    fn enum_fields_and_discriminant() {
        let b = B::new();
        let red = b.member(
            20,
            40,
            TypeMemberKind::EnumVariant(EnumVariantDecl {
                annotations: vec![],
                name: b.ident("Red", 20, 23),
                fields: vec![EnumVariantField {
                    id: b.nid(),
                    span: sp(24, 35),
                    name: b.ident("r", 28, 29),
                    ty: b.named_ty(31, "Int"),
                }],
                discriminant: Some(b.int(38, 39, 1)),
            }),
        );
        let blue = b.member(
            42,
            51,
            TypeMemberKind::EnumVariant(EnumVariantDecl {
                annotations: vec![],
                name: b.ident("Blue", 42, 46),
                fields: vec![],
                discriminant: Some(b.int(50, 51, 2)),
            }),
        );
        let enum_decl = b.item(
            0,
            53,
            ItemKind::Type(TypeDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: TypeKind::Enum,
                name: b.ident("Color", 5, 10),
                type_params: None,
                primary_ctor: None,
                supertypes: vec![SuperType {
                    id: b.nid(),
                    span: sp(13, 16),
                    ty: b.named_ty(13, "Int"),
                    args: vec![],
                }],
                where_clause: None,
                body: Some(TypeBody {
                    id: b.nid(),
                    span: sp(18, 53),
                    members: vec![red, blue],
                }),
            }),
        );
        let file = b.file(53, vec![enum_decl]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..53
  items:
    EnumDecl 0..53 name=Color
      supertypes:
        SuperType 13..16
          ty:
            PathType 13..16 path=Int
      body:
        TypeBody 18..53
          EnumVariant 20..40 name=Red
            fields:
              EnumVariantField 24..35 name=r
                PathType 31..34 path=Int
            discriminant:
              IntLit 38..39 value=1
          EnumVariant 42..51 name=Blue
            discriminant:
              IntLit 50..51 value=2
"#,
        );
    }

    // ------------------------------------------------------------------
    // 14. f-string：f"a${x}b${y + 1}" 的 Text/Expr 片段（+ raw 标志）
    // ------------------------------------------------------------------
    #[test]
    fn interpolated_string() {
        let b = B::new();
        let hole_y = {
            let y = b.ident_expr(14, 15, "y");
            let one = b.int(18, 19, 1);
            b.expr(
                14,
                19,
                ExprKind::Binary {
                    lhs: Box::new(y),
                    op: BinaryOp::Add,
                    rhs: Box::new(one),
                },
            )
        };
        let fstring = b.expr(
            8,
            21,
            ExprKind::InterpolatedString {
                raw: false,
                parts: vec![
                    StringPart::Text("a".to_string()),
                    StringPart::Expr(b.ident_expr(9, 10, "x")),
                    StringPart::Text("b".to_string()),
                    StringPart::Expr(hole_y),
                ],
            },
        );
        let val_s = b.item(
            0,
            21,
            ItemKind::Val(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Name(b.ident("s", 4, 5)),
                ty: None,
                init: Some(fstring),
            }),
        );
        // raw 空 f-string：f"""..."""
        let raw_fstring = b.expr(
            34,
            40,
            ExprKind::InterpolatedString {
                raw: true,
                parts: vec![StringPart::Text("raw".to_string())],
            },
        );
        let val_r = b.item(
            22,
            40,
            ItemKind::Val(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Name(b.ident("r", 26, 27)),
                ty: None,
                init: Some(raw_fstring),
            }),
        );
        let file = b.file(40, vec![val_s, val_r]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..40
  items:
    ValDecl 0..21 kind=val name=s
      init:
        InterpolatedString 8..21
          TextPart "a"
          ExprPart
            Ident 9..10 name=x
          TextPart "b"
          ExprPart
            Binary 14..19 op=+
              lhs:
                Ident 14..15 name=y
              rhs:
                IntLit 18..19 value=1
    ValDecl 22..40 kind=val name=r
      init:
        InterpolatedString 34..40 raw
          TextPart "raw"
"#,
        );
    }

    // ------------------------------------------------------------------
    // 15. 解构 val：tuple（带 .. rest）、struct（简写 + 子模式 + ..）、
    //     variant（带参数）
    // ------------------------------------------------------------------
    #[test]
    fn destructuring_val_patterns() {
        let b = B::new();

        // val (a, ..) = t
        let tuple_pat = b.pattern(
            12,
            19,
            PatternKind::Tuple(vec![
                b.bind_pat(13, 14, "a"),
                b.pattern(16, 18, PatternKind::Rest),
            ]),
        );
        let stmt1 = Stmt {
            id: b.nid(),
            span: sp(8, 23),
            kind: StmtKind::LocalVal(Box::new(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Pattern(tuple_pat),
                ty: None,
                init: Some(b.ident_expr(22, 23, "t")),
            })),
        };

        // val Point { x, y: b, .. } = p
        let struct_pat = b.pattern(
            32,
            51,
            PatternKind::Struct {
                path: b.path(32, &["Point"]),
                fields: vec![
                    StructPatternField {
                        id: b.nid(),
                        span: sp(40, 41),
                        name: b.ident("x", 40, 41),
                        pattern: None,
                    },
                    StructPatternField {
                        id: b.nid(),
                        span: sp(43, 47),
                        name: b.ident("y", 43, 44),
                        pattern: Some(b.bind_pat(46, 47, "b")),
                    },
                ],
                rest: Some(sp(49, 51)),
            },
        );
        let stmt2 = Stmt {
            id: b.nid(),
            span: sp(28, 55),
            kind: StmtKind::LocalVal(Box::new(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Pattern(struct_pat),
                ty: None,
                init: Some(b.ident_expr(54, 55, "p")),
            })),
        };

        // val Option.Some(v) = opt
        let variant_pat = b.pattern(
            64,
            78,
            PatternKind::Variant {
                path: b.path(64, &["Option", "Some"]),
                args: Some(vec![b.bind_pat(76, 77, "v")]),
            },
        );
        let stmt3 = Stmt {
            id: b.nid(),
            span: sp(60, 83),
            kind: StmtKind::LocalVal(Box::new(ValDecl {
                annotations: vec![],
                modifiers: vec![],
                kind: ValKind::Val,
                binding: ValBinding::Pattern(variant_pat),
                ty: None,
                init: Some(b.ident_expr(80, 83, "opt")),
            })),
        };

        let block = b.block(0, 85, vec![stmt1, stmt2, stmt3]);
        let fun = b.fun_item(
            0,
            85,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Block(block)),
                ..b.nofun()
            },
        );
        let file = b.file(85, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..85
  items:
    FunDecl 0..85 name=f
      body:
        Block 0..85
          LocalValStmt 8..23
            ValDecl 8..23 kind=val
              pattern:
                TuplePat 12..19
                  BindPat 13..14 name=a
                  RestPat 16..18
              init:
                Ident 22..23 name=t
          LocalValStmt 28..55
            ValDecl 28..55 kind=val
              pattern:
                StructPat 32..51 path=Point rest
                  StructPatField 40..41 name=x shorthand
                  StructPatField 43..47 name=y
                    BindPat 46..47 name=b
              init:
                Ident 54..55 name=p
          LocalValStmt 60..83
            ValDecl 60..83 kind=val
              pattern:
                VariantPat 64..78 path=Option.Some
                  BindPat 76..77 name=v
              init:
                Ident 80..83 name=opt
"#,
        );
    }

    // ------------------------------------------------------------------
    // 16. 其余表达式形态：lambda（解包/块体）、do/@Unsafe/@Safe、
    //     注解表达式、struct 字面量、class 字面量、类型应用（star/eff）、
    //     splice、!!、?.、cast/is、elvis、一元、命名/spread 实参、
    //     trailing lambda 折叠、tuple/array/unit、元组段成员、if/while/for
    // ------------------------------------------------------------------
    #[test]
    fn expressions_misc() {
        let b = B::new();
        let mut stmts: Vec<Stmt> = Vec::new();
        let mut pos = 0usize;

        // lambda：{ a: Int, b -> a }（主体解包为表达式）
        {
            let (s, e) = advance(&mut pos, 15);
            let int_ty = b.named_ty(s + 5, "Int");
            let lambda = LambdaExpr {
                is_safe: false,
                params: vec![
                    LambdaParam {
                        id: b.nid(),
                        span: sp(s + 2, s + 8),
                        name: b.ident("a", s + 2, s + 3),
                        ty: Some(int_ty),
                    },
                    LambdaParam {
                        id: b.nid(),
                        span: sp(s + 10, s + 11),
                        name: b.ident("b", s + 10, s + 11),
                        ty: None,
                    },
                ],
                body: LambdaBody::Expr(Box::new(b.ident_expr(s + 14, s + 15, "a"))),
            };
            let e2 = e + 1;
            stmts.push(b.expr_stmt(b.expr(s, e2, ExprKind::Lambda(lambda))));
        }
        // @Safe 闭包（is_safe）+ 块体 lambda
        {
            let (s, e) = advance(&mut pos, 12);
            let inner = b.block(s + 7, e - 1, vec![]);
            let lambda = LambdaExpr {
                is_safe: true,
                params: vec![],
                body: LambdaBody::Block(inner),
            };
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::Lambda(lambda))));
        }
        // do 块 / @Unsafe do / @Safe do
        {
            let (s, e) = advance(&mut pos, 6);
            let blk = b.block(s + 3, e, vec![]);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::DoBlock(blk))));
        }
        {
            let (s, e) = advance(&mut pos, 14);
            let blk = b.block(s + 11, e, vec![]);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::UnsafeBlock(blk))));
        }
        {
            let (s, e) = advance(&mut pos, 12);
            let blk = b.block(s + 9, e, vec![]);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::SafeBlock(blk))));
        }
        // @Suppress("x") expr（注解表达式）
        {
            let (s, e) = advance(&mut pos, 18);
            let ann = ast::AnnotationUse {
                id: b.nid(),
                span: sp(s, s + 14),
                target: None,
                path: b.path(s + 1, &["Suppress"]),
                args: vec![ast::AnnotationArg {
                    id: b.nid(),
                    span: sp(s + 10, s + 13),
                    name: None,
                    value: b.str_lit(s + 10, s + 13, "x"),
                }],
            };
            let inner = b.ident_expr(s + 15, e, "x");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::Annotated {
                    annotations: vec![ann],
                    expr: Box::new(inner),
                },
            )));
        }
        // struct 字面量 Point { x: 1 }
        {
            let (s, e) = advance(&mut pos, 14);
            let field = StructLitField {
                id: b.nid(),
                span: sp(s + 8, s + 13),
                name: b.ident("x", s + 8, s + 9),
                value: b.int(s + 11, s + 12, 1),
            };
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::StructLit {
                    name: b.ident("Point", s, s + 5),
                    fields: vec![field],
                },
            )));
        }
        // class 字面量 foo.Bar::class
        {
            let (s, e) = advance(&mut pos, 15);
            let path = b.path(s, &["foo", "Bar"]);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::ClassLit { path })));
        }
        // 类型应用 f<Int, *, eff Pure>
        {
            let (s, e) = advance(&mut pos, 20);
            let callee = b.ident_expr(s, s + 1, "f");
            let star = TypeArg {
                id: b.nid(),
                span: sp(s + 7, s + 8),
                kind: TypeArgKind::Star,
            };
            let eff = TypeArg {
                id: b.nid(),
                span: sp(s + 10, s + 19),
                kind: TypeArgKind::Effect(EffectRowExpr {
                    id: b.nid(),
                    span: sp(s + 14, s + 18),
                    terms: vec![EffectRowTerm {
                        id: b.nid(),
                        span: sp(s + 14, s + 18),
                        path: b.path(s + 14, &["Pure"]),
                        args: vec![],
                    }],
                    closed: None,
                }),
            };
            let apply = b.expr(
                s,
                e,
                ExprKind::TypeApply {
                    callee: Box::new(callee),
                    args: vec![b.tyarg(b.named_ty(s + 2, "Int")), star, eff],
                },
            );
            stmts.push(b.expr_stmt(apply));
        }
        // splice a.[b]、!!、?.、元组段 t.0
        {
            let (s, e) = advance(&mut pos, 6);
            let receiver = b.ident_expr(s, s + 1, "a");
            let field = b.ident_expr(s + 3, s + 4, "b");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::SpliceField {
                    receiver: Box::new(receiver),
                    field: Box::new(field),
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 3);
            let inner = b.ident_expr(s, s + 1, "x");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::NotNullAssert {
                    expr: Box::new(inner),
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 4);
            let receiver = b.ident_expr(s, s + 1, "a");
            let member = MemberName::Named(b.ident("b", s + 3, s + 4));
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::SafeMemberAccess {
                    receiver: Box::new(receiver),
                    member,
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 3);
            let receiver = b.ident_expr(s, s + 1, "t");
            let member = MemberName::TupleIndex {
                value: 0,
                span: sp(s + 2, s + 3),
            };
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::MemberAccess {
                    receiver: Box::new(receiver),
                    member,
                },
            )));
        }
        // cast / typecheck / elvis / 一元
        {
            let (s, e) = advance(&mut pos, 8);
            let inner = b.ident_expr(s, s + 1, "x");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::Cast {
                    expr: Box::new(inner),
                    op: CastOp::AsSafe,
                    ty: b.named_ty(s + 5, "Int"),
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 7);
            let inner = b.ident_expr(s, s + 1, "x");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::TypeCheck {
                    expr: Box::new(inner),
                    op: TypeCheckOp::NotIs,
                    ty: b.named_ty(s + 5, "Int"),
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 6);
            let lhs = b.ident_expr(s, s + 1, "a");
            let rhs = b.ident_expr(s + 5, s + 6, "b");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::Binary {
                    lhs: Box::new(lhs),
                    op: BinaryOp::Elvis,
                    rhs: Box::new(rhs),
                },
            )));
        }
        {
            let (s, e) = advance(&mut pos, 2);
            let inner = b.ident_expr(s + 1, s + 2, "x");
            stmts.push(b.expr_stmt(b.expr(
                s,
                e,
                ExprKind::Unary {
                    op: UnaryOp::BitNot,
                    expr: Box::new(inner),
                },
            )));
        }
        // 调用：位置 / 命名 / spread / 命名 spread
        {
            let (s, e) = advance(&mut pos, 26);
            let callee = b.ident_expr(s, s + 4, "func");
            let arg_pos = b.call_arg(b.int(s + 5, s + 6, 1));
            let named_val = b.int(s + 13, s + 14, 2);
            let arg_named = CallArg {
                id: b.nid(),
                span: sp(s + 8, s + 14),
                name: Some(b.ident("n", s + 8, s + 9)),
                is_spread: false,
                value: named_val,
            };
            let spread_val = b.ident_expr(s + 17, s + 19, "xs");
            let arg_spread = CallArg {
                id: b.nid(),
                span: sp(s + 16, s + 19),
                name: None,
                is_spread: true,
                value: spread_val,
            };
            let nspread_val = b.ident_expr(s + 24, s + 26, "ys");
            let arg_nspread = CallArg {
                id: b.nid(),
                span: sp(s + 21, s + 26),
                name: Some(b.ident("m", s + 21, s + 22)),
                is_spread: true,
                value: nspread_val,
            };
            stmts.push(b.expr_stmt(b.call(
                s,
                e,
                callee,
                vec![arg_pos, arg_named, arg_spread, arg_nspread],
            )));
        }
        // trailing lambda 折叠：combine(1) { .. } { .. } → 一个 Call 三个实参
        {
            let (s, e) = advance(&mut pos, 20);
            let callee = b.ident_expr(s, s + 7, "combine");
            let arg1 = b.call_arg(b.int(s + 8, s + 9, 1));
            let mk_lambda = |b: &B, ls: usize, le: usize| {
                let span = sp(ls, le);
                CallArg {
                    id: b.nid(),
                    span,
                    name: None,
                    is_spread: false,
                    value: Expr {
                        id: b.nid(),
                        span,
                        kind: ExprKind::Lambda(LambdaExpr {
                            is_safe: false,
                            params: vec![],
                            body: LambdaBody::Block(Block {
                                id: b.nid(),
                                span,
                                stmts: vec![],
                                last_trailing_semi: false,
                            }),
                        }),
                    },
                }
            };
            let lam1 = mk_lambda(&b, s + 11, s + 15);
            let lam2 = mk_lambda(&b, s + 16, s + 20);
            stmts.push(b.expr_stmt(b.call(s, e, callee, vec![arg1, lam1, lam2])));
        }
        // tuple / array / unit 字面量
        {
            let (s, e) = advance(&mut pos, 6);
            let one = b.int(s + 1, s + 2, 1);
            let two = b.int(s + 4, s + 5, 2);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::TupleLit(vec![one, two]))));
        }
        {
            let (s, e) = advance(&mut pos, 3);
            let one = b.int(s + 1, s + 2, 1);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::ArrayLit(vec![one]))));
        }
        {
            let (s, e) = advance(&mut pos, 2);
            stmts.push(b.expr_stmt(b.expr(s, e, ExprKind::UnitLit)));
        }
        // if / while / for / break / continue / return 值
        {
            let (s, e) = advance(&mut pos, 16);
            let cond = b.ident_expr(s + 4, s + 5, "c");
            let then_b = b.block_expr(s + 7, s + 9, vec![]);
            let else_b = b.block_expr(s + 15, s + 17, vec![]);
            stmts.push(b.expr_stmt(b.expr(
                s,
                e + 1,
                ExprKind::If {
                    cond: Box::new(cond),
                    then_branch: Box::new(then_b),
                    else_branch: Some(Box::new(else_b)),
                },
            )));
            pos += 1;
        }
        {
            let (s, e) = advance(&mut pos, 12);
            let cond = b.ident_expr(s + 7, s + 8, "c");
            let body = b.block(s + 10, e, vec![]);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::While { cond, body },
            });
        }
        {
            let (s, e) = advance(&mut pos, 15);
            let iter = b.ident_expr(s + 10, s + 12, "xs");
            let body = b.block(s + 13, e, vec![]);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::For {
                    binder: b.ident("x", s + 5, s + 6),
                    iter,
                    body,
                },
            });
        }
        {
            let (s, e) = advance(&mut pos, 5);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::Break,
            });
        }
        {
            let (s, e) = advance(&mut pos, 8);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::Continue,
            });
        }
        {
            let (s, e) = advance(&mut pos, 8);
            let value = b.int(s + 7, s + 8, 1);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::Return { value: Some(value) },
            });
        }
        // 空语句
        {
            let (s, e) = advance(&mut pos, 1);
            stmts.push(Stmt {
                id: b.nid(),
                span: sp(s, e),
                kind: StmtKind::Empty,
            });
        }

        let block = b.block(0, pos, stmts);
        let fun = b.fun_item(
            0,
            pos,
            FunDecl {
                name: b.ident("f", 0, 1),
                body: Some(FunBody::Block(block)),
                ..b.nofun()
            },
        );
        let file = b.file(pos, vec![fun]);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..288
  items:
    FunDecl 0..288 name=f
      body:
        Block 0..288
          ExprStmt 0..16
            Lambda 0..16
              params:
                LambdaParam 2..8 name=a
                  PathType 5..8 path=Int
                LambdaParam 10..11 name=b
              body:
                Ident 14..15 name=a
          ExprStmt 15..27
            Lambda 15..27 safe
              body:
                Block 22..26
          ExprStmt 27..33
            DoBlock 27..33
              Block 30..33
          ExprStmt 33..47
            UnsafeBlock 33..47
              Block 44..47
          ExprStmt 47..59
            SafeBlock 47..59
              Block 56..59
          ExprStmt 59..77
            Annotated 59..77
              annotations:
                Annotation 59..73 path=Suppress
                  args:
                    AnnotationArg 69..72
                      StringLit 69..72 value="x"
              expr:
                Ident 74..77 name=x
          ExprStmt 77..91
            StructLit 77..91 name=Point
              StructField 85..90 name=x
                IntLit 88..89 value=1
          ExprStmt 91..106
            ClassLit 91..106 path=foo.Bar
          ExprStmt 106..126
            TypeApply 106..126
              callee:
                Ident 106..107 name=f
              args:
                TypeArg 108..111
                  PathType 108..111 path=Int
                StarProjection 113..114
                EffectRowArg 116..125
                  EffectRow 120..124
                    EffectRowTerm 120..124 path=Pure
          ExprStmt 126..132
            SpliceField 126..132
              receiver:
                Ident 126..127 name=a
              field:
                Ident 129..130 name=b
          ExprStmt 132..135
            NotNullAssert 132..135
              Ident 132..133 name=x
          ExprStmt 135..139
            SafeMemberAccess 135..139 member=b
              Ident 135..136 name=a
          ExprStmt 139..142
            MemberAccess 139..142 member=0
              Ident 139..140 name=t
          ExprStmt 142..150
            Cast 142..150 op=as?
              expr:
                Ident 142..143 name=x
              ty:
                PathType 147..150 path=Int
          ExprStmt 150..157
            TypeCheck 150..157 op=!is
              expr:
                Ident 150..151 name=x
              ty:
                PathType 155..158 path=Int
          ExprStmt 157..163
            Binary 157..163 op=?:
              lhs:
                Ident 157..158 name=a
              rhs:
                Ident 162..163 name=b
          ExprStmt 163..165
            Unary 163..165 op=~
              Ident 164..165 name=x
          ExprStmt 165..191
            Call 165..191
              callee:
                Ident 165..169 name=func
              args:
                CallArg 170..171
                  IntLit 170..171 value=1
                CallArg 173..179 name=n
                  IntLit 178..179 value=2
                CallArg 181..184 spread
                  Ident 182..184 name=xs
                CallArg 186..191 name=m spread
                  Ident 189..191 name=ys
          ExprStmt 191..211
            Call 191..211
              callee:
                Ident 191..198 name=combine
              args:
                CallArg 199..200
                  IntLit 199..200 value=1
                CallArg 202..206
                  Lambda 202..206
                    body:
                      Block 202..206
                CallArg 207..211
                  Lambda 207..211
                    body:
                      Block 207..211
          ExprStmt 211..217
            TupleLit 211..217
              IntLit 212..213 value=1
              IntLit 215..216 value=2
          ExprStmt 217..220
            ArrayLit 217..220
              IntLit 218..219 value=1
          ExprStmt 220..222
            UnitLit 220..222
          ExprStmt 222..239
            If 222..239
              cond:
                Ident 226..227 name=c
              then:
                Block 229..231
              else:
                Block 237..239
          WhileStmt 239..251
            cond:
              Ident 246..247 name=c
            body:
              Block 249..251
          ForStmt 251..266 binder=x
            iter:
              Ident 261..263 name=xs
            body:
              Block 264..266
          BreakStmt 266..271
          ContinueStmt 271..279
          ReturnStmt 279..287
            IntLit 286..287 value=1
          EmptyStmt 287..288
"#,
        );
    }

    // ------------------------------------------------------------------
    // 17. annotation class：annotation 修饰符 + class + 主构造 val 参数
    //     （+ interface / struct / object / companion object / effect op /
    //     扩展属性 / typealias / 扩展函数的其他声明形态简测）
    // ------------------------------------------------------------------
    #[test]
    fn misc_decl_forms() {
        let b = B::new();
        let mut items = Vec::new();
        let mut pos = 0usize;

        // annotation class Marker(val name: String)
        {
            let (s, e) = advance(&mut pos, 41);
            let param = CtorParam {
                id: b.nid(),
                span: sp(s + 24, s + 40),
                annotations: vec![],
                property: Some(ValKind::Val),
                is_vararg: false,
                name: b.ident("name", s + 28, s + 32),
                ty: Some(b.named_ty(s + 34, "String")),
                default: None,
            };
            items.push(b.item(
                s,
                e,
                ItemKind::Type(TypeDecl {
                    annotations: vec![],
                    modifiers: vec![ast::Modifier {
                        kind: ast::ModifierKind::Annotation,
                        span: sp(s, s + 10),
                    }],
                    kind: TypeKind::Class,
                    name: b.ident("Marker", s + 17, s + 23),
                    type_params: None,
                    primary_ctor: Some(PrimaryCtorDecl {
                        id: b.nid(),
                        span: sp(s + 23, s + 41),
                        params: vec![param],
                    }),
                    supertypes: vec![],
                    where_clause: None,
                    body: None,
                }),
            ));
        }
        // interface Iface { fun op(x: Int): Unit }（无 body 成员）
        {
            let (s, e) = advance(&mut pos, 44);
            let op = b.member(
                s + 19,
                s + 42,
                TypeMemberKind::Fun(FunDecl {
                    name: b.ident("op", s + 23, s + 25),
                    params: vec![Param {
                        id: b.nid(),
                        span: sp(s + 26, s + 32),
                        annotations: vec![],
                        is_vararg: false,
                        name: b.ident("x", s + 26, s + 27),
                        ty: Some(b.named_ty(s + 29, "Int")),
                        default: None,
                    }],
                    return_ty: Some(b.unit_ty(s + 35, s + 39)),
                    body: None,
                    ..b.nofun()
                }),
            );
            items.push(b.item(
                s,
                e,
                ItemKind::Type(TypeDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    kind: TypeKind::Interface,
                    name: b.ident("Iface", s + 10, s + 15),
                    type_params: None,
                    primary_ctor: None,
                    supertypes: vec![],
                    where_clause: None,
                    body: Some(TypeBody {
                        id: b.nid(),
                        span: sp(s + 16, e),
                        members: vec![op],
                    }),
                }),
            ));
        }
        // effect Console { fun print(msg: String): Unit }（effect 操作）
        {
            let (s, e) = advance(&mut pos, 50);
            let op = b.member(
                s + 17,
                s + 48,
                TypeMemberKind::Fun(FunDecl {
                    name: b.ident("print", s + 21, s + 26),
                    params: vec![Param {
                        id: b.nid(),
                        span: sp(s + 27, s + 39),
                        annotations: vec![],
                        is_vararg: false,
                        name: b.ident("msg", s + 27, s + 30),
                        ty: Some(b.named_ty(s + 32, "String")),
                        default: None,
                    }],
                    return_ty: Some(b.unit_ty(s + 42, s + 46)),
                    body: None,
                    ..b.nofun()
                }),
            );
            items.push(b.item(
                s,
                e,
                ItemKind::Type(TypeDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    kind: TypeKind::Effect,
                    name: b.ident("Console", s + 7, s + 14),
                    type_params: None,
                    primary_ctor: None,
                    supertypes: vec![],
                    where_clause: None,
                    body: Some(TypeBody {
                        id: b.nid(),
                        span: sp(s + 15, e),
                        members: vec![op],
                    }),
                }),
            ));
        }
        // object Helpers { companion object }（嵌套 companion）→ 放在 class 里
        {
            let (s, e) = advance(&mut pos, 44);
            let companion = b.member(
                s + 16,
                s + 42,
                TypeMemberKind::Object(ObjectDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    name: None,
                    companion: true,
                    supertypes: vec![],
                    body: None,
                }),
            );
            items.push(b.item(
                s,
                e,
                ItemKind::Type(TypeDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    kind: TypeKind::Class,
                    name: b.ident("WithComp", s + 6, s + 14),
                    type_params: None,
                    primary_ctor: None,
                    supertypes: vec![],
                    where_clause: None,
                    body: Some(TypeBody {
                        id: b.nid(),
                        span: sp(s + 15, e),
                        members: vec![companion],
                    }),
                }),
            ));
        }
        // object Registry : Iface
        {
            let (s, e) = advance(&mut pos, 24);
            items.push(b.item(
                s,
                e,
                ItemKind::Object(ObjectDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    name: Some(b.ident("Registry", s + 7, s + 15)),
                    companion: false,
                    supertypes: vec![SuperType {
                        id: b.nid(),
                        span: sp(s + 18, s + 23),
                        ty: b.named_ty(s + 18, "Iface"),
                        args: vec![],
                    }],
                    body: None,
                }),
            ));
        }
        // typealias IntList = List<Int>
        {
            let (s, e) = advance(&mut pos, 29);
            let list_ty = TypeRef {
                id: b.nid(),
                span: sp(s + 20, s + 29),
                kind: TypeRefKind::Path {
                    path: b.path(s + 20, &["List"]),
                    args: vec![b.tyarg(b.named_ty(s + 25, "Int"))],
                },
            };
            items.push(b.item(
                s,
                e,
                ItemKind::TypeAlias(TypeAliasDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    name: b.ident("IntList", s + 10, s + 17),
                    type_params: None,
                    ty: list_ty,
                }),
            ));
        }
        // val List<Int>.size2: Int get() = 0（扩展属性）
        {
            let (s, e) = advance(&mut pos, 36);
            let receiver = TypeRef {
                id: b.nid(),
                span: sp(s + 4, s + 13),
                kind: TypeRefKind::Path {
                    path: b.path(s + 4, &["List"]),
                    args: vec![b.tyarg(b.named_ty(s + 9, "Int"))],
                },
            };
            let getter = AccessorDecl {
                id: b.nid(),
                span: sp(s + 31, s + 40),
                kind: AccessorKind::Get,
                param: None,
                param_ty: None,
                body: AccessorBody::Expr(Box::new(b.int(s + 39, s + 40, 0))),
            };
            items.push(b.item(
                s,
                e + 4,
                ItemKind::ExtensionProperty(ExtensionPropertyDecl {
                    annotations: vec![],
                    modifiers: vec![],
                    kind: ValKind::Val,
                    type_params: None,
                    receiver,
                    name: b.ident("size2", s + 14, s + 19),
                    ty: b.named_ty(s + 21, "Int"),
                    init: None,
                    accessors: vec![getter],
                }),
            ));
            pos += 4;
        }
        // fun Int.twice(): Int = this * 2（扩展函数 + operator 修饰符 + vararg 参数演示）
        {
            let (s, e) = advance(&mut pos, 45);
            let lhs = b.ident_expr(s + 28, s + 32, "this");
            let rhs = b.int(s + 35, s + 36, 2);
            let body = b.expr(
                s + 28,
                s + 36,
                ExprKind::Binary {
                    lhs: Box::new(lhs),
                    op: BinaryOp::Mul,
                    rhs: Box::new(rhs),
                },
            );
            items.push(b.fun_item(
                s,
                e,
                FunDecl {
                    modifiers: vec![ast::Modifier {
                        kind: ast::ModifierKind::Operator,
                        span: sp(s, s + 8),
                    }],
                    receiver: Some(b.named_ty(s + 13, "Int")),
                    name: b.ident("twice", s + 17, s + 22),
                    return_ty: Some(b.named_ty(s + 25, "Int")),
                    body: Some(FunBody::Expr(Box::new(body))),
                    ..b.nofun()
                },
            ));
        }
        // fun va(vararg xs: Int) {}（vararg 参数 + 默认值参数）
        {
            let (s, e) = advance(&mut pos, 40);
            let params = vec![
                Param {
                    id: b.nid(),
                    span: sp(s + 8, s + 21),
                    annotations: vec![],
                    is_vararg: true,
                    name: b.ident("xs", s + 15, s + 17),
                    ty: Some(b.named_ty(s + 19, "Int")),
                    default: None,
                },
                Param {
                    id: b.nid(),
                    span: sp(s + 23, s + 32),
                    annotations: vec![],
                    is_vararg: false,
                    name: b.ident("n", s + 23, s + 24),
                    ty: Some(b.named_ty(s + 26, "Int")),
                    default: Some(b.int(s + 31, s + 32, 0)),
                },
            ];
            items.push(b.fun_item(
                s,
                e,
                FunDecl {
                    name: b.ident("va", s + 4, s + 6),
                    params,
                    body: Some(FunBody::Block(b.block(s + 34, s + 36, vec![]))),
                    ..b.nofun()
                },
            ));
        }

        let file = b.file(pos, items);
        let dump = b.t.with_interner(|i| dump_file(&file, i));
        check(
            dump,
            r#"File 0..357
  items:
    ClassDecl 0..41 name=Marker mods=[annotation]
      primary_ctor:
        PrimaryCtor 23..41
          CtorParam 24..40 name=name property=val
            ty:
              PathType 34..40 path=String
    InterfaceDecl 41..85 name=Iface
      body:
        TypeBody 57..85
          FunDecl 60..83 name=op
            params:
              Param 67..73 name=x
                ty:
                  PathType 70..73 path=Int
            return_ty:
              UnitType 76..80
    EffectDecl 85..135 name=Console
      body:
        TypeBody 100..135
          FunDecl 102..133 name=print
            params:
              Param 112..124 name=msg
                ty:
                  PathType 117..123 path=String
            return_ty:
              UnitType 127..131
    ClassDecl 135..179 name=WithComp
      body:
        TypeBody 150..179
          CompanionObject 151..177
    ObjectDecl 179..203 name=Registry
      supertypes:
        SuperType 197..202
          ty:
            PathType 197..202 path=Iface
    TypeAliasDecl 203..232 name=IntList
      ty:
        PathType 223..232 path=List
          args:
            TypeArg 228..231
              PathType 228..231 path=Int
    ExtensionPropertyDecl 232..272 kind=val name=size2
      receiver:
        PathType 236..245 path=List
          args:
            TypeArg 241..244
              PathType 241..244 path=Int
      ty:
        PathType 253..256 path=Int
      accessors:
        Getter 263..272
          body:
            IntLit 271..272 value=0
    FunDecl 272..317 name=twice mods=[operator]
      receiver:
        PathType 285..288 path=Int
      return_ty:
        PathType 297..300 path=Int
      body:
        Binary 300..308 op=*
          lhs:
            Ident 300..304 name=this
          rhs:
            IntLit 307..308 value=2
    FunDecl 317..357 name=va
      params:
        Param 325..338 name=xs vararg
          ty:
            PathType 336..339 path=Int
        Param 340..349 name=n
          ty:
            PathType 343..346 path=Int
          default:
            IntLit 348..349 value=0
      body:
        Block 351..353
"#,
        );
    }

    // ------------------------------------------------------------------
    // 18. 确定性：同一 AST 渲染两次必须完全相同
    // ------------------------------------------------------------------
    #[test]
    fn dump_is_deterministic() {
        let b = B::new();
        let body = b.block(10, 12, vec![]);
        let fun = b.fun_item(
            0,
            12,
            FunDecl {
                name: b.ident("f", 4, 5),
                body: Some(FunBody::Block(body)),
                ..b.nofun()
            },
        );
        let file = b.file(12, vec![fun]);
        let first = b.t.with_interner(|i| dump_file(&file, i));
        let second = b.t.with_interner(|i| dump_file(&file, i));
        assert_eq!(first, second);
    }

    // ------------------------------------------------------------------
    // 19. span 纪律：组合节点的 span 覆盖其全部子节点
    // ------------------------------------------------------------------
    #[test]
    fn composite_spans_cover_children() {
        let b = B::new();
        let subject = b.ident_expr(6, 7, "x");
        let arm = WhenArm {
            id: b.nid(),
            span: sp(10, 20),
            pat: b.pattern(10, 11, PatternKind::Wildcard),
            guard: None,
            body: b.int(15, 16, 1),
        };
        let when = b.expr(
            0,
            22,
            ExprKind::When {
                subject: Box::new(subject),
                arms: vec![arm],
            },
        );
        let ExprKind::When { subject, arms } = &when.kind else {
            panic!("expected when")
        };
        assert!(when.span.start <= subject.span.start && when.span.end >= subject.span.end);
        let arm = &arms[0];
        assert!(when.span.start <= arm.span.start && when.span.end >= arm.span.end);
        assert!(arm.span.start <= arm.pat.span.start && arm.span.end >= arm.pat.span.end);
        assert!(arm.span.start <= arm.body.span.start && arm.span.end >= arm.body.span.end);

        // 嵌套可空：外层 span 覆盖内层
        let inner = TypeRef {
            id: b.nid(),
            span: sp(0, 1),
            kind: TypeRefKind::Path {
                path: b.path(0, &["T"]),
                args: vec![],
            },
        };
        let mid = TypeRef {
            id: b.nid(),
            span: sp(0, 2),
            kind: TypeRefKind::Nullable(Box::new(inner)),
        };
        let outer = TypeRef {
            id: b.nid(),
            span: sp(0, 3),
            kind: TypeRefKind::Nullable(Box::new(mid)),
        };
        let TypeRefKind::Nullable(mid) = &outer.kind else {
            panic!("expected nullable")
        };
        let TypeRefKind::Nullable(inner) = &mid.kind else {
            panic!("expected nullable")
        };
        assert!(outer.span.start <= mid.span.start && outer.span.end >= mid.span.end);
        assert!(mid.span.start <= inner.span.start && mid.span.end >= inner.span.end);
    }
}
