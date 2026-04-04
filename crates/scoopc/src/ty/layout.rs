//! 类型布局（type layout）与 enum 表示选择（early stage）。
//!
//! 目的（T0449）：
//! - 在类型系统层固定 rich enum/`Option<T>` 的布局选择规则（niche、boxing、lint）
//! - 给后续 codegen 提供稳定的“类型元数据形状”（具体字节布局由 codegen 按 target 落地）
//!
//! 说明：
//! - 当前实现使用“宿主机（host）指针大小/对齐”作为 target layout（T0803 会替换为真实 target machine）。
//! - 当前只建模我们在前端阶段需要的最小信息：size/align、niche domain、enum tag 类型、以及 variant boxing 决策。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetLayout {
    pub pointer_size: u64,
    pub pointer_align: u64,
}

impl TargetLayout {
    /// 基于当前宿主平台的 pointer size/align 构造 layout。
    ///
    /// 注意：这不是最终的跨平台方案；后续会由 codegen 初始化的 target machine 提供（T0803）。
    pub fn host() -> Self {
        Self {
            pointer_size: std::mem::size_of::<usize>() as u64,
            pointer_align: std::mem::align_of::<usize>() as u64,
        }
    }
}

/// niche 的存储类型（用于解释 `none_value`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NicheStorage {
    Pointer,
    U8,
}

/// 一个连续的 niche domain：`[next, end)`。
///
/// 解释：
/// - `next..end` 表示“仍可被占用”的 niche 值集合（按递增顺序分配）；
/// - 当某个 `Option<...>` 使用 niche 优化时，会从 inner domain 中取走一个值作为 `None` 编码，
///   并把剩余 domain 传递给外层（支持 nested niche，例如 `Option<Option<RefType>>`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NicheDomain {
    pub storage: NicheStorage,
    pub next: u64,
    pub end: u64,
}

impl NicheDomain {
    pub fn is_empty(self) -> bool {
        self.next >= self.end
    }

    pub fn count(self) -> u64 {
        self.end.saturating_sub(self.next)
    }

    /// 分配并占用一个 niche 值（返回分配的值）。
    pub fn take_one(&mut self) -> Option<u64> {
        if self.is_empty() {
            return None;
        }
        let v = self.next;
        self.next += 1;
        Some(v)
    }
}

/// 一个类型的最小布局信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: u64,
    pub align: u64,
    pub niche: Option<NicheDomain>,
}

impl TypeLayout {
    pub fn new(size: u64, align: u64) -> Self {
        Self {
            size,
            align: align.max(1),
            niche: None,
        }
    }

    pub fn with_niche(mut self, niche: NicheDomain) -> Self {
        if !niche.is_empty() {
            self.niche = Some(niche);
        }
        self
    }
}

/// enum tag 的表示类型（spec §2.3.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumTagType {
    U8,
    U16,
    U32,
}

impl EnumTagType {
    pub fn for_variant_count(count: usize) -> Self {
        if count <= 256 {
            return Self::U8;
        }
        if count <= 65_536 {
            return Self::U16;
        }
        Self::U32
    }

    pub fn size(self) -> u64 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }

    pub fn align(self) -> u64 {
        self.size()
    }
}

/// `Option<T>` / rich enum 的表示选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumRepr {
    /// 常规 tagged union：`tag + union`。
    TaggedUnion { tag: EnumTagType },

    /// niche 优化：使用某个非法值表示 `None`，因此无显式 tag。
    ///
    /// 说明（spec §2.3.2）：
    /// - `Option<RefType>`：`None = null (0x0)`
    /// - `Option<Option<RefType>>`：外层 `None` 使用 `0x1` 等非法地址值
    /// - `Option<Bool>`：`None = 2`
    Niche {
        storage: NicheStorage,
        none_value: u64,
    },
}

/// 单个 enum variant 的布局决策（只记录 boxing 与 payload layout）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantLayout {
    pub name: String,
    pub boxed: bool,
    pub payload: TypeLayout,
}

/// 一个 enum（或 enum-like，例如 `Option<T>`）的布局摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumLayout {
    pub repr: EnumRepr,
    pub layout: TypeLayout,
    pub tag_offset: u64,
    pub payload_offset: u64,
    pub payload: TypeLayout,
    pub variants: Vec<EnumVariantLayout>,
}
