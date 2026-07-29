//! 分发表生成：vtable / itable slot 分配。
//!
//! 从 `mir.backend_contracts` 读取 class_vtables / interfaces / class_itables，
//! 构建 `VtableLayout` / `ItableLayout` / `ClassItableLayout` 写入 LirProgram。

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_mir::mir::materialize::MaterializedMir;

use crate::*;

/// 主入口：生成分发表。
pub fn generate_dispatch_tables(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    _hir: &TypedHir,
    _interner: &Interner,
) {
    let contracts = &mir.backend_contracts;

    // 1. class vtable：每个 class 一个 VtableLayout，slot 按 virtual_methods 顺序分配。
    for cv in &contracts.class_vtables {
        let slots: Vec<VtableSlot> = cv
            .virtual_methods
            .iter()
            .enumerate()
            .map(|(i, (method_name, owner_fqn, overload_sig))| {
                let target_symbol = mangle_target_symbol(owner_fqn, method_name, overload_sig);
                VtableSlot {
                    slot_index: i as u32,
                    method_name: method_name.clone(),
                    owner_fqn: owner_fqn.clone(),
                    overload_sig: overload_sig.clone(),
                    target_symbol,
                }
            })
            .collect();
        program.vtables.push(VtableLayout {
            class_fqn: cv.class_fqn.clone(),
            slots,
        });
    }

    // 2. interface itable：每个 interface 一个 ItableLayout，全局唯一 interface_id。
    for (idx, ic) in contracts.interfaces.iter().enumerate() {
        let interface_id = stable_interface_id(&ic.interface_fqn, idx);
        let slots: Vec<ItableSlot> = ic
            .methods
            .iter()
            .enumerate()
            .map(|(i, (method_name, overload_sig))| ItableSlot {
                slot_index: i as u32,
                method_name: method_name.clone(),
                overload_sig: overload_sig.clone(),
            })
            .collect();
        program.itables.push(ItableLayout {
            interface_fqn: ic.interface_fqn.clone(),
            interface_id,
            slots,
        });
    }

    // 3. class × interface itable：每个 (class, interface) 一个 ClassItableLayout。
    for ci in &contracts.class_itables {
        for (iface_idx, iface_fqn) in ci.interface_fqns.iter().enumerate() {
            // 在已生成的 itables 中找该 interface 的定义。找到则复用其 interface_id；
            // 未发布（外部接口）则用 FQN hash 作 id。
            let interface_id = program
                .itables
                .iter()
                .find(|il| &il.interface_fqn == iface_fqn)
                .map(|il| il.interface_id)
                .unwrap_or_else(|| stable_interface_id(iface_fqn, iface_idx));
            // 构造每个 slot 的实现符号：owner 是该 class，方法名/sig 来自 itable 定义。
            let method_impls: Vec<Option<String>> = program
                .itables
                .iter()
                .find(|il| &il.interface_fqn == iface_fqn)
                .map(|il| {
                    il.slots
                        .iter()
                        .map(|slot| {
                            Some(mangle_target_symbol(
                                &ci.class_fqn,
                                &slot.method_name,
                                &slot.overload_sig,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            program.class_itables.push(ClassItableLayout {
                class_fqn: ci.class_fqn.clone(),
                interface_fqn: iface_fqn.clone(),
                interface_id,
                method_impls,
            });
        }
    }
}

/// 简单的 vtable/itable 目标符号 mangle：`<owner>.<method>` 中的点替换为下划线，
/// overload 签名附加在后（把 `,` / `<` / `>` 等对符号名不友好的字符也替换为下划线）。
/// 空 overload 签名时仅用 `<owner>_<method>`，保持与无重载方法的旧 mangle 兼容。
fn mangle_target_symbol(owner_fqn: &str, method_name: &str, overload_sig: &str) -> String {
    let owner = owner_fqn.replace('.', "_");
    let method = method_name.replace('.', "_");
    if overload_sig.is_empty() {
        format!("{}_{}", owner, method)
    } else {
        let sig = overload_sig
            .replace(',', "_")
            .replace('<', "_")
            .replace('>', "_");
        format!("{}_{}_{}", owner, method, sig)
    }
}

/// 回填 vtable/itable slot 信息到调用点。
/// 在 map_bodies 之后运行，遍历所有 callable 的 body，把 Virtual/Interface 调用
/// 的 slot_index 从 dispatch 表中查找并填入。
pub fn backfill_call_sites(program: &mut LirProgram) {
    // 构建 (class_fqn, method_name) → vtable_slot_index 查找表。
    // 早期实现只按 method_name 索引，会在多个 class 含同名方法时互相覆盖、
    // 导致 vtable_slot 错位。现在 LirCallKind::Virtual 携带 owner_fqn，可按
    // 声明该虚方法的类精确查找 slot。
    let mut vtable_slots: std::collections::HashMap<(String, String), u32> =
        std::collections::HashMap::new();
    for vt in &program.vtables {
        for slot in &vt.slots {
            vtable_slots.insert(
                (vt.class_fqn.clone(), slot.method_name.clone()),
                slot.slot_index,
            );
        }
    }
    let mut itable_slots: std::collections::HashMap<(String, String), (u64, u32)> =
        std::collections::HashMap::new();
    for il in &program.itables {
        for slot in &il.slots {
            itable_slots.insert(
                (il.interface_fqn.clone(), slot.method_name.clone()),
                (il.interface_id, slot.slot_index),
            );
        }
    }

    // 遍历所有 callable 的 body，回填调用点。
    for callable in &mut program.callables {
        if let Some(ref mut body) = callable.body {
            for block in &mut body.blocks {
                for stmt in &mut block.stmts {
                    if let LirStmtKind::Assign { value, .. } = &mut stmt.kind {
                        if let LirRvalue::Call(call) = value {
                            match &mut call.kind {
                                LirCallKind::Virtual {
                                    owner_fqn,
                                    method_name,
                                    vtable_slot,
                                    ..
                                } => {
                                    if let Some(&slot) =
                                        vtable_slots.get(&(owner_fqn.clone(), method_name.clone()))
                                    {
                                        *vtable_slot = slot;
                                    }
                                }
                                LirCallKind::Interface {
                                    interface_fqn,
                                    method_name,
                                    interface_id,
                                    itable_slot,
                                    ..
                                } => {
                                    if let Some(&(iid, slot)) = itable_slots
                                        .get(&(interface_fqn.clone(), method_name.clone()))
                                    {
                                        *interface_id = iid;
                                        *itable_slot = slot;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // field_offset 已在 map_stmt 中通过 compute_field_offset 计算，此处不覆写。
                }
            }
        }
    }
}

/// 计算 interface 的全局唯一 id：FQN 的稳定哈希 + 在 contracts 中的下标。
fn stable_interface_id(fqn: &str, idx: usize) -> u64 {
    // 简单稳定哈希：FNV-1a 64 位 + 下标。
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in fqn.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash.wrapping_add(idx as u64)
}
