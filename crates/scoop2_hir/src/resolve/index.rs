//! [`Index`]：跨 cone 的全局符号表（三命名空间 / FQN）。
//!
//! header 收集阶段把所有顶层声明按 FQN 登记进来；body 阶段与 typecheck 只读
//! 查询。函数命名空间是重载集（不判重）；类型/值命名空间同 FQN 至多一个，
//! 重复即 `duplicate_definition`（诊断由 collect 发出，[`Index::insert_*`]
//! 返回冲突项的首定义 span）。

use hashbrown::HashMap;

use scoop2_base::{FileId, Interner, Span, Symbol};

use crate::syntax::ast;

use super::symbol::{
    ConeId, ConeInfo, ConeKind, DeclSymbol, ModifierSet, NamespacedSymbols, SymbolKind, Visibility,
};

/// 一个待解析接收者的扩展声明（header 收集阶段登记，成员解析阶段消费）。
///
/// 接收者 `TypeRef` 的 FQN 需要 type resolution，故在此延后；本结构保留全部
/// 登记所需信息，不丢失数据。
#[derive(Clone, Debug)]
pub struct PendingExtension {
    pub receiver: ast::TypeRef,
    pub name: Symbol,
    pub span: Span,
    pub file: FileId,
    pub cone: ConeId,
    pub visibility: Visibility,
    pub modifiers: ModifierSet,
    pub kind: SymbolKind,
    /// 声明所在文件的 package 前缀（用于解析接收者 TypeRef）。
    pub package_prefix: String,
}

/// 全局符号表。
#[derive(Debug, Default)]
pub struct Index {
    /// FQN → 三命名空间内容。
    by_fqn: HashMap<Symbol, NamespacedSymbols>,
    /// cone 列表（下标即 [`ConeId`]）。
    cones: Vec<ConeInfo>,
    /// cone 名 → ConeId（用于去重/查找）。
    cone_by_name: HashMap<String, ConeId>,
    /// 文件 → 所属 cone。
    file_cone: HashMap<FileId, ConeId>,
    /// 扩展函数/属性（按接收者 FQN 分桶；签名/成员解析在后续阶段）。
    extensions: HashMap<Symbol, Vec<DeclSymbol>>,
    /// 待解析接收者的扩展声明（header 收集产出，成员解析消费）。
    pending_extensions: Vec<PendingExtension>,
}

/// 插入类型/值命名空间的结果：`Ok(())` 或 `Err(first)`（首定义的 span）。
pub type InsertResult = Result<(), Span>;

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    // ----- cone 注册 -----

    /// 注册或复用一个 cone（按名字幂等），返回其 [`ConeId`]。
    pub fn intern_cone(&mut self, name: &str, kind: ConeKind) -> ConeId {
        if let Some(&id) = self.cone_by_name.get(name) {
            return id;
        }
        let id = ConeId(self.cones.len() as u32);
        self.cones.push(ConeInfo {
            id,
            name: name.to_string(),
            kind,
        });
        self.cone_by_name.insert(name.to_string(), id);
        id
    }

    pub fn cone(&self, id: ConeId) -> &ConeInfo {
        // invariant: ConeId 由 intern_cone 在本 Index 产出。
        &self.cones[id.0 as usize]
    }

    /// 记录文件所属 cone。
    pub fn set_file_cone(&mut self, file: FileId, cone: ConeId) {
        self.file_cone.insert(file, cone);
    }

    pub fn file_cone(&self, file: FileId) -> Option<ConeId> {
        self.file_cone.get(&file).copied()
    }

    // ----- 命名空间插入（带重复检测） -----

    fn entry_mut(&mut self, fqn: Symbol) -> &mut NamespacedSymbols {
        self.by_fqn.entry(fqn).or_default()
    }

    /// 插入类型命名空间符号；若 FQN 已有类型符号，返回首定义 span。
    pub fn insert_type(&mut self, sym: DeclSymbol) -> InsertResult {
        let slot = self.entry_mut(sym.fqn);
        if let Some(first) = &slot.ty {
            return Err(first.span);
        }
        debug_assert!(sym.kind.is_type_namespace());
        slot.ty = Some(sym);
        Ok(())
    }

    /// 插入值命名空间符号；若 FQN 已有值符号，返回首定义 span。
    pub fn insert_value(&mut self, sym: DeclSymbol) -> InsertResult {
        let slot = self.entry_mut(sym.fqn);
        if let Some(first) = &slot.value {
            return Err(first.span);
        }
        debug_assert_eq!(sym.kind, SymbolKind::Value);
        slot.value = Some(sym);
        Ok(())
    }

    /// 追加一个函数符号到重载集（resolve 阶段不判重；签名判重由 typecheck 负责）。
    pub fn insert_fun(&mut self, sym: DeclSymbol) {
        debug_assert_eq!(sym.kind, SymbolKind::Fun);
        let slot = self.entry_mut(sym.fqn);
        slot.funs.push(sym);
    }

    /// 登记一个扩展函数/属性（按接收者 FQN 分桶）。
    pub fn insert_extension(&mut self, receiver_fqn: Symbol, sym: DeclSymbol) {
        self.extensions.entry(receiver_fqn).or_default().push(sym);
    }

    /// 暂存一个待解析接收者的扩展声明（成员解析阶段消费）。
    pub fn add_pending_extension(&mut self, ext: PendingExtension) {
        self.pending_extensions.push(ext);
    }

    /// 取所有待解析的扩展声明（成员解析阶段调用）。
    pub fn pending_extensions(&self) -> &[PendingExtension] {
        &self.pending_extensions
    }

    /// 解析所有待处理扩展：把接收者 TypeRef 解析为 FQN，按 `<receiver>.<name>` 登记
    /// （扩展函数→fun 命名空间，扩展属性→value 命名空间），使成员访问 `r.ext()` 命中。
    /// 接收者无法解析（未知类型）的扩展被丢弃（类型错误留给 typecheck）。
    pub fn resolve_extensions(&mut self, interner: &mut Interner) {
        let pending = std::mem::take(&mut self.pending_extensions);
        for ext in pending {
            // 登记到 `<receiver>.<name>` 时，扩展函数/属性成为接收者的成员 fun/value。
            let (member_kind, is_fun) = match ext.kind {
                SymbolKind::ExtensionFun => (SymbolKind::Fun, true),
                SymbolKind::ExtensionProperty => (SymbolKind::Value, false),
                _ => continue,
            };
            let Some(receiver_fqn) =
                type_ref_fqn(&ext.receiver, &ext.package_prefix, self, interner)
            else {
                continue;
            };
            let receiver_text = interner.resolve(receiver_fqn);
            let name_text = interner.resolve(ext.name);
            let member_fqn = interner.intern(&format!("{receiver_text}.{name_text}"));
            let sym = DeclSymbol {
                kind: member_kind,
                fqn: member_fqn,
                simple_name: ext.name,
                span: ext.span,
                file: ext.file,
                cone: ext.cone,
                visibility: ext.visibility,
                modifiers: ext.modifiers,
            };
            if is_fun {
                self.insert_fun(sym);
            } else {
                let _ = self.insert_value(sym);
            }
        }
    }

    // ----- 查询 -----

    pub fn lookup(&self, fqn: Symbol) -> Option<&NamespacedSymbols> {
        self.by_fqn.get(&fqn)
    }

    pub fn lookup_type(&self, fqn: Symbol) -> Option<&DeclSymbol> {
        self.by_fqn.get(&fqn).and_then(|n| n.ty.as_ref())
    }

    pub fn lookup_value(&self, fqn: Symbol) -> Option<&DeclSymbol> {
        self.by_fqn.get(&fqn).and_then(|n| n.value.as_ref())
    }

    pub fn lookup_funs(&self, fqn: Symbol) -> &[DeclSymbol] {
        self.by_fqn
            .get(&fqn)
            .map(|n| n.funs.as_slice())
            .unwrap_or(&[])
    }

    pub fn extensions_for(&self, receiver_fqn: Symbol) -> &[DeclSymbol] {
        self.extensions
            .get(&receiver_fqn)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// 已登记的 FQN 数量（测试/统计用）。
    pub fn len(&self) -> usize {
        self.by_fqn.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_fqn.is_empty()
    }

    /// 迭代所有 (FQN, namespaces)（顺序不确定；需要确定性输出时由调用方排序）。
    pub fn iter(&self) -> impl Iterator<Item = (Symbol, &NamespacedSymbols)> {
        self.by_fqn.iter().map(|(&fqn, ns)| (fqn, ns))
    }
}

impl SymbolKind {
    /// 是否属于类型命名空间（class/.../object/typealias）。
    pub fn is_type_namespace(self) -> bool {
        matches!(
            self,
            SymbolKind::Type | SymbolKind::Object | SymbolKind::TypeAlias
        )
    }
}

/// 把一个 TypeRef 的 nominal 根解析为 FQN（按 package 前缀 + Index 类型命名空间）。
/// 单段 → `<prefix>.<name>`；多段 → 完整路径。非 Path（tuple/function/nullable）→ `None`。
fn type_ref_fqn(
    ty: &ast::TypeRef,
    package_prefix: &str,
    index: &Index,
    interner: &Interner,
) -> Option<Symbol> {
    let ast::TypeRefKind::Path { path, .. } = &ty.kind else {
        return None;
    };
    let fqn_text = if path.segments.len() == 1 {
        let n = interner.resolve(path.segments[0].symbol);
        if package_prefix.is_empty() {
            n.to_string()
        } else {
            format!("{package_prefix}.{n}")
        }
    } else {
        path.segments
            .iter()
            .map(|s| interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join(".")
    };
    let fqn = interner.get(&fqn_text)?;
    if index.lookup(fqn).and_then(|ns| ns.ty.as_ref()).is_some() {
        Some(fqn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_base::{Interner, Span};

    fn ty_sym(interner: &mut Interner, fqn: &str) -> DeclSymbol {
        DeclSymbol {
            kind: SymbolKind::Type,
            fqn: interner.intern(fqn),
            simple_name: interner.intern(fqn.rsplit('.').next().unwrap()),
            span: Span::new(0, 1),
            file: FileId(0),
            cone: ConeId(0),
            visibility: Visibility::Public,
            modifiers: ModifierSet::default(),
        }
    }

    #[test]
    fn insert_type_then_duplicate() {
        let mut idx = Index::new();
        let mut it = Interner::new();
        let a = ty_sym(&mut it, "a.C");
        assert!(idx.insert_type(a.clone()).is_ok());
        match idx.insert_type(ty_sym(&mut it, "a.C")) {
            Err(first_span) => assert_eq!(first_span, Span::new(0, 1)),
            other => panic!("expected duplicate, got {other:?}"),
        }
    }

    #[test]
    fn type_and_value_share_fqn_distinct_namespaces() {
        let mut idx = Index::new();
        let mut it = Interner::new();
        let mut t = ty_sym(&mut it, "a.X");
        t.kind = SymbolKind::Type;
        let mut v = ty_sym(&mut it, "a.X");
        v.kind = SymbolKind::Value;
        assert!(idx.insert_type(t).is_ok());
        assert!(idx.insert_value(v).is_ok(), "value namespace is distinct");
    }

    #[test]
    fn funs_form_overload_set_no_dedup() {
        let mut idx = Index::new();
        let mut it = Interner::new();
        let mut f1 = ty_sym(&mut it, "a.f");
        f1.kind = SymbolKind::Fun;
        let mut f2 = ty_sym(&mut it, "a.f");
        f2.kind = SymbolKind::Fun;
        idx.insert_fun(f1);
        idx.insert_fun(f2);
        let fqn = it.intern("a.f");
        assert_eq!(idx.lookup_funs(fqn).len(), 2, "overload set retains both");
    }

    #[test]
    fn intern_cone_is_idempotent() {
        let mut idx = Index::new();
        let c1 = idx.intern_cone("scoop.core", ConeKind::Syslib);
        let c2 = idx.intern_cone("scoop.core", ConeKind::Syslib);
        assert_eq!(c1, c2);
        let c3 = idx.intern_cone("app", ConeKind::Bin);
        assert_ne!(c1, c3);
        assert_eq!(idx.cone(c1).name, "scoop.core");
    }
}
