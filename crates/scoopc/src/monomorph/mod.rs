//! 单态化（monomorphization）相关的数据结构与兼容包装。
//!
//! 当前边界：
//! - `MonomorphKey` 保留“typecheck 收集到的实例请求”语义；
//! - 真正的 backend-agnostic `InstanceKey` 与 generic MIR template → monomorphic instance
//!   materialization 已迁到 `crate::mir::materialize`；
//! - 本模块继续提供旧 `dump-ir` / 测试入口的兼容导出，避免一次性打断调用面。

mod lower;

use std::fmt;
use std::path::PathBuf;

use crate::span::Span;
use crate::ty::{EffectRow, TypeId};

pub use lower::{LoweredMonomorphMir, MonomorphLowerError, lower_for_dump};

/// 单态化目标（函数/类型等）的稳定引用。
///
/// 当前阶段只先支持函数（TODO T0704 的目标要求）。
///
/// 说明：
/// - 对于存在 overload 的情形，`fqn` 本身不足以区分候选，因此这里把 `(decl_file, decl_span)`
///   作为“最小唯一性”来源；
/// - 未来引入全局符号表时，可把该结构替换为更紧凑的 `SymbolId/DefId`。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonomorphSymbol {
    pub fqn: String,
    pub decl_file: PathBuf,
    pub decl_span: Span,
}

impl fmt::Debug for MonomorphSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}:{:?}",
            self.fqn,
            self.decl_file.display(),
            self.decl_span
        )
    }
}

/// 单态化请求键：`Symbol + type args + effect row args`。
///
/// 说明：
/// - 这里承载的是“调用点请求了哪个实例”的前端事实，而不是后续中端/后端共享的最终实例身份；
/// - `type_args`：实例请求携带的类型维度（`TypeId`）；
///   - 对 standalone generic fun：对应函数自身 type params；
///   - 对 generic owner member/getter：会先放 owner-specialization 的 concrete args，再接函数自身 type args；
///   - 其最终语义与布局在后续 MIR materialization 决定；
/// - `eff_args`：effect row 参数的实例（`EffectRow`），用于区分 `fun <eff E>` 下的不同调用形态。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonomorphKey {
    pub symbol: MonomorphSymbol,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
}

impl fmt::Debug for MonomorphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonomorphKey")
            .field("symbol", &self.symbol)
            .field("type_args", &TypeIdList(&self.type_args))
            .field("eff_args", &EffectRowList(&self.eff_args))
            .finish()
    }
}

struct TypeIdList<'a>(&'a [TypeId]);

impl fmt::Debug for TypeIdList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

struct TypeIdRepr(TypeId);

impl fmt::Debug for TypeIdRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.as_u32())
    }
}

struct EffectRowList<'a>(&'a [EffectRow]);

impl fmt::Debug for EffectRowList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(EffectRowRepr))
            .finish()
    }
}

struct EffectRowRepr<'a>(&'a EffectRow);

impl fmt::Debug for EffectRowRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_pure() {
            return write!(f, "Pure");
        }
        f.debug_list()
            .entries(self.0.terms.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use crate::span::Span;
    use crate::ty::{EffectRow, TypeStore};

    use super::{MonomorphKey, MonomorphSymbol};

    #[test]
    fn monomorph_key_dedup_same_key() {
        let mut store = TypeStore::new();
        let builtins = store.intern_builtins();

        let symbol = MonomorphSymbol {
            fqn: "a.f".to_string(),
            decl_file: PathBuf::from("a.scoop"),
            decl_span: Span::new(10, 20),
        };

        let key1 = MonomorphKey {
            symbol: symbol.clone(),
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::pure()],
        };
        let key2 = MonomorphKey {
            symbol,
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::pure()],
        };

        let mut set = HashSet::new();
        assert!(set.insert(key1));
        assert!(!set.insert(key2));
    }

    #[test]
    fn monomorph_key_diff_type_args_make_key_different() {
        let mut store = TypeStore::new();
        let builtins = store.intern_builtins();

        let symbol = MonomorphSymbol {
            fqn: "a.f".to_string(),
            decl_file: PathBuf::from("a.scoop"),
            decl_span: Span::new(10, 20),
        };

        let key1 = MonomorphKey {
            symbol: symbol.clone(),
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::pure()],
        };
        let key2 = MonomorphKey {
            symbol,
            type_args: vec![builtins.uint],
            eff_args: vec![EffectRow::pure()],
        };

        assert_ne!(key1, key2);

        let mut set = HashSet::new();
        set.insert(key1);
        set.insert(key2);
        assert_eq!(set.len(), 2);
    }
}
