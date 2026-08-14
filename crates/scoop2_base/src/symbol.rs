//! 字符串 interning：AST/HIR 中的标识符一律使用 [`Symbol`]。

use std::collections::HashMap;
use std::fmt;

/// interned 字符串句柄。比较与哈希都是 O(1) 的整数操作。
///
/// `Symbol` 只在产出它的 [`Interner`] 内有意义；调试输出需要配合 interner
/// 解析为文本（见 [`Interner::resolve`]）。
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Symbol(u32);

impl Symbol {
    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// 从原始 u32 构造（仅供语义阶段 / 测试用；正常 Symbol 应由 Interner 产出）。
    pub fn from_u32(raw: u32) -> Self {
        Symbol(raw)
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Symbol(0)
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sym#{}", self.0)
    }
}

/// 字符串 interner。同一文本多次 intern 返回同一 [`Symbol`]。
///
/// 可克隆（深拷贝 map + strings）；句柄值在新副本中保持稳定。用于把 typecheck
/// 产出的 `Symbol` 键表连同 interner 一起移入 typed-HIR / dump-hir 消费侧。
#[derive(Clone, Debug, Default)]
pub struct Interner {
    map: HashMap<Box<str>, Symbol>,
    strings: Vec<Box<str>>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// intern 一段文本，返回其稳定句柄。
    pub fn intern(&mut self, text: &str) -> Symbol {
        if let Some(&sym) = self.map.get(text) {
            return sym;
        }
        let sym = Symbol(self.strings.len() as u32);
        let owned: Box<str> = text.into();
        self.strings.push(owned.clone());
        self.map.insert(owned, sym);
        sym
    }

    /// 解析句柄为文本。
    ///
    /// # Panics
    /// `sym` 不是由本 interner 产出时 panic（属于编译器内部 bug）。
    pub fn resolve(&self, sym: Symbol) -> &str {
        &self.strings[sym.as_usize()]
    }

    /// 查找文本对应的已 intern 句柄；不存在则 `None`（**不创建**）。
    ///
    /// 用于只读查询场景（如按 `prefix.name` 文本探测某 FQN 是否已被 intern），
    /// 避免为了查询而副作用地新增句柄。
    pub fn get(&self, text: &str) -> Option<Symbol> {
        self.map.get(text).copied()
    }

    /// 尝试解析句柄；非法句柄返回 `None`。
    pub fn try_resolve(&self, sym: Symbol) -> Option<&str> {
        self.strings.get(sym.as_usize()).map(|s| &**s)
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

// ---------------------------------------------------------------------------
// serde：只序列化 strings（Vec 序 = intern 序，确定性）；map 是派生索引，
// 反序列化时重建。Symbol 句柄值在往返后保持不变。
// ---------------------------------------------------------------------------

impl serde::Serialize for Interner {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.strings.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Interner {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let strings: Vec<Box<str>> = Vec::deserialize(deserializer)?;
        let mut map = HashMap::with_capacity(strings.len());
        for (i, s) in strings.iter().enumerate() {
            map.insert(s.clone(), Symbol(i as u32));
        }
        Ok(Self { map, strings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_is_stable_and_deduplicating() {
        let mut it = Interner::new();
        let a1 = it.intern("alpha");
        let b = it.intern("beta");
        let a2 = it.intern("alpha");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert_eq!(it.len(), 2);
        assert_eq!(it.resolve(a1), "alpha");
        assert_eq!(it.resolve(b), "beta");
    }

    #[test]
    fn try_resolve_rejects_foreign_symbol() {
        let it = Interner::new();
        assert_eq!(it.try_resolve(Symbol::as_u32_raw(7)), None);
    }

    impl Symbol {
        fn as_u32_raw(raw: u32) -> Symbol {
            // 测试辅助：直接构造一个非法句柄。
            Symbol(raw)
        }
    }
}
