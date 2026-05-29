# 执行计划

## 当前约束

- `TODO.md` 是任务顺序、需求、依赖、验证和完成记录的唯一权威来源。
- 本次只完成 `TODO.md` 中第一个标题未以 `[DONE]` 开头的任务，然后停止。
- 不做开放式历史问题排查；只处理当前任务直接相关或验证中暴露且未被明确排期的失败。
- 若遇到阻塞当前任务的缺失特性、规格偏差或测试失败，优先修复；若不能在本次任务中完成，则在 `TODO.md` 中插入最小必要前置任务并提交后停止。
- `PLAN.md` 只在阶段级计划、依赖或完成标准发生变化时更新。
- 完成任务后必须更新 `TODO.md` 标题为 `[DONE]`，填写完成记录，运行要求的格式化、lint、测试/fixture 验证，并提交 Git commit。

## 初始执行步骤

1. 读取 `TODO.md`，严格按标题查找第一个未标记 `[DONE]` 的任务。
2. 查看最近提交信息，仅判断是否有与该任务直接相关的未完成事项。
3. 阅读当前任务正文、依赖、验收标准和完成记录，确定是否可以直接执行。
4. 如任务可执行，检查相关代码和测试，做最小正确实现。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，再运行相关测试；需要全量验证时使用不少于 30 分钟超时。
6. 若发现未被明确排期的测试/fixture 失败，修复或在 `TODO.md` 中加入最小前置/后续任务，且不把当前任务标为完成。
7. 完成后更新 `TODO.md`：任务标题加 `[DONE]`，补充完成记录和验证结果。
8. 仅当阶段级计划改变时更新 `PLAN.md`。
9. 检查 Git 状态和 diff，提交本次任务相关全部变更。
10. 提交后停止，不继续下一个任务。

## 进度记录

- 已创建初始执行计划；尚未读取 `TODO.md` 或运行任何项目命令。
- 已读取 `TODO.md`：第一个未完成任务是 `P0-T03：建立堆增长与字面量分配计数度量`。
- 已检查最近提交：`031191d6 [P0-T02R] Review immortal baseline`，未发现直接指向 `P0-T03` 的未完成事项。
- 已读取 `TODO-1.md`、`PLAN.md`、`GC_PACING.md`、`GC_IMMORTAL_FIX.md` 中与 `P0-T03` 相关的要求。

## P0-T03 具体执行计划

1. 定位现有 `scoop_gc_debug_*` helper、runtime C/Rust 测试组织、fixture/codegen IR 输出能力。
2. 为长程序堆增长建立诊断度量，优先作为不影响 pass/fail 的可复用测试或工具；baseline 下记录无界增长数值。
3. 为 String literal / `Platform` 读取建立 IR 中 `scoop_alloc_typed` 计数度量；baseline 下记录计数大于 0。
4. 保持运行期和编译期行为不变，仅新增度量/测试/fixture/脚本和文档记录。
5. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关 targeted 度量，再按任务要求运行 `cargo test --all --all-targets`；若 fixture 受影响或新增 fixture，则运行 fixture suite。
6. 更新 `TODO.md` 与 `TODO-1.md` 的 `P0-T03` 状态和完成记录；仅在阶段计划改变时才更新 `PLAN.md`。
7. 检查 diff/status 后提交 `[P0-T03] Establish GC allocation metrics`。

## P0-T03 落点决议

- 长程序堆增长度量落在现有 `crates/scoop_runtime/src/bin/gc_microbench.rs`，新增 `heap-growth` 场景；默认执行 10M 个小对象分配并按间隔记录 `allocated/freed/live/reserved` 曲线与峰值，不在循环中主动 GC，因此 baseline 能暴露无界增长。
- 字面量分配计数使用新增 `umb_fix` build fixture 加新增工具脚本：fixture 只覆盖一个 String literal 函数和一个 `getPlatform()` 函数；工具通过 `scoopc emit-artifact --kind llvm-ir` 生成 IR，并统计 `call/invoke @scoop_alloc_typed` 次数。
- `tools/run_fixtures.py` 已具备 `--emit-llvm` 与 IR substring/regex 断言能力，但没有通用“计数”语义；本任务不扩展 fixture runner，以免引入与当前度量无关的 runner 行为变化。

## P0-T03 当前执行结果

- 已新增 `gc_microbench heap-growth` 诊断场景，默认 10M 次 32-byte 小对象分配、每 1M 次采样。
- 已新增 `tests/fixtures/umb_fix/P0-T03-gc-metrics/pos_literal_alloc_metric.scoop` 与 `tools/literal_alloc_metric.py`，用于生成 LLVM IR 并统计 `call/invoke @scoop_alloc_typed`。
- targeted 验证通过：`cargo test -p scoop_runtime --bin gc_microbench` 通过；`python3 tools/literal_alloc_metric.py --expect-min 1` 与 `python3 tools/literal_alloc_metric.py --expect-calls 6` 输出 `scoop_alloc_typed_calls=6`；`python3 tools/run_fixtures.py tests/fixtures/umb_fix/P0-T03-gc-metrics/pos_literal_alloc_metric.scoop` 通过。
- 10M baseline heap-growth 度量已运行：`allocations=10000000`、`object_size=32`、`peak_allocated=320000000`、`peak_live=320000000`、`peak_reserved=322699264`，采样点显示 allocated/live 从 0 线性增长到 320000000，`freed=0`。
- 完整验证已通过：`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`（`fixtures: ok (1608)`）。随后只更新了任务记录和 Rust 顶部说明注释，并重新运行了 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`；fixture 移到可提交路径后也重新运行了 targeted 计数、单 fixture 与完整 fixture suite。
- 已将 `P0-T03` 在 `TODO.md` 与 `TODO-1.md` 中标为 `[DONE]`，并写入运行方式、baseline 数值与验证记录；`PLAN.md` 未变，因为阶段级计划未改变。
