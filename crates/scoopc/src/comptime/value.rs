//! const/comptime 的值模型（v0）。
//!
//! 说明：
//! - 该值模型用于“解释器内部的值”，与 MIR/LLVM 的常量表示解耦；
//! - 早期阶段优先保证：结构简单、错误可诊断、语义可扩展。

use std::collections::BTreeMap;

/// 解释器中的“编译期值”。
///
/// 当前已覆盖：
/// - 基础字面量：`Unit/Bool/Char/Int/Float/String`
/// - 聚合值：`Tuple/Struct/Enum`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Unit,
    Bool(bool),
    Char(char),
    Int(ConstInt),
    Float(ConstFloat),
    String(String),
    /// tuple 值；当前 v0 也用它承载 array literal / 其它“常量序列”结果，
    /// 以便在 comptime fixtures 中稳定观察到序列内容。
    Tuple(Vec<ConstValue>),
    Struct(ConstStruct),
    Enum(ConstEnum),
}

/// struct 值（字段名 → const value）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstStruct {
    pub ty: String,
    pub fields: BTreeMap<String, ConstValue>,
}

/// enum 值（variant + payload）。
///
/// 说明：
/// - `ty` 在缺少类型信息时允许为 `None`（例如未消歧的 `Some(1)`）；
/// - `payload` 目前只保留“位置参数”形态，字段名映射留给后续 typecheck/解释器接入时补齐。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEnum {
    pub ty: Option<String>,
    pub variant: String,
    pub payload: Vec<ConstValue>,
}

/// const 整数类型信息（位宽 + 符号位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstIntTy {
    pub bits: u32,
    pub signed: bool,
}

impl ConstIntTy {
    /// 使用“宿主机 word size（指针宽度）”作为 Int/UInt 的位宽（与当前后端约定一致）。
    pub fn host_word(signed: bool) -> Self {
        Self {
            bits: (std::mem::size_of::<usize>() as u32) * 8,
            signed,
        }
    }
}

/// const 整数值（按 `ConstIntTy.bits` 做 wrap/mask）。
///
/// 说明：
/// - 这里以 `raw_bits`（u128）表示二进制补码位模式；
/// - `signed` 仅影响“解释成有符号值”的操作（比较/除法/右移等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstInt {
    pub ty: ConstIntTy,
    pub raw_bits: u128,
}

impl ConstInt {
    /// 构造一个 const 整数，并按 `ty.bits` 自动 mask。
    pub fn new(ty: ConstIntTy, raw_bits: u128) -> Self {
        let masked = mask_to_bits(raw_bits, ty.bits);
        Self {
            ty,
            raw_bits: masked,
        }
    }

    /// 返回该整数类型下的 0。
    pub fn zero(ty: ConstIntTy) -> Self {
        Self::new(ty, 0)
    }

    /// 返回该整数类型下的 1。
    pub fn one(ty: ConstIntTy) -> Self {
        Self::new(ty, 1)
    }

    /// 以无符号方式读取原始位模式（已按位宽 mask）。
    pub fn as_u128(self) -> u128 {
        self.raw_bits
    }

    /// 以有符号方式解释原始位模式（两补码 + sign-extend）。
    pub fn as_i128(self) -> i128 {
        if self.ty.bits == 0 {
            return 0;
        }

        let bits = self.ty.bits.min(128);
        let masked = mask_to_bits(self.raw_bits, bits);
        if bits == 128 {
            return masked as i128;
        }

        let shift = 128 - bits;
        ((masked << shift) as i128) >> shift
    }
}

/// const 浮点类型信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstFloatTy {
    Float64,
    Float32,
}

impl ConstFloatTy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Float64 => "Float64",
            Self::Float32 => "Float32",
        }
    }
}

/// const 浮点值（按 IEEE-754 原始 bit pattern 存储）。
///
/// 说明：
/// - 使用 raw bits 存储，可稳定区分 `-0.0` / `0.0`，并在测试中保持可比较性；
/// - 真正执行算术/比较时再按各自精度还原为 `f64` / `f32`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstFloat {
    Float64(u64),
    Float32(u32),
}

impl ConstFloat {
    pub fn from_f64(value: f64) -> Self {
        Self::Float64(value.to_bits())
    }

    pub fn from_f32(value: f32) -> Self {
        Self::Float32(value.to_bits())
    }

    pub fn ty(self) -> ConstFloatTy {
        match self {
            Self::Float64(_) => ConstFloatTy::Float64,
            Self::Float32(_) => ConstFloatTy::Float32,
        }
    }

    pub fn as_f64(self) -> f64 {
        match self {
            Self::Float64(bits) => f64::from_bits(bits),
            Self::Float32(bits) => f64::from(f32::from_bits(bits)),
        }
    }

    pub fn as_f32(self) -> f32 {
        match self {
            Self::Float64(bits) => f64::from_bits(bits) as f32,
            Self::Float32(bits) => f32::from_bits(bits),
        }
    }

    pub fn cast(self, target: ConstFloatTy) -> Self {
        match target {
            ConstFloatTy::Float64 => Self::from_f64(self.as_f64()),
            ConstFloatTy::Float32 => Self::from_f32(self.as_f32()),
        }
    }
}

/// 把整数值截断到给定 bit 宽度（低位保留，高位清零）。
pub(crate) fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    let mask = (1u128 << bits) - 1;
    value & mask
}
