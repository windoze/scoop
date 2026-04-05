//! LLVM StackMap（`.llvm_stackmaps` / `__llvm_stackmaps`）的最小解析器。
//!
//! 说明：
//! - StackMap 格式既用于编译器侧的“产物可观测/可回归”（例如 header 断言），也用于运行时的 roots 枚举；
//! - 因此该模块不依赖 LLVM/inkwell，本身不需要启用 `scoopc` 的 `llvm` feature。
//! - 当前阶段（GC-FIX Phase A1）需要把 stackmap 的“roots 语义契约”固化为**可计算、可回归**的规则：
//!   - 运行时不应再依赖 heap membership 过滤来区分“看起来像指针但不是 GC roots”的值；
//!   - 因此我们需要能解析 stackmap records，并验证“哪些 locations 是 GC roots”的契约是否成立。
//!
//! 约定（v3）：
//! - 本解析器目前以 LLVM StackMap v3 为目标（`version == 3`）。
//! - 先按 little-endian 解析（host x86_64/arm64 均为 LE）。

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

    #[error("不支持的 stackmap 版本：{version}（当前仅支持 v3）")]
    UnsupportedVersion { version: u8 },

    #[error(
        "stackmap section 读取越界：offset={offset}, 需要 {need} 字节，但剩余 {remaining} 字节（{context}）"
    )]
    UnexpectedEof {
        offset: usize,
        need: usize,
        remaining: usize,
        context: &'static str,
    },

    #[error("stackmap section 结构非法：{message}")]
    Malformed { message: &'static str },
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

/// 一个完整的 stackmap section 解析结果（v3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackMapSection {
    pub header: StackMapHeader,
    pub functions: Vec<StackMapFunction>,
    pub constants: Vec<u64>,
    pub records: Vec<StackMapRecord>,
}

/// stackmap function entry（v3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackMapFunction {
    pub function_address: u64,
    pub stack_size: u64,
    pub record_count: u64,
}

/// stackmap record（v3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackMapRecord {
    /// 该 record 所属函数地址（来自 function entry）。
    pub function_address: u64,
    pub patchpoint_id: u64,
    pub instruction_offset: u32,
    pub locations: Vec<StackMapLocation>,
    pub live_outs: Vec<StackMapLiveOut>,
}

/// stackmap location（12 bytes，v3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackMapLocation {
    pub kind: StackMapLocationKind,
    pub size: u16,
    pub dwarf_reg: u16,
    pub offset: i32,
}

/// stackmap live-out（4 bytes，v3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackMapLiveOut {
    pub dwarf_reg: u16,
    pub size: u8,
}

/// StackMap v3 location kind。
///
/// 参考：LLVM StackMap section spec（v3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StackMapLocationKind {
    Register = 1,
    Direct = 2,
    Indirect = 3,
    Constant = 4,
    ConstantIndex = 5,
}

impl StackMapLocationKind {
    fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Register),
            2 => Some(Self::Direct),
            3 => Some(Self::Indirect),
            4 => Some(Self::Constant),
            5 => Some(Self::ConstantIndex),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackMapRootsContractConfig {
    /// 指针大小（字节），用于判断“pointer-sized locations”。
    pub pointer_size: u16,
    /// DWARF register number：SP/CFA。
    pub sp_dwarf_reg: u16,
    /// DWARF register number：FP（若该架构/ABI 使用且可接受）。
    pub fp_dwarf_reg: Option<u16>,
}

impl StackMapRootsContractConfig {
    fn is_allowed_base_reg(self, dwarf_reg: u16) -> bool {
        if dwarf_reg == self.sp_dwarf_reg {
            return true;
        }
        self.fp_dwarf_reg.is_some_and(|fp| fp == dwarf_reg)
    }
}

/// roots 契约校验失败：用于 `scoop dump-stackmaps --verify-roots` 与 `scoopc` 单测回归。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error(
    "stackmap roots 契约违反：record#{record_index} (func=0x{function_address:x}, inst_off=0x{instruction_offset:x}): {reason}"
)]
pub struct StackMapRootsContractViolation {
    pub record_index: usize,
    pub function_address: u64,
    pub instruction_offset: u32,
    pub reason: String,
}

/// 解析 stackmap section（v3）。
impl StackMapSection {
    pub fn parse(bytes: &[u8]) -> Result<Self, StackMapParseError> {
        let header = StackMapHeader::parse(bytes)?;
        if header.version != 3 {
            return Err(StackMapParseError::UnsupportedVersion {
                version: header.version,
            });
        }

        let mut r = Reader::new(bytes);
        r.off = StackMapHeader::BYTE_LEN;

        let functions = {
            let want = header.num_functions as usize;
            let mut out = Vec::with_capacity(want);
            for _ in 0..want {
                let function_address = r.read_u64_le("function_address")?;
                let stack_size = r.read_u64_le("stack_size")?;
                let record_count = r.read_u64_le("record_count")?;
                out.push(StackMapFunction {
                    function_address,
                    stack_size,
                    record_count,
                });
            }
            out
        };

        let constants = {
            let want = header.num_constants as usize;
            let mut out = Vec::with_capacity(want);
            for _ in 0..want {
                out.push(r.read_u64_le("constant")?);
            }
            out
        };

        // 基于 function records 解析 records。
        let mut planned_records: u64 = 0;
        for f in &functions {
            planned_records = planned_records.saturating_add(f.record_count);
        }
        if planned_records != header.num_records as u64 {
            return Err(StackMapParseError::Malformed {
                message: "records_sum != header.num_records",
            });
        }

        let mut records = Vec::with_capacity(header.num_records as usize);
        for f in &functions {
            for _ in 0..f.record_count {
                let patchpoint_id = r.read_u64_le("record.patchpoint_id")?;
                let instruction_offset = r.read_u32_le("record.instruction_offset")?;
                let _reserved0 = r.read_u16_le("record.reserved0")?;
                let num_locations = r.read_u16_le("record.num_locations")? as usize;

                let mut locations = Vec::with_capacity(num_locations);
                for _ in 0..num_locations {
                    let loc_type = r.read_u8("location.type")?;
                    let _reserved0 = r.read_u8("location.reserved0")?;
                    let size = r.read_u16_le("location.size")?;
                    let dwarf_reg = r.read_u16_le("location.dwarf_reg")?;
                    let _reserved1 = r.read_u16_le("location.reserved1")?;
                    let offset = r.read_i32_le("location.offset")?;

                    let Some(kind) = StackMapLocationKind::from_u8(loc_type) else {
                        return Err(StackMapParseError::Malformed {
                            message: "unknown location type",
                        });
                    };

                    locations.push(StackMapLocation {
                        kind,
                        size,
                        dwarf_reg,
                        offset,
                    });
                }

                // locations 之后按 8-byte 对齐（LLVM emission 常见约定；缺少会导致后续解析错位）。
                r.align_up(8)?;

                let num_live_outs = r.read_u16_le("record.num_live_outs")? as usize;
                let _reserved1 = r.read_u16_le("record.live_reserved")?;
                let mut live_outs = Vec::with_capacity(num_live_outs);
                for _ in 0..num_live_outs {
                    let dwarf_reg = r.read_u16_le("liveout.dwarf_reg")?;
                    let size = r.read_u8("liveout.size")?;
                    let _reserved = r.read_u8("liveout.reserved")?;
                    live_outs.push(StackMapLiveOut { dwarf_reg, size });
                }

                // record 末尾按 8-byte 对齐（LLVM emission 常见约定）。
                r.align_up(8)?;

                records.push(StackMapRecord {
                    function_address: f.function_address,
                    patchpoint_id,
                    instruction_offset,
                    locations,
                    live_outs,
                });
            }
        }

        Ok(Self {
            header,
            functions,
            constants,
            records,
        })
    }

    /// 校验 stackmap roots 的语义契约（Phase A1）。
    ///
    /// 契约（当前版本）：
    /// - 每个 record 的 locations 可以分为：
    ///   - 前缀：deopt/metadata（不要求可写回）；
    ///   - 后缀：GC roots（必须可写回，用于 moving roots update）。
    /// - GC roots **必须**表现为“连续后缀”的 pointer-sized Direct/Indirect locations，
    ///   且其 base reg 只能是 SP/FP（便于 runtime 通过 frame SP/FP 计算 slot 地址）。
    /// - roots locations 的数量必须为偶数（statepoint 语义：base/derived 成对出现；无 roots 时为 0）。
    pub fn verify_roots_contract(
        &self,
        cfg: StackMapRootsContractConfig,
    ) -> Result<(), StackMapRootsContractViolation> {
        for (record_index, rec) in self.records.iter().enumerate() {
            let roots_start = roots_suffix_start(rec, cfg);
            let roots_len = rec.locations.len().saturating_sub(roots_start);

            if (roots_len % 2) != 0 {
                return Err(StackMapRootsContractViolation {
                    record_index,
                    function_address: rec.function_address,
                    instruction_offset: rec.instruction_offset,
                    reason: format!("roots locations 数量必须为偶数，但实际为 {roots_len}"),
                });
            }

            // 1) roots 必须是连续后缀：因此前缀里不允许再出现“看起来像 root slot”的 location。
            for (i, loc) in rec.locations.iter().enumerate().take(roots_start) {
                if is_root_slot_location(*loc, cfg) {
                    return Err(StackMapRootsContractViolation {
                        record_index,
                        function_address: rec.function_address,
                        instruction_offset: rec.instruction_offset,
                        reason: format!(
                            "location[{i}] 形状符合 root slot（{:?} size={} reg={} off={}），但出现在 roots 后缀之前",
                            loc.kind, loc.size, loc.dwarf_reg, loc.offset
                        ),
                    });
                }

                // 额外健壮性：Direct/Indirect + pointer-sized 但 base reg 非 SP/FP 会让 runtime 无法计算 slot。
                if matches!(
                    loc.kind,
                    StackMapLocationKind::Direct | StackMapLocationKind::Indirect
                ) && loc.size == cfg.pointer_size
                    && !cfg.is_allowed_base_reg(loc.dwarf_reg)
                {
                    return Err(StackMapRootsContractViolation {
                        record_index,
                        function_address: rec.function_address,
                        instruction_offset: rec.instruction_offset,
                        reason: format!(
                            "location[{i}] 为 pointer-sized {:?}，但 base DWARF reg={} 非 SP/FP（SP={} FP={:?}）",
                            loc.kind, loc.dwarf_reg, cfg.sp_dwarf_reg, cfg.fp_dwarf_reg
                        ),
                    });
                }
            }

            // 2) roots 后缀里的每个 location 都必须是可写回 slot（Direct/Indirect + pointer-sized + SP/FP base）。
            for (i, loc) in rec.locations.iter().enumerate().skip(roots_start) {
                if !is_root_slot_location(*loc, cfg) {
                    return Err(StackMapRootsContractViolation {
                        record_index,
                        function_address: rec.function_address,
                        instruction_offset: rec.instruction_offset,
                        reason: format!(
                            "roots 后缀内 location[{i}] 不是可写回 root slot（{:?} size={} reg={} off={}）",
                            loc.kind, loc.size, loc.dwarf_reg, loc.offset
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

fn is_root_slot_location(loc: StackMapLocation, cfg: StackMapRootsContractConfig) -> bool {
    if !matches!(
        loc.kind,
        StackMapLocationKind::Direct | StackMapLocationKind::Indirect
    ) {
        return false;
    }
    if loc.size != cfg.pointer_size {
        return false;
    }
    cfg.is_allowed_base_reg(loc.dwarf_reg)
}

fn roots_suffix_start(rec: &StackMapRecord, cfg: StackMapRootsContractConfig) -> usize {
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

struct Reader<'a> {
    bytes: &'a [u8],
    off: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, off: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.off)
    }

    fn read_exact(
        &mut self,
        n: usize,
        context: &'static str,
    ) -> Result<&'a [u8], StackMapParseError> {
        if self.remaining() < n {
            return Err(StackMapParseError::UnexpectedEof {
                offset: self.off,
                need: n,
                remaining: self.remaining(),
                context,
            });
        }
        let start = self.off;
        self.off += n;
        Ok(&self.bytes[start..start + n])
    }

    fn read_u8(&mut self, context: &'static str) -> Result<u8, StackMapParseError> {
        Ok(self.read_exact(1, context)?[0])
    }

    fn read_u16_le(&mut self, context: &'static str) -> Result<u16, StackMapParseError> {
        let raw = self.read_exact(2, context)?;
        Ok(u16::from_le_bytes(raw.try_into().unwrap()))
    }

    fn read_u32_le(&mut self, context: &'static str) -> Result<u32, StackMapParseError> {
        let raw = self.read_exact(4, context)?;
        Ok(u32::from_le_bytes(raw.try_into().unwrap()))
    }

    fn read_u64_le(&mut self, context: &'static str) -> Result<u64, StackMapParseError> {
        let raw = self.read_exact(8, context)?;
        Ok(u64::from_le_bytes(raw.try_into().unwrap()))
    }

    fn read_i32_le(&mut self, context: &'static str) -> Result<i32, StackMapParseError> {
        let raw = self.read_u32_le(context)?;
        Ok(raw as i32)
    }

    fn align_up(&mut self, align: usize) -> Result<(), StackMapParseError> {
        if align == 0 {
            return Ok(());
        }
        let mask = align - 1;
        let aligned = (self.off + mask) & !mask;
        if aligned > self.bytes.len() {
            return Err(StackMapParseError::UnexpectedEof {
                offset: self.off,
                need: aligned - self.off,
                remaining: self.remaining(),
                context: "align_up",
            });
        }
        self.off = aligned;
        Ok(())
    }
}
