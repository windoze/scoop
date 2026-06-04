执行计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 查看最新提交摘要，确认是否有与该任务直接相关的未完成事项。
3. 阅读该任务相关的代码、测试和文档，确认需求、依赖与验证命令。
4. 如可直接完成，实施最小正确变更并补充或更新测试；如存在必须先修复的具体阻塞项，更新 `TODO.md` 排入最小 prerequisite 后提交并停止。
5. 依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关测试，以及必要的完整测试/fixture 套件。
6. 任务完成后更新 `TODO.md` 的标题 `[DONE]` 与 completion record；仅当阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务的全部相关改动，然后停止，不继续下一个任务。

进度记录

- 已建立初始计划；下一步读取 `TODO.md` 确定当前任务。
- 已读取 `TODO.md`，首个未完成任务是 `TC-02：plain 路径（mir_body/）改 walk LIR 指令`。
- 下一步查看最新提交摘要，确认是否存在与 `TC-02` 直接相关的未完成事项。
- 最新提交为 `[TC-01-R] Review LIR lift totalization`，未发现需要在 `TC-02` 前插入的直接未完成事项。
- 下一步梳理 `codegen_plain_callable_entry`、`mir_body/` 和 `LirStatement`/`LirRvalue`/`LirTerminator` 的接口，先改入口遍历，再逐个迁移子模块。
- 已梳理 plain 路径：当前无本地 effect-control 的 plain callable 仍从 `LateLoweredSourceBody = mir::Body` 取 block/stmt，并通过 `ValuePrimitives` 与 `codegen_plain_terminator` 消费 MIR。
- 实施策略：新增 LIR local/use/slot/statement/terminator lowering 入口，切换 plain direct body 到 `callable.executable_body()`；不创建 LIR→MIR 反向转换。
- 已将 plain direct body 和 closure body emission 切到 `LirExecutableBody` state/statement/terminator；删除 plain route-safe pre-walk gate 辅助和对应 raw-route gate 单测。
- 已完成初步验证：`cargo fmt`、`cargo check -p scoopc_codegen_llvm`、`cargo test -p scoopc_codegen_llvm` 通过；多个 plain run-pass fixture 抽样可运行。下一步执行 clippy、完整 Rust 测试和 fixture 基线。
- `cargo clippy --all-targets -- -D warnings` 通过；`cargo test -p scoop --test p7_default_pipeline` 仍有 `single_pipeline_runs_higher_order_function_value_handled_effect_cli` 失败，定位为 LIR plain lowering 缺少原 MIR `ValuePrimitives` 中的 effect-typed closure adapter parity。
- 已在 `TODO.md` 将 `TC-02-PRE1：补齐 LIR plain lowering 的 effect-typed closure adapter` 插入到 `TC-02` 前，并把 `TC-02` 依赖更新为 `TC-01 + TC-02-PRE1`。本轮不标记 `TC-02` 完成。
