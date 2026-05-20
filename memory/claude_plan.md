# 当前执行计划

## 原则

- 以 `TODO.md` 为唯一任务顺序来源，先识别第一个标题未带 `[DONE]` 的任务。
- 本轮只完成第一个未完成任务；完成后更新记录、提交 Git，并停止。
- 如果遇到阻塞当前任务的规范不匹配、缺失功能或实现边界，不做绕路；在 `TODO.md` 中添加最小必要前置任务并提交后停止。
- 本文件记录可审查的计划、依据、进度和验证结果；不记录不可公开的内部推理细节。

## 当前任务

- 第一个未完成任务：`P4-T02：sysroot loader 改为加载 sysroot/lib/*/Cone.toml`。
- 任务目标：sysroot loader 不再递归扫描整个 `sysroot/**/*.scoop`，只通过 `sysroot/lib/*/Cone.toml` 发现 sysroot cones，并用 source cone package rules 加载每个 cone。
- 指定验证：新增 `sysroot/docs/foo.scoop` 不加载测试；运行 `cargo test -p scoopc sysroot -- --nocapture`。
- 完成后只标记 `P4-T02`，不继续执行 `P4-T03`。

## 执行步骤

1. 在写入本计划后，查看最近提交信息，确认是否有直接关联 `P4-T02` 的未完成事项。
2. 阅读 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 中 P4 sysroot loader 要求，以及 `crates/scoopc/src/sysroot/mod.rs`、`crates/scoopc/src/cone/package.rs` 的现状测试与加载接口。
3. 修改 sysroot loader：只枚举 `sysroot/lib/*/Cone.toml`，对每个 manifest 所在目录调用 source cone package loader，保留 trusted syslib trust 标记，不再盲递归 sysroot 下所有 `.scoop`。
4. 更新或新增 sysroot 单元测试：覆盖 `sysroot/docs/*.scoop` 不进入 compilation unit；覆盖 `sysroot/lib/<cone>/Cone.toml` 发现路径；必要时覆盖无 manifest / 非 cone 目录被忽略或稳定诊断。
5. 运行格式化、定向 sysroot 测试，以及受影响的 build/typecheck fixture 或全量必要验证；若发现与当前任务直接相关的阻塞，先修复或把最小前置任务写入 `TODO.md` 后停止。
6. 更新 `memory/claude_plan.md` 的关键进度和验证结果。
7. 更新 `TODO.md`：给 `P4-T02` 标题加 `[DONE]`，补全完成记录；仅当阶段计划变化时更新 `PLAN.md`。
8. 提交前检查 `git status`、`git diff`、最近提交记录，确认提交包含本轮相关变更；用 `[P4-T02] ...` 格式提交，然后停止。

## 进度记录

- 已读取 `TODO.md`，确认 `P4-T02` 是第一个未完成任务。
- 已在执行 shell 命令前写入本计划。
- 最近提交为 `[P4-T01] Reshape sysroot source layout`，未在标题中暴露与 `P4-T02` 直接相关的未完成事项。
- 已阅读 P4 设计要求与现有 `sysroot/mod.rs`、`cone/package.rs`；确认当前 loader 仍会盲递归 base sysroot。
- 已开始实现：base sysroot 改为枚举 `sysroot/lib/*/Cone.toml` 并通过 source cone package loader 收集 sources；`syslib` trust 由 manifest kind 决定，普通 `lib` sysroot source 保持 sysroot origin 但不授予 trusted syslib 权限。
- Overlay 兼容扫描暂未扩大到 P4-T03 的最终规则；已知 `lib/<cone>/...` overlay source 按 owning cone manifest kind 继承 trust，旧式未知 overlay 迁移仍按 `P4-T03` 处理。
- 已更新 sysroot 单元测试，覆盖 `sysroot/docs/*.scoop` 不加载、manifest kind 控制 trusted syslib 权限、overlay 替换在新 `lib/<cone>/src` 布局下继续工作，以及 overlay 新增到已知 `lib` cone 的 source 不获得 trusted syslib 权限。
- 验证已通过：`cargo fmt`、`cargo test -p scoopc sysroot -- --nocapture`（首次 120s 超时后 600s 重跑通过，最终重跑通过）、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`（1563 checks）。
- `TODO.md` 已将 `P4-T02` 标记为 `[DONE]`，补全完成记录，并把当前状态推进到下一任务 `P4-T03`。
- 下一步检查 git status/diff/log，然后提交本轮变更并停止。
