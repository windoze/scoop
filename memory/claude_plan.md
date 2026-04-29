# 当前执行计划

说明：不记录内部私有推理；此文件仅记录可审计的执行计划、关键发现、实施步骤与进度更新。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如首个任务过大，拆分任务并更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
4. 实施当前任务所需代码修改，过程中若发现任何既有问题、规格不匹配或实现边界缺口，优先修复；若无法在本次直接修复，则先把前置任务插入 `TODO.md` 并更新 `PLAN.md`。
5. 运行与本次修改相关的测试，以及必要的格式化、lint、构建检查。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态或阻塞原因。
7. 按仓库约定创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 进度

- 已创建执行计划，下一步检查最新提交与 `TODO.md`。
- 已确认最新提交说明未显式记录新的待先修复问题；`TODO.md` 的首个未完成条目为 `T5000j3b1R Review：确认结构已知 closure/env 形状已进入 production MIR 主线`。

## 当前任务：T5000j3b1R

1. 审查 `T5000j3b1` 涉及的 production MIR bridge / reachability / pass artifacts 代码与回归测试。
2. 核对新增 closure 覆盖是否只消费既有 MIR 结构、summary/provenance/pass artifacts，而不是在 LLVM backend 现场重新推断 target-set。
3. 核对 `CaptureBox*`、opaque `FunValueCall`、`Return { value: None }`、effect/handle 等未支持形状是否仍稳定落回 HIR-compatible fallback。
4. 若发现既有边界泄漏或行为回退，先修复并补充回归；随后运行相关测试与 lint。
5. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，记录 review 结论并提交一次 Git commit。

## 当前发现

- 已确认一个真实边界问题：`crates/scoopc/src/llvm/codegen/mir_body.rs` 在发射 materialized MIR closure body 时，会把 child codegen 的 `current_source_id` 直接继承自调用者，而不是切到 closure 自身定义所在源文件。
- 影响：跨文件 closure 若在 production MIR 主线中被直接发射，其字面量解析、source-backed span 切片、以及依赖当前源文件的 call-site/source-path 查询都可能绑定到错误源码。
- 处理计划：
  1. 在 closure MIR 声明/发射入口按 closure 对应的 HIR 定义切换 `current_source_id`；
  2. 新增跨文件 closure production 回归，锁定 support source 中 closure body 的字面量/函数体会按正确定义源发射。

## 结果更新

- 修复方案已落地：`crates/scoopc/src/llvm/codegen/mir_body.rs` 现沿 closure `fn_ptr` 的 `.$lambda` owner 链回退到最近的非 lambda 宿主函数，并用该宿主函数的 `source_path` 恢复 materialized MIR closure body 的 `current_source_id`。
- 已新增回归：`crates/scoopc/src/llvm/tests.rs::production_codegen_uses_closure_definition_source_for_cross_file_raw_mir_body`，锁定跨文件 raw MIR closure body 会按定义源文件解析字面量。
- 已完成验证：
  1. `cargo fmt --all`
  2. `cargo fmt --all --check`
  3. `cargo test -p scoopc production_codegen_uses_closure_definition_source_for_cross_file_raw_mir_body -- --nocapture`
  4. `cargo test -p scoopc production_codegen_lowers_raw_mir_non_capturing_closure_body -- --nocapture`
  5. `cargo test -p scoopc production_codegen_lowers_raw_mir_immutable_capture_closure_body -- --nocapture`
  6. `cargo test -p scoopc llvm::tests -- --nocapture`
  7. `cargo test --all`
  8. `cargo clippy --all-targets -- -D warnings`
- 当前任务 `T5000j3b1R` 已完成；下一条应为 `T5000j3b2`。
