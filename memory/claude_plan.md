# 当前执行计划

## 约束说明

- 本文件记录本轮执行的计划、关键判断、进度更新与结论。
- 这里提供的是可审阅的高层推理摘要与执行步骤，不包含冗长的内部草稿式思考。
- 本轮目标是：先检查最新提交是否提到既有问题；若有则优先修复。随后读取 `TODO.md`，只完成第一个未完成任务并停止。

## 初始步骤

1. 查看最新一次 Git 提交信息，确认是否明确提到待修复的问题。
2. 检查工作区状态，避免误覆盖现有改动。
3. 读取 `TODO.md`、`PLAN.md`，识别第一个未完成任务及其上下文。
4. 判断该任务是否过大：
   - 若可在本轮完整实现，则直接实现、测试、更新文档并提交。
   - 若过大或被前置缺陷阻塞，则先拆分任务或补充前置任务，更新 `TODO.md` / `PLAN.md` 后提交并停止。

## 执行标准

- 不接受规避性实现、夹具特判或偏离规范的临时方案。
- 需要补测并运行相关验证；若改动范围允许，还要检查 `cargo clippy --all-targets -- -D warnings`。
- 完成后必须同步更新：
  - `TODO.md`
  - `PLAN.md`
  - 本文件
- 最后创建一次 Git 提交，只处理一个任务后停止。

## 进度记录

- 2026-04-19：已创建本计划文件，准备开始检查最新提交与任务列表。
- 2026-04-19：已检查最新提交 `ec2a3bd234ffe66149325fd44eadde54a32941a8`，提交内容是记录 `T4008cP` blocker，而非已修复的遗留问题；当前工作区仅有本文件的未提交修改。
- 2026-04-19：已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T4008cP`：修复普通 `perform` lowering 在多实参 payload 下只传递第 0 个实参的问题。其目标是为后续 `T4008cS` 与 receiver effect op 共用统一 transport 合同。
- 2026-04-19：下一步执行：
  1. 定位 `codegen_perform_expr` 与相关 effect payload transport / handler binder 读取代码。
  2. 构造或运行最小复现场景，确认当前失败形态。
  3. 实现普通 `perform` lowering 的多 payload 传输修复，并检查是否影响现有单 payload / 零 payload 路径。
  4. 新增或更新 run-pass / LLVM regression，覆盖“普通 callee perform 两个 payload，由外层 handle 读取两个 binder”。
  5. 运行定向测试；若改动范围允许，补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  6. 更新 `TODO.md`、`PLAN.md` 与本文件后提交。
- 2026-04-19：实现已完成。核心设计不是“给 perform slot 再塞一个特判分支”，而是把多 payload 收口为共享 transport 合同：
  1. typecheck 记录 effect-op `arg_mapping`，保留命名 / 位置实参到形参顺序的稳定绑定。
  2. HIR lowering 为普通 `perform` 记录 `EffectOpCallInfo { arg_mapping, payload_tuple_ty }`，并为多 binder handler arm 记录 `handle_payload_tuple_tys`。
  3. LLVM `codegen_perform_expr` 对 2+ payload 统一按“源码顺序求值、形参顺序打包 tuple transport value”的方式编码；handler 侧再把 transported tuple 一次性解码后按 binder 顺序投影。
- 2026-04-19：验证结果：
  - 最小 probe `fun go(): Int / Edge { return Edge.visit(3, 4) }` + 外层 `handle { go() } ...` 已输出 `3`、`4`，退出码为 `7`。
  - 新增 run-pass `tests/fixtures/run-pass/effect_indirect_multi_payload_transport_basic.scoop` 已通过定向 `scoop run`（stdout `left / 6`，退出码 `10`）。
  - LLVM 单测 `cargo test -p scoopc indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport` 已通过。
  - 全量门禁已通过：`cargo fmt --check`、`cargo run -q -p scoop -- test`（`fixtures: ok (1061)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 2026-04-19：下一步只剩同步 `TODO.md` / `PLAN.md`、检查工作区并创建提交；提交后停止，本轮不继续处理 `T4008cS`。
