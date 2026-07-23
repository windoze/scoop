//! 名称解析（AST → resolved symbols）。
//!
//! 本模块覆盖编译管线的 `resolve` 阶段：把源码中的标识符路径绑定到具体的
//! 声明符号上。解析是**两阶段**的：
//!
//! 1. **headers**（[`collect`]）：收集所有顶层声明（跨文件、跨 cone）到 [`Index`]，
//!    检测类型/值命名空间的重复定义与非法可见性组合；
//! 2. **bodies**：在函数体 / 初始化器内部解析名字，允许前向引用同文件顶层符号
//!    （后续增量补齐：import、作用域、成员/扩展解析、可见性跨 cone 过滤）。
//!
//! 解析结果以「resolved 引用」写回 NodeId 侧表（[`output::NodeIdTable`]），供
//! typecheck 只读消费；所有失败汇报为稳定诊断码（`scoop::resolve::*`）。
//!
//! 模块划分：
//! - [`symbol`]：cone / 可见性 / 修饰符 / 符号 / 三命名空间类型；
//! - [`index`]：全局符号表 [`Index`]（FQN → 命名空间）、cone 注册、扩展暂存；
//! - [`output`]：NodeId 致密侧表原语 [`output::NodeIdTable`]；
//! - [`collect`]：header 收集（顶层声明 → [`Index`] + 重复/可见性诊断）；
//! - [`errors`]：`scoop::resolve::*` 诊断构造辅助。

pub mod body;
pub mod collect;
pub mod errors;
pub mod imports;
pub mod index;
pub mod output;
pub mod scopes;
pub mod symbol;

pub use errors::*;
pub use index::{Index, PendingExtension};
pub use output::{NodeIdTable, Resolution, ResolvedValue};
pub use symbol::{
    ConeId, ConeInfo, ConeKind, DeclSymbol, ModifierSet, NamespacedSymbols, SymbolKind, Visibility,
};

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner};

/// 单文件 resolve 管线（header 收集 → import → body 名字解析），把诊断**追加**进
/// `diags`。
///
/// 适用于无 sysroot 的单文件检查（如 `check-source --phase resolve` 对不依赖内置
/// 类型的负向 fixture）。多文件 / 带 prelude 的解析在 db 集成增量补齐。
pub fn run_file(
    file: &crate::syntax::ast::File,
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
) {
    let mut index = Index::new();
    let cone = index.intern_cone("<user>", ConeKind::Bin);
    let prefix = collect::package_prefix_of(file, interner);
    collect::collect_file(file, FileId(0), cone, &mut index, interner, diags);
    let imports = imports::ImportTable::collect(file, FileId(0), &index, interner, diags);
    let mut resolution = Resolution::new();
    body::resolve_file_bodies(
        file,
        &index,
        &imports,
        interner,
        diags,
        &mut resolution,
        &prefix,
    );
    // resolution（值引用侧表）目前供 typecheck 消费；本阶段不输出，保留以防被优化掉。
    let _ = resolution.value_refs.len();
}
