//! `scoop dump-stackmaps` 子命令。
//!
//! 用途（TODO T1503a2）：
//! - 从链接产物（可执行文件优先；也兼容 `.o`）中定位 LLVM stackmap section；
//! - 输出稳定、可用于 fixtures/CI 断言的 header 摘要（records 数量等）。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use object::{Architecture, Object as _, ObjectSection as _};

pub fn run(input: PathBuf, verify_roots: bool) -> Result<()> {
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

    if verify_roots {
        let section = scoopc::stackmap::StackMapSection::parse(section_bytes)
            .into_diagnostic()
            .wrap_err("解析 stackmap section 失败（StackMapSection::parse）")?;

        let cfg = roots_contract_config_from_arch(obj.architecture())?;
        section
            .verify_roots_contract(cfg)
            .into_diagnostic()
            .wrap_err("stackmap roots 契约校验失败（--verify-roots）")?;

        println!("verify-roots: ok");
    }

    Ok(())
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
