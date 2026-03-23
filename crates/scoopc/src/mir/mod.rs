//! MIR（Mid-level IR）。
//!
//! MIR 的定位：在 HIR 之上引入**显式控制流**（基本块/CFG）与**显式临时变量**（locals），为后续阶段服务：
//! - `if/when/while` 等控制流 lowering（TODO T0708+）
//! - `try/finally`、effect handler 等需要 cleanup/unwinding 的语义建模（TODO T0707、T0612）
//! - 单态化与 LLVM codegen
//!
//! 当前阶段（TODO T0703）只落地**数据结构骨架**与最小 CFG 验证工具：
//! - 基本块（BB）+ terminator
//! - locals 声明列表
//! - CFG 连通性/合法性检查（用于单测与后续 pass 的断言）

use std::collections::VecDeque;
use std::fmt;

use crate::span::Span;
use crate::ty::TypeId;

/// 基本块 ID（在 `Body::blocks` 内的索引）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BasicBlockId(u32);

impl BasicBlockId {
    pub fn as_u32(self) -> u32 {
        self.0
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for BasicBlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// 局部变量 ID（在 `Body::locals` 内的索引）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(u32);

impl LocalId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.0)
    }
}

/// 一个函数（或顶层 initializer）在 MIR 中的 body。
#[derive(Debug, Clone)]
pub struct Body {
    /// locals 声明表（参数/局部/临时变量；后续会扩展 return local 等约定）。
    pub locals: Vec<LocalDecl>,
    /// 基本块列表（块内顺序执行 statements，最后以 terminator 结束）。
    pub blocks: Vec<BasicBlock>,
    /// CFG 入口块（通常为 `bb0`）。
    pub start: BasicBlockId,
}

impl Body {
    /// 创建一个空 body（调用方需要填充 blocks 并设置 `start`）。
    pub fn new_empty() -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            start: BasicBlockId(0),
        }
    }

    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).expect("too many locals"));
        self.locals.push(decl);
        id
    }

    pub fn push_block(&mut self, bb: BasicBlock) -> BasicBlockId {
        let id = BasicBlockId(u32::try_from(self.blocks.len()).expect("too many basic blocks"));
        self.blocks.push(bb);
        id
    }

    /// 检查 CFG 的**结构合法性**：
    /// - `start` 必须在 `blocks` 范围内
    /// - 所有 terminator 的 target 必须在范围内
    pub fn validate_cfg(&self) -> Result<(), MirValidationError> {
        if self.blocks.is_empty() {
            return Err(MirValidationError::EmptyBody);
        }
        if self.start.as_usize() >= self.blocks.len() {
            return Err(MirValidationError::InvalidStartBlock {
                start: self.start,
                blocks_len: self.blocks.len(),
            });
        }

        for (idx, block) in self.blocks.iter().enumerate() {
            let from = BasicBlockId(idx as u32);
            match &block.terminator.kind {
                TerminatorKind::Goto { target } => {
                    if target.as_usize() >= self.blocks.len() {
                        return Err(MirValidationError::InvalidTarget {
                            from,
                            target: *target,
                            blocks_len: self.blocks.len(),
                        });
                    }
                }
                TerminatorKind::Return
                | TerminatorKind::Unreachable
                | TerminatorKind::Perform { .. }
                | TerminatorKind::Handle { .. }
                | TerminatorKind::Todo(_) => {}
            }
        }

        Ok(())
    }

    /// 从 `start` 出发，计算可达的基本块集合（按 BFS 顺序返回）。
    pub fn reachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            order.push(bb);

            let block = &self.blocks[bb.as_usize()];
            block.terminator.kind.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        Ok(order)
    }

    /// 检查 CFG 是否“全连通”：所有基本块都从 `start` 可达。
    pub fn is_fully_reachable(&self) -> Result<bool, MirValidationError> {
        let reachable = self.reachable_blocks()?;
        Ok(reachable.len() == self.blocks.len())
    }

    /// 列出不可达的基本块（用于测试与后续 pass 的诊断）。
    pub fn unreachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            let block = &self.blocks[bb.as_usize()];
            block.terminator.kind.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        let mut unreachable = Vec::new();
        for (idx, ok) in visited.iter().copied().enumerate() {
            if !ok {
                unreachable.push(BasicBlockId(idx as u32));
            }
        }
        Ok(unreachable)
    }
}

/// 一个 local 的声明信息。
#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub span: Span,
    pub name: Option<String>,
    pub ty: TypeId,
}

/// MIR 基本块：顺序语句 + 终结指令（terminator）。
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub stmts: Vec<Statement>,
    pub terminator: Terminator,
}

/// MIR 语句（顺序执行）。
#[derive(Debug, Clone)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Nop,
    /// 未实现节点占位（用于尽早落地数据结构但避免 `todo!()`/panic）。
    Todo(&'static str),
}

/// MIR terminator（显式控制流）。
#[derive(Debug, Clone)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Return,
    Goto { target: BasicBlockId },
    Unreachable,
    /// effect operation 调用（对应 HIR 的 `ExprKind::Perform`）。
    ///
    /// 当前阶段仅保留“发生了哪一个 effect op”的信息；具体如何进入 handler/如何建模 unwinding
    /// 由后续 effect lowering 任务（TODO T0713/T0707）决定。
    Perform { op_fqn: String },
    /// effect handler 区域（对应 HIR 的 `ExprKind::Handle`）。
    ///
    /// 注意：该变体目前只是一个“结构占位”，并不携带 CFG target；后续会在 lowering 中把 handle
    /// 展开为显式基本块与 cleanup/handler 栈管理。
    Handle { arms: Vec<HandlerArm>, has_finally: bool },
    /// 未实现控制流占位（例如 if/switch/call/cleanup 等）。
    Todo(&'static str),
}

/// `handle` 在 MIR 视图下的一个 handler arm（结构占位）。
#[derive(Debug, Clone)]
pub struct HandlerArm {
    pub op_fqn: String,
    pub binder_count: usize,
}

impl TerminatorKind {
    /// 对 terminator 的后继基本块调用回调。
    ///
    /// 该接口适合做 CFG 分析（reachable/循环检测等），避免为每次查询分配 `Vec`。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        match self {
            TerminatorKind::Goto { target } => f(*target),
            TerminatorKind::Return
            | TerminatorKind::Unreachable
            | TerminatorKind::Perform { .. }
            | TerminatorKind::Handle { .. }
            | TerminatorKind::Todo(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirValidationError {
    /// MIR body 为空（没有任何基本块）。
    EmptyBody,
    /// `start` 超出 `blocks` 范围。
    InvalidStartBlock { start: BasicBlockId, blocks_len: usize },
    /// terminator 的 target 超出 `blocks` 范围。
    InvalidTarget {
        from: BasicBlockId,
        target: BasicBlockId,
        blocks_len: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::TypeStore;

    #[test]
    fn cfg_reachable_two_blocks_ok() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();

        let mut body = Body::new_empty();
        let _tmp = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
        });

        let bb0 = body.push_block(BasicBlock {
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Nop,
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(1),
                },
            },
        });
        let bb1 = body.push_block(BasicBlock {
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return,
            },
        });

        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
    }

    #[test]
    fn cfg_invalid_target_is_error() {
        let mut body = Body::new_empty();
        let bb0 = body.push_block(BasicBlock {
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(42),
                },
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_cfg(),
            Err(MirValidationError::InvalidTarget {
                from: BasicBlockId(0),
                target: BasicBlockId(42),
                blocks_len: 1,
            })
        );
    }
}
