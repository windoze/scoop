//! lang-items 注册表（M2-3/M2-6：core 周知条目 → 稳定句柄）。
//!
//! typecheck 启动时对 interner 一次性解析全部周知条目，之后各阶段经
//! [`LangItems`] 句柄消费——**禁止散落的 `interner.get("scoop.core.X")`
//! 字符串注入**（防用户遮蔽 + 单点登记）。条目缺失（sysroot 损坏）以
//! `Symbol::default()` 兜底并在装配期被完整性闸门暴露。

use scoop2_base::Symbol;

/// 周知 lang 条目句柄集。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LangItems {
    pub any: Symbol,
    pub option: Symbol,
    pub string: Symbol,
    pub array: Symbol,
    pub bool_: Symbol,
    pub char_: Symbol,
    pub int: Symbol,
    pub uint: Symbol,
    pub float32: Symbol,
    pub float64: Symbol,
    /// `Continuation`（compiler-owned interface）。
    pub continuation: Symbol,
}

impl LangItems {
    /// 从 interner 解析全部周知条目（缺失兜底 default Symbol）。
    pub fn resolve(interner: &scoop2_base::Interner) -> Self {
        let get = |name: &str| interner.get(name).unwrap_or_default();
        Self {
            any: get("scoop.core.Any"),
            option: get("scoop.core.Option"),
            string: get("scoop.core.String"),
            array: get("scoop.core.Array"),
            bool_: get("scoop.core.Bool"),
            char_: get("scoop.core.Char"),
            int: get("scoop.core.Int"),
            uint: get("scoop.core.UInt"),
            float32: get("scoop.core.Float32"),
            float64: get("scoop.core.Float64"),
            continuation: get("scoop.core.Continuation"),
        }
    }
}
