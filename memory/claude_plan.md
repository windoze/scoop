# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在检查最新提交、阅读任务、实现、测试或审查过程中发现任何既有问题，则先修复该问题，或将其作为前置任务插入 `TODO.md` 并更新 `PLAN.md`，随后停止。

## 约束与执行原则

- 先检查最新提交是否提到需先修复的问题。
- 必须先读取 `TODO.md`，确认第一个未完成任务。
- 如果该任务过大，先拆分任务并更新 `TODO.md`、`PLAN.md`，本轮只做拆分后的第一个子任务。
- 发现任何既有 bug、回归、规格不匹配、未完成实现边界或临时规避逻辑，都必须立即处理，不能绕过。
- 实现后必须运行相关测试，并尽量补充必要测试。
- 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit，然后停止。

## 初始步骤

1. 查看最新一次 git commit，确认是否有明确提到尚未修复的问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 评估任务规模与依赖：
   - 若可直接完成，则进入实现。
   - 若过大或被前置缺陷阻塞，则先拆分/重排 `TODO.md` 与 `PLAN.md`。
4. 阅读相关代码、测试、规格或文档，确定正确实现路径，避免引入规避方案。
5. 修改代码并补充/调整测试。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响面较大，补充运行更高层级测试。
   - 收尾时检查格式、编译、测试及必要的 lint。
7. 更新 `TODO.md`、`PLAN.md` 与本文件中的进度记录。
8. 提交 git commit，提交信息应清晰描述本轮完成内容。

## 进度记录

- 已创建本计划文件，尚未开始仓库检查。
- 已检查最新提交：`[T5000g] Implement MIR devirtualization`，提交说明未额外列出需先修的问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T5000gR Review：确认 devirtualization 已经是结构驱动而不是热点特判`。
- review 过程中已定位一个阻塞 `T5000gR` 结论的既有边界问题：
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs` 里的 `try_codegen_class_vtable_call_impl(...)` / `try_codegen_interface_itable_call_impl(...)` 仍按 `callee FQN` 猜测“这是不是 dispatch call”，而没有消费 `LoweredHir.dispatch_call_sites`；
  - 因此即便主 build 路径已经通过 MIR/HIR 兼容层把某个调用去虚化为 direct target，backend 仍可能再次把它识别成 vtable/itable 路径；
  - class vtable 路径甚至还保留了 `try_devirtualize_class_vtable_call_target_impl(...)` 这条 backend 内部去虚化分支，说明 `VirtualCall -> DirectCall` 还没有完全收口为 MIR 层统一改写。
- 该问题属于用户要求中的“既有 bug / incomplete implementation boundary”，必须先修复后才能把 `T5000gR` 标记完成。
- 修复计划调整为：
  1. 将 `dispatch_call_sites` side table 接入 LLVM codegen 的编译单元共享上下文；
  2. 让 class/interface dispatch lowering 仅在当前 call site 被显式标记为 `Virtual` / `Interface` 时才走 vtable/itable；
  3. 删除 backend 内部的 class-vtable 去虚化猜测逻辑，避免 codegen 继续承担去虚化判定；
  4. 添加回归，锁定“via MIR instance collection 的 directized member call 不再被 backend 重新当成 vtable/itable dispatch”；
  5. 运行相关测试，随后更新 `TODO.md`、`PLAN.md` 与本文件。
- 修复过程中又暴露并解决了一个被旧 backend 猜测路径遮住的 HIR 缺口：
  - `crates/scoopc/src/hir/lower/stmt.rs` 的 custom-iterator `for` 语法糖此前手工拼出了 `iterator()/next()` top-level call，但没有同步写入 `dispatch_call_sites`；
  - 这会在 backend 不再按 FQN 猜 dispatch 后把 interface iterator 协议错误降成 direct symbol call；
  - 现已为这类 synthetic call 新增统一的 dispatch kind 判定 / devirtualization / side table 写入逻辑，并通过 `for_in_custom_iterator_basic.scoop` 端到端回归验证恢复行为。
- 已完成代码与文档更新：
  - LLVM codegen 现通过 `dispatch_call_sites` 决定 class/interface dispatch lowering，已移除 backend 内部 class-vtable 去虚化猜测；
  - `TODO.md` / `PLAN.md` 已把 `T5000gR` 标记完成，并记录 review 发现与修复内容；
  - 新增 LLVM 回归测试，锁定 directized class/interface 调用不会被 backend 重新解释为 dispatch。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm:: -- --nocapture`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/for_in_custom_iterator_basic.scoop`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：检查变更摘要并提交 git commit，然后停止本轮。
