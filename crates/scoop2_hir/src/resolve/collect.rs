//! Header 收集：resolve 第一阶段。
//!
//! 把一个文件的**顶层声明**按 FQN 登记进 [`Index`] 的三命名空间，并检测：
//!
//! - 类型 / 值命名空间的**重复定义**（`scoop::resolve::duplicate_definition`）；
//! - **非法可见性组合**（多个 public/internal/private，`scoop::resolve::invalid_visibility`）。
//!
//! 当前覆盖的非扩展顶层 item：`typealias` / `fun`（无接收者）/ `val`(Name 绑定) /
//! `object` / `class`·`interface`·`struct`·`enum`·`effect`。
//!
//! 以下**不在本阶段**（后续增量补齐，数据不丢失）：
//! - 扩展函数（`fun T.f`）/ 扩展属性：接收者 FQN 需 type resolution，暂存为
//!   [`PendingExtension`](super::index::PendingExtension)；
//! - 类型体成员（嵌套 FQN）、import、作用域、`val` 解构绑定、函数体名字解析。
//!
//! 函数命名空间是重载集：同名函数直接追加，不在此判重（签名判重由 typecheck 负责）。

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner, Symbol as Sym};

use crate::syntax::ast::{
    self, File, Ident, Item, ItemKind, Modifier, TypeMember, TypeMemberKind, ValBinding,
};

use super::errors;
use super::index::{Index, PendingExtension};
use super::symbol::{ConeId, DeclSymbol, ModifierSet, SymbolKind, Visibility};

/// 收集一个文件的顶层声明到 `index`。
///
/// `cone` 由调用方（db/session）按文件所属 cone 解析后传入。
pub fn collect_file(
    file: &File,
    file_id: FileId,
    cone: ConeId,
    index: &mut Index,
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
) {
    index.set_file_cone(file_id, cone);
    let package_prefix = package_prefix_of(file, interner);
    let mut c = Collector {
        interner,
        diags,
        index,
        file: file_id,
        cone,
        package_prefix,
    };
    for item in &file.items {
        c.collect_item(item);
    }
}

/// 由 `package a.b.c` 得到点分前缀 `"a.b.c"`；无 package 则空串。
pub(crate) fn package_prefix_of(file: &File, interner: &Interner) -> String {
    match &file.package {
        Some(pkg) => pkg
            .path
            .segments
            .iter()
            .map(|seg| interner.resolve(seg.symbol))
            .collect::<Vec<_>>()
            .join("."),
        None => String::new(),
    }
}

struct Collector<'a> {
    interner: &'a mut Interner,
    diags: &'a mut DiagnosticSink,
    index: &'a mut Index,
    file: FileId,
    cone: ConeId,
    package_prefix: String,
}

impl<'a> Collector<'a> {
    /// 由简单名构造 FQN（`package_prefix.name`，无前缀则 `name`）。
    fn fqn_of(&mut self, simple: Sym) -> Sym {
        let simple_text = self.interner.resolve(simple).to_string();
        let fqn_text = if self.package_prefix.is_empty() {
            simple_text
        } else {
            format!("{}.{}", self.package_prefix, simple_text)
        };
        self.interner.intern(&fqn_text)
    }

    fn make_symbol(&mut self, kind: SymbolKind, simple: Ident, mods: &[Modifier]) -> DeclSymbol {
        let visibility = Visibility::from_modifiers(mods);
        let modifiers = ModifierSet::from_modifiers(mods);
        DeclSymbol {
            kind,
            fqn: self.fqn_of(simple.symbol),
            simple_name: simple.symbol,
            span: simple.span,
            file: self.file,
            cone: self.cone,
            visibility,
            modifiers,
        }
    }

    /// 校验可见性组合；非法时记录诊断。
    fn check_visibility(&mut self, mods: &[Modifier], span: scoop2_base::Span) {
        if Visibility::count_modifiers(mods) > 1 {
            self.diags.push(errors::invalid_visibility(span));
        }
    }

    /// 由 owner FQN + 简单名构造成员 FQN（`owner.name`）。
    fn fqn_under(&mut self, owner: Sym, simple: Sym) -> Sym {
        let owner_text = self.interner.resolve(owner);
        let simple_text = self.interner.resolve(simple);
        self.intern_str(&format!("{owner_text}.{simple_text}"))
    }

    fn intern_str(&mut self, text: &str) -> Sym {
        self.interner.intern(text)
    }

    /// 构造一个成员符号（FQN 由调用方给出，不用 package 前缀）。
    fn make_member_symbol(
        &mut self,
        kind: SymbolKind,
        simple: Ident,
        mods: &[Modifier],
        fqn: Sym,
    ) -> DeclSymbol {
        let visibility = Visibility::from_modifiers(mods);
        let modifiers = ModifierSet::from_modifiers(mods);
        DeclSymbol {
            kind,
            fqn,
            simple_name: simple.symbol,
            span: simple.span,
            file: self.file,
            cone: self.cone,
            visibility,
            modifiers,
        }
    }

    /// 收集类型体成员到 `owner_fqn` 之下（成员函数→fun；属性/variant/ctor-param→value；
    /// 嵌套类型→type）。companion object 的成员挂到 owner（class）名下。
    fn collect_type_body_members(&mut self, owner_fqn: Sym, members: &[TypeMember]) {
        for m in members {
            match &m.kind {
                TypeMemberKind::Fun(d) => {
                    let fqn = self.fqn_under(owner_fqn, d.name.symbol);
                    let sym = self.make_member_symbol(SymbolKind::Fun, d.name, &d.modifiers, fqn);
                    self.index.insert_fun(sym);
                }
                TypeMemberKind::Property(d) => {
                    let fqn = self.fqn_under(owner_fqn, d.name.symbol);
                    let sym = self.make_member_symbol(SymbolKind::Value, d.name, &d.modifiers, fqn);
                    self.insert_value(sym, d.name);
                }
                TypeMemberKind::EnumVariant(d) => {
                    let fqn = self.fqn_under(owner_fqn, d.name.symbol);
                    let sym = self.make_member_symbol(SymbolKind::Value, d.name, &[], fqn);
                    self.insert_value(sym, d.name);
                }
                TypeMemberKind::Object(d) => {
                    if d.companion {
                        // companion 成员挂到 owner（class）名下。
                        if let Some(body) = &d.body {
                            self.collect_type_body_members(owner_fqn, &body.members);
                        }
                    } else if let Some(name) = d.name {
                        let fqn = self.fqn_under(owner_fqn, name.symbol);
                        let sym =
                            self.make_member_symbol(SymbolKind::Object, name, &d.modifiers, fqn);
                        self.insert_type(sym, name);
                        if let Some(body) = &d.body {
                            self.collect_type_body_members(fqn, &body.members);
                        }
                    }
                }
                TypeMemberKind::Type(d) => {
                    let fqn = self.fqn_under(owner_fqn, d.name.symbol);
                    let sym = self.make_member_symbol(SymbolKind::Type, d.name, &d.modifiers, fqn);
                    self.insert_type(sym, d.name);
                    if let Some(body) = &d.body {
                        self.collect_type_body_members(fqn, &body.members);
                    }
                }
                TypeMemberKind::InitBlock(_) | TypeMemberKind::SecondaryCtor(_) => {}
            }
        }
    }

    fn collect_item(&mut self, item: &Item) {
        match &item.kind {
            ItemKind::TypeAlias(d) => {
                self.check_visibility(&d.modifiers, item.span);
                let sym = self.make_symbol(SymbolKind::TypeAlias, d.name, &d.modifiers);
                self.insert_type(sym, d.name);
            }
            ItemKind::Fun(d) => match &d.receiver {
                None => {
                    self.check_visibility(&d.modifiers, item.span);
                    let sym = self.make_symbol(SymbolKind::Fun, d.name, &d.modifiers);
                    self.index.insert_fun(sym);
                }
                Some(receiver) => {
                    // 扩展函数：接收者 FQN 需 type resolution，暂存。
                    self.add_pending_extension(
                        receiver.clone(),
                        d.name,
                        &d.modifiers,
                        SymbolKind::ExtensionFun,
                    );
                }
            },
            ItemKind::Val(d) => match &d.binding {
                ValBinding::Name(name) => {
                    self.check_visibility(&d.modifiers, item.span);
                    let sym = self.make_symbol(SymbolKind::Value, *name, &d.modifiers);
                    self.insert_value(sym, *name);
                }
                ValBinding::Pattern(_) => {
                    // 解构绑定的多个顶层绑定由 pattern 解析阶段处理。
                }
            },
            ItemKind::ExtensionProperty(d) => {
                self.add_pending_extension(
                    d.receiver.clone(),
                    d.name,
                    &d.modifiers,
                    SymbolKind::ExtensionProperty,
                );
            }
            ItemKind::Object(d) => {
                if let Some(name) = d.name {
                    self.check_visibility(&d.modifiers, item.span);
                    let sym = self.make_symbol(SymbolKind::Object, name, &d.modifiers);
                    let owner_fqn = sym.fqn;
                    self.insert_type(sym, name);
                    if let Some(body) = &d.body {
                        self.collect_type_body_members(owner_fqn, &body.members);
                    }
                }
                // 顶层无名 object（非法，应由 parser 拒绝）此处忽略。
            }
            ItemKind::Type(d) => {
                self.check_visibility(&d.modifiers, item.span);
                let sym = self.make_symbol(SymbolKind::Type, d.name, &d.modifiers);
                let owner_fqn = sym.fqn;
                self.insert_type(sym, d.name);
                // 主构造 param-property（`class C(val x: T)`）：x 是 C 的属性成员。
                if let Some(ctor) = &d.primary_ctor {
                    for cp in &ctor.params {
                        if cp.property.is_some() {
                            let fqn = self.fqn_under(owner_fqn, cp.name.symbol);
                            let sym = self.make_member_symbol(SymbolKind::Value, cp.name, &[], fqn);
                            self.insert_value(sym, cp.name);
                        }
                    }
                }
                if let Some(body) = &d.body {
                    self.collect_type_body_members(owner_fqn, &body.members);
                }
            }
        }
    }

    fn insert_type(&mut self, sym: DeclSymbol, _name_ident: Ident) {
        let fqn_text = self.interner.resolve(sym.fqn).to_string();
        let span = sym.span;
        match self.index.insert_type(sym) {
            Ok(()) => {}
            Err(first_span) => {
                self.diags
                    .push(errors::duplicate_definition(&fqn_text, first_span, span));
            }
        }
    }

    fn insert_value(&mut self, sym: DeclSymbol, _name_ident: Ident) {
        let fqn_text = self.interner.resolve(sym.fqn).to_string();
        let span = sym.span;
        match self.index.insert_value(sym) {
            Ok(()) => {}
            Err(first_span) => {
                self.diags
                    .push(errors::duplicate_definition(&fqn_text, first_span, span));
            }
        }
    }

    fn add_pending_extension(
        &mut self,
        receiver: ast::TypeRef,
        name: Ident,
        mods: &[Modifier],
        kind: SymbolKind,
    ) {
        self.check_visibility(mods, name.span);
        let visibility = Visibility::from_modifiers(mods);
        let modifiers = ModifierSet::from_modifiers(mods);
        self.index.add_pending_extension(PendingExtension {
            receiver,
            name: name.symbol,
            span: name.span,
            file: self.file,
            cone: self.cone,
            visibility,
            modifiers,
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::symbol::ConeKind;
    use scoop2_base::SourceFile;
    use scoop2_syntax::parser::parse_file;

    /// 解析 + 收集；返回 (index, 收集所用的 interner, 诊断码列表)。
    /// 查询 index 必须用返回的 interner（Symbol 是 interner 局部的）。
    fn resolve(text: &str) -> (Index, scoop2_base::Interner, Vec<String>) {
        let src = SourceFile::new_virtual("<mem>", text);
        let result = parse_file(&src);
        let mut interner = result.interner;
        let mut index = Index::new();
        let mut diags = DiagnosticSink::new();
        let cone = index.intern_cone("test", ConeKind::Bin);
        collect_file(
            &result.file,
            FileId(0),
            cone,
            &mut index,
            &mut interner,
            &mut diags,
        );
        let codes = diags.iter().map(|d| d.code.to_string()).collect();
        (index, interner, codes)
    }

    #[test]
    fn collects_top_level_decls_no_duplicates() {
        let (index, mut it, codes) = resolve("package app\nfun f() {}\nclass C\nval x: Int = 0\n");
        assert!(codes.is_empty(), "no diagnostics: {codes:?}");
        assert!(index.lookup_type(it.intern("app.C")).is_some());
        assert!(index.lookup_value(it.intern("app.x")).is_some());
        assert_eq!(index.lookup_funs(it.intern("app.f")).len(), 1);
    }

    #[test]
    fn detects_duplicate_type() {
        let (_, _, codes) = resolve("class C\nclass C\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::duplicate_definition"),
            "{codes:?}"
        );
    }

    #[test]
    fn detects_duplicate_value() {
        let (_, _, codes) = resolve("val x: Int = 0\nval x: Int = 1\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::duplicate_definition"),
            "{codes:?}"
        );
    }

    #[test]
    fn funs_with_same_name_form_overload_set() {
        let (index, mut it, codes) = resolve("fun f() {}\nfun f() {}\n");
        assert!(codes.is_empty(), "overloads are not duplicates: {codes:?}");
        assert_eq!(index.lookup_funs(it.intern("f")).len(), 2);
    }

    #[test]
    fn detects_invalid_visibility_combo() {
        let (_, _, codes) = resolve("public private class C\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::invalid_visibility"),
            "{codes:?}"
        );
    }
}
