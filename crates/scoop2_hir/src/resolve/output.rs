//! 解析输出侧表原语。
//!
//! resolve/typecheck 的分析结果存放在以 [`NodeId`](scoop2_base::NodeId) 为键的
//! **致密侧表**（`Vec<Option<T>>`，下标即 `NodeId.as_usize()`）中，替代旧前端的
//! `HashMap<Span, _>` 写回模式。前提：编译期使用**单一共享的 NodeId 分配器**
//!（见 Phase 0 的 `parse_file_with`），使 NodeId 跨文件全局唯一。
//!
//! 本文件先落地通用 [`NodeIdTable`]；完整的 `Resolution`（value/type/member 引用、
//! scope、this、local 声明等表）随 body 解析阶段补齐。

use std::collections::HashMap;

use scoop2_base::{NodeId, Symbol};

/// 一个值引用（`Ident` / 调用 callee）的解析结果。
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResolvedValue {
    /// 局部绑定（参数 / 局部 `val`·`var`）；`decl` 为绑定声明节点（`for`/模式等
    /// 无专属节点的绑定为 `None`，待 typecheck 落地时改为合成 NodeId）。
    Local { decl: Option<NodeId> },
    /// 顶层值（顶层 `val`/`var`）。
    TopLevelValue { fqn: Symbol },
    /// 顶层函数（重载集；具体重载由 typecheck 决议）。
    TopLevelFun { fqn: Symbol },
}

/// resolve 阶段输出的 NodeId 侧表集合（随阶段推进逐步填充）。
///
/// 当前填充：[`value_refs`](Resolution::value_refs)（函数体 / 初始化器内的值
/// 引用）。type/member/scope 等表随对应增量补齐。
#[derive(Debug)]
pub struct Resolution {
    pub value_refs: NodeIdTable<ResolvedValue>,
}

impl Resolution {
    pub fn new() -> Self {
        Self {
            value_refs: NodeIdTable::new(),
        }
    }
}

impl Default for Resolution {
    fn default() -> Self {
        Self::new()
    }
}

/// 以 [`NodeId`] 为键的致密侧表。
///
/// 稀疏写入（`set` 自动扩展到足够容量）；读取返回 `Option`。
/// typecheck 中间态使用（允许缺失）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeIdTable<T> {
    slots: Vec<Option<T>>,
}

// 手动实现 Default，使 `NodeIdTable<T>` 对任意 `T` 都 `Default`（空表）。
// `#[derive(Default)]` 会错误地要求 `T: Default`，但 `Vec::new()` 无此需要。
impl<T> Default for NodeIdTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NodeIdTable<T> {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// 在 `id` 处写入 `v`（必要时扩展）。
    pub fn set(&mut self, id: NodeId, v: T) {
        let i = id.as_usize();
        if i >= self.slots.len() {
            self.slots.resize_with(i + 1, || None);
        }
        self.slots[i] = Some(v);
    }

    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.slots.get(id.as_usize()).and_then(|o| o.as_ref())
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.slots.get_mut(id.as_usize()).and_then(|o| o.as_mut())
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|o| o.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Freeze（compact）：把可能缺失的稀疏表转换为**无空槽的 dense 表**。
    ///
    /// 遍历 `expected`（所有**应该有条目**的 NodeId，由遍历 AST 收集），
    /// 对每个 expected NodeId 若表中缺失（`Option::None`），调用 `on_missing`
    /// 报告诊断（真实输入程序错误 / typecheck 缺口）。
    ///
    /// 然后把所有 `Some` 条目 compact 进 `HashMap<NodeId, T>`——**没有空槽**，
    /// 每一项都有明确内容。
    pub fn freeze(
        self,
        expected: &[NodeId],
        mut on_missing: impl FnMut(NodeId),
    ) -> FrozenNodeIdTable<T> {
        for &id in expected {
            if !self.contains(id) {
                on_missing(id);
            }
        }
        let mut dense = HashMap::with_capacity(self.len());
        for (i, slot) in self.slots.into_iter().enumerate() {
            if let Some(v) = slot {
                dense.insert(NodeId::from_u32(i as u32), v);
            }
        }
        FrozenNodeIdTable { entries: dense }
    }
}

/// **冻结后的** NodeId 侧表：compact 后无空槽，每一项都有明确内容。
///
/// 由 [`NodeIdTable::freeze`] 产出。内部是 `HashMap<NodeId, T>`——只含有数据的
/// 条目，没有空槽。`get()` 返回 `&T`（非 `Option`）。
///
/// **安全契约**：消费者只查询 freeze 时 `expected` 列表中的 NodeId。
/// 这些 NodeId 在 freeze 时已校验存在（若缺失已报诊断，driver 会中止 pipeline）。
/// 查询未在 expected 中的 NodeId 是消费者 bug——会 panic。
#[derive(Debug, Clone)]
pub struct FrozenNodeIdTable<T> {
    entries: HashMap<NodeId, T>,
}

impl<T> FrozenNodeIdTable<T> {
    /// 查询 `id` 处的条目。返回 `&T`（非 `Option`）。
    pub fn get(&self, id: NodeId) -> &T {
        self.entries
            .get(&id)
            .expect("FrozenNodeIdTable::get: 节点不在表中（消费者查询了未 freeze 的 NodeId）")
    }

    /// 查询 `id` 处的条目（Copy 类型直接返回值）。
    pub fn get_copy(&self, id: NodeId) -> T
    where
        T: Copy,
    {
        *self.get(id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.entries.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_base::NodeIdAllocator;

    fn alloc_n(n: u32) -> Vec<NodeId> {
        let mut a = NodeIdAllocator::new();
        (0..n).map(|_| a.alloc()).collect()
    }

    #[test]
    fn set_and_get_sparse() {
        let mut t: NodeIdTable<u32> = NodeIdTable::new();
        let ids = alloc_n(8);
        t.set(ids[0], 10);
        t.set(ids[7], 70);
        assert_eq!(t.get(ids[0]), Some(&10));
        assert_eq!(t.get(ids[7]), Some(&70));
        assert_eq!(t.get(ids[3]), None);
        assert!(t.contains(ids[7]));
        assert!(!t.contains(ids[3]));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn overwrite() {
        let mut t: NodeIdTable<u32> = NodeIdTable::new();
        let ids = alloc_n(3);
        t.set(ids[2], 1);
        t.set(ids[2], 9);
        assert_eq!(t.get(ids[2]), Some(&9));
        assert_eq!(t.len(), 1);
    }
}
