# 执行计划与进度记录

说明：此文件记录可公开的执行计划、关键决策、发现的问题、进度更新与验证结果；不记录逐字内部推理。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息中是否提到任何已知遗留问题；若提到，则先定位并修复该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的背景、依赖和预期边界。
4. 评估该任务是否可以在本轮完整完成：
   - 若可以，直接实现。
   - 若过大或存在前置缺陷/缺失特性阻塞，则先把任务拆分或插入前置修复任务，并同步更新 `TODO.md` 与 `PLAN.md`。
5. 对实现或计划调整做充分验证：
   - 运行相关测试。
   - 按要求运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，若范围过大则至少先运行与改动直接相关的验证，并在必要时补全全量验证。
6. 完成后更新：
   - `TODO.md`：标记当前任务完成，或在阻塞时重排任务顺序并保留为待办。
   - `PLAN.md`：记录当前状态、依赖调整、下一步。
   - 本文件：追加关键进展与验证结论。
7. 生成一次 Git 提交，只完成一个任务后停止。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新一次 Git 提交 `598f4d5f3883650730a43ddd146cadebfb8c6bc0`（`[T5000b4b] Extract function/body codegen context`）。
  - 提交说明未直接点名需要先修复的遗留 issue，因此继续按 `TODO.md` 顺序推进。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务为 `T5000b4bR Review：确认 function/body 级上下文边界成立`。
- 已完成第一轮结构复核：
  - `MainCodegen` 上原先属于函数 / body 生命周期的字段访问已迁移为 `self.function_cx.*`，未发现旧字段仍直接残留在 `MainCodegen` 上的调用点；
  - `call/resume.rs` 的 callee resume entry 与 `effect/state_machine_emitter.rs` 的 step/dispatch runtime function 发射入口，已改为整组 `take_function_body_cx()` / `restore_function_body_cx()` 保存恢复；
  - `MainCodegen` 当前除 `function_cx` 外保留的主要可变状态只剩 `current_source_id` 与 effect 专属字段；其中 effect 专属字段正对应下一任务 `T5000b4c` 的范围，`current_source_id` 暂判断为通用 lowering/诊断上下文，而非明显遗漏的函数局部运行态。
- 正在做第二轮复核：
  - 继续确认 effect emitter 内剩余的 `return_context` / `current_fun_return_ty` 临时覆写是否只是函数内局部语义切换，而非仍需在本任务中进一步抽离的普通函数级上下文；
  - 运行测试与 lint，验证当前 review 结论没有遗漏真实回归。
- 第二轮复核已完成：
  - 已确认 effect emitter 入口的跨函数 / 跨 runtime-function 状态交换已经收敛为整组 `function_cx`，剩余单独保存的 `return_context` / `current_fun_return_ty` 仅是同一 runtime function 内的局部语义覆写；
  - 已确认 `MainCodegen` 中剩余未抽离的主要可变字段正好收敛为两类：`current_source_id`（generic lowering / 诊断上下文）与下一任务 `T5000b4c` 要处理的 effect emitter 专属状态。
- 验证已完成：
  - `cargo test -p scoopc llvm::` 通过；
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 当前结论：
  - `T5000b4bR` 可标记完成；
  - 未发现需要插入到 `T5000b4c` 之前的新前置缺陷任务；
  - 下一条任务为 `T5000b4c 抽出 effect/state-machine emitter 专用上下文`。
