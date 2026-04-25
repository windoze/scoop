# 执行计划

## 约束说明

- 按要求先记录执行计划，再开始读取仓库和执行命令。
- 计划文件会在关键步骤完成或计划变化时持续更新。
- 这里记录的是可审计的执行步骤与决策，不包含冗长的内部推理草稿。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否明确提到已有问题、回归、临时修复或待补事项。
2. 如果最新提交中提到需要先处理的既有问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 阅读 `PLAN.md`，核对当前计划、依赖关系和已有拆分是否与 `TODO.md` 一致。
5. 判断第一个未完成任务是否足够小且可以在本次调用中完整交付。
6. 如果任务过大或存在未建模依赖：
   - 在 `PLAN.md` 中补充分解后的计划；
   - 在 `TODO.md` 中把该任务拆成更小的前置子任务，并把当前应执行的第一个子任务排到最前；
   - 本次仅执行新的第一个子任务。

## 执行步骤

1. 在实现前先阅读相关代码、测试、规范或运行路径，确认修改边界。
2. 实现当前目标任务，不引入规避性 workaround。
3. 在实现过程中如果发现既有缺陷、规格不匹配、实现边界缺失或测试回归：
   - 立即将其视为当前范围内问题；
   - 若可以直接修复，则优先修复；
   - 若它阻塞当前任务且不能在本轮直接完成，则把修复任务作为前置任务写入 `TODO.md`，同步更新 `PLAN.md`，提交后停止。
4. 对改动运行充分验证，至少包括与任务直接相关的测试；若改动影响较广，还要运行更高层级验证。
5. 运行质量检查，目标包含：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时运行针对性的 fixture/spec 命令
6. 修复验证中发现的所有问题，直到相关检查通过或明确形成新的前置任务。

## 收尾步骤

1. 更新 `TODO.md`，将本次完成的唯一任务标记为完成。
2. 更新 `PLAN.md`，记录当前状态、已完成内容、后续影响和必要调整。
3. 再次更新本文件，记录关键结果与最终执行状态。
4. 检查工作区改动，确认未误改无关文件。
5. 使用清晰的 Git 提交信息提交本次改动。
6. 提交后停止，不继续处理下一个任务。

## 本轮目标

- 当前唯一执行任务：`T5000e1R Review：确认 InstanceKey 与 dump-ir materializer 的边界正确`。
- 最新提交 `75d109f18de5da63bb8ac7c95c6321ed04cb9b8e` 的提交正文仅为 `[T5000e1] Materialize dump-ir instances from MIR templates`，未显式挂出需要先修复的既有问题。

## 针对当前任务的执行细化

1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000e1` 与 `T5000e1R` 的范围、验收与完成记录。
2. 审查 `crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/monomorph/{mod,lower}.rs`、`crates/scoop/src/commands/dump_ir.rs`、`crates/scoopc/src/hir/lower/mod.rs` 的实现接缝。
3. 重点核对三件事：
   - `InstanceKey` 是否已经是最终实例身份，而 `MonomorphKey` 是否退回为“实例请求”；
   - `dump-ir` 是否只消费 generic MIR template + MIR materializer，而不是对实例重新做 HIR lowering；
   - per-`InstanceKey` cache 与 fixed-point 发现是否能稳定覆盖直接泛型调用与 nested closure family。
4. 若 review 暴露既有缺陷：
   - 能直接修复则立即修复，并补测试；
   - 若形成新的前置阻塞，则先更新 `TODO.md` / `PLAN.md` 后停止。
5. 运行与该 review 相关的验证命令，必要时补更窄的 targeted tests，再跑全量质量门禁。
6. 若 review 通过，则更新 `TODO.md`、`PLAN.md` 与本文件，提交本轮结果并停止。

## 当前状态

- 已完成：初始化计划；检查最新提交；定位首个未完成任务；完成 `T5000e1R` 的代码审查与最小复现探测。
- 已确认的阻塞点：
  1. `dump-ir` 对 imported / sysroot generic fun 仍会失败。
     - 复现：`cargo run -q -p scoop -- dump-ir /tmp/e1r_sysroot2.scoop`
     - 现象：`scoop.core.print<T>` 直接触发 `missing_generic_template`。
     - 原因摘要：
       - `record_monomorph_call(...)` 仍把声明文件写成调用点文件；
       - dump-ir materializer 只为当前输入源文件建立 template catalog。
  2. effect-row 泛型实例尚未进入 `InstanceKey` / materializer 闭环。
     - 复现：
       - `cargo run -q -p scoop -- dump-ir /tmp/e1r_eff.scoop`
       - `cargo run -q -p scoop -- dump-mir /tmp/e1r_eff.scoop`
     - 现象：
       - effect-only generic `forward<eff E>` 在 `dump-ir` 中返回空实例集；
       - `dump-mir` 中调用仍直接指向 generic `forward`。
     - 原因摘要：
       - monomorph 请求收集只在 `type_args` 非空时入队，`eff_args` 固定为空；
       - `InstanceKey` 的 `eff_args` 未进入 instance 命名、cache 与 fixed-point；
       - HIR lowering 当前未保留 effect-row 参数绑定语义。
- 计划调整：
  - `T5000e1R` 不能在当前状态下通过；
  - 已决定把阻塞点拆成前置任务并写入 `TODO.md` / `PLAN.md`：
    - `T5000e1a`：跨文件 / sysroot template identity 与请求声明源修复；
    - `T5000e1b`：effect-row 实参进入 `InstanceKey` / materializer 闭环。
- 进行中：更新 `TODO.md` / `PLAN.md` / 本文件，然后提交本轮“前置任务重排”结果并停止。
