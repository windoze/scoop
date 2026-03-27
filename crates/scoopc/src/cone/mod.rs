//! Cone（包/稳定 IR/分发）相关基础设施。
//!
//! 当前阶段（T1101）只落地两件事：
//! - `Cone.toml` 的最小解析（name/version/deps）；
//! - 从目录（或任意子路径）发现 `Cone.toml` 的位置，供 driver 后续接入。

pub mod manifest;

pub use manifest::{CONE_TOML_FILE_NAME, ConeManifest, ConeSection, discover_cone_manifest_path, discover_cone_root};
