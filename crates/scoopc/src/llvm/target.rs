//! LLVM target 相关辅助：host target machine、data layout 与基础 target 信息。
//!
//! 当前阶段（T0803）只支持 **宿主平台（host）**：
//! - 初始化 native target（一次性）；
//! - 基于 host triple/cpu/features 创建 target machine；
//! - 用其 data layout 配置 LLVM module；
//! - 暴露 pointer size 等信息，供后续类型映射（例如 word size）使用。

use inkwell::OptimizationLevel;
use inkwell::module::Module;
use inkwell::targets::{
    ByteOrdering, CodeModel, InitializationConfig, RelocMode, Target, TargetData, TargetMachine,
};
use miette::Diagnostic;
use thiserror::Error;

use std::sync::OnceLock;

/// LLVM target 初始化与构造过程可能出现的错误。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmTargetError {
    #[error("LLVM target 初始化失败：{message}")]
    #[diagnostic(code(scoop::llvm::target_init_failed))]
    TargetInitFailed { message: String },

    #[error("无法从 triple 获取 LLVM target：{message}")]
    #[diagnostic(code(scoop::llvm::target_from_triple_failed))]
    TargetFromTripleFailed { message: String },

    #[error("创建 LLVM target machine 失败（triple={triple}）")]
    #[diagnostic(code(scoop::llvm::create_target_machine_failed))]
    CreateTargetMachineFailed { triple: String },

    #[error("LLVM data layout 不是有效 UTF-8：{message}")]
    #[diagnostic(code(scoop::llvm::invalid_data_layout_string))]
    InvalidDataLayoutString { message: String },
}

/// 宿主 target 的最小信息集合（后续类型/ABI 映射会用到）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTargetInfo {
    /// LLVM target triple（例如 `x86_64-apple-darwin`）。
    pub triple: String,
    /// LLVM module data layout 字符串表示（例如 `e-m:o-i64:64-n32:64-S128`）。
    pub data_layout: String,
    /// 指针大小（字节）。
    pub pointer_byte_size: u32,
    /// 指针大小（比特），通常用于 `UIntPtr`/word size 推导。
    pub pointer_bit_width: u32,
    /// 端序（影响字节序相关的 ABI 细节）。
    pub byte_ordering: ByteOrdering,
}

impl HostTargetInfo {
    /// 返回“机器字（word）”大小（以比特计）。
    ///
    /// 在当前阶段，我们把 word size 等同于 pointer width（host 的自然 word）。
    pub fn word_bit_width(&self) -> u32 {
        self.pointer_bit_width
    }
}

/// 为 host 创建 target machine，并返回 `TargetMachine` 与 `HostTargetInfo`。
///
/// 注意：
/// - 目前只支持 host；交叉编译的 triple/cpu/features 选择留给后续任务。
/// - 该函数会完成 LLVM native target 初始化（一次性）。
pub fn host_target_machine() -> Result<(TargetMachine, HostTargetInfo), LlvmTargetError> {
    init_native_target()?;

    let triple = TargetMachine::get_default_triple();
    let triple_str = triple.as_str().to_string_lossy().into_owned();

    let target =
        Target::from_triple(&triple).map_err(|e| LlvmTargetError::TargetFromTripleFailed {
            message: e.to_string(),
        })?;

    let cpu = TargetMachine::get_host_cpu_name().to_string();
    let features = TargetMachine::get_host_cpu_features().to_string();

    // 早期阶段默认 O0（None）即可；优化策略属于后续任务（T080x）。
    let target_machine = target
        .create_target_machine(
            &triple,
            &cpu,
            &features,
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| LlvmTargetError::CreateTargetMachineFailed {
            triple: triple_str.clone(),
        })?;

    let target_data = target_machine.get_target_data();
    let data_layout = target_data.get_data_layout();
    let data_layout_str = data_layout
        .as_str()
        .to_str()
        .map_err(|e| LlvmTargetError::InvalidDataLayoutString {
            message: e.to_string(),
        })?
        .to_string();

    let pointer_byte_size = target_data.get_pointer_byte_size(None);

    let info = HostTargetInfo {
        triple: triple_str,
        data_layout: data_layout_str,
        pointer_byte_size,
        pointer_bit_width: pointer_byte_size * 8,
        byte_ordering: target_data.get_byte_ordering(),
    };

    Ok((target_machine, info))
}

/// 为 host 创建 target machine，并返回其 `TargetData` 与 `HostTargetInfo`。
///
/// 注意：
/// - 目前只支持 host；交叉编译的 triple/cpu/features 选择留给后续任务。
pub fn host_target_data() -> Result<(TargetData, HostTargetInfo), LlvmTargetError> {
    let (target_machine, info) = host_target_machine()?;
    Ok((target_machine.get_target_data(), info))
}

/// 配置 LLVM module 的 target triple 与 data layout（按 host）。
///
/// 返回 `HostTargetInfo`，供后续类型映射使用。
pub fn configure_module_for_host<'ctx>(
    module: &Module<'ctx>,
) -> Result<HostTargetInfo, LlvmTargetError> {
    let (target_data, info) = host_target_data()?;
    // 目标三元组：用于后端/链接器识别目标平台。
    let triple = TargetMachine::get_default_triple();
    module.set_triple(&triple);

    // data layout：类型大小/对齐/pointer size 等 ABI 关键数据。
    let data_layout = target_data.get_data_layout();
    module.set_data_layout(&data_layout);

    Ok(info)
}

fn init_native_target() -> Result<(), LlvmTargetError> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let result = INIT.get_or_init(|| Target::initialize_native(&InitializationConfig::default()));

    match result {
        Ok(()) => Ok(()),
        Err(message) => Err(LlvmTargetError::TargetInitFailed {
            message: message.clone(),
        }),
    }
}
