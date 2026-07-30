//! LIR 文本导出：调试用的可读表示。

use crate::*;

/// 把 LirProgram 渲染为调试用文本。
pub fn dump_program(program: &LirProgram) -> String {
    let mut out = String::new();
    out.push_str("=== LirProgram ===\n");

    // 类型布局。
    out.push_str(&format!(
        "-- type_layouts ({} entries) --\n",
        program.type_layouts.entries.len()
    ));
    let mut ids: Vec<_> = program.type_layouts.entries.iter().collect();
    ids.sort_by_key(|(t, _)| t.0);
    for (ty, layout) in &ids {
        out.push_str(&format!(
            "  {:?}: size={} align={} kind={}\n",
            ty,
            layout.size,
            layout.align,
            format_layout_kind(&layout.kind)
        ));
    }

    // Callable。
    out.push_str(&format!("-- callables ({} ) --\n", program.callables.len()));
    for c in &program.callables {
        out.push_str(&format!("  {} -> {}\n", c.fqn, c.symbol_name));
        out.push_str(&format!("    abi: {:?}\n", c.abi));
        for p in &c.params {
            out.push_str(&format!("    param {}: {:?} ({:?})\n", p.name, p.ty, p.abi));
        }
        out.push_str(&format!(
            "    return: {:?} ({:?})\n",
            c.return_ty, c.return_abi
        ));
        if let Some(body) = &c.body {
            out.push_str(&format!(
                "    body: {} locals, {} blocks, start=bb{}\n",
                body.locals.len(),
                body.blocks.len(),
                body.start_block
            ));
            for l in &body.locals {
                out.push_str(&format!(
                    "    local {} {}{}: {:?} (gc={})\n",
                    l.id,
                    l.name.as_deref().unwrap_or(""),
                    if l.mutable { " mut" } else { "" },
                    l.ty,
                    l.gc_traceable
                ));
            }
            for b in &body.blocks {
                out.push_str(&format!("    bb{}:\n", b.id));
                for s in &b.stmts {
                    out.push_str(&format!("      {:?}\n", s.kind));
                }
                out.push_str(&format!("      -> {:?}\n", b.terminator));
            }
        }
    }

    // Declarations。
    out.push_str(&format!(
        "-- declarations ({}) --\n",
        program.declarations.len()
    ));
    for d in &program.declarations {
        out.push_str(&format!(
            "  {} -> {} (extern={}, sym={:?})\n",
            d.fqn, d.symbol_name, d.is_extern, d.extern_symbol
        ));
    }

    // vtable / itable。
    out.push_str(&format!("-- vtables ({}) --\n", program.vtables.len()));
    for v in &program.vtables {
        out.push_str(&format!("  {}: {} slots\n", v.class_fqn, v.slots.len()));
        for s in &v.slots {
            out.push_str(&format!(
                "    [{}] {} (owner={}) -> {}\n",
                s.slot_index, s.method_name, s.owner_fqn, s.target_symbol
            ));
        }
    }
    out.push_str(&format!("-- itables ({}) --\n", program.itables.len()));
    for it in &program.itables {
        out.push_str(&format!(
            "  {} (id={}): {} slots\n",
            it.interface_fqn,
            it.interface_id,
            it.slots.len()
        ));
    }
    out.push_str(&format!(
        "-- class_itables ({}) --\n",
        program.class_itables.len()
    ));
    for ci in &program.class_itables {
        out.push_str(&format!(
            "  {} implements {} (id={}): {} impls\n",
            ci.class_fqn,
            ci.interface_fqn,
            ci.interface_id,
            ci.method_impls.len()
        ));
    }

    // GC type descriptors。
    out.push_str(&format!(
        "-- type_descriptors ({}) --\n",
        program.type_descriptors.len()
    ));
    for td in &program.type_descriptors {
        // 渲染 trace_offsets 的实际偏移值（而非仅计数），便于核对 GC 描述符。
        let offsets_str = if td.trace_offsets.is_empty() {
            "[]".to_string()
        } else {
            format!(
                "[{}]",
                td.trace_offsets
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        out.push_str(&format!(
            "  {} (id={}): size={} align={} trace_offsets={} parent={:?} release={:?}\n",
            td.type_fqn,
            td.type_id,
            td.size,
            td.align,
            offsets_str,
            td.parent_type_id,
            td.release_fn,
        ));
    }

    // Global init。
    out.push_str(&format!(
        "-- global_init ({} entries) --\n",
        program.global_init.entries.len()
    ));
    for e in &program.global_init.entries {
        out.push_str(&format!(
            "  {} ({:?}): init={}\n",
            e.fqn, e.ty, e.init_callable
        ));
    }

    // Synthetic types。
    out.push_str(&format!(
        "-- synthetic_types ({}) --\n",
        program.synthetic_types.len()
    ));
    for s in &program.synthetic_types {
        out.push_str(&format!(
            "  {:?} {}: size={} align={}\n",
            s.kind, s.fqn, s.layout.size, s.layout.align
        ));
    }

    // Closure layouts。
    out.push_str(&format!(
        "-- closure_layouts ({}) --\n",
        program.closure_layouts.len()
    ));
    for cl in &program.closure_layouts {
        out.push_str(&format!(
            "  {}: env_size={} env_align={} captures={}\n",
            cl.invoke_fqn,
            cl.env_size,
            cl.env_align,
            cl.captures.len()
        ));
    }

    // Class inits。
    out.push_str(&format!(
        "-- class_inits ({}) --\n",
        program.class_inits.len()
    ));
    for ci in &program.class_inits {
        out.push_str(&format!(
            "  {}: fields={} super={:?}\n",
            ci.class_fqn,
            ci.field_inits.len(),
            ci.super_init
        ));
    }

    // GC info (per callable)。
    for c in &program.callables {
        if let Some(gc) = &c.gc_info {
            out.push_str(&format!(
                "-- gc_info for {} ({} gc_locals, {} safepoints) --\n",
                c.fqn,
                gc.gc_locals.len(),
                gc.safepoints.len()
            ));
            for sp in &gc.safepoints {
                out.push_str(&format!(
                    "  safepoint bb{} stmt{} {:?}: {} live roots\n",
                    sp.block_id,
                    sp.stmt_index,
                    sp.kind,
                    sp.live_gc_locals.len()
                ));
            }
        }
        if let Some(fs) = &c.frame_schema {
            out.push_str(&format!(
                "-- frame_schema for {} ({} slots) --\n",
                c.fqn,
                fs.slots.len()
            ));
        }
        if let Some(sl) = &c.step_layout {
            out.push_str(&format!(
                "-- step_layout for {} ({} variants) --\n",
                c.fqn,
                sl.effect_variants.len() + 1
            ));
        }
        if let Some(cl) = &c.continuation_layout {
            out.push_str(&format!(
                "-- continuation_layout for {} ({} fields) --\n",
                c.fqn,
                cl.fields.len()
            ));
            for f in &cl.fields {
                out.push_str(&format!(
                    "    {} off={} ty={:?} kind={:?}\n",
                    f.name, f.offset, f.ty, f.kind
                ));
            }
        }
    }

    out
}

/// 渲染 TypeLayoutKind 的简短文本。
fn format_layout_kind(kind: &TypeLayoutKind) -> String {
    match kind {
        TypeLayoutKind::Scalar { scalar_kind } => format!("Scalar({:?})", scalar_kind),
        TypeLayoutKind::Struct { fields } => format!("Struct({} fields)", fields.len()),
        TypeLayoutKind::Tuple { elements } => format!("Tuple({} elements)", elements.len()),
        TypeLayoutKind::Option {
            storage,
            payload_size,
            ..
        } => {
            format!("Option({:?}, payload={})", storage, payload_size)
        }
        TypeLayoutKind::Enum {
            tag_size,
            tag_offset,
            variants,
            ..
        } => format!(
            "Enum(tag={}, off={}, {} variants)",
            tag_size,
            tag_offset,
            variants.len()
        ),
        TypeLayoutKind::Reference {
            gc_traceable,
            ref_kind,
        } => format!("Reference(gc={}, {:?})", gc_traceable, ref_kind),
        TypeLayoutKind::Function => "Function".to_string(),
        TypeLayoutKind::Nothing => "Nothing".to_string(),
    }
}
