# 执行计划

本文件记录本轮任务的可执行计划与进度摘要；不记录私密推理链。

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 找到第一个未完成任务。
2. 阅读该任务的要求、依赖、验证方式和完成记录，并只处理这一项任务。
3. 如任务存在必须先修复的阻塞问题，更新 `TODO.md` 插入最小前置任务并提交后停止；否则完成任务实现。
4. 运行与任务相关的测试；如有失败，修复后重跑验证。
5. 将完成情况写入 `TODO.md`，在任务标题前加 `[DONE]`，仅在阶段级计划变化时更新 `PLAN.md`。
6. 检查 Git 状态、差异和最近提交，提交本轮变更，然后停止。

当前状态：已确认首个未完成任务为 `P4-T03：sysroot overlay 迁移到 overlay/lib/<cone>/...`。

## P4-T03 执行步骤

1. 阅读 `PLAN.md` 中 P4 要求、`crates/scoopc/src/sysroot/mod.rs` overlay merge 实现，以及现有 `.sysroot` fixture 布局。
2. 将 overlay active path 收口为 `overlay/lib/<cone>/...` 镜像新 sysroot 布局，替换按 cone/file-relative path 处理，不再盲扫旧 overlay 根目录。
3. 更新 sysroot 单元测试，覆盖新 overlay replacement/addition、旧结构不再加载、docs/bin 不参与加载。
4. 迁移或删除旧 `.sysroot` overlay fixtures，确保 active fixtures 使用 `lib/<cone>/...` 结构。
5. 运行定向 sysroot tests、相关 fixture suite、格式化、构建、clippy，并按需要运行全量验证。
6. 在 `TODO.md` 标记 `P4-T03` 为 `[DONE]` 并记录改动范围、核心决策和验证结果；随后提交本轮变更。

进度更新：已将 overlay 合并逻辑改为只处理已知 sysroot cone 的 `overlay/lib/<cone>/src/**/*.scoop`，并新增旧 overlay 路径被忽略的单元测试。旧 `.sysroot/fixtures/...` 与 `.sysroot/scoop.*` fixture 文件已迁移到 `.sysroot/lib/scoop.core/src/...`。

下一步：运行格式化和定向验证，先覆盖 `scoopc sysroot` 单测与包含 overlay 的 build/typecheck/run-pass/UMB fixtures。

验证更新：`cargo test -p scoopc sysroot -- --nocapture` 通过，但发现一个已无调用的旧 helper 产生 `dead_code` warning；已删除该 helper，待重新格式化并重跑验证。

完成更新：格式化、定向 sysroot/fixture 验证、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 和完整 `cargo run -p scoop -- test` 均已通过。`TODO.md` 已将 `P4-T03` 标记为 `[DONE]` 并写入完成记录，下一步检查 Git 差异并提交。
