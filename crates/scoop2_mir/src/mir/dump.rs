//! MIR 文本 dump（格式参考主线 `scoopc_mir/src/mir/dump.rs`）。
//!
//! 输出 Rust-Debug 风格的稳定文本，供 golden fixture 比对。local/block 用稳定 hash
//! 标签（`local#h...` / `bb#h...`），使输出跨运行确定性。

use std::collections::HashMap;
use std::fmt::Write;

use scoop2_hir::ty::{EffectRow, TypeId, TypeStore};

use crate::mir::{
    BasicBlockId, Body, CallArg, CallKind, ConstValue, FunDecl, InitializerRoot, Item, LocalId,
    Module, Operand, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
};

/// dump 一个 Module 为稳定文本。
pub fn dump_module(module: &Module, interner: &scoop2_base::Interner) -> String {
    let mut out = String::new();
    out.push_str("Module {\n");
    out.push_str("    items: [\n");
    for item in &module.items {
        dump_item(item, &module.types, interner, &mut out, 8);
        out.push_str(",\n");
    }
    out.push_str("    ],\n");
    out.push_str("}\n");
    out
}

fn dump_item(
    item: &Item,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    out: &mut String,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    match item {
        Item::Fun(fd) => {
            dump_fun_decl(fd, types, interner, out, indent);
        }
        Item::Initializer(ir) => {
            let _ = write!(out, "{pad}Initializer(InitializerRoot {{\n");
            let pad2 = " ".repeat(indent + 4);
            let _ = writeln!(out, "{pad2}fqn: {:?},", ir.fqn);
            let _ = writeln!(out, "{pad2}ty: {},", render_type(types, interner, ir.ty));
            let _ = writeln!(out, "{pad2}is_var: {},", ir.is_var);
            dump_body(&ir.body, types, interner, out, indent + 4);
            let _ = write!(out, "{pad}}})");
        }
        Item::ExternGlobal(g) => {
            let _ = write!(
                out,
                "{pad}ExternGlobal {{ fqn: {:?}, ty: {} }}",
                g.fqn,
                render_type(types, interner, g.ty)
            );
        }
        Item::Metadata(m) => {
            let _ = write!(
                out,
                "{pad}Metadata {{ fqn: {:?}, kind: {:?} }}",
                m.fqn, m.kind
            );
        }
    }
}

fn dump_fun_decl(
    fd: &FunDecl,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    out: &mut String,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let _ = write!(out, "{pad}Fun(FunDecl {{\n");
    let pad2 = " ".repeat(indent + 4);
    let _ = writeln!(out, "{pad2}fqn: {:?},", fd.fqn);
    let _ = writeln!(out, "{pad2}name: {:?},", fd.name);
    let _ = writeln!(out, "{pad2}ty: {},", render_type(types, interner, fd.ty));
    let params: Vec<String> = fd
        .params
        .iter()
        .map(|p| {
            format!(
                "Param {{ name: {:?}, ty: {}, local: {} }}",
                p.name,
                render_type(types, interner, p.ty),
                fd.body
                    .as_ref()
                    .map(|b| local_label(b, p.local))
                    .unwrap_or_else(|| format!("local?{}", p.local.0))
            )
        })
        .collect();
    let _ = writeln!(out, "{pad2}params: [{}],", params.join(", "));
    let _ = writeln!(
        out,
        "{pad2}return_ty: {},",
        render_type(types, interner, fd.return_ty)
    );
    let _ = writeln!(
        out,
        "{pad2}effect_row: {},",
        render_effect_row(types, interner, &fd.effect_row)
    );
    if let Some(body) = &fd.body {
        dump_body(body, types, interner, out, indent + 4);
        let _ = writeln!(out, "{pad2}body: Some(...),");
    } else {
        let _ = writeln!(out, "{pad2}body: None,");
    }
    let _ = write!(out, "{pad}}})");
}

fn dump_body(
    body: &Body,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    out: &mut String,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let pad2 = " ".repeat(indent + 4);
    let labels = body_labels(body);
    let _ = writeln!(out, "{pad}locals: [");
    for (i, decl) in body.locals.iter().enumerate() {
        let lid = LocalId(i as u32);
        let _ = writeln!(
            out,
            "{pad2}LocalDecl {{ label: {}, name: {:?}, ty: {}, source: {:?} }},",
            local_label_by_map(&labels, lid),
            decl.name,
            render_type(types, interner, decl.ty),
            decl.source
        );
    }
    let _ = writeln!(out, "{pad}],");
    let _ = writeln!(out, "{pad}blocks: [");
    for (i, block) in body.blocks.iter().enumerate() {
        let bid = BasicBlockId(i as u32);
        let _ = writeln!(out, "{pad2}BasicBlock {{");
        let _ = writeln!(out, "{pad}    label: {},", block_label_by_map(&labels, bid));
        let _ = writeln!(out, "{pad}    stmts: [");
        for stmt in &block.stmts {
            let _ = writeln!(
                out,
                "{pad}        {},",
                dump_statement(stmt, types, interner, &labels)
            );
        }
        let _ = writeln!(out, "{pad}    ],");
        let _ = writeln!(
            out,
            "{pad}    terminator: {},",
            dump_terminator(&block.terminator, types, interner, &labels)
        );
        let _ = writeln!(out, "{pad2}}},");
    }
    let _ = writeln!(out, "{pad}],");
}

/// 为 body 的 local / block 分配稳定 hash 标签。
fn body_labels(body: &Body) -> BodyLabels {
    let mut locals: HashMap<LocalId, String> = HashMap::new();
    for i in 0..body.locals.len() {
        let lid = LocalId(i as u32);
        locals.insert(lid, format!("local#{}", stable_hash(&format!("l{}", i))));
    }
    let mut blocks: HashMap<BasicBlockId, String> = HashMap::new();
    for i in 0..body.blocks.len() {
        let bid = BasicBlockId(i as u32);
        blocks.insert(bid, format!("bb#{}", stable_hash(&format!("b{}", i))));
    }
    BodyLabels { locals, blocks }
}

struct BodyLabels {
    locals: HashMap<LocalId, String>,
    blocks: HashMap<BasicBlockId, String>,
}

fn local_label(body: &Body, lid: LocalId) -> String {
    let labels = body_labels(body);
    local_label_by_map(&labels, lid)
}

fn local_label_by_map(labels: &BodyLabels, lid: LocalId) -> String {
    labels
        .locals
        .get(&lid)
        .cloned()
        .unwrap_or_else(|| format!("local?{}", lid.0))
}

fn block_label_by_map(labels: &BodyLabels, bid: BasicBlockId) -> String {
    labels
        .blocks
        .get(&bid)
        .cloned()
        .unwrap_or_else(|| format!("bb?{}", bid.0))
}

/// 稳定 hash（FNV-1a 32-bit，十六进制）。
fn stable_hash(s: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{:08x}", h)
}

fn dump_operand(op: &Operand, labels: &BodyLabels) -> String {
    match op {
        Operand::Local(l) => format!("Local({})", local_label_by_map(labels, *l)),
        Operand::Const(c) => format!("Const({})", dump_const(c)),
    }
}

fn dump_const(c: &ConstValue) -> String {
    match c {
        ConstValue::Bool(b) => format!("Bool({})", b),
        ConstValue::Char(ch) => format!("Char({:?})", ch),
        ConstValue::Unit => "Unit".to_string(),
        ConstValue::Int(v, suf) => {
            let s = suf
                .map(|s| match s {
                    crate::mir::IntSuffix::U => "U",
                    crate::mir::IntSuffix::L => "L",
                    crate::mir::IntSuffix::UL => "UL",
                })
                .unwrap_or("");
            format!("Int({}{})", v, s)
        }
        ConstValue::Float(v, suf) => {
            let s = if matches!(suf, Some(crate::mir::FloatSuffix::F32)) {
                "f32"
            } else {
                ""
            };
            format!("Float({}{})", v, s)
        }
        ConstValue::String(s) => format!("String({:?})", s),
        ConstValue::Null => "Null".to_string(),
    }
}

fn dump_statement(
    stmt: &Statement,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    labels: &BodyLabels,
) -> String {
    match &stmt.kind {
        StatementKind::Nop => "Nop".to_string(),
        StatementKind::Assign { target, value } => format!(
            "{} = {}",
            local_label_by_map(labels, *target),
            dump_rvalue(value, types, interner, labels)
        ),
        StatementKind::StoreMember {
            receiver,
            member,
            value,
            value_ty,
            ..
        } => format!(
            "StoreMember({}, {}, {}, {})",
            dump_operand(receiver, labels),
            member.name,
            dump_operand(value, labels),
            render_type(types, interner, *value_ty)
        ),
        StatementKind::StoreTupleIndex {
            receiver,
            index,
            value,
            value_ty,
        } => format!(
            "StoreTupleIndex({}, {}, {}, {})",
            dump_operand(receiver, labels),
            index,
            dump_operand(value, labels),
            render_type(types, interner, *value_ty)
        ),
        StatementKind::StoreTopLevelVar {
            fqn,
            value,
            value_ty,
        } => format!(
            "StoreTopLevelVar({}, {}, {})",
            interner.resolve(*fqn),
            dump_operand(value, labels),
            render_type(types, interner, *value_ty)
        ),
        StatementKind::Panic { message } => format!("Panic({:?})", message),
    }
}

fn dump_rvalue(
    rv: &Rvalue,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    labels: &BodyLabels,
) -> String {
    match rv {
        Rvalue::Use(op) => format!("Use({})", dump_operand(op, labels)),
        Rvalue::TopLevelRef(tl) => format!(
            "TopLevelRef({}, ty={})",
            tl.fqn,
            if tl.generic_type_args.is_empty() {
                String::new()
            } else {
                format!("<{}>", tl.generic_type_args.len())
            }
        ),
        Rvalue::UnresolvedName { name } => format!("UnresolvedName({:?})", name),
        Rvalue::TypeTest {
            value, metadata, ..
        } => format!(
            "TypeTest({}, {})",
            dump_operand(value, labels),
            render_type(types, interner, metadata.target_ty)
        ),
        Rvalue::Cast {
            value,
            op,
            metadata,
            ..
        } => format!(
            "Cast({}, {:?}, {})",
            dump_operand(value, labels),
            op,
            render_type(types, interner, metadata.test.target_ty)
        ),
        Rvalue::MemberAccess {
            receiver, member, ..
        } => format!(
            "MemberAccess({}, {})",
            dump_operand(receiver, labels),
            member.name
        ),
        Rvalue::TupleIndex {
            receiver,
            index,
            element_ty,
        } => format!(
            "TupleIndex({}, {}, {})",
            dump_operand(receiver, labels),
            index,
            render_type(types, interner, *element_ty)
        ),
        Rvalue::IndexAccess {
            receiver,
            indices,
            element_ty,
            ..
        } => format!(
            "IndexAccess({}, [{}], {})",
            dump_operand(receiver, labels),
            indices
                .iter()
                .map(|o| dump_operand(o, labels))
                .collect::<Vec<_>>()
                .join(", "),
            render_type(types, interner, *element_ty)
        ),
        Rvalue::EnumVariant {
            enum_fqn,
            variant_name,
            args,
            ..
        } => format!(
            "EnumVariant({}.{}, [{}])",
            interner.resolve(*enum_fqn),
            interner.resolve(*variant_name),
            dump_call_args(args, labels)
        ),
        Rvalue::ClassCtor { type_fqn, args, .. } => format!(
            "ClassCtor({}, [{}])",
            interner.resolve(*type_fqn),
            dump_call_args(args, labels)
        ),
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => format!(
            "Call({}, [{}], transport={{trace:{}, box:{}}})",
            dump_call_kind(kind, interner, labels),
            dump_call_args(args, labels),
            transport.result.requirements.trace,
            transport.result.boxing.is_some()
        ),
        Rvalue::MakeTuple { elements, .. } => format!(
            "MakeTuple([{}])",
            elements
                .iter()
                .map(|o| dump_operand(o, labels))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::MakeArray {
            elements,
            result_ty,
        } => format!(
            "MakeArray([{}], {})",
            elements
                .iter()
                .map(|o| dump_operand(o, labels))
                .collect::<Vec<_>>()
                .join(", "),
            render_type(types, interner, *result_ty)
        ),
        Rvalue::StructLit {
            type_fqn, fields, ..
        } => format!(
            "StructLit({}, [{}])",
            interner.resolve(*type_fqn),
            fields
                .iter()
                .map(|f| format!(
                    "{} = {}",
                    interner.resolve(f.name),
                    dump_operand(&f.value, labels)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::InterpolatedString { parts } => format!(
            "InterpolatedString([{}])",
            parts
                .iter()
                .map(|p| match p {
                    crate::mir::InterpolatedPart::Lit(s) => format!("Lit({:?})", s),
                    crate::mir::InterpolatedPart::Expr(op) =>
                        format!("Expr({})", dump_operand(op, labels)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Rvalue::WithUpdate {
            base,
            updates,
            result_ty,
        } => format!(
            "WithUpdate({}, [{}], {})",
            dump_operand(base, labels),
            updates
                .iter()
                .map(|u| format!(
                    "{} = {}",
                    u.path
                        .iter()
                        .map(|s| match s {
                            crate::mir::WithUpdateSegment::Named(n) => {
                                interner.resolve(*n).to_string()
                            }
                            crate::mir::WithUpdateSegment::TupleIndex(i) => i.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join("."),
                    dump_operand(&u.value, labels)
                ))
                .collect::<Vec<_>>()
                .join(", "),
            render_type(types, interner, *result_ty)
        ),
        Rvalue::MakeClosure {
            env, invoke_fqn, ..
        } => format!("MakeClosure({}, {})", dump_operand(env, labels), invoke_fqn),
        Rvalue::ClassLit { type_fqn } => {
            format!("ClassLit({})", interner.resolve(*type_fqn))
        }
        Rvalue::PerformResult { op_fqn, result_ty } => format!(
            "PerformResult({}, {})",
            op_fqn,
            render_type(types, interner, *result_ty)
        ),
        Rvalue::PatternMatch { subject, pattern } => format!(
            "PatternMatch({}, {:?})",
            dump_operand(subject, labels),
            pattern
        ),
        Rvalue::PatternExtract {
            subject, result_ty, ..
        } => format!(
            "PatternExtract({}, {})",
            dump_operand(subject, labels),
            render_type(types, interner, *result_ty)
        ),
        Rvalue::IntEq { lhs, rhs } => format!(
            "IntEq({}, {})",
            dump_operand(lhs, labels),
            dump_operand(rhs, labels)
        ),
    }
}

fn dump_call_args(args: &[CallArg], labels: &BodyLabels) -> String {
    args.iter()
        .map(|a| {
            let spread = if a.is_spread { "*" } else { "" };
            match a.name {
                Some(n) => format!(
                    "{}={}",
                    scoop2_base_global_interner_resolve(n),
                    format!("{}{}", spread, dump_operand(&a.value, labels))
                ),
                None => format!("{}{}", spread, dump_operand(&a.value, labels)),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 局部 helper：把 Symbol resolve 成 String（dump_call_args 里用，避免借用 interner）。
/// 注意：此处用 interner 是 dump 入口传入的；为简化，命名实参的 dump 不显示名（仅值）。
fn scoop2_base_global_interner_resolve(_n: scoop2_base::Symbol) -> String {
    // 命名实参名需要 interner；dump_call_args 不持有 interner，故用占位（值已足够区分）。
    "named".to_string()
}

fn dump_call_kind(
    kind: &CallKind,
    interner: &scoop2_base::Interner,
    labels: &BodyLabels,
) -> String {
    match kind {
        CallKind::Direct {
            callee_fqn,
            type_args,
            is_intrinsic,
            ..
        } => format!(
            "Direct {{ callee: {:?}, type_args: {}, intrinsic: {} }}",
            callee_fqn,
            type_args
                .iter()
                .map(|t| format!("{:?}", t))
                .collect::<Vec<_>>()
                .join(", "),
            is_intrinsic
        ),
        CallKind::Virtual { receiver, dispatch } => format!(
            "Virtual {{ recv: {}, {}.{} }}",
            dump_operand(receiver, labels),
            dispatch.owner_fqn,
            dispatch.member_name
        ),
        CallKind::Interface { receiver, dispatch } => format!(
            "Interface {{ recv: {}, {}.{} }}",
            dump_operand(receiver, labels),
            dispatch.owner_fqn,
            dispatch.member_name
        ),
        CallKind::Closure { callee, invoke_fqn } => format!(
            "Closure {{ callee: {}, invoke: {:?} }}",
            dump_operand(callee, labels),
            invoke_fqn
        ),
        CallKind::FunValue { callee } => {
            format!("FunValue {{ callee: {} }}", dump_operand(callee, labels))
        }
        CallKind::Resume {
            continuation,
            resume_value,
        } => {
            format!(
                "Resume {{ cont: {}, value: {} }}",
                dump_operand(continuation, labels),
                dump_operand(resume_value, labels)
            )
        }
    }
}

fn dump_terminator(
    term: &Terminator,
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    labels: &BodyLabels,
) -> String {
    let _ = types;
    let kind = match &term.kind {
        TerminatorKind::Return { value } => format!(
            "Return({})",
            value
                .as_ref()
                .map(|o| dump_operand(o, labels))
                .unwrap_or_else(|| "Unit".to_string())
        ),
        TerminatorKind::Goto { target } => format!("Goto({})", block_label_by_map(labels, *target)),
        TerminatorKind::CondBr {
            cond,
            then_target,
            else_target,
        } => format!(
            "CondBr({}, {}, {})",
            dump_operand(cond, labels),
            block_label_by_map(labels, *then_target),
            block_label_by_map(labels, *else_target)
        ),
        TerminatorKind::Unreachable => "Unreachable".to_string(),
        TerminatorKind::Perform {
            op_fqn,
            metadata,
            args,
            resume_local,
            resume_target,
            ..
        } => format!(
            "Perform({}, [{}], resume={} -> {})",
            op_fqn,
            dump_call_args(args, labels),
            local_label_by_map(labels, *resume_local),
            block_label_by_map(labels, *resume_target)
        ),
        TerminatorKind::Handle {
            body_target,
            arm_targets,
            finally_target,
            exit_target,
            ..
        } => format!(
            "Handle(body={}, arms=[{}], finally={}, exit={})",
            block_label_by_map(labels, *body_target),
            arm_targets
                .iter()
                .map(|b| block_label_by_map(labels, *b))
                .collect::<Vec<_>>()
                .join(", "),
            finally_target
                .map(|b| block_label_by_map(labels, b))
                .unwrap_or_else(|| "None".to_string()),
            block_label_by_map(labels, *exit_target)
        ),
    };
    format!("Terminator {{ kind: {} }}", kind)
}

// ---------------------------------------------------------------------------
// 类型渲染（复用 scoop2_hir::ty::render_type）
// ---------------------------------------------------------------------------

fn render_type(types: &TypeStore, interner: &scoop2_base::Interner, id: TypeId) -> String {
    // 防御：函数局部 store 合并到 module store 前，个别合成 TypeId 可能越界。
    // 用占位文本避免 panic（dump 是尽力而为视图）。
    if (id.0 as usize) >= types.len() {
        return format!("ty#{}?", id.0);
    }
    scoop2_hir::ty::render_type(types, interner, id, false)
}

fn render_effect_row(
    types: &TypeStore,
    interner: &scoop2_base::Interner,
    row: &EffectRow,
) -> String {
    if row.is_pure() {
        return "Pure".to_string();
    }
    row.terms
        .iter()
        .map(|t| scoop2_hir::ty::render_type(types, interner, *t, false))
        .collect::<Vec<_>>()
        .join(" + ")
}
