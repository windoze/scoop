## 当前执行计划

说明：按要求记录简明执行计划与进度；不写入完整内部推理，仅记录可审计的步骤、发现与变更。

1. 查看最新一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大；若过大，则更新 `PLAN.md` 与 `TODO.md`，拆成更小子任务，并执行第一个子任务。
4. 实现当前目标任务，期间如果发现既有 bug / 回归 / 规格不匹配 / 缺失能力：
   - 先修复该问题；或
   - 若当前无法直接修复，则在 `TODO.md` 中把它加入为前置任务并调整顺序，然后停止。
5. 运行相关验证；至少覆盖与本任务直接相关的测试，并在可能时运行更高层级检查。
6. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞依赖。
7. 按仓库提交信息风格创建一次 Git 提交，然后停止，不继续下一个任务。

## 进度

- 已检查最新提交：`[T5001d2R] Fix explicit frame teardown on unreachable exits`，未发现提交信息中仍声明有待先修复的问题。
- 已读取 `TODO.md` / `PLAN.md`：当前首个未完成任务为 `T5001d3 把源码级 roots 与编译器内部临时 roots 全部收敛到 frame home slots`。
- 已完成首轮代码定位，发现当前实现中的关键缺口：
  - explicit frame slot 目前主要还是 safepoint 前后的临时 mirror，不是长期稳定 home slot；
  - `extra_gc_root_slots` 仍承担内部临时 roots 的动态 register/unregister 角色；
  - indirect GC aggregate params 目前只预留了 frame layout，但没有把 incoming slot 与 frame slot 建立持久映射；
  - hidden sret / deferred spill / native 边界保守 roots 仍部分依赖旧的 spill/native slot 路径补洞。
- 当前执行方案：
  1. 让 stack-backed GC root 的 store / 初始化路径同步写入 explicit frame leaf home slots。
  2. 把内部临时 roots 从“动态 extra root slot id”收口成固定 spill/home-slot 跟踪，并在失活时清 `NULL`。
  3. 补 indirect aggregate param 与 hidden sret 等缺口的 frame 同步。
  4. 增加 LLVM 回归，覆盖“参数/临时根确实写入 frame home slots、native 边界不再依赖旧 caller slot 数组形态”的关键合同。
- 已完成代码修改：
  - 源码级 stack-backed locals / aggregates 的 store 后会同步写入 explicit frame home slots；
  - indirect GC aggregate params 会在 entry 建立 slot 映射并预热到 frame；
  - 内部临时 roots 已从 `extra_gc_root_slots` / root-slot-id 动态注册切换为固定 spill/home-slot 跟踪；
  - ordinary safepoint keepalive 已改为从 frame home slot 读取，并在返回后同时写回原槽位与 frame home slot；
  - hidden sret 结果会在 call 返回后立即同步进 frame，并在 load 后清理其 home slot。
- 已完成验证：
  - `cargo test -p scoopc --lib`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
- 下一步：更新 `TODO.md` / `PLAN.md` 完成记录，检查工作区差异并创建本次提交，然后停止。
