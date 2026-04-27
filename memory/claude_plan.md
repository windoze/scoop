# 执行计划记录

## 边界说明

本文件记录本次任务的可审计执行计划、关键依据、决策摘要、进度和验证结果。不会记录不可公开的内部推理流；所有影响实现的判断都会以结论和依据形式写在这里。

## 初始计划

1. 检查最新 Git 提交，确认提交信息或改动中是否提到已有问题、回归、临时方案或未完成边界。
2. 如最新提交暴露任何既有问题，优先修复这些问题；若必须作为前置任务排期，则更新 `TODO.md` 和 `PLAN.md` 后提交并停止。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 判断该任务是否过大；若过大，拆分为更小任务，更新 `PLAN.md` 和 `TODO.md`，提交拆分结果并停止或执行拆出的第一个任务。
5. 若任务可直接执行，先阅读相关实现、规格、测试和夹具，确认现有模式和正确实现边界。
6. 实现第一个未完成任务，不采用绕过、夹具专用逻辑或弱化规格的做法。
7. 添加或更新聚焦测试，运行相关测试；如变更风险较大，运行更广测试。
8. 若测试、编译或审查暴露任何既有 bug、规格不匹配或实现缺口，立即转为修复该问题；若不能立即修复，则按要求把它作为前置任务加入 `TODO.md`，更新 `PLAN.md`，提交并停止。
9. 完成后更新 `TODO.md` 标记该任务完成，并更新 `PLAN.md` 反映状态。
10. 运行最终验证，确保没有警告或明显回归。
11. 用清晰的任务提交信息提交全部相关变更。
12. 停止，不继续处理下一个任务。

## 当前状态

- 状态：已检查仓库状态、最新提交、`TODO.md`、`PLAN.md` 与 `ISSUES.md`。

## 最新提交与任务判断

- 最新提交：`da306355 [T5000i1P1] Fix build MIR request roots`。
- 该提交已修复 `ISSUES.md` 中一个 P1：build frontend 对 stdlib/sysroot support sources 的 request roots 过宽。
- 但同一 `ISSUES.md` 仍保留开放 P1：`MonomorphKey` 没有 call-site/source 来源，materializer 无法在 seed 阶段准确过滤非 request-source 请求。
- 按“最新提交提到的既有问题优先修复”的要求，本轮先处理该开放 P1，而不是继续 `TODO.md` 中的 `T5000i2`。

## 修复 P1 的执行计划

1. [DONE] 阅读 `monomorph`、typecheck monomorph key 收集、HIR lowering/materializer request seed、single-file frontend 与 build frontend 相关代码。
2. [DONE] 设计最小但规格正确的数据模型：
   - 保留 `MonomorphKey` 作为实例身份；
   - 新增带来源的 request wrapper，携带 `key`、`request_source_path` 与必要的 call-site span；
   - 让 materializer seed 阶段优先消费带来源 request，并基于 `request_source_paths` 过滤 initial seeds。
3. [DONE] 更新 typecheck 收集入口和 frontend 调用面，确保 request source 信息在收集阶段被保留。
4. [DONE] 更新 HIR lowering / MIR materializer API，避免只靠裸 `MonomorphKey` 推断请求来源。
5. [DONE] 增加回归测试：非 request support source 中的 generic 调用即使被收集到 request wrapper，也不能成为 initial seed；request source 中的同类请求仍能正常 seed。
6. [DONE] 更新 `ISSUES.md`、`TODO.md`、`PLAN.md`：把该 P1 记为已修复，并将其作为 `T5000i2` 的前置完成项。
7. [DONE] 运行聚焦测试、全量测试和 clippy，修复所有暴露问题。
8. [DONE] 提交本轮修复后停止。

## 提交记录

- 已提交：`[T5000i1P2] Fix monomorph request source filtering`

## 当前实现记录

- 新增 `MonomorphRequest { key, request_source_path, call_span }`，`MonomorphKey` 继续仅表示实例身份。
- typecheck 的 request-aware 入口 `check_file_exprs_with_monomorph_requests(...)` 会从 `TypeLowering` 当前 `SourceFile` 与调用点 span 记录来源。
- build frontend 与 single-file LLVM frontend 已改为收集并传递 `MonomorphRequest`。
- MIR materializer 现在在 `seed_requests(...)` 中按 `request_sources` 过滤 initial monomorph seeds；support source 中收集到的 request 不会因裸 key 进入实例根。
- 新增回归 `materializer_filters_initial_monomorph_requests_by_call_site_source`。

## 已运行验证

- `cargo check -p scoopc`：通过。
- `cargo test -p scoopc --no-run`：通过。
- `cargo test -p scoopc materializer_filters_initial_monomorph_requests_by_call_site_source -- --nocapture`：通过。
- `cargo test -p scoopc mir::materialize -- --nocapture`：通过。
- `cargo test -p scoop --no-run`：通过。
- `cargo test -p scoop build_frontend_ -- --nocapture`：通过。
- `cargo fmt --all`：通过。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo run -p scoop -- test`：通过，`fixtures: ok (1201)`。
- 最终变量名清理后复跑：
  - `cargo fmt --all --check`：通过。
  - `cargo test -p scoopc mir::materialize -- --nocapture`：通过。
  - `cargo clippy --all-targets -- -D warnings`：通过。
