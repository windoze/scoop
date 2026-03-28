//! const/comptime 的值模型（v0）。
//!
//! 说明：
//! - 该值模型用于“解释器内部的值”，与 MIR/LLVM 的常量表示解耦；
//! - 早期阶段优先保证：结构简单、错误可诊断、语义可扩展。

/// 解释器中的“编译期值”（v0：只覆盖少量字面量与整数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Unit,
    Bool(bool),
    Int(ConstInt),
    String(String),
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
