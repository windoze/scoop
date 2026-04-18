# TODO（Scoop：下一轮核心语言 / codegen 与 Task 设计）

> 生成时间：2026-04-18  
> 历史归档：`TODO-4.md` / `PLAN-4.md`  
> 顺序约束：严格按 `T4001 -> T4001R -> T4002 -> ... -> T4009R` 推进；不得跨条目并行实现。  
> 本轮只覆盖 `ISSUES.md` 中以下九项，并保持用户指定顺序。

## 全局约束

- 前七项属于核心语言 / codegen 主线；完成前不得启动 effect / `Task` 两项。
- 每个实现任务后必须立即做 review 任务；review 只审查生产代码与规范一致性，不以测试命名代替结论。
- 若某项实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md` 或相关 sysroot 文档。
- 本轮不设计 executor framework；所有与 executor、wakeup、queueing、work-stealing、spawn scheduling 相关内容一律留待后续。

## T4001：泛型约束、参数化超类型与 star projection

### T4001 [DONE] 收口泛型约束、参数化超类型与 star projection 语义
- 范围：
  - `where` 子句支持带类型实参的 nominal bound。
  - type env 记录 direct supertypes 时保留 type args。
  - assignable / 上转规则支持参数化超类型。
  - `*` 不再简单退化为 `Any`，需要有真实 star projection 语义。
- 验收：
  - 覆盖 typecheck、assignable、lowering、必要的 run-pass / regression。
  - `ISSUES.md` 第 5 条收窄或关闭。
- 完成：
  - 已为 `TypeEnv` / `TypeLowering` / `assignable` / RTTI / LLVM codegen 补齐参数化超类型与 star projection 主线。
  - 已新增 6 条回归 fixture，覆盖参数化 where bound、参数化超类型上转与 `Array<*>` 读视图。
  - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、定向 run-pass、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 依赖：无

### T4001R [TODO] Review：确认参数化超类型与 star projection 没有退回特判
- 重点：
  - 不允许只对个别 interface / collection 做旁路特判。
  - star projection 不能只是换个位置继续降成 `Any`。
- 验收：
  - review 结论明确写入提交说明或文档变更中。
- 依赖：T4001

## T4002：lambda 推断与 receiver lambda

### T4002 [TODO] 补齐 lambda expected-type 推断与 receiver lambda 基本语义
- 范围：
  - 放宽“无 expected type 就直接报错”的当前门禁。
  - 扩展 expected-type 向下传播，不再只停在 0/1/2 参数。
  - receiver lambda body 中补齐 `this` 注入语义。
- 验收：
  - 新增对应 typecheck / run-pass fixtures。
  - `ISSUES.md` 第 3 条收窄或关闭。
- 依赖：T4001R

### T4002R [TODO] Review：确认 lambda 推断主线统一，不靠局部 call-shape 补丁
- 重点：
  - 不允许只为某个调用形态单独补推断。
  - receiver lambda 的 `this` 语义必须和普通 receiver function type 对齐。
- 依赖：T4002

## T4003：调用语义早期门禁

### T4003 [TODO] 收口函数值 / funptr / constructor delegation 的调用语义差异
- 范围：
  - 函数值与 funptr 的命名实参支持边界。
  - `callee<T>` 一等值传递。
  - receiver function type 在调用路径上的统一语义。
  - `super(...)` / `this(...)` 构造器委托调用不再只允许位置参数。
- 验收：
  - 对应调用形态都有回归。
  - `ISSUES.md` 第 4 条收窄或关闭。
- 依赖：T4002R

### T4003R [TODO] Review：确认调用系统不再按 callee 形态分裂
- 重点：
  - direct call、member call、function-value call、funptr call 不能各自维护不同规则分支。
- 依赖：T4003

## T4004：顶层 pattern binding

### T4004 [TODO] 打通顶层 `val` / `var` 的 pattern binding
- 范围：
  - 顶层声明头接受 pattern binding。
  - 与局部解构绑定保持一致的 lowering / binding 规则。
- 验收：
  - 新增 typecheck / lowering / run-pass fixtures。
  - `ISSUES.md` 第 6 条收窄或关闭。
- 依赖：T4003R

### T4004R [TODO] Review：确认顶层与局部 pattern binding 复用同一套语义
- 重点：
  - 不接受“顶层单独走一套 ad-hoc lowering”。
- 依赖：T4004

## T4005：Elvis `?:` lowering / codegen

### T4005 [TODO] 把 Elvis `?:` 从静态规则推进到可执行 lowering / codegen
- 范围：
  - HIR lowering 不再落回 `Any` fallback。
  - LLVM codegen 支持 Elvis 主路径。
  - nullable / rhs type 规则与执行语义保持一致。
- 验收：
  - 对应 fixtures 从 typecheck 扩展到 run-pass。
  - `ISSUES.md` 第 13 条收窄或关闭。
- 依赖：T4004R

### T4005R [TODO] Review：确认 Elvis 不再停留在“语法通过但不可执行”
- 重点：
  - 不允许保留 parser/typecheck 接受、lowering/codegen 拒绝的裂缝。
- 依赖：T4005

## T4006：跨文件 / 跨包编译链路

### T4006 [TODO] 收口跨文件顶层值、跨文件实例化与跨包扩展解析
- 范围：
  - 顶层值类型表不再只看当前文件。
  - 单态化 lowering 支持跨文件顶层函数实例化。
  - 扩展函数解析不再限于同包。
- 验收：
  - 新增多文件 / 多包 regression。
  - `ISSUES.md` 第 14 条收窄或关闭。
- 依赖：T4005R

### T4006R [TODO] Review：确认 compilation-unit 维度规则已统一
- 重点：
  - 不允许只靠“入口文件特权”维持通过。
- 依赖：T4006

## T4007：RTTI 参数化支持

### T4007 [TODO] 为 RTTI 补齐泛型与 `eff` 参数化类型支持
- 范围：
  - generic type 与带 `eff` 参数的类型不再直接 `unsupported_generic_type`。
  - 运行时类型描述符与前端类型表示保持一致。
- 验收：
  - 新增 RTTI 定向测试与必要的文档同步。
  - `ISSUES.md` 第 15 条收窄或关闭。
- 依赖：T4006R

### T4007R [TODO] Review：确认 RTTI 不再只覆盖未参数化类型
- 重点：
  - 不允许对泛型 / `eff` 类型继续静默跳过或降级成未参数化描述符。
- 依赖：T4007

## T4008：effect / continuation 完整性

### T4008 [TODO] 补齐 `Task` 手动 stepping 所需的 effect / continuation 语义缺口
- 范围：
  - richer effect polymorphism 与 continuation 类型语义。
  - receiver effect op 与相关 lowering。
  - escape continuation 组合能力，避免多 suspend / 多 await 仍要拆成多段 `handle` 的现状。
- 验收：
  - `ISSUES.md` 第 1 条收窄或关闭。
  - 文档明确区分“已支撑 `Task` manual stepping 的能力”和“仍留待后续的 executor 语义”。
- 依赖：T4007R

### T4008R [TODO] Review：确认 effect 完整性收口没有引入新的 shape-based lowering
- 重点：
  - continuation / effect codegen 不能回流到按源码形状补丁选路。
- 依赖：T4008

## T4009：`Task` 设计定型

### T4009 [TODO] 把 `Task<T>` 定型为通用的 pollable object，并隐藏 raw continuation
- 范围：
  - 明确 `Task<T>` 是 general API，`Continuation<T, eff E>` 是 advanced API。
  - 定义 `Task.poll()` / `step()` 与 `Poll<T>` 合同。
  - 支持 manual stepping，不依赖 executor framework 才能成立。
  - 清理 executor-centric、handle-based `Task` 叙事与对应文档。
- 验收：
  - `ISSUES.md` 第 2 条收窄或关闭。
  - `SCOOP_FULL_SPEC.md` 对 `Task` / `Continuation` / async surface 的边界表述一致。
  - 如 runtime / sysroot 合同改变，相关文档同步更新。
- 依赖：T4008R

### T4009R [TODO] Review：确认 `Task` 本体已脱离 executor 前提
- 重点：
  - `Task` 必须能在 manual polling 下成立。
  - raw continuation 不应继续成为易误用的默认 API。
  - executor 相关内容若仍未设计，只能作为明确的 deferred item 留下。
- 依赖：T4009
