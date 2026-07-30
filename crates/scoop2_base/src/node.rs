//! AST 节点身份：语义阶段致密侧表的键。

use std::fmt;

/// AST 节点 ID。在单个文件的解析过程中单调分配、全局唯一（文件内）。
///
/// 语义阶段（resolve/typecheck）的分析结果存放在以 `NodeId` 为下标的致密
/// 侧表（`Vec<Option<T>>`）中，替代旧前端的 `HashMap<Span, _>` 写回模式。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// 从原始 u32 构造（仅供语义阶段 / 测试用；正常 NodeId 应由 parser 分配）。
    pub fn from_u32(raw: u32) -> Self {
        NodeId(raw)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        NodeId(0)
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node#{}", self.0)
    }
}

/// `NodeId` 分配器。每个文件的 parser 持有一个，从 0 开始单调分配。
#[derive(Debug, Default, Clone)]
pub struct NodeIdAllocator {
    next: u32,
}

impl NodeIdAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self) -> NodeId {
        let id = NodeId(self.next);
        self.next += 1;
        id
    }

    /// 已分配的 ID 数量（即下一个将分配的下标）。
    pub fn len(&self) -> usize {
        self.next as usize
    }

    pub fn is_empty(&self) -> bool {
        self.next == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_is_monotonic() {
        let mut alloc = NodeIdAllocator::new();
        let a = alloc.alloc();
        let b = alloc.alloc();
        assert_eq!(a.as_u32(), 0);
        assert_eq!(b.as_u32(), 1);
        assert_eq!(alloc.len(), 2);
    }
}
