//! Cone（包/稳定 IR/分发）相关基础设施。
//!
//! 当前阶段已落地：
//! - T1101：`Cone.toml` 的最小解析（name/version/deps）；
//! - T1102：source package 的加载规则（cone root → sources 列表 + main 入口定位）。

pub mod manifest;
pub mod package;
pub mod scoopir;

pub use manifest::{CONE_TOML_FILE_NAME, ConeManifest, ConeSection, discover_cone_manifest_path, discover_cone_root};
pub use package::{CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, ConeSourcePackage, load_cone_source_package};
