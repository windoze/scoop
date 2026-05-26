//! Compiler-owned diagnostic/tooling commands exposed through the `scoopc` binary.

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use object::{Architecture, Object as _, ObjectSection as _, ObjectSymbol as _, SymbolKind};

use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitArtifactKind {
    LlvmIr,
    Object,
    Asm,
}

impl EmitArtifactKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "llvm-ir" | "llvm" | "ll" => Ok(Self::LlvmIr),
            "obj" | "object" => Ok(Self::Object),
            "asm" => Ok(Self::Asm),
            other => Err(miette::miette!(
                "未知 emit artifact kind `{other}`（期望 llvm-ir|obj|asm）"
            )),
        }
    }

    fn llvm_kind(self) -> crate::pipeline::LlvmArtifactKind {
        match self {
            EmitArtifactKind::LlvmIr => crate::pipeline::LlvmArtifactKind::LlvmIr,
            EmitArtifactKind::Object => crate::pipeline::LlvmArtifactKind::Object,
            EmitArtifactKind::Asm => crate::pipeline::LlvmArtifactKind::Asm,
        }
    }
}

pub fn run_emit_artifact(
    input: PathBuf,
    output: PathBuf,
    kind: EmitArtifactKind,
    opt_level: crate::opt::OptLevel,
    session_options: SessionOptions,
) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法创建输出目录：{}", parent.display()))?;
    }
    let session = Session::with_options(session_options.clone())?;
    let project = crate::frontend::load_project_input_from_path(&input, None, &session_options)?;
    let context = crate::frontend::ProjectContext::new(project);
    let front = crate::frontend::run_project_frontend(&session, context)?;
    crate::pipeline::emit_project_llvm_artifact_to_file(
        &session,
        &front,
        &output,
        opt_level,
        kind.llvm_kind(),
    )?;
    Ok(())
}

pub fn run_dump_ast(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let ast_output = crate::pipeline::load_ast_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    println!("{:#?}", ast_output.ast());
    Ok(())
}

pub fn run_dump_hir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_hir_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_mir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_direct_style_mir_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_ir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let materialized = crate::pipeline::materialize_direct_style_mir_for_dump(&session, &file)
        .map_err(|err| miette::Report::from(*err))?;
    print!("{}", materialized.stable_dump());
    Ok(())
}

pub fn run_dump_effect_facts(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_effect_facts_stage_output_for_dump(&session, &file)
        .map_err(|err| miette::miette!(err.to_string()))?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_effect_lowered(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_lir_stage_output_for_dump(&session, &file)
        .map_err(|err| miette::miette!(err.to_string()))?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_rtti(
    input: PathBuf,
    type_name: Option<String>,
    session_options: SessionOptions,
) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let dump = crate::rtti::type_desc::dump_file_type_desc(&session, &file)
        .map_err(miette::Report::from)?;

    if let Some(query) = type_name {
        if let Some(found) = dump.types.iter().find(|t| t.name == query) {
            println!("{}", serde_json::to_string_pretty(found).into_diagnostic()?);
            return Ok(());
        }
        if let Some(found) = dump.interfaces.iter().find(|i| i.name == query) {
            println!("{}", serde_json::to_string_pretty(found).into_diagnostic()?);
            return Ok(());
        }
        enum DumpItem<'a> {
            Type(&'a crate::rtti::type_desc::TypeDesc),
            Interface(&'a crate::rtti::type_desc::InterfaceDesc),
        }
        let mut by_simple: std::collections::BTreeMap<&str, Vec<DumpItem<'_>>> =
            std::collections::BTreeMap::new();
        for ty in &dump.types {
            let simple = ty.name.rsplit('.').next().unwrap_or(ty.name.as_str());
            by_simple
                .entry(simple)
                .or_default()
                .push(DumpItem::Type(ty));
        }
        for iface in &dump.interfaces {
            let simple = iface.name.rsplit('.').next().unwrap_or(iface.name.as_str());
            by_simple
                .entry(simple)
                .or_default()
                .push(DumpItem::Interface(iface));
        }
        let Some(cands) = by_simple.get(query.as_str()) else {
            return Err(miette::miette!("未知类型：{query}"));
        };
        if cands.len() == 1 {
            match cands[0] {
                DumpItem::Type(ty) => {
                    println!("{}", serde_json::to_string_pretty(ty).into_diagnostic()?)
                }
                DumpItem::Interface(iface) => {
                    println!("{}", serde_json::to_string_pretty(iface).into_diagnostic()?)
                }
            }
            return Ok(());
        }
        let names = cands
            .iter()
            .map(|c| match c {
                DumpItem::Type(ty) => ty.name.as_str(),
                DumpItem::Interface(iface) => iface.name.as_str(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(miette::miette!("类型名不唯一：{query}（候选：{names}）"));
    }

    println!("{}", serde_json::to_string_pretty(&dump).into_diagnostic()?);
    Ok(())
}

fn canonical_input(input: PathBuf) -> Result<PathBuf> {
    input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")
}

pub fn run_dump_stackmaps(input: PathBuf, verify_roots: bool, dump_records: bool) -> Result<()> {
    let input = canonical_input(input)?;
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
    let header = crate::stackmap::StackMapHeader::parse(section_bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("解析 stackmap header 失败（section: {section_name}）"))?;
    println!("stackmaps:");
    println!("section: {section_name}");
    println!("version: {}", header.version);
    println!("functions: {}", header.num_functions);
    println!("constants: {}", header.num_constants);
    println!("records: {}", header.num_records);
    if verify_roots || dump_records {
        let section = crate::stackmap::StackMapSection::parse(section_bytes)
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
    let mut out = Vec::new();
    for sym in obj.symbols().chain(obj.dynamic_symbols()) {
        if sym.kind() != SymbolKind::Text || sym.address() == 0 {
            continue;
        }
        let Ok(name) = sym.name() else { continue };
        if !name.trim().is_empty() {
            out.push(TextSymbol {
                addr: sym.address(),
                name: name.to_string(),
            });
        }
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
    cfg: crate::stackmap::StackMapRootsContractConfig,
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
        format!("-0x{:x}", v.unsigned_abs())
    }
}

fn is_root_slot_location(
    loc: crate::stackmap::StackMapLocation,
    cfg: crate::stackmap::StackMapRootsContractConfig,
) -> bool {
    matches!(
        loc.kind,
        crate::stackmap::StackMapLocationKind::Direct
            | crate::stackmap::StackMapLocationKind::Indirect
    ) && loc.size == cfg.pointer_size
        && (loc.dwarf_reg == cfg.sp_dwarf_reg
            || cfg.fp_dwarf_reg.is_some_and(|fp| fp == loc.dwarf_reg))
}

fn roots_suffix_start(
    rec: &crate::stackmap::StackMapRecord,
    cfg: crate::stackmap::StackMapRootsContractConfig,
) -> usize {
    let mut i = rec.locations.len();
    while i > 0 {
        let idx = i - 1;
        if is_root_slot_location(rec.locations[idx], cfg) {
            i -= 1;
        } else {
            break;
        }
    }
    i
}

fn dump_record_root_slots(
    section: &crate::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: crate::stackmap::StackMapRootsContractConfig,
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
    section: &crate::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: crate::stackmap::StackMapRootsContractConfig,
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

fn find_stackmaps_section<'data>(obj: &object::File<'data>) -> Result<(&'data str, &'data [u8])> {
    for section in obj.sections() {
        let name = section.name().ok();
        let Some(name) = name else { continue };
        if !(name == ".llvm_stackmaps"
            || name == "__llvm_stackmaps"
            || name.ends_with("llvm_stackmaps"))
        {
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

fn roots_contract_config_from_arch(
    arch: Architecture,
) -> Result<crate::stackmap::StackMapRootsContractConfig> {
    match arch {
        Architecture::Aarch64 => Ok(crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 31,
            fp_dwarf_reg: Some(29),
        }),
        Architecture::X86_64 => Ok(crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 7,
            fp_dwarf_reg: Some(6),
        }),
        other => Err(miette::miette!(
            "暂不支持的目标架构：{other:?}（--verify-roots 目前仅支持 aarch64/x86_64）"
        )),
    }
}
