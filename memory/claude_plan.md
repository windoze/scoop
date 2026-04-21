# 执行计划记录

## 约束说明

- 按要求先写入本文件，再执行仓库检查、构建、测试或其他命令。
- 本文件记录的是可审计的执行思路、决策依据、步骤计划与后续进展，不包含逐字内部推理。
- 本轮目标是：先检查最新提交是否提到既有问题并优先修复；然后读取 `TODO.md`，完成第一个未完成任务；完成后更新计划文件、`TODO.md`、`PLAN.md`，运行相关测试，提交 Git commit，并停止。

## 初始执行步骤

1. 检查最新提交信息，确认是否明确提到已有问题、回归、临时方案或待修复项。
2. 读取 `TODO.md` 与 `PLAN.md`，识别当前第一个未完成任务，并理解依赖关系。
3. 评估该任务是否过大：
   - 若可直接完成，则进入实现。
   - 若过大或被前置缺陷阻塞，则先把缺陷或拆分后的前置子任务写入 `TODO.md`/`PLAN.md`，提交后停止。
4. 在实现前先收集最小必要上下文：
   - 相关源码模块；
   - 相关测试/fixtures；
   - 当前工作树状态，避免覆盖用户已有改动。
5. 实现任务时同步检查是否暴露出既有问题：
   - 若发现既有 bug、spec mismatch、回归、未完成边界，立即转为当前优先事项；
   - 修复后继续，或若无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 并停止。
6. 完成实现后运行充分验证：
   - 先跑与改动最相关的测试；
   - 再按需要跑更广的测试；
   - 满足要求时补跑 `cargo clippy --all-targets -- -D warnings`（若影响范围允许）。
7. 更新文档与计划：
   - 在本文件记录关键进展与结论；
   - 更新 `TODO.md` 完成状态或依赖顺序；
   - 更新 `PLAN.md` 的当前状态、拆分方案或阻塞原因。
8. 检查 `git diff`，确认只包含本轮应提交的改动，然后创建描述清晰的 commit。
9. 提交后停止，不继续做下一个任务。

## 风险与判定标准

- 不能通过缩小语义范围、改变表示方式、调整 fixture 形状、加入临时特判等方式绕过缺陷。
- 若任务依赖缺失语言特性、编译器能力、运行时行为或标准库支持，必须先把该缺口作为前置任务记录并调整顺序。
- 若发现仓库已存在与本任务无关的脏改动，默认不回退，仅读取并避让。

## 进展日志

- 已创建计划文件，尚未开始仓库检查。
- 已检查最新提交 `b0b7e6a7b42b556950ab09aa394d02b9a840f000`：
  - 提交主题为 `[T4016T1b] Reject function runtime casts`；
  - 提交消息本身未额外点名需要优先修复的既有 issue。
- 已读取 `TODO.md` 与 `PLAN.md`：
  - 当前顺序最前的实际未完成执行条目是 `T4016T1c`，其后依次为 `T4016T1R -> T4016T2 -> T4016T3`。
  - `T4016T1c` 目标是：对 opaque function values 统一以静态 function type 的 effect row 上界决定 may-suspend 编译，并补齐 wrapper/member/higher-order 返回等路径。
- 下一步：
  1. 阅读 `T4016T1c` 的具体说明与相关源码位置；
  2. 复现当前行为或现有缺口；
  3. 如发现比 `T4016T1c` 更早的既有缺陷，会先修复或写入前置任务；
  4. 否则直接实现 `T4016T1c`，补测试并完成提交。
- 已完成最小复现：
  - 新增临时 probe `memory/t4016t1c_probe_wrapper_member_effect.scoop`；
  - `dump-hir` 显示 `wrapper.f()` 的 callee `MemberAccess` 在 HIR 中仍是非精确类型；
  - 运行产物输出为：
    - `body-before`
    - `result`
    - `0`
  - 预期中的 handler arm（`caught` / `seed`）没有触发，确认这是实际语义错误，不只是测试覆盖不足。
- 当前判断：
  - `T4016T1c` 确实是当前最前的未完成任务，且问题已在其范围内；
  - 高概率是 planner 的 “function value may suspend” 判定仍只对局部变量/少数显式路径生效，而 codegen 另一侧已经能把 `MemberAccess` 当成 callable value 直接调用，导致两边结论脱节。
- 接下来：
  1. 定位 planner 可用的 concrete-type 恢复入口；
  2. 让 opaque callable 的 suspendability 统一取决于静态 function type 的 effect row；
  3. 补 member/wrapper/higher-order/branch 方向回归；
  4. 跑定向测试与全量相关验证。
- 已完成实现：
  - planner / suspend analysis / codegen 现在都能对 opaque callable 恢复更具体的函数类型，覆盖：
    - wrapper/member field direct call；
    - higher-order 返回值直接调用；
    - `block` / `if` / `when` 产出的函数值 callee；
    - object property / struct field / class field 上的函数值 concrete-type 恢复。
  - `typecheck/expr/call.rs` 已补齐 “callee 是普通表达式且其类型为 function/FunPtr” 的调用类型推导，因此 `choose(mode)()` 这类 higher-order 返回值直调不再在 typecheck 阶段被 `UnsupportedExpr { kind: "call" }` 拒绝。
  - 已新增回归：
    - Rust state-machine dump 单测：wrapper member direct call、higher-order returned function value direct call；
    - run-pass fixtures：`effect_indirect_perform_nonresuming_function_value_wrapper_member_direct*`、`effect_indirect_perform_nonresuming_function_value_higher_order_when_direct*`。
- 已完成验证：
  - `cargo test -p scoopc segment_dump_classifies_ -- --nocapture`
  - `cargo test -p scoopc unified_state_machine_transforms_all_segment_kinds_from_feature_matrix -- --nocapture`
  - 新增两个 fixture 均已单独 `build + run`，stdout 符合预期；
  - `cargo run -p scoop -- test --fixtures <temp-dir-with-new-fixtures>` 通过；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` 通过；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 通过；
  - `cargo run -p scoop -- test` 通过；
  - `cargo test --all` 通过；
  - `cargo run -p scoop_tools -- spec-fixtures check` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 剩余收尾：
  1. 更新 `TODO.md` / `PLAN.md` 标记 `T4016T1c` 完成；
  2. 检查 diff；
  3. 提交 commit 后停止。
