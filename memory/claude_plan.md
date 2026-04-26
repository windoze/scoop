# 执行计划

说明：
- 你要求我在执行任何代码或命令前，先把计划写入此文件。我先记录当前可公开的执行计划与检查路径。
- 我不会在这里写入逐字的内部推理过程，但会持续维护足够详细的决策摘要、执行步骤、阻塞原因与状态更新，便于你检查进度。

当前目标：
- 完成 `TODO.md` 中第一个未完成任务，然后停止。

执行步骤：
1. 检查最新一次 Git 提交的信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划、依赖和任务上下文。
4. 如果首个未完成任务过大或存在隐藏前置依赖，先把它拆分为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`。
5. 对当前应执行的首个任务进行实现。
6. 运行相关测试、格式化、lint；发现既有问题时优先修复，若问题构成前置依赖，则先更新 `TODO.md` / `PLAN.md` 并停止在该前置任务。
7. 完成后更新 `TODO.md` 与 `PLAN.md` 的状态。
8. 提交 Git commit，提交信息应清晰描述本次完成的任务。
9. 停止，不进入下一个任务。

执行原则：
- 不接受规避实现边界的 workaround。
- 任何探查、测试、评审中发现的既有缺陷都立即纳入当前范围。
- 若被前置问题阻塞，必须先修复，或把该问题作为新的前置任务插入 `TODO.md` 后停止。

当前进展：
- 已检查最新提交 `e83218516fbf6e5f762b0e4f948f909e3e12c809`，提交说明未点名新的待修复既有问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T5000e2 把编译单元 frontend/build 路径的 instance collection / materialization 迁到 MIR 层`。
- 已确认当前 build / single-file LLVM frontend 仍直接调用 `hir::lower_for_compilation_unit_multi_files_with_type_env(...)`。
- 已确认该 HIR 多文件 lowering 主路径内部仍会：
  - 调用 `collect_generic_fun_instantiations(...)` 做 standalone generic fun 的 fixed-point 实例化；
  - 调用 `collect_generic_member_fun_instantiations(...)` 做 owner-specialized member fun eager 实例化。
- 已确认 `dump-ir` 的 MIR materializer 目前只服务 dump/debug 路径，还没有直接接到编译单元 build/frontend 主路径。

当前判断：
- `T5000e2` 的核心不是单纯“替换一个入口函数”，而是要把“实例集合的发现与缓存”迁到 MIR，同时给当前仍消费 HIR 的 LLVM codegen 保留兼容输入。
- 需要优先确认两个现实问题：
  1. 现有 build/frontend 路径是否存在 effect-row generic instance 无法正确实例化的既有缺陷；
  2. 现有 HIR member specialization 是否承担了 MIR 侧尚未覆盖的 owner/nominal 实例发现职责。

已完成事项：
- 已将过大的 `T5000e2` 拆分为 `T5000e2a`～`T5000e2c` 与对应 review，并回写 `TODO.md` / `PLAN.md`。
- 已复现 build/frontend 主路径的真实问题：`scoop build --emit-llvm` 会把 `wrap<Int, eff Boom>` 与 `wrap<Int, eff Zap>` 坍缩成同一个 `@"wrap::<Int>"` 符号；该问题现继续由后续 `T5000e2c` 跟踪。
- 已完成 `T5000e2a`：
  - 在 `crates/scoopc/src/mir/materialize.rs` 抽出 `materialize_compilation_unit_from_typechecked_inputs(...)`；
  - 将 compilation-unit template catalog 与 AST site binding 收集收口为可复用内部 API；
  - 新增回归测试，锁定基于同一组 typechecked inputs 的编译单元 materialization 会区分相同 type args、不同 effect-row 的实例身份。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::materialize -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

接下来的执行步骤：
1. 将 `T5000e2a` 标记完成，确保 `TODO.md` / `PLAN.md` 与实际状态一致。
2. 检查本轮变更并提交 Git commit。
3. 停止，本轮不进入 `T5000e2aR`。

状态：
- `T5000e2a` 已完成并通过验证，当前处于收尾记录与提交阶段。
