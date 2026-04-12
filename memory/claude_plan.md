# 本轮执行计划

## 约束说明

- 按用户要求，本文件会先于其他命令/代码执行被创建。
- 我不会写出不可审计的私有推理细节，但会持续记录可核查的思路摘要、执行步骤、关键判断与进度变更。
- 本轮目标是：先处理最新提交中提到的既有问题，再完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交的信息与改动，判断是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或子任务。
5. 运行相关测试与质量检查，至少覆盖受影响范围；若需要，补充或修复测试。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
7. 提交本轮改动，提交后停止，不继续下一个任务。

## 关键检查点

- 若发现规范不匹配、缺失能力、已有缺陷或依赖顺序错误：
  - 不做规避性实现；
  - 在 `TODO.md` 中添加/重排前置任务；
  - 在 `PLAN.md` 与本文件中记录原因；
  - 提交后停止。

## 进度

- 已创建本计划文件。
- 已检查最新提交 `af013aa7cac2e05d2f86232ceb2c843174e76f37`：
  - 提交说明为 `[T2003c0c2b1] Support no-immediate direct escape multi-arm dispatch`。
  - 提交本身没有额外声明一个尚未修复的既有缺陷；它主要完成了 `T2003c0c2b1` 并把后续工作拆分为 `T2003c0c2b2` / `T2003c0c2b3`。
- 已读取 `TODO.md` / `PLAN.md` 并确认首个未完成任务为：
  - `T2003c0c2b2 [TODO] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，single indirect escape site）`
- 当前判断：该任务规模可直接实现，不需要继续拆分。

## 当前任务理解

- 目标子集：
  - 无 immediate-resume；
  - 一个 escape-continuation arm；
  - 允许 0..N 个 sibling non-resuming arms；
  - body 中仅支持一个 top-level indirect call site 触发 escape effect；
  - continuation step 需要把 resume payload 写回 callee suspend state，并在 replay 期间继续支持 sibling non-resuming dispatch。
- 现状：
  - direct single-site 版本已存在：`codegen_handle_expr_escape_with_nonresuming_siblings_direct`。
  - mixed-arm immediate+escape 的 indirect 版本已存在，可复用其 callee-suspend replay 结构。
  - 当前无-immediate single indirect site 仍直接报错：`handle multi-arm without immediate-resume (single indirect escape site not yet supported)`。

## 实现计划（细化）

1. 在 `crates/scoopc/src/llvm/codegen/effect.rs` 中新增无-immediate + single indirect escape site 的 lowering 函数，并在 `codegen_handle_expr_escape_with_nonresuming_siblings` 中接入分派。
2. 复用现有 no-immediate direct 版本的 sibling non-resuming dispatch / cleanup 语义：
   - 主 body 中 sibling `Raise.raise` / custom non-resuming 的 dispatch；
   - continuation step 中 sibling dispatch；
   - `finally`、state/k 的 pin/unpin 与 detach/restore。
3. 复用 indirect continuation 现有 callee-suspend replay 语义：
   - step trampoline 写回 `resume_word + resume_gc_ref`；
   - 重新调用 indirect call site；
   - 继续执行 site 之后的 tail。
4. 新增 run-pass fixtures，至少覆盖：
   - escape-only indirect single-site；
   - escape + sibling non-resuming indirect single-site。
5. 运行 `cargo fmt`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings`。

## 风险点

- indirect site 的 body-lift/capture 集需要覆盖 call-site 本身，否则 closure 或 call 实参中的局部可能在 step replay 时丢失。
- body 执行期与 continuation step 都可能触发 sibling non-resuming，需要分别保证 dispatch/no-match/cleanup 语义一致。

## 执行记录

- 已在 `crates/scoopc/src/llvm/codegen/effect.rs` 中做了一轮无-immediate + single indirect escape multi-arm lowering 的探索性接线，并让工作树保持可编译。
- 为定位回归，我临时构造并编译了多组最小变体，得到关键结论：
  - 新路径样例里若在 indirect escape arm body 中直接使用 binder，例如 `println(seed)`，会报 `sysroot print/println arg type`；
  - 若改成 `seed + 1`，会报 `integer binary op lhs`；
  - 同样的 binder 使用方式在既有的 direct mixed-arm 路径可正常编译；
  - 但在既有 single-arm indirect escape-continuation 路径中也会复现同类错误。
- 结论：
  - 这不是本轮新增 multi-arm indirect lowering 独有的问题；
  - 真正的前置缺口是：**single-arm indirect escape-continuation 的 arm binder 并没有以真实 op 参数类型 materialize 到 LLVM codegen 环境中**。
  - 因此 `T2003c0c2b2` 被一个更基础、且此前未显式登记的既有问题阻塞。

## 已采取的调整

1. 在 `TODO.md` 中新增前置任务 `T2003c0c2b1a`：
   - 修正 indirect escape-continuation arm binder 的真实类型与 payload decode。
2. 将 `T2003c0c2b2` 的依赖改为 `T2003c0c2b1a`，并保留其为 `[TODO]`。
3. 在 `PLAN.md` 中记录本次阻塞原因与新的执行顺序。
4. 删除了本轮草拟但依赖该缺口被绕开的新 fixture 文件，避免把 workaround 留在仓库里。

## 已验证

- `cargo fmt --all`
- `cargo test -p scoopc --lib`
- `cargo run -p scoop --features llvm -- test`
- `cargo clippy --workspace --all-targets -- -D warnings`

以上均已通过。

## 当前状态

- 本轮不继续推进 `T2003c0c2b2`。
- 按流程，当前应提交“新增前置任务并顺延当前任务”的变更，然后停止。
