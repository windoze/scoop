//! 阶段无关的稳定身份与 archive 版本地基（PLAN.md M0-4）。
//!
//! - [`StableHashScope`] / [`stable_hash`]：scope 前缀 FNV-1a（16 位 hex）。同文本
//!   在不同用途（dump / abi / rtti / def-path / instance）产生不同 hash，防跨用途
//!   碰撞。编码核心曾位于 `scoop2_mir::stable_id`，上移至本 crate 供各阶段共用。
//! - [`StableConeKey`]：cone 的**跨构建稳定身份**，从包名派生。会话内 dense
//!   [`ConeId`](../../scoop2_hir/resolve/symbol/struct.ConeId.html) 依赖注册顺序、
//!   不跨构建稳定；序列化与跨 cone 引用一律使用本 key（PLAN.md C2）。
//! - archive 版本与指纹：C7 的缓存失效键地基。schema 版本 + compiler 版本 + 输入
//!   集合（成员 cone key 排序集 + 全局参数）共同决定指纹；指纹不同即缓存失效。

/// FNV-1a 64 位偏移基。
pub const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
/// FNV-1a 64 位素数。
pub const FNV_PRIME: u64 = 0x100000001b3;

/// FNV-1a 64 位单字节步进。
#[inline]
pub fn fnv1a_byte(h: u64, b: u8) -> u64 {
    (h ^ b as u64).wrapping_mul(FNV_PRIME)
}

/// FNV-1a 64 位字节列步进。
#[inline]
pub fn fnv1a_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = fnv1a_byte(h, b);
    }
    h
}

/// 稳定哈希用途域：同文本在不同用途产生不同 hash。
///
/// 各层 key 构造必须显式选择 scope，防止「dump 文本哈希」与「ABI 符号哈希」等
/// 不同用途意外共享碰撞空间。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StableHashScope {
    /// 调试 / dump 渲染。
    Dump,
    /// ABI / 符号名派生。
    Abi,
    /// 运行时类型信息。
    Rtti,
    /// HIR 定义身份（`StableDefKey`）。
    DefPath,
    /// MIR 实例身份（`StableInstanceKey`）。
    Instance,
    /// 其他私有用途。
    Private,
}

impl StableHashScope {
    fn prefix(self) -> &'static str {
        match self {
            StableHashScope::Dump => "dump",
            StableHashScope::Abi => "abi",
            StableHashScope::Rtti => "rtti",
            StableHashScope::DefPath => "defpath",
            StableHashScope::Instance => "instance",
            StableHashScope::Private => "priv",
        }
    }
}

/// scope 前缀 FNV-1a，输出 16 位 hex（与既有 `scoop2_mir` 实现字节一致）。
pub fn stable_hash(scope: StableHashScope, text: &str) -> String {
    let prefixed = format!("{}:{}", scope.prefix(), text);
    let h = fnv1a_bytes(FNV_OFFSET_BASIS, prefixed.as_bytes());
    format!("{h:016x}")
}

/// cone 的跨构建稳定身份。
///
/// 派生规则（PLAN.md M0-2）：包名（点分前缀）verbatim；空名归一化为 `<root>`。
/// 会话内的 `ConeId` 只是注册表下标，**不**序列化、不作跨构建身份。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableConeKey(pub String);

impl StableConeKey {
    /// 从 cone 名（包名或 resolve 层的 fallback 名）派生稳定 key。
    pub fn from_cone_name(name: &str) -> Self {
        let trimmed = name.trim();
        let key = if trimmed.is_empty() {
            "<root>"
        } else {
            trimmed
        };
        Self(key.to_string())
    }

    /// 稳定 key 文本。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StableConeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// archive 格式 schema 版本：v0（M1 过渡的前端捆绑，允许 AST 片段与集合级共享
/// arena 段）→ v1（M2 起的正式 per-cone HIR archive）。
///
/// 规则（C7）：加载方对比版本，不匹配即拒绝（不做迁移）。
pub mod archive_schema {
    /// 过渡格式：per-cone 序列化「AST + TypedHir 现状」（打通即弃）。
    pub const V0: u32 = 0;
    /// 正式格式：per-cone arena、element/body 分段。
    pub const V1: u32 = 1;
    /// 当前最新 schema。
    pub const LATEST: u32 = V1;
}

/// 编译器版本（workspace 统一版本；所有 scoop2 crate 共享，可作 archive 兼容性
/// 判定的一部分）。
pub fn compiler_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// archive 所属阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveStage {
    Hir,
    Mir,
    Lir,
}

impl ArchiveStage {
    /// 阶段标签（进 canonical 文本 / 指纹输入）。
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiveStage::Hir => "hir",
            ArchiveStage::Mir => "mir",
            ArchiveStage::Lir => "lir",
        }
    }
}

/// archive 指纹构建器（C7 缓存失效键）。
///
/// 指纹输入 = schema 版本 + compiler 版本 + 阶段 + 本 cone key + 输入集合
/// （依赖成员的稳定 key **排序集** + 影响产出的全局参数排序集）。所有 `feed_str`
/// 均带长度前缀，避免拼接歧义（`"ab"+"c"` ≠ `"a"+"bc"`）。
///
/// 语义：**指纹相同 ⇒ 产出可复用；指纹不同 ⇒ 缓存必须失效**。MIR 的可达性决策与
/// 去虚化依赖整个输入集合（新增 cone 可能引入子类使旧去虚化结论失效），故集合
/// 构成是指纹的一等成员。
#[derive(Clone, Debug)]
pub struct ArchiveFingerprintBuilder {
    h: u64,
}

impl Default for ArchiveFingerprintBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveFingerprintBuilder {
    pub fn new() -> Self {
        Self {
            h: FNV_OFFSET_BASIS,
        }
    }

    /// 喂入一个字符串（长度前缀 + UTF-8 字节）。
    pub fn feed_str(&mut self, s: &str) {
        self.h = fnv1a_byte(self.h, 0x01);
        let len = s.as_bytes().len() as u64;
        self.h = fnv1a_bytes(self.h, &len.to_le_bytes());
        self.h = fnv1a_bytes(self.h, s.as_bytes());
    }

    /// 喂入一个 u32。
    pub fn feed_u32(&mut self, v: u32) {
        self.h = fnv1a_byte(self.h, 0x02);
        self.h = fnv1a_bytes(self.h, &v.to_le_bytes());
    }

    /// 喂入一个字符串集合：排序去重后逐个 `feed_str`（集合成员序无关）。
    pub fn feed_sorted_str_set(&mut self, items: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut v: Vec<String> = items.into_iter().map(|s| s.as_ref().to_string()).collect();
        v.sort_unstable();
        v.dedup();
        self.h = fnv1a_byte(self.h, 0x03);
        let len = v.len() as u64;
        self.h = fnv1a_bytes(self.h, &len.to_le_bytes());
        for s in &v {
            self.feed_str(s);
        }
    }

    /// 完成指纹（u64）。
    pub fn finish(self) -> u64 {
        self.h
    }
}

/// 便捷构造：一个 archive 的输入指纹。
///
/// - `schema_version` / `stage` / `cone_key`：本 archive 自身的格式与身份。
/// - `input_cone_keys`：本阶段消费的上游 archive 成员稳定 key（排序集）。
/// - `params`：影响产出的全局参数（键值对，内部按键排序）。
pub fn archive_fingerprint(
    schema_version: u32,
    stage: ArchiveStage,
    cone_key: &StableConeKey,
    input_cone_keys: impl IntoIterator<Item = impl AsRef<str>>,
    params: &[(String, String)],
) -> u64 {
    let mut b = ArchiveFingerprintBuilder::new();
    b.feed_u32(schema_version);
    b.feed_str(compiler_version());
    b.feed_str(stage.as_str());
    b.feed_str(cone_key.as_str());
    b.feed_sorted_str_set(input_cone_keys);
    // 全局参数：按键排序后逐对喂入（值参与、顺序无关）。
    let mut sorted: Vec<&(String, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in sorted {
        b.feed_str(k);
        b.feed_str(v);
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic_and_scoped() {
        let a = stable_hash(StableHashScope::Dump, "pkg.f");
        let b = stable_hash(StableHashScope::Dump, "pkg.f");
        assert_eq!(a, b, "same scope+text → same hash");
        let c = stable_hash(StableHashScope::Abi, "pkg.f");
        assert_ne!(a, c, "different scope → different hash");
        // 与上移前的 scoop2_mir 实现字节一致（回归锚点）。
        let d = stable_hash(StableHashScope::Private, "test");
        let mut h: u64 = 0xcbf29ce484222325;
        for byte in b"priv:test" {
            h ^= *byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        assert_eq!(d, format!("{h:016x}"));
    }

    #[test]
    fn cone_key_derivation() {
        assert_eq!(
            StableConeKey::from_cone_name("scoop.core").as_str(),
            "scoop.core"
        );
        assert_eq!(StableConeKey::from_cone_name("").as_str(), "<root>");
        assert_eq!(StableConeKey::from_cone_name("  ").as_str(), "<root>");
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let mk = || {
            archive_fingerprint(
                archive_schema::V1,
                ArchiveStage::Hir,
                &StableConeKey::from_cone_name("app"),
                ["scoop.core", "app"],
                &[(String::from("entry"), String::from("app.main"))],
            )
        };
        assert_eq!(mk(), mk());
    }

    #[test]
    fn fingerprint_input_set_is_order_independent() {
        let a = archive_fingerprint(
            archive_schema::V1,
            ArchiveStage::Mir,
            &StableConeKey::from_cone_name("app"),
            ["a", "b", "c"],
            &[],
        );
        let b = archive_fingerprint(
            archive_schema::V1,
            ArchiveStage::Mir,
            &StableConeKey::from_cone_name("app"),
            ["c", "a", "b"],
            &[],
        );
        assert_eq!(a, b, "member set order must not matter");
    }

    #[test]
    fn fingerprint_sensitive_to_inputs() {
        let base = (
            archive_schema::V1,
            ArchiveStage::Hir,
            StableConeKey::from_cone_name("app"),
        );
        let f0 = archive_fingerprint(base.0, base.1, &base.2, ["dep"], &[]);
        // 集合变化 → 失效。
        let f1 = archive_fingerprint(base.0, base.1, &base.2, ["dep", "dep2"], &[]);
        assert_ne!(f0, f1);
        // 参数变化 → 失效。
        let f2 = archive_fingerprint(
            base.0,
            base.1,
            &base.2,
            ["dep"],
            &[(String::from("entry"), String::from("other.main"))],
        );
        assert_ne!(f0, f2);
        // schema 版本变化 → 失效。
        let f3 = archive_fingerprint(archive_schema::V0, base.1, &base.2, ["dep"], &[]);
        assert_ne!(f0, f3);
    }

    #[test]
    fn feed_str_is_length_prefixed() {
        let mut a = ArchiveFingerprintBuilder::new();
        a.feed_str("ab");
        a.feed_str("c");
        let mut b = ArchiveFingerprintBuilder::new();
        b.feed_str("a");
        b.feed_str("bc");
        assert_ne!(a.finish(), b.finish(), "concat ambiguity must not exist");
    }
}
