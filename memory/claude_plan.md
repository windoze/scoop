# 本轮执行计划

## 目标

按照仓库根目录 `TODO.md` 的顺序，只完成第一个未完成任务；若发现前置缺陷、规范不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md`，并在本轮内只处理调整后的首个任务。

## 执行思路摘要

1. 检查最新提交信息，确认是否明确提到已有遗留问题；若有，先修复这些问题，再进入 `TODO.md` 任务流。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并判断是否需要拆分为更小子任务。
3. 若任务可直接执行，先梳理相关代码与测试位置，再实施修改；若存在规范缺口、实现边界缺失或任务阻塞，则先把缺口转化为新的前置任务并更新计划文件。
4. 对实现结果执行必要验证，优先运行与改动直接相关的测试；若任务落点较大，再补充工作区要求的格式化、lint 或更广范围测试。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态、依赖变化和验证结果。
6. 提交本轮所有变更，提交信息应清晰描述本轮完成的任务，然后停止，不继续处理下一个任务。

## 预期检查点

- 检查最新提交是否提及待修复问题
- 定位第一个未完成任务
- 判断是否需要拆分任务
- 实施代码修改
- 运行测试与必要的静态检查
- 更新计划与任务状态
- 生成提交

## 动态记录

- 已检查最新提交 `813a9a8d23835ed561c0236a72b072cf1dbf48a6`：
  - 提交仅更新 `PLAN.md` / `TODO.md` / `memory/claude_plan.md`，未修改生产代码。
  - 提交中前置暴露的遗留问题是 object member-call 的 receiver ABI / 表示失配，对应当前新前置任务 `T3010b2b0a0b`。
- 已读取 `TODO.md` / `PLAN.md`：
  - 当前第一个未完成任务是 `T3010b2b0a0b`：修正 object 单例值的 LLVM 表示与 `Ref` ABI 失配，恢复 object member call。
  - 后续 `T3010b2b0a`、`T3010b2b0R`、`T3010b2b1`、`T3010b2b` 都依赖该任务，因此本轮范围明确为先修复 object 单例值表示问题。
- 下一步：
  1. 定位 object 单例值生成、member call receiver lowering、以及相关 ABI 类型定义。
  2. 复现最小 verifier 失败用例，确认当前 IR 失配具体发生点。
  3. 修改表示/ABI 后补充或更新定向测试。
  4. 运行相关测试与质量门槛命令。
  5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 实施结果：
  - 已复现最小失败：`Helper.run()` 在 LLVM verifier 报 `ptr @__scoop_object_instance__a.Helper` 传给 `ptr addrspace(1)` receiver。
  - 已完成修复：
    - object 单例值改为 `ptr addrspace(1)` 全局槽 + once init 内 `scoop_alloc_typed` 分配的 header-only GC singleton object。
    - `codegen_object_value_access` 改为 init 后加载 GC-managed receiver。
    - object nominal runtime type check 接入 object type descriptor。
    - 新增 LLVM IR 单测与 run-pass fixture 覆盖 object member call。
- 验证结果：
  - `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture` 通过。
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/object_member_call_basic.scoop -o /tmp/object_member_call_basic.out && /tmp/object_member_call_basic.out` 输出与预期一致（`41`、`42`）。
  - 最小 repro `Helper.run()` 已可 `build`，不再出现 `module_verification_failed`。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 复跑后，首个失败点仍是已知 blocker `effect_escape_continuation_finally_arm_raise.scoop`，说明没有引入更早回归。
- 待收尾：
  1. 已把 `T3010b2b0a0b` 标记为完成。
  2. 已更新 `PLAN.md` 的执行顺序，下一项切换为 `T3010b2b0a`。
  3. 当前只剩生成本轮提交并停止。
