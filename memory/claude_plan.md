# 执行计划（初始）

## 说明

按要求，我会先记录一份**可公开的推理摘要**与执行计划，再开始任何仓库检查与命令执行。这里不包含内部私有思维细节，但会完整记录可验证的判断依据、执行步骤、变更原因与后续进展。

## 当前目标

本次调用只完成一件事：处理 `TODO.md` 中**第一个未完成任务**，并在完成后停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，确定第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前项目计划、任务编号与依赖关系。
4. 如果第一个未完成任务过大，则将其拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 对目标任务做最小必要的代码与测试面调查，确认实现路径，并识别任何已存在问题。
6. 如果调查过程中发现既有 bug、回归、规格不匹配、实现边界不完整或依赖缺失：
   - 优先修复该问题；或
   - 如果无法在当前调用内直接修复，则把它作为前置任务插入 `TODO.md` 中目标任务之前，更新 `PLAN.md`，提交后停止。
7. 如果不存在阻塞问题，则完整实现当前目标任务。
8. 运行相关格式化、lint 与测试，至少覆盖：
   - 相关最小测试集；
   - 必要时运行更大范围回归；
   - `cargo clippy --all-targets -- -D warnings`（若工程当前配置允许）。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中将已完成任务标记为完成；
   - 在 `PLAN.md` 中记录当前状态和后续计划；
   - 按进度更新本文件。
10. 提交 Git commit，然后停止，不继续处理下一个任务。

## 约束

- 不接受 workaround、夹带特判、缩小规格、改造测试形状来绕过问题。
- 发现既有问题时，必须先修复，或将其显式加入 `TODO.md` 作为前置任务。
- 不回退用户已有改动。
- 所有输出与记录使用中文。

## 进度记录

- [x] 已写入初始计划
- [x] 检查最新提交
- [x] 读取 `TODO.md`
- [x] 读取 `PLAN.md`
- [x] 确定本次目标任务
- [x] 判断是否需要拆分任务
- [x] 实现或插入前置修复任务
- [x] 运行测试与 lint
- [x] 更新 `TODO.md` / `PLAN.md` / 本文件
- [ ] 提交 commit

## 已完成调查

1. 最新提交仅包含主题 `[T4015b] Extend comptime control flow and local execution`，没有在 commit message 中直接声明需要先修复的遗留问题。
2. `TODO.md` / `PLAN.md` 一致表明首个未完成任务是 `T4015c`：重新收口 `const fun` 的 effect-row / `eff` 参数 contract。
3. 当前实现现状：
   - `crates/scoopc/src/typecheck/headers.rs` 已在 header phase 拒绝：
     - `const fun` 的 non-`Pure` effect row；
     - `const fun` 的 `eff` 参数。
   - `crates/scoopc/src/comptime/interpreter.rs` 文档与防御性检查仍停留在“最小门禁 / unsupported signature”口径。
   - `SCOOP_FULL_SPEC.md` 的 `§6.2 const fun` 说明了“禁止任何 effect”，但没有把
     “只能省略 effect row 或显式写 `Pure/Pure!`，且不能声明 `<eff ...>`”
     写成明确的声明级合同。
   - `ISSUES.md` 第 12 条也仍把这部分描述成“header phase 一刀切早退”，而不是已经定稿的保守合同。

## 本轮实现决策

本轮采用**保守且明确**的 `const fun` 合同，不扩展到 effect-polymorphic `const fun`：

1. `const fun` 自身必须保持纯：
   - 可以省略 effect row；
   - 也可以显式声明 `/ Pure` 或 `/ Pure!`；
   - 不允许声明任何 non-`Pure` effect row。
2. `const fun` 不允许声明 `eff` 参数（`<eff E = ...>`）；
   这是明确的语言合同，而不是“解释器暂时不支持”的偶然限制。
3. 编译期解释器继续只执行纯计算；更复杂的 effectful/effect-polymorphic 形状不在本轮范围内。

## 具体执行步骤（更新）

1. 收口代码注释与防御性说明，使 header phase / comptime interpreter / 文档口径一致。
2. 在 `SCOOP_FULL_SPEC.md` 中显式写出 `const fun` 的 effect-row / `eff` 参数合同。
3. 视需要同步 `README.md` 与 `ISSUES.md`，把该限制从“隐式早退”改写为“明确 contract”。
4. 新增最小正向回归，证明显式 `/ Pure!` 的 `const fun` 能进入 comptime 求值主线。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 已完成实现

1. 代码与注释：
   - 已更新 `crates/scoopc/src/typecheck/headers.rs`，把 `const fun` 的 effect-row / `eff` 参数规则写成明确的声明级合同，并补充更精确的 help 文案。
   - 已更新 `crates/scoopc/src/comptime/mod.rs` 与 `crates/scoopc/src/comptime/interpreter.rs`，统一解释器侧说明，并加入对 non-`Pure` effect row 的防御性签名检查。
2. 文档：
   - 已更新 `SCOOP_FULL_SPEC.md` §6.2，明确 `const fun` 只能省略 effect row，或显式写 `/ Pure` / `/ Pure!`，且不能声明 `<eff ...>`。
   - 已同步 `README.md` 与 `ISSUES.md` 第 12 条，消除“header phase 一刀切早退”的旧叙事。
3. 回归：
   - 已新增 `tests/fixtures/comptime/const_fun_closed_pure_basic.scoop` / `.comptime`，锁定显式 `/ Pure!` 的 `const fun` 可进入 comptime 求值主线。
   - 已更新两个既有负例 fixture 的注释与错误位置，继续锁定 non-`Pure` effect row / `<eff ...>` 的 header-phase 报错。

## 已完成验证

- `cargo fmt --check`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
- `cargo run -p scoop -- test --fixtures tests/fixtures/comptime`
- `cargo run -p scoop_tools -- spec-fixtures check`
- `cargo run -p scoop -- test`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 当前结论

- `T4015c` 可按“保守且明确的纯契约”关闭，不需要继续拆分。
- 下一步应由后续调用执行 `T4015R`。
