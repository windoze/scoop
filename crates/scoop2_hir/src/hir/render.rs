//! typed HIR 的稳定文本渲染（`dump-hir` 的 golden 格式）。
//!
//! 格式与 `scoop2_syntax::dump::dump_file`（dump-ast）**完全一致**的缩进树，
//! 唯一区别：每个表达式节点行追加 `ty=<推断类型>`。类型文本由
//! [`crate::ty::render_type`] 产出（spec 表面语法）。
//!
//! 渲染策略：
//!
//! - 对每个 User 文件，构造 `NodeId → 类型文本` 查找闭包（查 [`super::TypedFile`]
//!   的 `expr_types` 表），再调用 [`scoop2_syntax::dump::dump_file_typed`]。
//! - 多个 User 文件依次渲染，文件之间以空行分隔；Sysroot 文件不渲染（仅作为
//!   符号来源）。
//! - 类型缺失（typecheck 未覆盖的节点）不追加 `ty=`，而非报错——dump 是尽力而为
//!   的调试视图；完整性由 [`crate::completeness`] 闸门在需要时强制。

use scoop2_base::{FileId, NodeId};

use super::TypedHir;
use crate::ty::render_type;

/// 渲染整个 typed HIR（所有 User 文件）为文本。
///
/// `files` 是 `(FileId, &File)` 的迭代器；只渲染在 [`TypedHir::files`] 中登记了
/// `expr_types` 表的文件（即 User 文件）。
pub fn render_hir<'f>(
    hir: &TypedHir,
    files: impl Iterator<Item = (FileId, &'f crate::syntax::ast::File)>,
) -> String {
    let mut out = String::new();
    let mut first = true;
    for (file_id, file) in files {
        // 只渲染有 typed 产物（expr_types 表）的文件。
        let Some(typed_file) = hir.file(file_id) else {
            continue;
        };
        if !first {
            out.push('\n');
        }
        first = false;
        let store = &hir.store;
        let expr_types = &typed_file.expr_types;
        let effect_rows = &typed_file.facts.expr_effect_rows;
        let type_of = |id: NodeId| -> Option<String> {
            let ty_text = expr_types
                .get(id)
                .map(|&ty| render_type(store, &hir.interner, ty, true));
            let eff_text = effect_rows.get(id).map(|row| {
                if row.is_pure() {
                    String::new()
                } else {
                    row.terms
                        .iter()
                        .map(|t| render_type(store, &hir.interner, *t, false))
                        .collect::<Vec<_>>()
                        .join(" + ")
                }
            });
            match (ty_text, eff_text) {
                (Some(ty), Some(eff)) if !eff.is_empty() => Some(format!("{ty} eff={eff}")),
                (Some(ty), _) => Some(ty),
                (None, _) => None,
            }
        };
        let chunk = crate::syntax::dump::dump_file_typed(file, &hir.interner, &type_of);
        out.push_str(&chunk);
    }
    out
}
