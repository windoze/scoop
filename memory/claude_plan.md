# Current Invocation Plan — P9-T07 cone 两层拆分

## 目标
- 把 `cone/manifest.rs` / `package.rs` / `graph.rs` 与 sysroot 的 “数据/FS 层” 迁到 base crate `scoopc_project_model`，使其仍只依赖 base crates。
- 抽出新 crate `scoopc_cone`，承载 `archive.rs`、`scoopir/`、`annotations.rs`、`visibility.rs`、`pre_specialize.rs`、`consume.rs` 操作层（依赖所有 stage crate + project_model）。
- `scoopc::cone::*` 作为 façade re-export 保持向后兼容。
- `dependency_gate` 加入 `scoopc_cone`：禁止任何 stage crate 反向依赖。

## 关键约束 / 设计注意
- `scoopc_project_model` 不能新增对 `scoopc_ast` / `scoopc_hir` 等 stage crate 的依赖；保留 `scoopc_span` / `scoopc_source` / `scoopc_ids` / `scoopc_types` 范围。
- 当前 `Sysroot { files: Vec<SysrootFile { path, source, ast }> }` 持有 `ast::File`，是 stage 依赖。处理：
  - 把 path / manifest 级 sysroot 工具（`SysrootSourceConePackage`、`SysrootSourceEntry`、`collect_*` 系列、`select_auto_*`、`sysroot_source_cone_names`、`SYSROOT_OVERLAY_ENV`、`DEFAULT_AUTO_DEPENDENCY_CONES`、`Sysroot::default_path` 入口）迁到 project_model。
  - AST-持有的 `Sysroot { files }`、`SysrootFile { ast }` 仍留在 `scoopc_hir`，但内部改为调用 project_model 的 path 层 API；`Session` 与 `Sysroot` 的关联保持不变，避免与 stage crate 形成循环依赖。
  - `scoopc::sysroot::*` 与 `scoopc_hir::sysroot::*` 通过 re-export 维持向后兼容。

## 步骤
1. project_model 新增模块（manifest/package/graph/sysroot 的 path 层），lib.rs re-export。
2. 删除 `scoopc/src/cone/{manifest,package,graph}.rs` 与 `scoopc_hir/src/cone/{manifest,package}.rs` 重复 wrapper（改为直接 re-export project_model）。
3. 改 `scoopc_hir/src/sysroot/mod.rs`：保留 AST 部分；其它 path 层函数改为 re-export 或薄包装到 project_model。
4. 修复 `scoopc_hir/src/lib.rs` 的 `pub mod cone {...}` 与现有 `cone` 子模块（保持向后兼容路径）。
5. 新建 `crates/scoopc_cone/`，迁入 archive / scoopir / annotations / visibility / pre_specialize / consume；内部 import 改为 stage crate 路径。
6. 修改 `crates/scoopc/src/lib.rs`：`pub mod cone` 改为 façade re-export；frontend / pipeline 的导入路径同步更新。
7. workspace `Cargo.toml`、`scoopc/Cargo.toml`、`scoopc_cone/Cargo.toml` 同步。
8. `dependency_gate` 增加 `scoopc_cone` 类型与反向依赖禁令。
9. 验证：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop_tools -- dependency-gate`、`cargo tree -p scoopc_project_model`（仅 base）、`cargo tree -p scoopc_cone`（含所有 stage crate）、`git diff --check`、`cargo clippy --all-targets -- -D warnings`。
10. 更新 `TODO.md`、`TODO-7.md`，commit。

## 风险
- `scoopc_hir/src/cone` 与 `scoopc/src/cone` 的重复 wrapper 删除后，所有 `crate::cone::*` 引用需要更新。
- `scoopc_cone` 内部 `crate::session::Session`、`crate::resolve::Index` 等 import 需重写为 `scoopc_hir::session::Session` 等。
- `Sysroot::default_path()` 转发后，`env!("CARGO_MANIFEST_DIR")` 路径变成 project_model；需要继续指向 workspace 根的 `sysroot/`。

## 进度
- [ ] 步骤 1：project_model 接收 manifest/package/graph/sysroot path-level
- [ ] 步骤 2-4：清理 scoopc / scoopc_hir 重复 wrapper
- [ ] 步骤 5-7：建立 scoopc_cone + façade + Cargo
- [ ] 步骤 8：dependency_gate
- [ ] 步骤 9-10：验证 + TODO 同步与提交
