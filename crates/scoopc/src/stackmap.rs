//! LLVM StackMap（`.llvm_stackmaps` / `__llvm_stackmaps`）的最小解析器。
//!
//! 说明：
//! - StackMap 格式既用于编译器侧的“产物可观测/可回归”（例如 header 断言），也用于运行时的 roots 枚举；
//! - 因此该模块不依赖 LLVM/inkwell，本身不需要启用 `scoopc` 的 `llvm` feature。
//! - 当前阶段（TODO T1503a1/T1503a2）只需要能解析 section header，以便在单测/CLI 中断言：
//!   1) 产物包含 stackmap section；
//!   2) header 可读且 records 非空。

use thiserror::Error;

/// LLVM StackMap section 的 header（固定 16 字节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackMapHeader {
    pub version: u8,
    pub num_functions: u32,
    pub num_constants: u32,
    pub num_records: u32,
}

#[derive(Debug, Error)]
pub enum StackMapParseError {
    #[error("stackmap section 太短：期望至少 {expected} 字节，实际 {actual} 字节")]
    TooShort { expected: usize, actual: usize },
}

impl StackMapHeader {
    pub const BYTE_LEN: usize = 16;

    /// 从 stackmap section 起始处解析 header。
    ///
    /// 注意：stackmap 格式在主流平台上按目标端序编码；当前实现先按 little-endian 解析
    ///（host x86_64/arm64 均为 LE）。如未来引入 big-endian target，可在这里扩展为按
    /// object file 的 endianness 选择解析方式。
    pub fn parse(bytes: &[u8]) -> Result<Self, StackMapParseError> {
        if bytes.len() < Self::BYTE_LEN {
            return Err(StackMapParseError::TooShort {
                expected: Self::BYTE_LEN,
                actual: bytes.len(),
            });
        }

        // Header layout:
        //  u8  Version
        //  u8  Reserved0
        //  u16 Reserved1
        //  u32 NumFunctions
        //  u32 NumConstants
        //  u32 NumRecords
        let version = bytes[0];
        let num_functions = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let num_constants = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let num_records = u32::from_le_bytes(bytes[12..16].try_into().unwrap());

        Ok(Self {
            version,
            num_functions,
            num_constants,
            num_records,
        })
    }
}

