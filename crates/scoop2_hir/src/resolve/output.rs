//! 解析输出侧表原语。
//!
//! resolve/typecheck 的分析结果存放在以 [`NodeId`](scoop2_base::NodeId) 为键的
//! **致密侧表**（`Vec<Option<T>>`，下标即 `NodeId.as_usize()`）中，替代旧前端的
//! `HashMap<Span, _>` 写回模式。前提：编译期使用**单一共享的 NodeId 分配器**
//!（见 Phase 0 的 `parse_file_with`），使 NodeId 跨文件全局唯一。
//!
//! 本文件先落地通用 [`NodeIdTable`]；完整的 `Resolution`（value/type/member 引用、
//! scope、this、local 声明等表）随 body 解析阶段补齐。

use scoop2_base::{NodeId, Symbol};

/// 一个值引用（`Ident` / 调用 callee）的解析结果。
#[derive(Clone, Copy, Debug)]
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
#[derive(Debug, Default)]
pub struct NodeIdTable<T> {
    slots: Vec<Option<T>>,
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
