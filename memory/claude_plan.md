# 执行计划与进度记录

## 说明

按用户要求，本文件用于记录本轮执行的计划、关键判断依据摘要、执行进度和必要的计划调整。
这里记录的是可审计的步骤与理由摘要，不包含逐字的内部私有推理。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤计划

1. 检查最新一次 Git 提交，确认提交信息是否提到任何已知问题、后续修复项或未完成问题。
2. 若最新提交提到需要先修复的既有问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，确定第一个未完成任务。
4. 阅读 `PLAN.md`，确认当前计划、依赖顺序和任务背景。
5. 评估该任务是否足够小且可在本轮完整交付。
6. 如果任务过大或依赖缺失：
   - 在 `PLAN.md` 中拆分为更小子任务；
   - 在 `TODO.md` 中调整顺序和依赖；
   - 选择新的第一个子任务作为本轮执行对象。
7. 实现本轮目标任务，遵守现有规范和项目结构，不引入规避性实现。
8. 运行与改动相关的测试，并补充必要测试。
9. 运行格式化、检查和严格 lint，目标包括：
   - `cargo fmt`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   如任务影响范围较大，再补充更有针对性的命令。
10. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮任务标记为完成；
   - 在 `PLAN.md` 中更新当前状态与后续计划；
   - 如有必要，补充 `README.md` 或代码内注释。
11. 检查工作区变更，确认未误改无关内容。
12. 使用清晰提交信息提交本轮结果。
13. 停止，不继续下一个任务。

## 目前状态

- 已创建本文件并写入初始计划。
- 已检查最新提交。
- 已读取 `TODO.md` / `PLAN.md`。
- 尚未开始实现代码改动。

## 变更记录

- 2026-04-19：建立本轮执行计划文件。
- 2026-04-19：已检查最新提交 `2c7f01b8bb34395b7e9c17c98ecf685e86ee686a`，提交信息为“`[T4004b] 清理 run_pass_cone 生成产物`”，未提及需要先修复的既有问题，因此无需在本轮目标前插入额外修复任务。
- 2026-04-19：已读取 `TODO.md` 与 `PLAN.md`。第一个未完成的大项是 `T4004`，其拆分子任务 `T4004a1`、`T4004a2`、`T4004b` 已完成；当前首个需要实际执行的未完成子任务为 `T4004R`。

## 当前执行目标

### 目标任务

- `T4004R`：Review：确认顶层与局部 pattern binding 复用同一套语义。

### 当前判断

- 该任务是 review / 收口任务，规模适合在本轮直接完成，不需要继续拆分。
- 但若复审中发现顶层实现仍通过单独 ad-hoc lowering、匿名值旁路或重复求值等方式工作，则必须先修复问题，再补充回归，最后才能将该任务标记完成。

### 接下来执行步骤

1. 读取与 `T4004b` / 局部 destructuring / 顶层 immutable value 相关的实现代码，定位顶层 pattern binding 的 lowering 与 codegen 主线。
2. 验证顶层 path 是否直接复用局部 destructuring 的投影/校验 helper，以及普通顶层 immutable value 的 once-init / guard 主线。
3. 搜索是否存在为顶层 pattern 单独开的匿名值读取、顶层专用投影或其它 ad-hoc 分支。
4. 运行最小必要测试与 probe，确认：
   - initializer 只求值一次；
   - 顶层 tuple / struct / enum binder 在同文件与跨文件场景可执行；
   - 运行期匹配失败路径与局部规则一致；
   - 不存在因 review 暴露的回归。
5. 若发现问题，先修复并补回归。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，然后提交本轮结果并停止。

## 复审进展

### 已确认事项

- 顶层 pattern lowering 入口 `lower_top_level_pattern_val_items` 直接复用局部 destructuring 已有的 `synth_pattern_runtime_check_expr` 与 `synth_pattern_binding_init_expr`；没有新增顶层专用的 tuple/struct/variant 投影语义。
- 顶层 pattern binder 与隐藏 subject/check 都统一记录到 `top_level_immutable_values`，读取继续走普通顶层 immutable value 的 once-init 主线，而不是走匿名值读取旁路。
- LLVM `codegen_val_decl` 仍显式拒绝匿名 `val`，说明当前可执行路径不能靠“匿名 val + 特判读取”蒙混过关。

### 新发现的问题

- 在普通路径下，顶层 immutable value 访问会在内部 init call 之后做 `ordinary_call_effect_propagation_check`，再做 guard 已初始化检查。
- 但在 `try/catch` / `handle` 使用的 state-machine 路径中，顶层 immutable value 访问被当作隐藏 suspend boundary，却没有完整接入与 object init access 对齐的 active/inactive 处理：
  - `SuspendSiteKind::TopLevelValueInitAccess` 没有像 `ObjectInitAccess` 那样进入 inactive-continue 分支；
  - `codegen_top_level_immutable_value_access` 在 state-machine 环境里也仍会在 init call 之后立即检查 guard 是否 initialized。
- 结果是：当顶层 pattern 的隐藏 check 在 init 期间触发 `Raise.raise(RuntimeError.NullAssertionFailed)` 时，state-machine caller 还没来得及根据 active flag 走 handler dispatch，就先把 guard 的 `initializing` 状态误判成递归初始化并 `exit(1)`。
- 这与局部 pattern binding 的“抛出可捕获的 `RuntimeError`”语义不一致，因此 `T4004R` 目前不能直接标记完成。

### 修复计划

1. 修改顶层 immutable value 访问 codegen：在 state-machine / 非 ordinary propagation 场景下，init call 之后若 effect 已 active，则直接返回一个占位默认值给当前 boundary，跳过 guard initialized 检查与实际 load，让外层 suspend terminator 统一处理 active 路径。
2. 修改 state-machine emitter：让 `SuspendSiteKind::TopLevelValueInitAccess` 与 `ObjectInitAccess` 一样走 inactive/active 分支，而不是遗漏在 inactive-continue 路径之外。
3. 新增 run-pass 回归，覆盖：
   - handle 中读取成功匹配的顶层 pattern binder，inactive 路径应继续执行后续表达式；
   - handle 中读取 mismatch 的顶层 pattern binder，active 路径应进入 handler，不能 `exit(1)` 或继续执行 tail。
4. 完成修复后，重跑定向 fixtures、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 最终结果

### 已完成事项

- 已完成 `T4004R`，并同步将父任务 `T4004` 标记为完成。
- 已确认顶层与局部 pattern binding 的 tuple/struct/variant 校验、投影与 binder 提取复用同一组 lowering helper。
- 已修复一个复审中发现的既有 bug：
  - state-machine plan 先前不会把隐藏 suspend 的顶层 `VarRef` 当作 suspend subtree；
  - state-machine emitter 先前也漏把 `TopLevelValueInitAccess` 视为与 `ObjectInitAccess` 对齐的 inactive/active boundary；
  - `codegen_top_level_immutable_value_access` 在 state-machine 环境里先前会在 init call 之后立刻检查 guard/load，导致顶层 pattern mismatch 还没机会进入 handler dispatch 就被误判成递归初始化并 `exit(1)`。
- 已新增正式回归 `tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`，覆盖：
  - 顶层 pattern binder 在 handle 中匹配成功时继续执行；
  - 顶层 pattern binder 在 handle 中 mismatch 时进入 handler，而不是继续执行 tail 或退出进程。

### 已执行验证

- `target/debug/scoop run tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`
- `target/debug/scoop run tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
- `target/debug/scoop run tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
- `target/debug/scoop run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
- `target/debug/scoop run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`
- `target/debug/scoop run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
- `cargo fmt`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

### 当前状态

- 本轮目标已完成。
- `TODO.md` / `PLAN.md` 已更新，下一项为 `T4005`。
- 下一步只剩检查 diff、提交，并停止。
