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
pub mod type_refs;

pub use errors::*;
pub use index::{Index, PendingExtension};
pub use output::{NodeIdTable, Resolution, ResolvedValue};
pub use symbol::{
    ConeId, ConeInfo, ConeKind, DeclSymbol, ModifierSet, NamespacedSymbols, SymbolKind, Visibility,
};

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner};

/// 一个待解析的输入文件。
#[derive(Clone, Copy)]
pub struct InputFile<'a> {
    pub file: &'a crate::syntax::ast::File,
    pub file_id: FileId,
    pub origin: InputOrigin,
}

/// 输入来源：决定 cone 种类与是否解析 body。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputOrigin {
    /// 用户文件（解析 body / 类型引用）。
    User,
    /// sysroot 文件（只收集 header，作为 prelude/依赖符号来源）。
    Sysroot,
}

/// 多文件 resolve 管线：先收集所有文件（sysroot + 用户）的 header 到共享
/// [`Index`]，再对**用户文件**做 import / 类型引用 / body 名字解析；诊断追加进
/// `diags`。
///
/// sysroot 文件**不**解析 body（受信任，仅提供符号）。cone 按各文件 `package`
/// 声明归类（sysroot → [`ConeKind::Syslib`]，用户 → [`ConeKind::Bin`]）。
pub fn run_program(inputs: &[InputFile], interner: &mut Interner, diags: &mut DiagnosticSink) {
    let mut index = Index::new();
    // Phase 1：收集所有 header。
    for inp in inputs {
        let cone_name = collect::package_prefix_of(inp.file, interner);
        let cone_kind = match inp.origin {
            InputOrigin::User => ConeKind::Bin,
            InputOrigin::Sysroot => ConeKind::Syslib,
        };
        let cone = if cone_name.is_empty() {
            let fallback = match inp.origin {
                InputOrigin::User => "<user>",
                InputOrigin::Sysroot => "<sysroot>",
            };
            index.intern_cone(fallback, cone_kind)
        } else {
            index.intern_cone(&cone_name, cone_kind)
        };
        collect::collect_file(inp.file, inp.file_id, cone, &mut index, interner, diags);
    }
    // 解析待处理扩展（接收者 → FQN，登记为 `<receiver>.<name>` 成员）。
    index.resolve_extensions(interner);
    // Phase 2：解析用户文件。
    for inp in inputs.iter().filter(|i| i.origin == InputOrigin::User) {
        let prefix = collect::package_prefix_of(inp.file, interner);
        let imports = imports::ImportTable::collect(inp.file, inp.file_id, &index, interner, diags);
        type_refs::resolve_file_type_refs(inp.file, &index, &imports, interner, diags, &prefix);
        let mut resolution = Resolution::new();
        body::resolve_file_bodies(
            inp.file,
            &index,
            &imports,
            interner,
            diags,
            &mut resolution,
            &prefix,
        );
        let _ = resolution.value_refs.len();
    }
}

/// 单文件 resolve 管线（无 sysroot）；等价于只含一个用户文件的 [`run_program`]。
pub fn run_file(
    file: &crate::syntax::ast::File,
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
) {
    run_program(
        &[InputFile {
            file,
            file_id: FileId(0),
            origin: InputOrigin::User,
        }],
        interner,
        diags,
    );
}
