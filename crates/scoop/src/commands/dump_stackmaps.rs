//! `scoop dump-stackmaps` 子命令。
//!
//! 用途（GC-FIX Phase E2）：
//! - 从链接产物（可执行文件优先；也兼容 `.o`）中定位 LLVM stackmap section；
//! - 输出稳定、可用于 fixtures/CI 断言的 header 摘要（records 数量等）；
//! - （可选）输出每条 record 的 roots slot 明细：function/offset/patchpoint_id + roots locations；
//! - （可选）校验 roots “可写回 slot”契约（纯 stackmap GC 的强不变量，违反时应 fail-fast）。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use object::{Architecture, Object as _, ObjectSection as _, ObjectSymbol as _, SymbolKind};

pub fn run(input: PathBuf, verify_roots: bool, dump_records: bool) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let bytes = std::fs::read(&input)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取输入文件失败：{}", input.display()))?;

    let obj = object::File::parse(bytes.as_slice())
        .into_diagnostic()
        .wrap_err("解析二进制文件失败（object::File::parse）")?;

    let (section_name, section_bytes) = find_stackmaps_section(&obj).wrap_err_with(|| {
        format!(
            "未找到 stackmap section（期望 `.llvm_stackmaps` / `__llvm_stackmaps`）：{}",
            input.display()
        )
    })?;

    let header = scoopc::stackmap::StackMapHeader::parse(section_bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("解析 stackmap header 失败（section: {section_name}）"))?;

    // 输出格式约定（稳定，供 fixtures/CI 断言）：
    // - 必须包含 `records: <n>` 行；
    // - 保留其它字段以便排查（但不强制断言）。
    println!("stackmaps:");
    println!("section: {section_name}");
    println!("version: {}", header.version);
    println!("functions: {}", header.num_functions);
    println!("constants: {}", header.num_constants);
    println!("records: {}", header.num_records);

    if verify_roots || dump_records {
        let section = scoopc::stackmap::StackMapSection::parse(section_bytes)
            .into_diagnostic()
            .wrap_err("解析 stackmap section 失败（StackMapSection::parse）")?;

        let symbols = collect_text_symbols(&obj);
        let cfg = roots_contract_config_from_arch(obj.architecture())?;
        if verify_roots {
            section
                .verify_roots_contract(cfg)
                .into_diagnostic()
                .wrap_err("stackmap roots 契约校验失败（--verify-roots）")?;
            println!("verify-roots: ok");
        }

        dump_record_root_slots(&section, &symbols, cfg);

        if dump_records {
            dump_records_locations(&section, &symbols, cfg);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TextSymbol {
    addr: u64,
    name: String,
}

fn collect_text_symbols(obj: &object::File<'_>) -> Vec<TextSymbol> {
    // 说明：
    // - 并非所有产物都会包含可用符号（可能被 strip）；这不是错误；
    // - 这里尽力收集 text symbols，用于把 `function_address` 映射到“更可读”的函数名。
    let mut out = Vec::new();
    for sym in obj.symbols().chain(obj.dynamic_symbols()) {
        if sym.kind() != SymbolKind::Text {
            continue;
        }
        let addr = sym.address();
        if addr == 0 {
            continue;
        }
        let Ok(name) = sym.name() else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }
        out.push(TextSymbol {
            addr,
            name: name.to_string(),
        });
    }
    out.sort_by_key(|s| s.addr);
    out
}

fn symbol_name_by_addr(symbols: &[TextSymbol], addr: u64) -> Option<&str> {
    let idx = symbols.binary_search_by_key(&addr, |s| s.addr).ok()?;
    Some(symbols[idx].name.as_str())
}

fn format_function_label(symbols: &[TextSymbol], function_address: u64) -> String {
    if let Some(name) = symbol_name_by_addr(symbols, function_address) {
        format!("{name} (0x{function_address:x})")
    } else {
        format!("0x{function_address:x}")
    }
}

fn format_base_reg_name(
    cfg: scoopc::stackmap::StackMapRootsContractConfig,
    dwarf_reg: u16,
) -> String {
    if dwarf_reg == cfg.sp_dwarf_reg {
        return format!("SP({dwarf_reg})");
    }
    if cfg.fp_dwarf_reg.is_some_and(|fp| fp == dwarf_reg) {
        return format!("FP({dwarf_reg})");
    }
    format!("reg({dwarf_reg})")
}

fn format_i32_signed_hex(v: i32) -> String {
    if v >= 0 {
        format!("+0x{:x}", v as u32)
    } else {
        let abs = v.unsigned_abs();
        format!("-0x{abs:x}")
    }
}

fn is_root_slot_location(
    loc: scoopc::stackmap::StackMapLocation,
    cfg: scoopc::stackmap::StackMapRootsContractConfig,
) -> bool {
    matches!(
        loc.kind,
        scoopc::stackmap::StackMapLocationKind::Direct
            | scoopc::stackmap::StackMapLocationKind::Indirect
    ) && loc.size == cfg.pointer_size
        && (loc.dwarf_reg == cfg.sp_dwarf_reg
            || cfg.fp_dwarf_reg.is_some_and(|fp| fp == loc.dwarf_reg))
}

fn roots_suffix_start(
    rec: &scoopc::stackmap::StackMapRecord,
    cfg: scoopc::stackmap::StackMapRootsContractConfig,
) -> usize {
    let mut i = rec.locations.len();
    while i > 0 {
        let idx = i - 1;
        if is_root_slot_location(rec.locations[idx], cfg) {
            i -= 1;
            continue;
        }
        break;
    }
    i
}

fn dump_record_root_slots(
    section: &scoopc::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: scoopc::stackmap::StackMapRootsContractConfig,
) {
    println!();
    println!("root-slots:");
    println!(
        "config: ptr={} sp={} fp={}",
        cfg.pointer_size,
        cfg.sp_dwarf_reg,
        cfg.fp_dwarf_reg
            .map_or("none".to_string(), |v| v.to_string())
    );

    for (record_index, rec) in section.records.iter().enumerate() {
        let ra = rec
            .function_address
            .saturating_add(rec.instruction_offset as u64);
        let roots_start = roots_suffix_start(rec, cfg);
        let roots_len = rec.locations.len().saturating_sub(roots_start);

        let func = format_function_label(symbols, rec.function_address);
        println!(
            "- record[{record_index}] func={func} inst_off=0x{:x} ra=0x{ra:x} patchpoint_id=0x{:x} roots={roots_len}",
            rec.instruction_offset, rec.patchpoint_id
        );

        if roots_len == 0 {
            continue;
        }

        // roots 契约规定 roots locations 成对出现（base/derived）；这里按顺序 2-by-2 打印。
        for pair in 0..(roots_len / 2) {
            let base_i = roots_start + pair * 2;
            let derived_i = base_i + 1;

            let base = rec.locations[base_i];
            let derived = rec.locations[derived_i];

            println!(
                "  pair[{pair}] base loc[{base_i}] kind={:?} base={} off={:+}({}) size={}",
                base.kind,
                format_base_reg_name(cfg, base.dwarf_reg),
                base.offset,
                format_i32_signed_hex(base.offset),
                base.size
            );
            println!(
                "  pair[{pair}] derived loc[{derived_i}] kind={:?} base={} off={:+}({}) size={}",
                derived.kind,
                format_base_reg_name(cfg, derived.dwarf_reg),
                derived.offset,
                format_i32_signed_hex(derived.offset),
                derived.size
            );
        }
    }
}

fn dump_records_locations(
    section: &scoopc::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: scoopc::stackmap::StackMapRootsContractConfig,
) {
    println!();
    println!("records-detail:");
    for (record_index, rec) in section.records.iter().enumerate() {
        let ra = rec
            .function_address
            .saturating_add(rec.instruction_offset as u64);
        let roots_start = roots_suffix_start(rec, cfg);
        let roots_len = rec.locations.len().saturating_sub(roots_start);
        let func = format_function_label(symbols, rec.function_address);

        println!(
            "- record[{record_index}] func={func} inst_off=0x{:x} ra=0x{ra:x} patchpoint_id=0x{:x} locs={} roots_start={roots_start} roots={roots_len}",
            rec.instruction_offset,
            rec.patchpoint_id,
            rec.locations.len()
        );
        for (loc_index, loc) in rec.locations.iter().enumerate() {
            let role = if loc_index >= roots_start {
                "root"
            } else {
                "meta"
            };
            let writable = if is_root_slot_location(*loc, cfg) {
                "writable"
            } else {
                "non-writable"
            };
            println!(
                "  loc[{loc_index}] role={role} {writable} kind={:?} size={} base={} off={:+}({})",
                loc.kind,
                loc.size,
                format_base_reg_name(cfg, loc.dwarf_reg),
                loc.offset,
                format_i32_signed_hex(loc.offset),
            );
        }
    }
}

/// 在 object file（可执行文件/`.o`）中查找 stackmap section 并返回其名称与字节内容。
///
/// 说明：
/// - ELF：通常为 `.llvm_stackmaps`
/// - Mach-O：通常为 `__llvm_stackmaps`（segment `__LLVM_STACKMAPS`）
fn find_stackmaps_section<'data>(obj: &object::File<'data>) -> Result<(&'data str, &'data [u8])> {
    for section in obj.sections() {
        let name = section.name().ok();
        let Some(name) = name else { continue };
        if !is_stackmaps_section_name(name) {
            continue;
        }

        let data = section
            .data()
            .into_diagnostic()
            .wrap_err_with(|| format!("读取 section 数据失败：{name}"))?;
        return Ok((name, data));
    }

    Err(miette::miette!("stackmap section not found"))
}

fn is_stackmaps_section_name(name: &str) -> bool {
    // 允许未来扩展：我们只关心 `llvm_stackmaps` 这一后缀即可。
    name == ".llvm_stackmaps" || name == "__llvm_stackmaps" || name.ends_with("llvm_stackmaps")
}

fn roots_contract_config_from_arch(
    arch: Architecture,
) -> Result<scoopc::stackmap::StackMapRootsContractConfig> {
    // 与 `runtime/c/scoop_stackmap.c` 中的 DWARF reg 编号约定保持一致。
    //
    // 说明：当前 `scoop --features llvm` 默认按 host 目标编译，因此这里以输入文件的 arch 为准。
    match arch {
        Architecture::Aarch64 => Ok(scoopc::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 31,
            fp_dwarf_reg: Some(29),
        }),
        Architecture::X86_64 => Ok(scoopc::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 7,
            fp_dwarf_reg: Some(6),
        }),
        other => Err(miette::miette!(
            "暂不支持的目标架构：{other:?}（--verify-roots 目前仅支持 aarch64/x86_64）"
        )),
    }
}
