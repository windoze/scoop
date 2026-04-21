# 本轮执行计划（初始）

说明：本文件记录可审阅的推理摘要、执行计划、进度更新与关键决策，不包含逐字内部思维。

## 目标

按照 `TODO.md` 的顺序只完成第一个未完成任务；如果存在前置问题、规格不匹配或实现缺口，先把这些问题修复或转化为新的前置任务并更新计划，然后停止在本轮应停止的位置。

## 初始判断

1. 必须先检查最新一次 Git 提交，确认提交信息里是否提到任何已有问题；如果提到，这些问题都属于当前范围，必须先修复。
2. 随后读取 `TODO.md`，定位第一个未完成任务。
3. 如果该任务过大，需要把它拆分成可落地的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
4. 执行任务时，不能接受规避方案、夹具特判或与规范不一致的“临时可用”实现；一旦发现依赖缺失或规格偏差，必须先在 `TODO.md`/`PLAN.md` 中显式建模为前置任务。
5. 实现完成后，必须进行充分验证，至少覆盖与改动相关的测试，并检查无告警构建/静态检查要求是否满足。
6. 完成后需要：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 视进展更新本文件
   - 提交 Git commit
   - 停止，不继续做下一个任务

## 分步计划

1. 检查最新提交：
   - 读取最新 commit 的 message 和必要上下文
   - 判断是否提到待修复的既有问题
2. 读取任务清单：
   - 打开 `TODO.md`
   - 定位第一个未完成项
   - 评估复杂度与前置依赖
3. 必要时拆分任务：
   - 更新 `PLAN.md`
   - 更新 `TODO.md`
   - 重新确认本轮要执行的首个子任务
4. 实施改动：
   - 阅读相关代码与测试
   - 实现最小但完整的规范正确改动
5. 验证：
   - 运行相关测试
   - 运行必要的全局检查（至少包含无告警要求对应的检查，若成本可接受则覆盖更广）
6. 文档与提交：
   - 标记任务完成或记录阻塞调整
   - 更新 `PLAN.md`
   - 更新本文件的结果与剩余风险
   - 提交 Git commit

## 进度

- [x] 已创建本文件并写入初始计划。
- [x] 检查最新提交。
- [x] 读取 `TODO.md`。
- [x] 确定本轮目标任务为 `T4012b2`（`@Deprecated` 的 declaration/use-site warning 合同）。
- [ ] 测试、更新文档并提交。

## 最新进展（2026-04-21）

1. 已检查最新提交 `ed3815186e9d03b5a85db88328b231d63ca14bb5`，提交说明未额外声明需要优先修复的遗留问题。
2. 已读取 `TODO.md` / `PLAN.md`：
   - `T4016` 组的子任务与 review 均已完成，当前文件中保留为 `[TODO]` 更像汇总状态未同步，不构成本轮新的实现目标。
   - 按当前计划与顺序，下一项可执行的叶子任务是 `T4012b2`：为 `@Deprecated` 建立最小可测的 declaration/use-site warning 合同。
3. 下一步将集中阅读：
   - built-in annotation 定义与 typecheck 入口
   - 诊断/告警基础设施
   - 声明元数据是否能跨文件携带到 use-site

## 当前实现方案（定稿前摘要）

1. `@Deprecated` 不仅需要 use-site warning，还需要先补齐 sysroot declaration surface；当前 `sysroot/core.scoop` 缺少 `Deprecated` 声明。
2. 现有工程没有通用的结构化编译 warning 通道，只有少量 `tracing::warn!`。本轮将新增一个轻量 warning capture/print 机制，供 `scoop build/run` 统一输出 warning，并在内部做去重。
3. deprecation 元数据不会塞进 `Index`，而是挂到 `TypeEnv`：
   - `TypeEnv` 在所有用户源文件 + sysroot 全部纳入后、表达式 typecheck 前统一构建；
   - 这正好满足跨文件 / sysroot 声明元数据传播的需要。
4. use-site warning 挂点将尽量集中：
   - 类型使用：`TypeLowering::lower_type_path`
   - 顶层值/对象引用：值 ident 推导与赋值 lhs
   - 成员/扩展属性引用：成员解析写回点
   - 函数/扩展函数/成员方法调用：最终选定 overload 的少数中心路径
   - 构造调用：选定 nominal ctor / enum variant ctor 的中心路径
5. `T4012b2` 先按任务描述完成“最小可测合同”：
   - built-in `@Deprecated(message, replaceWith?)`
   - target contract 先覆盖当前可稳定告警的函数 / 类型 / 属性路径
   - regression 通过 typecheck 失败用例 + run-pass/单元测试覆盖
## 2026-04-21 继续执行计划（接手续做）

### 已知上下文
- 本轮目标仍然是只完成 `TODO.md` 中第一个可执行未完成叶子任务 `T4012b2`：为 `@Deprecated` 建立最小可测的 declaration/use-site warning 合同，然后停止。
- 已检查最新提交 `ed3815186e9d03b5a85db88328b231d63ca14bb5`，提交说明里没有额外声明必须先修复的历史问题。
- 上一轮已经完成一部分实现：新增 warning capture 基础设施、在 `TypeEnv` 收集 deprecated 元数据、在 type lowering 和部分 value/property 使用路径上发 warning，并将 `@Deprecated` 纳入 built-in annotation 检查路径。
- 当前仍存在明确缺口：`sysroot/core.scoop` 尚未定义 `Deprecated` 注解；CLI 还没有打印捕获到的 warning；函数/方法调用与函数值引用 warning 还未全部接通；还缺少 fixtures；还未跑测试和 `clippy`；任务尚未在 `TODO.md` / `PLAN.md` 中完成落账。

### 约束与原则
- 全程中文记录。
- 只完成一个任务 `T4012b2`，完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit 后停止。
- 不允许 workaround；若发现规范缺口或实现边界阻塞正确实现，必须先把该缺口显式记入 `TODO.md` 并调整依赖顺序，然后提交并停止。
- 手工文件编辑使用 `apply_patch`。
- 需要完成实现、测试、文档落账和提交；质量门槛包含相关测试以及 `cargo clippy --all-targets -- -D warnings`。

### 分步执行计划
1. 读取 `TODO.md`、`PLAN.md`、相关源码文件，确认 `T4012b2` 仍是第一个未完成叶子任务，并核对目前代码状态与上轮摘要是否一致。
2. 完成语言/前端剩余实现：
   - 在 `sysroot/core.scoop` 中补齐 `annotation class Deprecated(...)` 定义。
   - 在 warning 模块补充 CLI 打印 warning 所需的只读访问接口。
   - 在 `scoop build` 路径中安装 warning capture，并把编译 warning 统一打印到 stderr，包含 `path:line:col` 与 warning kind/message。
   - 接通函数/方法调用、扩展调用、函数值引用等 use-site deprecation warning，确保 declaration-site 元数据能够在实际使用点发出稳定 warning。
3. 增加最小但充分的测试/fixtures：
   - typecheck 侧覆盖 `@Deprecated` 非法 target 与参数规则。
   - run-pass 侧至少覆盖 deprecated function call、deprecated type use、deprecated value/property use 的 warning 输出合同。
4. 运行验证：
   - 先跑针对性的 fixture 测试和必要的 Rust 测试。
   - 再跑 `cargo test --all`。
   - 最后跑 `cargo clippy --all-targets -- -D warnings`。
   - 若验证中暴露真实缺陷，先修复并回归测试；若暴露新的规范缺口且无法在本任务内直接正确完成，则按要求回写 `TODO.md` / `PLAN.md` 后停止。
5. 任务完成后落账：
   - 将 `TODO.md` 中 `T4012b2` 标记为完成。
   - 更新 `PLAN.md` 记录本次完成内容和验证命令。
   - 更新本文件记录关键步骤完成情况与任何计划调整。
   - 生成一次清晰的 git commit，然后停止，不继续处理下一个任务。

### 当前判断
- 依据已有实现和剩余缺口判断，`T4012b2` 仍然是一个可在本轮完成的单任务，不需要进一步拆分。

## 2026-04-21 实施进展（第一批代码已落地）

### 已完成的关键步骤
1. 已在 `sysroot/core.scoop` 补齐 `annotation class Deprecated(val message: String = "", val replaceWith: String = "")`，使 built-in `@Deprecated` 拥有真实的 sysroot 声明面。
2. 已在 `crates/scoopc/src/warnings.rs` 增加 `file()` / `span()` 只读访问接口，供 driver 计算定位并打印 warning。
3. 已在 `crates/scoop/src/commands/build.rs` 接入 warning capture：
   - `run_frontend(...)` 之前安装 capture；
   - 前端成功后统一把 warning 打到 stderr；
   - 输出格式为 `path:line:col: warn[...] ...`。
4. 已在 `crates/scoopc/src/typecheck/expr/call.rs` 接通函数 use-site warning：
   - 顶层函数调用；
   - 顶层函数重载调用；
   - 成员方法/扩展函数调用；
   - 单扩展候选调用；
   - 函数值引用（top-level function value）。

### 当前剩余工作
1. 增加 `T4012b2` 的 typecheck / run-pass fixtures。
2. 编译并运行定向测试，修复回归。
3. 通过 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
4. 完成 `TODO.md` / `PLAN.md` 落账并提交。

## 2026-04-21 收尾进展（验证完成，准备提交）

### 新增回归
1. typecheck：
   - `deprecated_file_annotation_is_error.scoop`
   - `deprecated_second_positional_arg_is_error.scoop`
   - `deprecated_message_must_be_string_is_error.scoop`
2. run-pass：
   - `deprecated_fun_call_warning_basic.scoop`
   - `deprecated_type_use_warning_basic.scoop`
   - `deprecated_property_use_warning_basic.scoop`

### 已完成验证
1. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：通过（`fixtures: ok (371)`）。
2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：通过（`fixtures: ok (378)`）。
3. `cargo test --all`：通过。
4. `cargo clippy --all-targets -- -D warnings`：通过。

### 最终结论
1. `T4012b2` 已达到“最小可测的 declaration/use-site warning 合同”目标：
   - built-in `@Deprecated` 有稳定的 sysroot 声明面；
   - declaration metadata 能跨文件带到 use-site；
   - `scoop build/run` 能把 deprecation warning 稳定打印到 stderr；
   - 函数、类型、顶层属性 use-site 已有回归覆盖。
2. 下一步仅剩更新 `TODO.md` / `PLAN.md` 落账并提交 commit；本轮提交后停止，不进入 `T4012b3`。
