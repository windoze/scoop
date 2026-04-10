//! MIR（Mid-level IR）。
//!
//! MIR 的定位：在 HIR 之上引入**显式控制流**（基本块/CFG）与**显式临时变量**（locals），为后续阶段服务：
//! - `if/when/while` 等控制流 lowering（TODO T0708+）
//! - `try/finally`、effect handler 等需要 cleanup/unwinding 的语义建模（TODO T0707、T0612）
//! - 单态化与 LLVM codegen
//!
//! 当前阶段（TODO T0703/T0708）落地：
//! - 基本块（BB）+ terminator
//! - locals 声明列表
//! - CFG 连通性/合法性检查（用于单测与后续 pass 的断言）
//! - 最小 MIR lowering（用于 `dump-mir`/fixtures 回归；未覆盖节点用 `Todo(...)` 占位）

mod lower;

use std::collections::VecDeque;
use std::fmt;

use crate::span::Span;
use crate::ty::TypeId;

pub(crate) use lower::lower_hir_file_for_dump;
pub use lower::{LoweredMir, MirLowerError, lower_for_dump};

/// 一个源文件 lowering 后的 MIR（当前阶段主要用于 dump/fixtures）。
#[derive(Debug, Clone)]
pub struct File {
    pub items: Vec<Item>,
}

/// 顶层条目（top-level items）。
#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    /// 未纳入当前阶段 MIR 的条目占位（例如顶层 val/global init、type decl 等）。
    Todo {
        span: Span,
        kind: &'static str,
    },
}

/// 函数声明在 MIR 视图下的承载。
#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Body>,
}

/// 参数在 MIR 视图下的表示：它同时对应一个 local。
#[derive(Debug, Clone)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub local: LocalId,
}

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
    /// - 所有 terminator 的 target 必须在范围内（包含 cleanup/unwind target）
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

            // 注意：不分配 Vec，保持验证逻辑轻量；一旦发现无效 target 即返回。
            let mut invalid_target: Option<BasicBlockId> = None;
            block.terminator.for_each_successor(|target| {
                if invalid_target.is_some() {
                    return;
                }
                if target.as_usize() >= self.blocks.len() {
                    invalid_target = Some(target);
                }
            });
            if let Some(target) = invalid_target {
                return Err(MirValidationError::InvalidTarget {
                    from,
                    target,
                    blocks_len: self.blocks.len(),
                });
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
            block.terminator.for_each_successor(|succ| {
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
            block.terminator.for_each_successor(|succ| {
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
    /// 是否为 cleanup block（用于 `finally`/effect unwinding）。
    ///
    /// 该标记本身不影响 CFG 连通性；主要用于 dump/诊断与后续更严格的 MIR 规则。
    pub is_cleanup: bool,
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
    /// `target = value`（最小赋值语句，用于 if/when merge 等场景）。
    Assign {
        target: LocalId,
        value: Rvalue,
    },
    /// 未实现节点占位（用于尽早落地数据结构但避免 `todo!()`/panic）。
    Todo(&'static str),
}

/// 一个“可以被使用的值”（最小 operand 模型）。
#[derive(Debug, Clone)]
pub enum Operand {
    Local(LocalId),
    Const(ConstValue),
}

/// 常量值（当前阶段不保留字面量原始内容，仅保留种类）。
#[derive(Debug, Clone)]
pub enum ConstValue {
    Bool(bool),
    Unit,
    Int,
    String,
}

/// 右值（最小 rvalue 模型）。
#[derive(Debug, Clone)]
pub enum Rvalue {
    Use(Operand),
    /// 创建一个 tuple 值（最小 aggregate，用于 env struct 等场景）。
    MakeTuple {
        elements: Vec<Operand>,
    },
    /// 读取 tuple 的某个字段：`tuple[index]`（按捕获顺序索引）。
    TupleGet {
        tuple: Operand,
        index: usize,
    },
    /// 创建一个“可变捕获盒”（T0714）。
    ///
    /// 语义：把一个值装入 heap cell，并返回该 cell 的引用；同一个 cell 可被多个 closure 共享捕获，
    /// 从而保证 `var` 在内外层的读写具备别名一致性。
    CaptureBoxNew {
        value: Operand,
    },
    /// 从“可变捕获盒”读取当前值（T0714）。
    CaptureBoxGet {
        box_operand: Operand,
    },
    /// 向“可变捕获盒”写入新值（T0714）。
    ///
    /// 注意：该 rvalue 本身的“结果值”在当前阶段并不重要（通常写入一个 `Unit` 临时 local），
    /// 主要用于在 MIR dump/fixtures 中显式体现写回语义。
    CaptureBoxSet {
        box_operand: Operand,
        value: Operand,
    },
    /// 创建一个函数值（closure）：`{ env_struct, fn_ptr }`（T0710/T0711）。
    ///
    /// 当前阶段：
    /// - `env` 支持 `Unit`（无捕获）或最小 tuple env（T0711）；
    /// - 更丰富的 env 表示（真正的 struct/layout/heap/GC）会在后续 codegen/runtime 任务补齐。
    MakeClosure {
        env: Operand,
        fn_ptr: String,
    },
    Todo(&'static str),
}

/// MIR terminator（显式控制流）。
#[derive(Debug, Clone)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
    /// 当该 terminator 发生 unwinding（例如 effect 传播）时应采取的动作。
    pub unwind: UnwindAction,
}

/// terminator 在发生 unwinding 时应采取的动作（最小模型，用于 `finally`/effect unwinding）。
#[derive(Debug, Clone)]
pub enum UnwindAction {
    /// 该 terminator 不会发生 unwinding。
    NoUnwind,
    /// 若发生 unwinding，则先跳转到 cleanup block 执行清理逻辑。
    Cleanup { target: BasicBlockId },
    /// 未实现占位：表示“可能会 unwind，但具体行为尚未建模”。
    Todo(&'static str),
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Return,
    /// cleanup block：执行完清理逻辑后继续向上传播 unwinding。
    ResumeUnwind,
    Goto {
        target: BasicBlockId,
    },
    /// 条件分支：若 `cond` 为真跳转到 `then_target`，否则跳转到 `else_target`。
    CondBr {
        cond: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Unreachable,
    /// effect operation 调用（对应 HIR 的 `ExprKind::Perform`）。
    ///
    /// 当前阶段仅保留“发生了哪一个 effect op”的信息；具体如何进入 handler/如何建模 unwinding
    /// 由后续 effect lowering 任务（TODO T0713/T0707）决定。
    Perform {
        op_fqn: String,
    },
    /// effect handler 区域（对应 HIR 的 `ExprKind::Handle`）。
    ///
    /// 注意：该变体目前只是一个“结构占位”，并不携带 CFG target；后续会在 lowering 中把 handle
    /// 展开为显式基本块与 cleanup/handler 栈管理。
    Handle {
        arms: Vec<HandlerArm>,
        has_finally: bool,
    },
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
    /// 对 terminator 的“正常”后继基本块调用回调（不包含 cleanup/unwind 边）。
    ///
    /// 该接口适合做 CFG 分析（reachable/循环检测等），避免为每次查询分配 `Vec`。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        match self {
            TerminatorKind::Goto { target } => f(*target),
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                f(*then_target);
                f(*else_target);
            }
            TerminatorKind::Return
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Unreachable
            | TerminatorKind::Perform { .. }
            | TerminatorKind::Handle { .. }
            | TerminatorKind::Todo(_) => {}
        }
    }
}

impl Terminator {
    /// 对 terminator 的后继基本块调用回调（包含 cleanup/unwind 边）。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        self.kind.for_each_successor(&mut f);
        if let UnwindAction::Cleanup { target } = self.unwind {
            f(target);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirValidationError {
    /// MIR body 为空（没有任何基本块）。
    EmptyBody,
    /// `start` 超出 `blocks` 范围。
    InvalidStartBlock {
        start: BasicBlockId,
        blocks_len: usize,
    },
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
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Nop,
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(1),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return,
                unwind: UnwindAction::NoUnwind,
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
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(42),
                },
                unwind: UnwindAction::NoUnwind,
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

    #[test]
    fn cfg_cleanup_edge_is_reachable() {
        // 模拟一个“可能 unwind 的 terminator”：
        // - 正常路径不存在（Perform 目前作为占位 terminator）
        // - unwind 路径跳到 cleanup block，然后用 ResumeUnwind 继续传播
        let mut body = Body::new_empty();

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                },
                unwind: UnwindAction::Cleanup {
                    target: BasicBlockId(1),
                },
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: true,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::ResumeUnwind,
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
        assert!(body.blocks[bb1.as_usize()].is_cleanup);
    }
}
