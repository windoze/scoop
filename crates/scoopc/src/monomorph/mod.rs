//! 单态化（monomorphization）相关的数据结构与兼容包装。
//!
//! 当前边界：
//! - `MonomorphKey` 保留“被请求实例的身份”语义；
//! - `MonomorphRequest` 额外记录 call-site source/span，供 materializer 按 request roots 过滤初始种子；
//! - 真正的 backend-agnostic `InstanceKey` 与 generic MIR template → monomorphic instance
//!   materialization 已迁到 `crate::mir::materialize`；
//! - 本模块继续提供旧 `dump-ir` / 测试入口的兼容导出，避免一次性打断调用面。

mod lower;

pub use lower::{LoweredMonomorphMir, MonomorphLowerError, lower_for_dump};
pub use scoopc_hir::monomorph::{MonomorphKey, MonomorphRequest, MonomorphSymbol};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use crate::span::Span;
    use crate::ty::{EffectRow, TypeStore};

    use super::{MonomorphKey, MonomorphRequest, MonomorphSymbol};

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

    #[test]
    fn monomorph_request_keeps_call_site_source_separate_from_key_identity() {
        let mut store = TypeStore::new();
        let builtins = store.intern_builtins();

        let key = MonomorphKey {
            symbol: MonomorphSymbol {
                fqn: "a.f".to_string(),
                decl_file: PathBuf::from("a.scoop"),
                decl_span: Span::new(10, 20),
            },
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::pure()],
        };

        let from_main = MonomorphRequest::new(
            key.clone(),
            PathBuf::from("main.scoop"),
            Span::new(100, 110),
        );
        let from_support =
            MonomorphRequest::new(key, PathBuf::from("support.scoop"), Span::new(200, 210));

        assert_ne!(from_main, from_support);
        assert_eq!(from_main.key, from_support.key);
    }
}
