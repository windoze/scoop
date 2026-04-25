# 本轮执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在推进计划任务前，先检查最新提交是否提到已有问题；若提到，则优先修复该问题。
- 在执行过程中，任何发现的既有缺陷、规格不匹配、回归、未完成实现边界或依赖缺口，都必须立即纳入当前范围；如果它阻塞当前任务，则需要先修复，或将其作为前置任务写回 `TODO.md` 并停止。
- 不采用变通方案、特判、缩小规格或规避 broken path 的方式推进。

## 初始步骤

1. 查看最新一次 Git 提交，确认是否明确提到需要先修复的既有问题。
2. 打开 `TODO.md`，定位第一个未完成任务。
3. 打开 `PLAN.md`，核对现有计划与该任务的上下文。
4. 判断该任务是否过大：
   - 若可直接完成，则继续执行。
   - 若过大，则拆分为更小的子任务，更新 `PLAN.md` 和 `TODO.md`，然后执行新的第一个子任务。
5. 在实现前，补充本文件，记录对任务范围、依赖、风险和验证路径的理解。
6. 实现任务所需代码改动。
7. 运行相关测试，并在必要时修复测试暴露出的既有问题。
8. 运行格式化、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，以及与任务直接相关的额外命令；若全量测试过重，则至少先运行与改动直接相关的验证，并根据结果决定是否补充全量验证。
9. 更新 `TODO.md`，将本轮完成的任务标记为已完成。
10. 更新 `PLAN.md`，反映当前进展、剩余任务和任何新增依赖。
11. 更新本文件，记录完成情况、测试结果和提交前状态。
12. 使用清晰的 Git 提交消息提交改动，然后停止。

## 当前已知信息

- 最新提交为 `1f2f0140fb20e11b98a330c9df545f3d78f3313d`，标题为 `[T4014R] Review ordinary FFI boundary contract`；提交信息本身未显式提出需要优先修复的新遗留问题。
- `TODO.md` 中第一个未完成任务是 `T4015a [TODO] 收口 const fun 的解析 / 选择 / 跨文件调用主线`。
- `PLAN.md` 已明确当前主线已进入 `T4015a -> T4015b -> T4015c -> T4015R`。
- 工作树当前存在未提交改动：`memory/claude_plan.md`（本轮计划文件）与 `run_agent.sh`。后者视为既有改动，不回退、不擅自纳入本轮实现，除非任务需要。

## 已完成的上下文核对

1. 已核对 `TODO.md` / `PLAN.md` / `ISSUES.md`：
   - `ISSUES.md` 第 12 条与 `T4015a` 完全一致，明确指出当前缺口是：
     - `const fun` 解释器只支持同文件；
     - 只按“函数名 + 参数个数”做最小选择；
     - generic const fun / 统一 declaration context 尚未接入主线。
2. 已定位 `const/comptime` 当前实现：
   - `crates/scoopc/src/comptime/interpreter.rs`
     - `eval_const_bindings_in_file(...)` 只接收单个文件；
     - `ConstInterpreter` 仅维护当前文件的 `funs_by_name` / `types_by_name`；
     - `call_const_fun(...)` 只按 `callee_name + arity` 选择；
     - `eval_fun_call(...)` 直接拒绝 `receiver`、`type_params`、`eff_param` 等复杂签名。
   - `crates/scoopc/src/comptime/eval.rs`
     - 普通函数调用只通过 `host.call_fun(call_span, callee_name, type_args, args)` 下沉；
     - 这意味着解释器宿主已经拿到 `call_span`，可作为后续“按 typecheck 选定目标回放”的锚点。
3. 已定位可复用的统一主线：
   - `resolve` 已能为调用点写回 `ResolvedCall` 候选集合；
   - `typecheck` 已能在调用点完成 overload 选择、显式类型实参实例化、most-specific 选择与 monomorph key 记录；
   - `ast::File` / `TypeLowering` 已有多张 side table（例如 `TopLevelFunValueRef`、ctor/effect-op call binding），说明“把 typecheck 选中的绑定结果写回 AST 供后续阶段消费”是仓库已有模式。
4. 已确认当前 `comptime` fixture 入口仍是单文件：
   - `crates/scoop/src/fixtures/mod.rs` 的 `comptime_fixture(...)` 直接调用 `eval_const_bindings_in_file(...)`；
   - 仓库尚无 `comptime_multi` 一类多文件 fixture phase。

## 当前判断

- `T4015a` 涉及至少三块可分离工作：
  1. 为 const/comptime 建立统一的 resolve/typecheck 上下文，并让调用点能拿到“普通主线最终选择的目标”。
  2. 让解释器消费该选择结果，先打通跨文件与重载选择，而不是继续按“名字 + 参数个数”旁路。
  3. 支持 generic const fun 的实例化与必要的 type-substitution，使其不再被解释器以 `generic type params` 直接拒绝。
- 以上三块存在明显依赖顺序，且第 3 块与前两块相比复杂度更高；因此很可能需要将 `T4015a` 再拆分为更小子任务，然后执行第一个子任务。
- 在正式决定拆分前，还需要再确认：
  - 是否能通过新增 typecheck side table 来最小化改动面；
  - generic const fun 的最小可行支持边界应落在哪一层。

## 已做决策

- 已将 `T4015a` 正式拆分为：
  - `T4015a1`：接入 compilation-unit resolve/typecheck 绑定，让 non-generic 顶层 `const fun` 调用脱离“同文件 + 名字/参数个数”旁路。
  - `T4015a2`：支持 generic `const fun` 的实例化与 type-substitution。
- `TODO.md` 与 `PLAN.md` 已同步更新；本轮当前要完成的任务变为 `T4015a1`。

## T4015a1 实施计划

1. 为 typecheck 增加“顶层函数调用绑定”side table，记录调用点最终选中的顶层函数目标（至少包含 FQN、声明文件、声明 span；为后续 generic 扩展预留 type args）。
2. 在顶层函数调用的 typecheck 选择路径中写回该绑定，确保 non-generic overload / import / 可见性后的最终目标可被后续阶段复用。
3. 重构 `ConstInterpreter`：
   - 让其能管理多个文件的 source/AST 与当前执行上下文栈；
   - 函数/类型注册不再假设“只有当前文件”；
   - 调用执行不再假设 caller/callee 共用同一个 `SourceFile`。
4. 新增基于 compilation-unit 的 const-eval 入口，流程对齐：
   - 裁剪 package-level `comptime if`；
   - 构建 index / type env；
   - resolve + typecheck；
   - 用解释器执行目标文件的 `const val`。
5. 更新现有单文件入口/fixtures/unit tests 到新入口。
6. 增加并运行回归：
   - 跨文件 non-generic const 调用；
   - non-generic overload 选择；
   - 错误路径（例如 ambiguous / no matching / non-const callee）至少覆盖一种稳定诊断。
7. 若实现后 `ISSUES.md` 第 12 条需要收窄描述，则一并更新。

## 当前执行进展（本轮继续）

- 已重新运行 `cargo test -p scoopc comptime --lib`。
- 结果：编译通过，但仍有 6 个既有 `comptime` 单测失败；失败已不在 `const fun` 主线本身，而是暴露出“新前端接线后，旧 reflection/intrinsic 单测输入与真实前端语义不一致”的问题：
  - `nameOf/fieldsOf/getPlatform/alignOf/paramsOf/...` 这些 `scoop.core` 名字在普通前端里依赖 `import scoop.core.*`，但对应 Rust 单测源码目前未显式导入；
  - 单测/fixture 里本地声明 `annotation class Deprecated(val msg: String)`，而普通前端把未限定名 `@Deprecated` 视为 builtin annotation surface，因此会按 `scoop.core.Deprecated(message, replaceWith)` 校验，导致旧测试不再成立。
- 当前判断：
  - 第一类属于测试输入未对齐真实前端环境，应补显式 `import scoop.core.*`；
  - 第二类需要把测试里的本地注解名改成不与 builtin annotation 冲突的名字（例如 `Anno`），避免继续依赖旧解释器“按 simple name 私下读取”的非前端主线行为。
- 下一步：
  1. 更新 `crates/scoopc/src/comptime/tests.rs` 中相关 reflection / platform 单测源码；
  2. 同步更新 `tests/fixtures/comptime/*` 里使用本地 `Deprecated` 的 fixture；
  3. 复跑 `cargo test -p scoopc comptime --lib`，确认旧失败已清零；
  4. 在此基础上新增 `T4015a1` 的 cross-file / overload 单测，继续验证真正任务目标。
# 2026-04-25 本轮续做计划（T4015a1）

## 当前目标

- 延续上一轮进展，完成 `TODO.md` 中当前第一个未完成任务 `T4015a1`：让 non-generic 顶层 `const fun` 调用在 comptime 里复用 compilation-unit resolve/typecheck 绑定，不再依赖“同文件 + 名字/参数个数”旁路。
- 上一轮已经把主干管线接通；当前剩余阻塞是 `tests/fixtures/comptime/const_fun_string_methods.scoop` 失败，报错 `const fun 只能调用 const fun/编译器 intrinsic：scoop.core.indexOf`。

## 已知状态

- 最新提交 `1f2f0140fb20e11b98a330c9df545f3d78f3313d` 没有在提交信息里显式声明必须先修的遗留问题。
- `TODO.md` / `PLAN.md` 已经把 `T4015a` 拆成 `T4015a1` 和 `T4015a2`；本轮只处理 `T4015a1`。
- `cargo test -p scoopc comptime --lib` 在上一轮末尾已经通过。
- `cargo run -p scoop -- test --fixtures tests/fixtures/comptime` 仍失败，当前已知失败点是字符串方法 `indexOf` 在 `const fun` 内没有被 comptime 当作字符串 intrinsic 折叠。
- `crates/scoopc/src/comptime/eval.rs` 和 `crates/scoopc/src/comptime/interpreter.rs` 里存在上一轮加的 `eprintln!` 调试代码，本轮在定位完成后必须清理。

## 本轮最新进展

- 已通过新增单测确认根因：`const_fun_string_methods.scoop` 的失败不是解释器执行失败，而是 `check_file_exprs` 阶段把 `scoop.core.indexOf` 等 sysroot String 扩展函数当成普通非 const 函数，提前触发 `ConstFunCallForbidden`。
- 已在 `crates/scoopc/src/typecheck/expr/call.rs` 中补齐 const gate：把 comptime 解释器会直接以内建逻辑执行的 `scoop.core.substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 视作 const 上下文可调用目标。
- 已新增聚焦回归测试 `const_eval_const_fun_string_methods_match_fixture_behavior`，并确认通过。
- 在扩大验证到 `tests/fixtures/comptime` 时，又暴露出既有前端缺口：`const_fun_string_ops_basic.scoop` 中的 `String + String` 被误走成整数加法类型规则。
- 该问题未被绕过，已立即纳入本轮修复；当前已在 `crates/scoopc/src/typecheck/expr/ops.rs` 补充 `String + String -> String` 的内建规则，接下来继续验证。
- 在继续扩大验证时，又暴露出两层同一条既有缺口：
  - `splice_field_access_v0_basic.scoop` 里的 `const val P = Point { ... }` 没有被收进顶层值类型表；
  - 同一个 fixture 里的 `FieldMeta { name: "y" }` 被前端当作普通完整 struct literal 检查，错误要求补齐 `FieldMeta` 全部字段。
- 这两点都已经补上：
  - `crates/scoopc/src/typecheck/expr/collect.rs` 现在会推断“无注解顶层名字绑定”的类型，不再只覆盖 pattern binding；
  - `crates/scoopc/src/typecheck/expr/member.rs` 现在对 splice-field 的 struct 描述符执行专用 v0 检查：要求存在 `name: String`，且在 `name` 为字符串字面量时恢复精确字段类型。

## 本轮完成情况

- `cargo run -p scoop -- test --fixtures tests/fixtures/comptime` 已通过（`fixtures: ok (20)`）。
- `cargo test --all` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过；期间顺手清掉了 `crates/scoopc/src/comptime/tests.rs` 里已有的 `map_identity` 告警。
- 已回写任务状态：
  - `TODO.md`：`T4015a1` 标记为完成；
  - `PLAN.md`：当前主线切换到 `T4015a2`；
  - `ISSUES.md`：第 12 条已收窄，不再把 non-generic `const fun` 仍走“同文件 + 名字/参数个数”旁路当作现状。
- 下一步只剩提交本轮修改，然后停止，不推进 `T4015a2`。

## 执行步骤

1. 检查当前工作树，确认上一轮改动与用户已有改动状态，避免误覆盖。
2. 直接重跑 `cargo run -p scoop -- test --fixtures tests/fixtures/comptime`，读取当前调试输出，确认 `const_fun_string_methods.scoop` 里字符串方法调用在 AST/comptime 分派中的实际形状。
3. 阅读并修正 comptime 字符串成员调用识别逻辑，原则是修主线语义，不做 workaround：
   - 优先修 `crates/scoopc/src/comptime/eval.rs` 中 member-call intrinsic fast path；
   - 如前端绑定结果导致 `call_fun_or_intrinsic` 需要补充识别，则在 `crates/scoopc/src/comptime/interpreter.rs` 做与 typechecked 绑定一致的修正；
   - 不把 `sysroot/string.scoop` 的普通函数简单改成 `const fun`。
4. 移除所有临时 `eprintln!` 调试代码，确保最终实现干净。
5. 重新验证：
   - `cargo test -p scoopc comptime --lib`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/comptime`
   - 若通过，再按任务完成标准继续跑更大范围校验，至少覆盖 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`；必要时补 `cargo fmt`。
6. 若验证通过：
   - 更新 `TODO.md`，将 `T4015a1` 标记完成；
   - 更新 `PLAN.md` 说明 `T4015a1` 已完成、`T4015a2` 成为下一任务；
   - 视情况更新 `ISSUES.md` 对应条目，只移除本轮真正解决的部分；
   - 更新本文件记录关键完成情况；
   - 提交 git commit，然后停止。
7. 若遇到新的真实 blocker：
   - 先判定其是否属于现有遗留问题或 `T4015a1` 前置缺口；
   - 按要求把前置任务插入 `TODO.md`、更新 `PLAN.md` 和本文件，提交后停止。

## 额外约束提醒

- 全程中文沟通。
- 不回退用户已有改动，特别是 `run_agent.sh`。
- 只完成一个任务后停止，不推进到 `T4015a2`。
