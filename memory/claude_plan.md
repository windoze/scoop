# 执行计划

## 说明

用户要求先写入思路与执行计划。这里记录可审计的高层计划、决策依据、执行进度与后续调整；不写入冗长的内部推理细节，但会持续更新关键判断和已完成步骤。

## 初始计划

1. 检查最新一次 git 提交内容，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有上下文或依赖说明。
4. 若第一个未完成任务过大，则拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
5. 实现该任务所需代码修改。
6. 运行相关测试、格式化、`clippy` 或其他必要校验，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况。
8. 使用清晰的提交信息提交本次变更。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已完成：创建计划文件。
- 已完成：检查最新提交；提交信息仅为 `[T2003b2] Support immediate-resume if-branch perform`，没有额外正文或待先修复的遗留问题说明。
- 已完成：阅读 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T2003b3`。
- 已完成：评估任务规模，当前不再继续拆分子任务。
- 已完成：修改 `crates/scoopc/src/llvm/codegen/effect.rs`，补齐 immediate-resume `while` frame、扫描规则与 state0/state1 迭代恢复逻辑。
- 已完成：新增 fixtures：
  - `tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
  - `tests/fixtures/run-pass/effect_resume_while_body_single_perform.stdout`
  - `tests/fixtures/build/effect_resume_while_nested_perform_is_error.scoop`
- 已完成：通过新增正向/负向 fixture 的定点验证。
- 已完成：通过全量校验与 lint。

## 当前任务：T2003b3

- 目标：支持 immediate-resume 在 `while` 循环体中的 direct `perform`，并保证 resume 后能继续当前迭代尾部、再按循环条件决定是否进入下一次迭代。
- 约束：
  - 本轮只放开 `while` body 内“顶层 statement-position 的 direct perform（`val x = perform ...`）”。
  - `while` condition 中的 perform、`while` body 内更深层的嵌套 perform（例如再嵌进 `if` / `block` / 其他 value expression）继续视为未支持，但要给出稳定诊断。
  - 保持现有 one-shot `resume(value)` 语义；每次循环重新命中同一 perform 时，仍复用同一 resume 入口与局部槽位。

## 完成结果

1. immediate-resume 现已支持 `while` body 中的顶层 direct `perform`：
   - 初次命中时会进入 arm；
   - `resume(value)` 后会回到当前迭代尾部；
   - 后续迭代再次命中同一 `perform` 时，会复用同一 arm state machine 与 binding slot。
2. 仍未支持的形状已收口为稳定诊断：
   - `while` condition 中出现 matching perform；
   - `while` body 中更深层的 nested perform（例如再嵌进 `if` / `block` / value expression）。
3. 已完成验证：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - 定点命令：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
   - 定点命令：`cargo run -p scoop --features llvm -- build tests/fixtures/build/effect_resume_while_nested_perform_is_error.scoop --emit-llvm -o /tmp/effect_resume_while_nested_perform_is_error.ll`
4. 下一步仅剩：
   - 更新 git 状态并提交本轮 `T2003b3`。
