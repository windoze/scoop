//! 作用域栈（局部绑定查找）。
//!
//! 函数体解析时维护一个帧栈：每帧是该作用域内的局部绑定表（参数 / 局部
//! `val`·`var` / `for` 绑定 / `when`/`handle` arm 绑定 / lambda 参数）。
//! 查找按内层优先；嵌套作用域可**遮蔽**外层（合法），**同帧重名**是
//! `duplicate_definition`（由调用方报告）。

use hashbrown::HashMap;

use scoop2_base::{NodeId, Span, Symbol};

/// 一个局部绑定（值引用解析为 [`ResolvedValue::Local`](super::output::ResolvedValue::Local) 时携带）。
#[derive(Clone, Copy, Debug)]
pub struct LocalBinding {
    /// 绑定声明节点（参数 id / `val` 所在语句 id）；`for`/模式等无专属节点的绑定为 `None`
    ///（typecheck 落地时改为按 `node_count` 之后的合成 NodeId）。
    pub decl: Option<NodeId>,
    /// 绑定名 span（用于重复定义诊断）。
    pub span: Span,
}

/// `define` 的结果。
#[derive(Clone, Copy, Debug)]
pub enum DefineOutcome {
    /// 新绑定。
    Defined,
    /// 当前帧已有同名绑定（同帧重定义）。
    Redefined { prev: LocalBinding },
}

/// 作用域帧栈。
#[derive(Default, Debug)]
pub struct ScopeStack {
    frames: Vec<HashMap<Symbol, LocalBinding>>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// 进入一个新作用域。
    pub fn enter(&mut self) {
        self.frames.push(HashMap::new());
    }

    /// 离开当前作用域。
    pub fn leave(&mut self) {
        self.frames.pop();
    }

    /// 当前帧深度。
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// 在当前帧定义一个绑定。
    pub fn define(&mut self, name: Symbol, binding: LocalBinding) -> DefineOutcome {
        // invariant: 调用方已 enter 至少一帧。
        let frame = self
            .frames
            .last_mut()
            .expect("scope stack has at least one frame");
        if let Some(prev) = frame.get(&name) {
            DefineOutcome::Redefined { prev: *prev }
        } else {
            frame.insert(name, binding);
            DefineOutcome::Defined
        }
    }

    /// 由内向外查找名字。
    pub fn resolve(&self, name: Symbol) -> Option<&LocalBinding> {
        self.frames.iter().rev().find_map(|f| f.get(&name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_base::{Interner, Span};

    fn alloc_ids(n: u32) -> Vec<NodeId> {
        let mut a = scoop2_base::NodeIdAllocator::new();
        (0..n).map(|_| a.alloc()).collect()
    }

    fn lb(id: NodeId, off: usize) -> LocalBinding {
        LocalBinding {
            decl: Some(id),
            span: Span::new(off, off + 1),
        }
    }

    #[test]
    fn inner_shadows_outer() {
        let mut it = Interner::new();
        let x = it.intern("x");
        let ids = alloc_ids(2);
        let mut s = ScopeStack::new();
        s.enter();
        s.define(x, lb(ids[0], 0));
        s.enter();
        s.define(x, lb(ids[1], 1)); // 遮蔽
        assert_eq!(s.resolve(x).unwrap().decl, Some(ids[1]));
        s.leave();
        assert_eq!(s.resolve(x).unwrap().decl, Some(ids[0]));
    }

    #[test]
    fn same_frame_redefinition_reported() {
        let mut it = Interner::new();
        let y = it.intern("y");
        let ids = alloc_ids(2);
        let mut s = ScopeStack::new();
        s.enter();
        assert!(matches!(s.define(y, lb(ids[0], 0)), DefineOutcome::Defined));
        assert!(matches!(
            s.define(y, lb(ids[1], 5)),
            DefineOutcome::Redefined { prev } if prev.decl == Some(ids[0])
        ));
    }
}
