# 本轮计划（执行前）

## 目标

本轮只处理 `TODO.md` 中第一个未完成任务，并在完成后停止。根据上一轮交接摘要，当前预期的下一项任务是 `T4010a2`，主题是为 `with` copy-update 补全 enum payload 语义与实现；但在真正实施前，仍需先核对最新提交、`TODO.md`、`PLAN.md` 与工作树状态。

## 决策依据摘要

- 上一轮已经完成 `T4010a1`，并提交为 `790722d`。
- `with` 对 struct/tuple 的统一扩展已经落地，enum payload 仍未支持。
- 当前已知边界是：如果 base 不是 struct/tuple，则类型检查阶段仍报 `with_update_base_not_supported`。
- 这一轮的关键风险不在“怎么把 enum 硬接进去”，而在“先把 enum payload 的静态语义定义清楚”，否则容易引入 shape-based 特判或与既有 member access 语义冲突。

## 执行步骤

1. 检查最新提交说明，确认是否提到了需优先修复的既有问题。
2. 核对 `TODO.md`，确认第一个未完成任务确实是 `T4010a2`；同时阅读 `PLAN.md` 获取当前任务拆分与依赖背景。
3. 检查工作树状态，确认是否存在未提交变更；若有，谨慎判断是否为当前任务相关内容，避免覆盖用户改动。
4. 阅读与 `with`、enum payload、member access、类型检查和 lowering 相关的实现与测试，明确当前语义边界：
   - parser / AST 中 `with` 路径如何表示；
   - typecheck 当前如何为 `with` 记录路径前缀的 aggregate 类型；
   - lowering 当前如何根据 aggregate 类型重建 struct/tuple；
   - enum payload 现有的字段/位置访问在 parser、typecheck、lowering 中是否已有统一主线。
5. 判断 `T4010a2` 是否可以在本轮完整闭环：
   - 如果语义可以清晰落地，则直接实现、补测试、跑验证；
   - 如果发现缺少明确前置能力或存在规格不一致，则按用户要求修改 `TODO.md` / `PLAN.md` 重新排依赖，并提交后停止。
6. 若实施：
   - 扩展类型检查，让 enum payload 的 `with` 有明确且可诊断的静态规则；
   - 扩展 lowering，在不破坏“base 只求值一次”和冲突检测的前提下，支持 enum payload 更新；
   - 增加 fixture / 单测覆盖成功路径、错误路径、嵌套路径与单次求值语义。
7. 运行必要验证，至少覆盖：
   - 相关单测；
   - 相关 fixture 套件；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞原因。
9. 提交本轮改动，提交信息使用任务号前缀；然后停止。

## 预期输出

- 若任务可完成：代码、测试、计划文档与任务状态一并更新，并形成单个提交。
- 若任务不可直接完成：补充前置任务、重排依赖、更新计划文档，并形成单个提交。

## 执行中更新（已确认）

- 已检查最新提交 `790722d [T4010a1] Generalize with-update to tuples`；提交信息本身未显式提到需要先修的额外既有问题。
- 已核对 `TODO.md` / `PLAN.md`：当前第一项未完成任务确认为 `T4010a2`。
- 已用最小探针确认当前边界：
  - 普通 enum payload member access 仍未打通：`r.value` 当前报 `scoop::resolve::unresolved_member`。
  - enum variant pattern / `when` 解构主线可执行。
  - enum `with` 当前仍报 `scoop::typecheck::with_update_base_not_supported`。

## 当前实现决策

- 本轮不把 enum payload `with` 建立在“未完成的普通 enum payload member access”之上，也不采用按字段名全局猜 variant 的规则。
- 采用显式 variant 前缀语义：
  - enum 路径在遇到 enum 节点时，下一段必须先写 variant 名；
  - variant 名之后必须继续写该 variant 的 payload 字段名；
  - 例如：`result with { Ok.value: 2 }`、`holder with { state.Ok.point.x: 9 }`。
- 语义约束：
  - `with` 仍保持 immutable copy-update；
  - enum 更新不会切换 variant；
  - 只对“当前运行时 tag 命中的 variant”应用该 variant 下的更新；
  - 未命中的 variant 路径在该分支上不生效，结果保留原值。
- 这样做的原因：
  - 避免 `value` / `error` 这类字段名在不同 variant 间的歧义；
  - 不需要把 enum `with` 错绑到尚未存在的普通 payload member access；
  - lowering 仍可复用现有“单次求值 + 递归重建”主线，只需把 enum 分支降为按 variant `when` 重建。

## 下一步实现

1. 扩展 `with` typecheck side table：不再只记录“prefix -> TypeId”，而是记录 lowering 所需的 copy-update target 信息，补上 enum variant payload 元数据。
2. 在 typecheck 中新增 enum 路径解析与诊断：
   - 非法 variant 前缀；
   - variant 后缺字段；
   - 非法 payload 字段；
   - 嵌套路径继续进入 struct / tuple / enum 时的 expected-type 下推。
3. 在 HIR lowering 中新增 enum 分支：
   - `base` 仍只求值一次；
   - 针对被更新到的 variant 生成 `when` arm，绑定 payload 字段后重建同一 variant；
   - 未命中的 variant 走 `else -> $with_base`。
4. 补 typecheck / run-pass / lowering 单测与 spec 文本，再跑全量验证。

## 本轮最终结论

- 在把上述语义接到最小实现后，独立 `when` probe 与 enum `with` nested-path probe 都稳定暴露出同一个更底层 blocker：
  - `enum Result { Ok(val point: Point), Err(val code: Int) }`
  - `when (r) { Ok(point) -> point.x ... }`
  - build 阶段报 `scoop::llvm::unsupported_main_body: enum payload (non-scalar)`
- 这证明当前缺口不止是 `with` 自身；inline 非标量 enum payload 的 `when` / variant binder codegen 主线本来就未收口。
- 按任务规则，不能靠“只选标量 payload”或“只选 boxed variant”的例子把 `T4010a2` 做成表面完成；那会掩盖真实前置能力缺口。
- 因此本轮决定：
  - 撤回未完成的 `T4010a2` 生产代码与临时回归；
  - 把原 `T4010a2` 重排为 `T4010a2a -> T4010a2b`；
  - 当前提交只记录 blocker、依赖重排与原因说明；
  - 下一轮从 `T4010a2a` 开始，先修 `when` / codegen 对 inline 非标量 enum payload 的解构支持。
