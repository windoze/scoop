# 执行计划

说明：本文件记录可审阅的执行计划、关键决策、进度和验证结果；不记录私有逐步推理细节。

状态：`P1-T01` 已实现、验证并更新 TODO 记录；待提交。

当前任务：`P1-T01` - `scoop.lang.string` 空 cone 落地。

最新提交检查：最新提交为 `P0-T01` 完成提交；未发现与 `P1-T01` 直接相关的未完成事项。

执行计划：

1. 检查 `sysroot/` 与现有 sysroot loader/test 中 `Sysroot::load_from`、`index_files`、`collect_compilable_sysroot_files` 的相关路径。
2. 新增 `sysroot/lang_string.scoop`，仅包含 `package scoop.lang.string` 与 `import scoop.core.*`。
3. 在合适的 sysroot 测试位置新增 owner test `lang_string_cone_visible_in_sysroot`，断言默认 sysroot 能索引该 package 且没有 type/fun 顶层声明导出。
4. 运行 `cargo test -p scoopc lang_string_cone_visible_in_sysroot -- --nocapture`、`cargo run -p scoop -- test`，并修复当前任务相关阻塞。
5. 更新 `TODO.md` 与 `TODO-1.md` 完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 用 `P1-T01` 提交信息提交相关变更，然后停止。

进度记录：

- 已新增 `sysroot/lang_string.scoop`，作为没有声明导出的 `scoop.lang.string` placeholder package。
- 已在 `crates/scoopc/src/sysroot/mod.rs` 新增 `lang_string_cone_visible_in_sysroot` owner test。
- 验证已通过：`cargo test -p scoopc lang_string_cone_visible_in_sysroot -- --nocapture`、触及 Rust 文件的 rustfmt check、全量 `cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。
- 已在 `TODO.md` 与 `TODO-1.md` 将 `P1-T01` 标记为 `[DONE]`；`PLAN.md` 未变化。
