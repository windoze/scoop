# M22 执行计划

版本：0.3

最后更新：2026-09-06

状态：进行中

本文是 `docs/milestone22/DESIGN.md` 的执行账本，不替代语言、runtime 或实现规范。语义冲突时以 `docs/specs/` 下的规范为准；发现设计缺口时先修订规范，再继续实现。

## 1. 执行纪律

- 每个可独立验收的功能作为一个提交；实现、单元测试、negative fixture 与相关 golden 同提交，不把已知失败留给后续提交。
- 每批代码先执行 `cargo fmt --all` 和 `cargo clippy --workspace --all-targets -- -D warnings`，再运行对应 crate 测试、fixture 严格回放和必要的全 workspace 回归。
- HIR / MIR / LIR 的信息必须结构完备；stage 之间只通过各自 IR crate 通信，不在下游重新按名称、FQN、ordinal 或上下文猜测上游语义。
- 失败 candidate、pattern plan 或 lowering transaction 不得污染永久 arena、scope、warning、实例化请求或 statement sink；`Error` 是失败基线，warning 只随成功路径提交。
- 每个功能提交除专项完成门外都必须执行 `git diff --check` 并确认没有 `.snap.new`；实际命令与结果写入第 7 节。
- 本文件持续更新：开始切片时标为“进行中”，完成验证并提交后记录 commit 与验证结果；范围或顺序变化时同步写明原因。

## 2. 不可变范围决定

- `Int` / `UInt` 固定 32 位，`Long` / `ULong` 固定 64 位；其他固定宽度类型、透明别名及旧 64 位 source surface 的迁移遵守 DESIGN 第 1 章。
- M22 只实现无标签 `break` / `continue` 和 `for`；不实现 `do-while`、标签控制流、一般 jump expression 或源码可命名的 `Nothing`。
- binding pattern 用于 `val` / `var`、lambda 与 `for`，必须递归不可失败；match pattern 仅用于 `when`，允许 literal 与 enum variant。
- copy update 只适用于具有声明字段的 exact struct。任何 enum、class 或其他类型都在建立字段 candidate、lower RHS 或生成运行期路径前报稳定编译错误。不得为 enum copy update 生成 active-variant 检查、payload 投影、`IllegalStateException` 或其他异常路径。
- `for` 只消费普通 typed `iterator` operator 和一次验证后保存的 exact `Iterator<T>` / `Option<T>` core contract，不增加按名称查找的专用旁路。
- 四种 range 是独立 nominal type，不是 alias；不引入 platform-native integer。

## 3. 当前基线

| 工作流 | 状态 | 已落地证据 / 下一门 |
| --- | --- | --- |
| machine scalar 与 fixed-width integer 设计 | 已完成基线 | `1be13ad`、`8447ddc`、32 位 source integer 规范基线 `9347cb9` |
| target layout、pointer provenance、八种整数、透明 alias 与 exact pointer ABI | 已完成基线，最终组合验收待 M22 收口 | `31b47fb`、`9950e8f`、透明 alias `7eb6643` |
| 递归 pattern matrix、missing witness 与命名字段递归子模式 | 已完成基线 | `1179679`、`ac38ee2` |
| 表示无关 enum primitive 与 Option consumer 迁移 | 已完成 | `272801a`、`28e36ba` |
| while header 正规化 | 已完成前置 | `f1745b5`；break / continue target 与 cleanup 尚未实现 |
| contextual bare enum variant、typed source pattern 与 catch-all warning | 已完成基线 | `526b2d7`、`998bb67`、`a19de09` |
| struct copy update | 已完成并按最新决定收窄 | `2a32133`、修正提交 `50a300e`；enum 不进入 plan，也没有状态检查或异常路径 |
| compiler-owned enum producer identity | 已完成 | `9706b4b` |
| binding / match 分流与原子 binding 语义规范 | 已完成 | `efed112` |
| binding 裸名分流与 pattern transaction | 已完成 | `a324559`；binding / match / Unit 分流、递归失败回滚及 full-pipeline fixture 均已锁定 |
| 共享 `IrrefutableBindingPlan` | 进行中 | 4.1 已闭合；正在复核现有 val / lambda / class component lowering，随后按 4.2 实现 |
| typed loop target、cleanup 与 suspend 控制转移 | 待实现 | 按 4.3 顺序落地 |
| Iterator / Iterable、`for` 与四种 range | 待实现 | 依赖共享 binding plan 和 typed loop target |
| 32 字节 aggregate 参数 ABI | 待修复 | 已有最小复现线索；须在 `for` / range 组合扩张前独立归因、修复并提交 |
| M22 全量组合验收 | 待完成 | 依赖所有主线切片完成 |

## 4. 剩余执行顺序

### 4.1 binding 裸名分流与事务边界（已完成：`a324559`）

目标：先纠正不依赖 component action plan 的名称语义，并把失败 pattern 的编译期状态隔离闭合。

- binding 上下文中的普通裸标识符恒建立新 binding，不查询 subject enum、import 或既有 value；`val None = value` 合法。
- `Unit` / `()`仍是内建 literal，在 binding 位置拒绝。字段 shorthand 精确等价于 `field: field`，所以 `{ Unit }` 也拒绝，显式 `{ Unit: value }` 可绑定。
- match 上下文保持 variant-first：bare unit variant 成为 variant pattern；bare payload variant 缺 shape 时诊断；名称未命中才成为带 warning 的 catch-all binding。
- 整个递归 pattern 在 `Lowerer` clone 上构造；成功且无新增 `Error` 才提交（包括该成功路径产生的 warning），失败只传播该路径新增的 `Error`，并回滚 local、scope、arena、warning、counter 和 sink。
- 旧 class tuple 特判在迁入共享 planner 前也由外围事务包住，避免后一个 component / leaf 失败时泄漏前面的状态。
- 测试覆盖 val、嵌套 struct + tuple、lambda、显式 enum shape、Unit shorthand、match variant-first、bare payload 错误，以及失败后后续引用仍为 unknown variable。
- full-pipeline 正向 fixture 逐项锁定嵌套绑定值；negative fixture 锁定所有错误位置与事务回滚后的第二条诊断。

完成门：

1. `cargo fmt --all -- --check`；
2. `cargo clippy --workspace --all-targets -- -D warnings`；
3. `cargo test -p scoop-parser -p scoop-hir-lower`；
4. `INSTA_UPDATE=no cargo test -p scoopc --test fixtures`；
5. `cargo test --workspace`；
6. `git diff --check`，并确认没有 `.snap.new`；
7. 独立审查确认 binding / match / Unit / transaction 无回归后提交。

### 4.2 共享 irrefutable binding plan

目标：让 val / var、lambda 和后续 for 共享同一套不可失败 binding 规划语义，同时明确两种生命周期：val / var、lambda 的内部 planner 在产出 Export HIR 前展开为现有 typed HIR statement；generic `for` 的 binding plan 作为完整 `ForIterationPlan` 的一部分保留在 Export HIR，concretization 只映射既有 typed refs，source `For` 最迟在 LocalConcrete HIR 结束前展开，MIR 不接收 source plan。

按可独立提交的两个切片执行：

1. val / var planner
   - 新建内部 `IrrefutableBindingPlan` builder；输入一个 typed subject、AST pattern、叶 binding mutability 与 source span。
   - tuple / struct 使用 checked typed field identity；完整 shape 按声明序，运行期 projection / action 按源码序 depth-first。
   - subject 与每个 projection 结果使用 immutable hidden local；`var` 只让最终用户叶 binding mutable。
   - 失败时整体回滚；成功后展开为普通 `ValDecl` 和 `FieldAccess`。
2. class component、lambda、effect 与恢复语义
   - 删除仅服务 val 的 `lower_class_destructuring` 特判及“扫描 component 索引推断总元数”的逻辑。
   - class pattern 禁止 rest；源码写出 N 个位置就精确解析 `component1..N`，每次结果先进入独立 immutable hidden local，再递归处理子模式。
   - lambda 的一个 composite pattern 保持一个 logical source / function-type 参数和一个 typed ABI classification entry；不得按叶 binding flatten。
   - ordinary 上下文选择 suspend component 是 effect error。
   - component throw 保留此前外部副作用并跳过后续 action。
   - suspend 正常恢复后保存本次结果继续；异常恢复等价于原 call 点 throw；subject 与已完成 temporary 不重跑。
   - moving-GC fixture 锁定跨挂起存活的 subject / temporary root 与 relocation。

每个切片均需独立 HIR / MIR / LIR golden、正向组合 fixture 和稳定 negative fixture，完成后单独提交并更新第 3 节状态。第二个切片只有在普通、throw、suspend 恢复与 moving-GC 路径全部通过时才能提交，不把 effect 或恢复正确性留给后续补丁。

### 4.3 typed loop target 与 abrupt cleanup

依赖：4.2 的 planner 接口稳定；while header 前置已由 `f1745b5` 完成。

1. AST / parser 只加入无标签 `break` / `continue`；继续明确拒绝 label 和 `do-while`。`for` source shape 延后到 4.4，与完整 HIR 协议原子提交。
2. HIR 引入不可跨 callable 的 typed `LoopId` 和 loop-target stack；进入 lambda、匿名函数或局部函数时重置 stack。
3. 用 `ControlOutcome::{Fallthrough, Return, Throw, Break(LoopId), Continue(LoopId)}` 替换只表示 fallthrough 的布尔分析。
4. 统一 return / break / continue 的 catch / finally cleanup 规划；finally 可以按既有规则覆盖 pending transfer，每个 `EndCatch` 恰好一次。
5. Export / LocalConcrete HIR 保留结构化 typed `LoopId`；MIR lowering 将其一次解析为 CFG block、cleanup route 与 resume target，LIR 只接收 CFG edge 和 typed poll / statepoint，不携带源码 loop target。所有 continue 与普通回边都经过对应 header poll。
6. 覆盖普通、嵌套、try/catch/finally、finally 覆盖、挂起 finally 与跨 callable negative。

提交边界建议：break / continue parser + typed HIR target；普通 CFG；EH/finally；coroutine/poll。任一提交都必须保持 workspace 可构建、现有 while 行为不退化。

### 4.4 Iterator / Iterable 与 source `for`

1. AST / parser 的 source `for`、一次验证的非可选 HIR `IterationCore` 与完整 HIR lowering 同一原子切片落地；`IterationCore` 固定 Iterator template、`next` exact slot、Option template、Some payload field 与 None variant。
2. 每个 source `for` 保存完整 `ForIterationPlan`：source temporary、唯一 iterator winner/result、exact `Iterator<T>`、conformance/boxing witness、specialized next slot、exact `Option<T>` refs、element type、binding plan 和 `LoopId`。
3. source 与 iterator 各求值一次；`iterator()` 可以 suspend，`next()` 固定为 ordinary；每轮只通过 typed `next()` 和 Option primitive 分支，不按名称重查协议。
4. 把 4.2 的 binding planner 接到每轮 Some payload；refutable `for` pattern 继续为编译错误。
5. generic body 保存已选 typed witness，concretization 只替换类型，不重新做 operator / protocol resolution。
6. 每轮创建 fresh binding scope；lambda / local callable capture 必须指向该轮独立 binding，不复用上一轮存储。
7. 加入 Array / MutableArray 的普通 Iterable conformance与端到端 fixture。

### 4.5 32 字节 aggregate 参数 ABI 修复

该问题从风险项提升为显式完成门，并在 `for` / range 组合扩大前独立处理：

1. 建立最小 full-pipeline fixture，锁定“32 字节 tuple 作为 struct 构造器参数”在 HIR / MIR / LIR 正确但运行值损坏的边界。
2. 沿 calling convention、参数分类、LLVM function type 与 call-site attribute 双向核对根因；不得通过调整错误 golden 或绕开 aggregate 传参掩盖。
3. 修复后加入边界尺寸、嵌套 aggregate、caller/callee 分离及 C/Scoop ABI 组合回归，并单独提交。
4. 完成门包括专项 stage golden、端到端输出、`cargo test --workspace`、全 workspace clippy、`git diff --check` 与无 `.snap.new`。

### 4.6 四种 range 与整数循环组合

1. 在 core 源码中加入独立 nominal `IntRange`、`UIntRange`、`LongRange`、`ULongRange` 及对应 iterator。
2. 补齐 closed / open / `until` / `downTo` / `step` / `contains`，计数器始终使用 element exact integer kind。
3. 空区间、单元素、方向、alignment、非法 step、MIN / MAX 端点均不得溢出或死循环。
4. HIR / MIR / LIR golden 锁定 32 / 64 位宽与 signedness；LLVM 验收继续禁止 wrapping 路径的 `nsw` / `nuw` 和未守卫 poison。

### 4.7 M22 收口

- 运行 DESIGN 第 8、9 章要求的全部 negative、warning、stage golden、C/Scoop ABI artifact 与 moving-GC stress 组合。
- 增加至少一个串联 fixed-width range → for destructuring → recursive when → struct copy update → try/finally continue/break → suspend/moving-GC 的组合 fixture。
- 搜索并清除：旧 64 位 `Int` / `UInt`解释、Option 专用裸 variant、return-only cleanup、按 enum 名/ordinal 猜 identity、source plan 泄漏到 MIR、enum/class copy update、绕过 header poll 的回边。
- 依次执行格式化、全 workspace clippy、全 workspace test、strict fixture replay，并检查没有 `.snap.new`、占位 `TODO` / `unimplemented!` 或未提交文件。
- 所有完成门满足后更新 `docs/ROADMAP.md` 的 M22 状态，并提交最终收口变更。

## 5. 验证矩阵

| 变更类型 | 最低验证 |
| --- | --- |
| parser / AST | parser 单元测试、span / recovery negative、AST fixture golden |
| HIR 语义 / resolver | HIR 单元测试、失败 transaction、Export / LocalConcrete golden |
| MIR / CFG / cleanup | MIR 单元测试、dominance / cleanup / target validation、MIR golden |
| LIR / ABI / codegen | LIR validation、LLVM artifact 检查、端到端运行 |
| coroutine / GC | ordinary + suspend 路径、异常恢复、moving-GC stress |
| spec / design | `git diff --check`、跨三份 spec 引用复核；若无 runtime ABI 变化，明确记录无需修改 runtime spec 的理由 |

## 6. 已知风险与旁支

- 全量 clone `Lowerer` 能保证 transaction 正确，但在大量 pattern 下可能产生二次复杂度。当前优先闭合语义；4.2 planner 落地时应减少嵌套 clone，保证每个 owner 至多一个 transaction。
- 新 binding fixture 曾复现一个既有问题：32 字节 tuple 作为 struct 构造器参数时，HIR / MIR / LIR 正确而运行值损坏。当前 fixture 已改为不掩盖该缺陷且仍逐项验证 binding；4.5 已把它提升为 `for` / range 前的独立任务和完成门，不能把错误输出接受为 golden。
- `for` 当前仍在 parser 阶段明确拒绝；不能在 LoopId、cleanup、IterationCore 未结构化完成前先做仅语法可用的半实现。
- runtime spec 仅在 ABI、对象模型、GC 或 runtime function contract 变化时修改。binding 名称分流和 compile-time transaction 复用既有 EH / coroutine / root 契约，不单独增加 runtime 规则。

## 7. 更新记录

- 2026-09-06：建立本计划。记录 copy update 已收窄为 exact struct-only；enum 在编译期拒绝且不生成状态检查或异常路径。
- 2026-09-06：binding / match 原子语义规范已提交为 `efed112`；binding 裸名分流、Unit shorthand 与 pattern transaction 正在完成最终回归。
- 2026-09-06：记录新 fixture 暴露的既有 32 字节聚合参数 ABI 问题，列为 M22 收口前必须单独处理的风险，不在 binding fixture 中接受错误输出。
- 2026-09-06：根据独立计划复核消歧 binding plan 生命周期与 LoopId 边界；`for` parser 延后到完整协议切片；把 32 字节 aggregate 参数 ABI 提升为独立任务，并补齐逐提交卫生门。
- 2026-09-06：完成 binding 裸名分流与 pattern transaction，提交 `a324559`。验证通过：`cargo fmt --all -- --check`；全 workspace clippy（`-D warnings`）；全 workspace test（含 HIR lowering 670 项、parser 394 项及 full-pipeline fixtures 4/4）；额外 strict fixture replay 4/4；`git diff --check`；无 `.snap.new`。独立审查未发现 correctness blocker。该切片只改变编译期名称分类与事务提交，不改变 runtime ABI / 对象模型 / GC / runtime function contract，因此无需修改 runtime spec。
- 2026-09-06：4.2 进入设计复核；重点是 val / lambda 在 Export HIR 前展开、generic `for` 的 plan 生命周期、class component effect / suspend / GC 完成门，以及每个 owner 至多一次 transaction。
