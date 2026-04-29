## 当前执行计划

说明：我不会记录或暴露内部私有推理过程，但会持续维护一份可公开的执行计划与进度摘要，便于检查当前工作状态。

1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题；如果有，先处理该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与任务背景。
4. 判断该任务是否过大；如果过大，则将其拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
5. 实施当前目标任务所需的最小正确改动。
6. 运行相关测试、检查和必要的验证；若发现既有问题或规格不匹配，优先修复，或按要求将其加入 `TODO.md` 作为前置任务并停止。
7. 完成后更新 `TODO.md` 与 `PLAN.md`，记录任务完成情况或阻塞原因。
8. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 进度

- 已创建初始执行计划，下一步开始检查最新提交与任务列表。
- 已检查最新提交、`TODO.md`、`PLAN.md`。
- 最新提交未声明需要先处理的新旧问题；当前首个未完成任务为 `T5001f2`：修复 `class_init_order_primary_secondary_basic.scoop` 的类初始化顺序回归。
- 下一步：先复现该 fixture 的失败现象，再检查类构造/属性初始化 lowering 与相关回归测试，定位 primary ctor 参数属性、property initializer、`init {}`、secondary ctor body 的执行顺序缺口。
- 已复现并定位根因：不是源码级 class init 顺序调度错误，而是 ctor-inline `this` local 只做了裸 `store`，没有同步 explicit-frame home slot；property initializer 中的 `println(...)` 先触发 safepoint 后，`this.x` 会从未同步的 `this` 槽位 reload，导致运行期段错误。
- 已完成修复：把 `class_ctor.rs` 中 `this` 的初始化改为 `store_local_value_exact(...)`，并新增 LLVM 回归 `class_ctor_this_local_reloads_from_explicit_frame_after_safepoint`。
- 已完成验证：
  - `cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`
- 继续执行 `cargo run -p scoop -- test` 时发现新的既有回归：`effect_escape_continuation_gc_stress_multi_string.scoop` 的 stdout 与 golden 不一致。按要求，已把该问题前置为新的 TODO 任务 `T5001f3/T5001f3R`，本次将在更新计划文件并提交后停止。
