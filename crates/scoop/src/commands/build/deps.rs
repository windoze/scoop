//! `scoop build` 的 cone 依赖解析（T1107）。
//!
//! 当前阶段的目标是：在 build 处理“cone 包目录输入”时，根据 `Cone.toml` 的 `[dependencies]`
//! 递归加载 `.cone` 归档并形成一个 DAG（不支持循环依赖），然后把依赖的 public API 注入到
//! 当前编译单元（resolver/typecheck/codegen）中。
//!
//! 设计取舍（早期阶段，便于落地与回归）：
//! - 版本要求暂按“精确匹配字符串”处理（`req == manifest.cone.version`）；
//! - `.cone` 的定位通过一组“搜索路径”完成：
//!   - 环境变量 `SCOOP_CONE_PATH`（类似 PATH，使用平台分隔符）；
//!   - 以及 consumer 包目录下的 `cone/`、`deps/`、`.`（便于简单本地试用与 fixtures）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

/// `.cone` 依赖搜索路径环境变量。
pub const SCOOP_CONE_PATH_ENV: &str = "SCOOP_CONE_PATH";

/// 解析 consumer `Cone.toml` 的依赖图，并返回“拓扑序（依赖在前）”的 `.cone` 依赖列表。
pub fn load_dependency_graph(
    consumer_manifest: &scoopc::cone::ConeManifest,
    consumer_root: &Path,
) -> Result<Vec<scoopc::cone::ConeArchiveApi>> {
    let search_paths = build_search_paths(consumer_root);

    let mut resolved: HashMap<String, scoopc::cone::ConeArchiveApi> = HashMap::new();
    let mut visiting: Vec<String> = Vec::new();
    let mut order: Vec<String> = Vec::new();

    for (dep_name, dep_req) in &consumer_manifest.dependencies {
        visit_dep(
            dep_name,
            dep_req,
            &search_paths,
            &mut resolved,
            &mut visiting,
            &mut order,
        )?;
    }

    let mut out: Vec<scoopc::cone::ConeArchiveApi> = Vec::with_capacity(order.len());
    for name in order {
        if let Some(dep) = resolved.remove(&name) {
            out.push(dep);
        }
    }
    Ok(out)
}

fn build_search_paths(consumer_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    if let Some(paths) = std::env::var_os(SCOOP_CONE_PATH_ENV) {
        out.extend(std::env::split_paths(&paths));
    }

    // 便于本地/fixtures：允许把依赖 `.cone` 放在 consumer 包目录附近。
    out.push(consumer_root.join("cone"));
    out.push(consumer_root.join("deps"));
    out.push(consumer_root.to_path_buf());

    out
}

fn visit_dep(
    dep_name: &str,
    dep_req: &str,
    search_paths: &[PathBuf],
    resolved: &mut HashMap<String, scoopc::cone::ConeArchiveApi>,
    visiting: &mut Vec<String>,
    order: &mut Vec<String>,
) -> Result<()> {
    // sysroot 由编译器内建加载，不需要也不应该从 `.cone` 搜索。
    if dep_name == "scoop-core" {
        return Ok(());
    }

    if let Some(existing) = resolved.get(dep_name) {
        if existing.manifest.cone.version != dep_req {
            return Err(miette!(
                "同名依赖 `{dep_name}` 版本要求冲突：已解析 {}，但又要求 {}",
                existing.manifest.cone.version,
                dep_req
            ));
        }
        return Ok(());
    }

    if visiting.iter().any(|n| n == dep_name) {
        // 给出一个可读的 cycle 路径：A -> B -> C -> A
        let mut chain = visiting.join(" -> ");
        if !chain.is_empty() {
            chain.push_str(" -> ");
        }
        chain.push_str(dep_name);
        return Err(miette!("检测到 cone 依赖循环：{chain}"));
    }

    visiting.push(dep_name.to_string());

    let path = find_cone_archive_path(dep_name, dep_req, search_paths)?;
    let api = scoopc::cone::load_cone_archive_api(&path)
        .wrap_err_with(|| format!("加载依赖 `.cone` 失败：{}", path.display()))?;

    if api.manifest.cone.name != dep_name {
        return Err(miette!(
            "依赖 `.cone` 的 cone.name 不匹配：期望 `{dep_name}`，但得到 `{}`（归档：{}）",
            api.manifest.cone.name,
            path.display()
        ));
    }
    // v0：版本要求按“字符串精确匹配”处理。
    if api.manifest.cone.version != dep_req {
        return Err(miette!(
            "依赖 `.cone` 版本不匹配：`{dep_name}` 期望 {dep_req}，但得到 {}（归档：{}）",
            api.manifest.cone.version,
            path.display()
        ));
    }

    // 先递归解析该依赖的依赖，保证 order 为拓扑序。
    for (child_name, child_req) in &api.manifest.dependencies {
        visit_dep(
            child_name,
            child_req,
            search_paths,
            resolved,
            visiting,
            order,
        )?;
    }

    visiting.pop();
    resolved.insert(dep_name.to_string(), api);
    order.push(dep_name.to_string());

    Ok(())
}

fn find_cone_archive_path(
    dep_name: &str,
    dep_req: &str,
    search_paths: &[PathBuf],
) -> Result<PathBuf> {
    // 1) 先按默认命名尝试：`<name>-<version>.cone`（与 `scoop package` 的默认文件名一致）。
    let expected_file_name = format!("{dep_name}-{dep_req}.cone");

    for root in search_paths {
        if root.is_file() {
            if root.extension().is_some_and(|ext| ext == "cone")
                && archive_manifest_matches(root, dep_name, dep_req)? {
                    return Ok(root.to_path_buf());
                }
            continue;
        }

        if !root.is_dir() {
            continue;
        }

        let direct = root.join(&expected_file_name);
        if direct.is_file()
            && archive_manifest_matches(&direct, dep_name, dep_req)? {
                return Ok(direct);
            }
    }

    // 2) 回退：扫描搜索目录下所有 `.cone`，通过读取 `Cone.toml` 精确匹配。
    for root in search_paths {
        if !root.is_dir() {
            continue;
        }

        for entry in std::fs::read_dir(root)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法读取 cone 搜索目录：{}", root.display()))?
        {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "cone") {
                continue;
            }

            if archive_manifest_matches(&path, dep_name, dep_req)? {
                return Ok(path);
            }
        }
    }

    let consumer_root = consumer_root_hint(search_paths);
    Err(miette!(
        "找不到依赖 `.cone`：{dep_name} ({dep_req})；请设置 `{SCOOP_CONE_PATH_ENV}`，或把归档放到 `{consumer_root}/cone` / `{consumer_root}/deps`"
    ))
}

fn consumer_root_hint(search_paths: &[PathBuf]) -> String {
    // 为错误消息选一个相对合理的 hint：优先使用最后一个搜索路径（通常是 consumer_root）。
    search_paths
        .last()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn archive_manifest_matches(path: &Path, dep_name: &str, dep_req: &str) -> Result<bool> {
    let manifest = scoopc::cone::read_cone_manifest_from_archive(path)?;
    Ok(manifest.cone.name == dep_name && manifest.cone.version == dep_req)
}
