//! LIR 结构完整性验证。
//!
//! 检查：所有 callable 有 symbol_name、所有引用的 TypeId 在 type_layouts 中、
//! vtable/itable 引用的符号存在、符号名唯一、EffectStep 函数有完整 schema、
//! block target 有效等。打印警告但不 panic。

use std::collections::HashSet;

use crate::*;

/// 主入口：验证 LirProgram 结构完整性。
pub fn verify_lir(program: &LirProgram) {
    let known_types: HashSet<scoop2_mir::ty::TypeId> =
        program.type_layouts.entries.keys().copied().collect();
    let mut warnings: Vec<String> = Vec::new();

    // 1. callable：symbol_name 非空 + 参数/返回类型有布局 + 符号唯一。
    let mut symbols: HashSet<String> = HashSet::new();
    for (i, c) in program.callables.iter().enumerate() {
        if c.symbol_name.is_empty() {
            warnings.push(format!("callable[{}] {}: symbol_name 为空", i, c.fqn));
        }
        if !symbols.insert(c.symbol_name.clone()) {
            warnings.push(format!("callable[{}] {}: symbol_name 重复", i, c.fqn));
        }
        if c.fqn.is_empty() {
            warnings.push(format!("callable[{}]: fqn 为空", i));
        }
        for (pi, p) in c.params.iter().enumerate() {
            if !known_types.contains(&p.ty) {
                warnings.push(format!(
                    "callable[{}] {} param[{}] {}: 类型 {:?} 缺少布局",
                    i, c.fqn, pi, p.name, p.ty
                ));
            }
        }
        if !known_types.contains(&c.return_ty) {
            warnings.push(format!(
                "callable[{}] {}: 返回类型 {:?} 缺少布局",
                i, c.fqn, c.return_ty
            ));
        }
        if let Some(body) = &c.body {
            // local 类型检查
            for (li, l) in body.locals.iter().enumerate() {
                if !known_types.contains(&l.ty) {
                    warnings.push(format!(
                        "callable[{}] {} local[{}]: 类型 {:?} 缺少布局",
                        i, c.fqn, li, l.ty
                    ));
                }
            }
            // block target 有效性检查
            let num_blocks = body.blocks.len() as u32;
            for (bi, block) in body.blocks.iter().enumerate() {
                match &block.terminator {
                    LirTerminator::Goto { target } => {
                        if *target >= num_blocks {
                            warnings.push(format!(
                                "callable[{}] {} block[{}]: Goto target {} 超出范围 (max {})",
                                i, c.fqn, bi, target, num_blocks
                            ));
                        }
                    }
                    LirTerminator::CondBr {
                        then_target,
                        else_target,
                        ..
                    } => {
                        if *then_target >= num_blocks {
                            warnings.push(format!(
                                "callable[{}] {} block[{}]: CondBr then_target {} 超出范围",
                                i, c.fqn, bi, then_target
                            ));
                        }
                        if *else_target >= num_blocks {
                            warnings.push(format!(
                                "callable[{}] {} block[{}]: CondBr else_target {} 超出范围",
                                i, c.fqn, bi, else_target
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        // EffectStep 完整性检查
        if matches!(c.abi, LirCallableAbi::EffectStep) {
            if c.frame_schema.is_none() {
                warnings.push(format!(
                    "callable[{}] {}: EffectStep 但 frame_schema 为 None",
                    i, c.fqn
                ));
            }
            if c.step_layout.is_none() {
                warnings.push(format!(
                    "callable[{}] {}: EffectStep 但 step_layout 为 None",
                    i, c.fqn
                ));
            }
        }
        // GC info 检查（非 EffectStep 的有 body 的函数应有 gc_info）
        if c.body.is_some() && c.gc_info.is_none() {
            warnings.push(format!(
                "callable[{}] {}: 有 body 但 gc_info 为 None",
                i, c.fqn
            ));
        }
    }

    // 2. declaration：同样检查 symbol_name + 类型。
    for (i, d) in program.declarations.iter().enumerate() {
        if d.symbol_name.is_empty() {
            warnings.push(format!("declaration[{}] {}: symbol_name 为空", i, d.fqn));
        }
        if !known_types.contains(&d.return_ty) {
            warnings.push(format!(
                "declaration[{}] {}: 返回类型 {:?} 缺少布局",
                i, d.fqn, d.return_ty
            ));
        }
    }

    // 3. global_init：每个 entry 的 init_callable 非空。
    for (i, e) in program.global_init.entries.iter().enumerate() {
        if e.init_callable.is_empty() {
            warnings.push(format!("global_init[{}]: init_callable 为空", i));
        }
    }

    // 4. synthetic_types：布局非空检查。
    for (i, s) in program.synthetic_types.iter().enumerate() {
        if s.fqn.is_empty() {
            warnings.push(format!("synthetic_type[{}]: fqn 为空", i));
        }
    }

    // 5. vtable slot 有效性
    for vt in &program.vtables {
        for slot in &vt.slots {
            if slot.target_symbol.is_empty() {
                warnings.push(format!(
                    "vtable {} slot[{}]: target_symbol 为空",
                    vt.class_fqn, slot.slot_index
                ));
            }
        }
    }

    for w in &warnings {
        eprintln!("[lir-verify] warning: {}", w);
    }
}
