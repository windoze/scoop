//! ABI 决策：符号 mangle + 参数传递方式。
//!
//! - `mangle_symbol`：把 FQN mangle 成 codegen 符号名（点 → 下划线 + stable key hash 后缀）。
//! - `param_abi_for_type`：标量/引用直接传，大聚合（>16 字节）间接传。
//! - `decide_abi`：整体 ABI pass，当前是 no-op（per-callable ABI 在 map_callable 中计算）。

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_hir::ty::TypeId;
use scoop2_mir::mir::materialize::MaterializedMir;
use scoop2_mir::mir::transport::StableTemplateKey;

use crate::*;

/// 大聚合的间接传递阈值（字节）：超过此值的聚合通过 hidden pointer 传。
const INDIRECT_THRESHOLD: u64 = 16;

/// Mangle FQN 成 codegen 符号名。
///
/// 规则：
/// - FQN 中的点 `.` 替换为下划线 `_`；
/// - 若存在 stable_template_key，追加 stable key 的 hash（去重）。
pub fn mangle_symbol(fqn: &str, stable_key: &Option<StableTemplateKey>) -> String {
    let base = fqn.replace('.', "_");
    match stable_key {
        Some(key) if !key.hash.is_empty() => format!("{}_{}", base, key.hash),
        _ => base,
    }
}

/// 计算单个类型的参数 ABI。
///
/// - 标量 / 引用 / 函数值：Direct（按值传）。
/// - 大聚合（size > 16 字节）：Indirect（通过 hidden pointer 传）。
/// - Unit/Nothing：Direct。
/// - Option<Ref>：Direct（指针 niche）。
/// - Option<聚合>：按聚合大小判定。
pub fn param_abi_for_type(ty: TypeId, layouts: &TypeLayoutTable) -> ParamAbi {
    let Some(layout) = layouts.get(ty) else {
        // 布局未知：保守用 Direct（标量/引用占多数）。
        return ParamAbi::Direct;
    };
    // 引用 / 函数值：指针，直接传。
    match &layout.kind {
        TypeLayoutKind::Reference { .. } | TypeLayoutKind::Function => return ParamAbi::Direct,
        TypeLayoutKind::Scalar { .. } | TypeLayoutKind::Nothing => return ParamAbi::Direct,
        _ => {}
    }
    // 聚合按 size 判定。
    if layout.size > INDIRECT_THRESHOLD {
        ParamAbi::Indirect
    } else {
        ParamAbi::Direct
    }
}

/// 整体 ABI pass。
///
/// 处理跨-callable 的 ABI 校正：
/// - extern 函数保留原始符号名（不 mangle）
/// - 闭包对象布局计算
/// - EffectStep 签名校正
pub fn decide_abi(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    _hir: &TypedHir,
    _interner: &Interner,
) {
    // 1. extern 函数：保留原始符号名。
    // 遍历 MIR module 中 body 为 None 的函数（extern/intrinsic 声明），
    // 把它们的 LirDeclaration.extern_symbol 设为原始 FQN。
    for item in &mir.module.items {
        if let scoop2_mir::mir::Item::Fun(fd) = item {
            if fd.body.is_none() {
                // extern 声明：在 program.declarations 中找到对应条目并设置 extern_symbol。
                let orig_fqn = &fd.fqn;
                for decl in &mut program.declarations {
                    if decl.fqn == *orig_fqn {
                        decl.extern_symbol = Some(orig_fqn.clone());
                        // extern 函数的 symbol_name 应为原始 FQN（不 mangle）。
                        decl.symbol_name = orig_fqn.clone();
                        decl.is_extern = true;
                    }
                }
            }
        }
    }

    // 2. 闭包对象布局：遍历 MIR 模块中的 MakeClosure，为每个 invoke_fqn 构建布局。
    // 闭包对象 = { invoke_fn_ptr: ptr, env_ptr: ptr }
    // env 布局由 captures 列表决定。
    for item in &mir.module.items {
        if let scoop2_mir::mir::Item::Fun(fd) = item {
            if let Some(body) = &fd.body {
                for block in &body.blocks {
                    for stmt in &block.stmts {
                        if let scoop2_mir::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                            if let scoop2_mir::mir::Rvalue::MakeClosure {
                                invoke_fqn,
                                env_contract,
                                ..
                            } = value
                            {
                                // 检查是否已为这个 invoke_fqn 添加过布局。
                                if program
                                    .closure_layouts
                                    .iter()
                                    .any(|cl| cl.invoke_fqn == *invoke_fqn)
                                {
                                    continue;
                                }
                                // 构建 captures 布局。
                                let mut captures: Vec<ClosureCaptureLayout> = Vec::new();
                                let mut env_offset: u64 = 0;
                                let mut env_align: u64 = 1;
                                for cap in &env_contract.captures {
                                    let cap_name = cap.name.clone();
                                    let cap_size = program
                                        .type_layouts
                                        .get(cap.transport.source_ty)
                                        .map(|l| l.size)
                                        .unwrap_or(8);
                                    let cap_align = program
                                        .type_layouts
                                        .get(cap.transport.source_ty)
                                        .map(|l| l.align)
                                        .unwrap_or(8);
                                    let cap_gc = crate::gc::is_gc_traceable_type(
                                        cap.transport.source_ty,
                                        &program.type_layouts,
                                    );
                                    env_offset = align_to(env_offset, cap_align);
                                    captures.push(ClosureCaptureLayout {
                                        name: cap_name,
                                        offset: env_offset,
                                        ty: cap.transport.source_ty,
                                        gc_traceable: cap_gc,
                                    });
                                    env_offset += cap_size;
                                    if cap_align > env_align {
                                        env_align = cap_align;
                                    }
                                }
                                let env_size = align_to(env_offset, env_align).max(1);
                                program.closure_layouts.push(ClosureLayout {
                                    invoke_fqn: invoke_fqn.clone(),
                                    captures,
                                    env_size,
                                    env_align,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. EffectStep 签名：EffectStep 函数的 ABI 已在 map_callable 中标记为 EffectStep。
}

/// 把 `offset` 向上对齐到 `align` 的倍数。
fn align_to(offset: u64, align: u64) -> u64 {
    if align <= 1 {
        return offset;
    }
    let mask = align - 1;
    (offset + mask) & !mask
}
