# M22 执行计划

版本：0.8

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
| 共享 `IrrefutableBindingPlan` | 已完成 | 4.2a val / var tuple / struct planner `0049933`；4.2b class component、lambda、effect / EH、suspend 恢复与 moving-GC `dde7063` |
| typed loop target、cleanup 与 suspend 控制转移 | 待实现 | 按 4.4 的 downstream-first 顺序落地，parser 最后开放语法 |
| Iterator / Iterable、`for` 与四种 range | 待实现 | 依赖共享 binding plan 和 typed loop target |
| Scoop aggregate 参数 ABI classification | 进行中 | `dde7063` / `20820cf` 后进入 4.3；正在并行复现 shape matrix，并审计 LIR classifier、callee / direct / indirect caller 与 codegen artifact |
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

1. val / var planner（已完成：`0049933`）
   - 新建内部 `IrrefutableBindingPlan` builder；输入一个 typed subject、AST pattern、叶 binding mutability 与 source span。
   - tuple / struct 使用 checked typed field identity；完整 shape 按声明序，运行期 projection / action 按源码序 depth-first。
   - subject 与每个 projection 结果使用 immutable hidden local；`var` 只让最终用户叶 binding mutable。
   - 失败时整体回滚；成功后展开为普通 `ValDecl` 和 `FieldAccess`。
   - 端到端 golden 已锁定显式 subject / projection / bind 序列及逆声明序的 named-field 源码求值顺序；旧 composite val pattern 不再跨 Export HIR 边界。
   - 完成验证：格式检查、全 workspace clippy、HIR lowering 671/671、strict fixture 4/4、全 workspace tests、`git diff --check`、无 `.snap.new`；独立审查未发现 correctness blocker。
2. class component、lambda、effect 与恢复语义（已完成：`dde7063`）
   - 删除仅服务 val 的 `lower_class_destructuring` 特判及“扫描 component 索引推断总元数”的逻辑。
   - class pattern 禁止 rest；源码写出 N 个位置就精确解析 `component1..N`，每次结果先进入独立 immutable hidden local，再递归处理子模式。
   - `Component` action 接入前必须结构化校验 subject、index、exact typed winner、call receiver / result 与 setup 的一致性；不得保留忽略 `source` 的未验证展开路径。
   - lambda 的一个 composite pattern 保持一个 logical source / function-type 参数和一个 typed ABI classification entry；不得按叶 binding flatten。
   - ordinary 上下文选择 suspend component 是 effect error。
   - component throw 保留此前外部副作用并跳过后续 action。
   - suspend 正常恢复后保存本次结果继续；异常恢复等价于原 call 点 throw；subject 与已完成 temporary 不重跑。
   - moving-GC fixture 锁定跨挂起存活的 subject / temporary root 与 relocation。

   完成证据：

   - 删除旧 val-only class 特判与全局 component 索引扫描；class rest 在任何 component resolution 前拒绝，源码 N 个位置只解析 `component1..N`，action 与递归子模式按源码序 depth-first 展开，`_` 也执行对应 component。
   - `Component` action 保存 exact typed winner、source、index、setup、result 与 call；构造期校验 member / extension call 形状，并覆盖同型、class → base、class → interface receiver 适配。
   - composite lambda 只建立一个 logical source、FunctionType 参数与 ABI 参数，直接从 `$arg.N` 展开 plan；整个 lambda header、body、capture 与生成实体共享一个 owner transaction，失败不泄漏 local、binding 或 counter。
   - ordinary suspend-context negative、throw 短路、正常恢复、异常恢复及 moving-GC stress fixture 均通过；挂起后 subject 与已完成 temporary 不重复求值，并在 relocation 后继续使用。
   - 验证通过：格式检查、全 workspace clippy、HIR lowering 679/679、strict 全 workspace tests（端到端 fixture 4/4，324.84 秒）、snapshot refresh、手工 baseline / moving-GC 输出一致、`git diff --check`、无 `.snap.new`；三路独立审查最终均无 blocker。

每个切片均需独立 HIR / MIR / LIR golden、正向组合 fixture 和稳定 negative fixture，完成后单独提交并更新第 3 节状态。第二个切片只有在普通、throw、suspend 恢复与 moving-GC 路径全部通过时才能提交，不把 effect 或恢复正确性留给后续补丁。

### 4.3 Scoop aggregate 参数 ABI classification（进行中）

该问题已从单个“32 字节 tuple”风险归因为一般 Scoop typed ABI 缺口，必须在 `for` / range 扩大 aggregate 传参组合前独立修复：

1. 保留原始“32 字节 tuple 作为 struct 构造器参数”最小 full-pipeline 复现，并补入能够区分类别的矩阵。当前 Darwin / AArch64 + LLVM 22.1 证据显示嵌套 24 字节 aggregate 也可失败，而平坦四 `Long` 的 32 字节 aggregate 可通过，因此不得用单一 size threshold 修补。
2. 在 LIR 建立规范要求的 typed `AbiArgument::{ElidedZst, Direct, Indirect}` 与对应 return convention；同一 target/profile classifier 同时产生 callee definition、direct/indirect call site 与 extern contract 的物理签名，禁止两端各自从 LLVM type 猜测。
3. `Indirect` storage、含 managed ref aggregate 的 caller-root publication、relocation 后 reload、closure / suspend hidden 参数必须保持 exact type、layout、scan 与 pointer provenance；不能通过禁用 statepoint 或绕开 aggregate 传参掩盖问题。
4. 加入边界尺寸、平坦/嵌套 aggregate、caller/callee 分离、generic materialization、普通/managed call 及 C/Scoop ABI 隔离回归，并单独提交。
5. 完成门包括专项 HIR / MIR / LIR golden、LLVM function/call attribute artifact、端到端输出、`cargo test --workspace`、全 workspace clippy、`git diff --check` 与无 `.snap.new`。

当前审计基线：LIR function、typed call 与 Scoop extern signature 仍只保存 logical `LirType`；codegen 分别从 definition 和 call site 的类型列表构造 LLVM 参数，并仅以 `uses_return_slot` 闭合 aggregate return。4.3 不接受按单个 size / shape 打补丁。

已锁定的实现形状：

- ABI ownership 从 MIR → LIR 开始；HIR / MIR 继续保留 logical exact signature。struct / enum layout 完成后、任何 body lowering 前，对所有最终 MIR function 与 Scoop extern 预分类；callee definition、local direct call、dispatch / closure call 与 Scoop extern 必须复用共同签名，不重新分类。
- `ScoopAbiSignature` 按 logical 顺序只保存一组 `AbiArgument` 与一个 `AbiReturn`，每个 entry 原子绑定 storage type、size / alignment、scan 与 `ElidedZst / Direct / Indirect`。不保存可与之矛盾的第二份 physical vector 或 parameter index；物理顺序固定为可选 sret，再按 logical 顺序跳过 ZST、发射 direct value 或 indirect storage pointer。
- Darwin / AArch64 首版保守分类：Unit result 为 `UnitVoid`；任意 size 0 参数与非 Unit result 为 `ElidedZst`；scalar、qualified pointer 与 niche enum 为 `Direct`；所有非空 tuple / struct / tagged enum / exception record 为 `Indirect`。未来放宽 aggregate direct 必须新增明确 coercion pieces，不得回到 size threshold。
- indirect 参数使用 caller-owned fresh exact storage，并以 `byval(exact LLVM type) align N` 发射；indirect result 作为首个物理参数并以 `sret(exact LLVM type) align N` 发射。声明、普通 call / invoke 与 dispatch 使用物理索引 `i`，显式 managed statepoint invoke 把同一属性平移到 intrinsic 参数 `5 + i`，callee `elementtype` 仍在参数 2。
- `AbiCallArgument` 区分 elided logical value、direct value与 refined indirect storage。含 managed ref 的 indirect storage 自身是递归 root region；native-borrowed handshake 前发布该稳定 storage，relocation 后把同一个已更新 storage pointer 传给 callee。AS0 storage pointer 本身不是 `gc-live`，只按 exact scan 处理其 AS1 leaves。
- `UnitVoid` 与非 Unit `ElidedZst` 在 LIR 中保持不同 return / call arm；后者物理返回 void，但在正常边生成 exact logical ZST value。address-taken elided parameter 使用独立、满足对齐的 1-byte place token，不发射空 LLVM aggregate 参数。

最小 correctness-closed 行为提交必须原子包含：LIR sum / checked classifier 与 validator；definition、direct / dispatch、call / invoke、Scoop extern 的共同物理签名；callee logical → physical parameter mapping、caller ABI storage 与 ZST elision；`sret / byval / align` 及显式 statepoint 属性；ordinary / exceptional / native root relocation；pre / post-RS4GC artifact 与 full-pipeline 复现。不会在 definition 与 caller 不一致时作为完成提交。

### 4.4 typed loop target 与 abrupt cleanup

依赖：4.2 的 planner 接口稳定；while header 前置已由 `f1745b5` 完成。实现采用 downstream-first，parser 在所有 cleanup / coroutine / poll 路径闭合后才开放源码语法：

1. 在 parser 仍拒绝 `break` / `continue` 时，先把 HIR lowering 内部控制流分析统一为 `ControlOutcome::{Fallthrough, Return, Throw, Break(LoopId), Continue(LoopId)}`；用构造 AST / IR 的单元测试锁定合并与 finally 覆盖规则。
2. 引入不可跨 callable 的 typed `LoopId` 与 loop-target stack；进入 lambda、匿名函数或局部函数时重置 stack。Export / LocalConcrete HIR 保留结构化 target，MIR lowering 一次解析为 CFG block、cleanup route 与 resume target，并统一 return / break / continue 的 catch / finally cleanup；每个 `EndCatch` 恰好一次。
3. 在 parser 仍拒绝新语法时闭合 coroutine pending transfer：挂起 finally 的正常/异常恢复、finally 覆盖及 resume target 都必须可验证。MIR 为规范化 loop header 保存显式 typed poll marker，coroutine 变换必须保留；LIR 机械映射为 poll / statepoint，不再依靠变换后可能被 resume edge 扰乱的自然回边识别。
4. 最后才让 AST / parser 接受无标签 `break` / `continue`，并在同一提交跑通 HIR → MIR → LIR → codegen；继续明确拒绝 label 与 `do-while`。source `for` 语法延后到 4.5 的完整协议切片。
5. 覆盖普通、嵌套、try/catch/finally、finally 覆盖、挂起 finally、所有 continue/header poll 路径与跨 callable negative。

提交边界按内部 outcome、typed target + 普通/EH cleanup、coroutine + 显式 poll、parser 开放 + full pipeline 顺序切分。任一中间提交都必须保持 parser 对尚未端到端可用语法的拒绝、workspace 可构建且现有 while 行为不退化。

### 4.5 Iterator / Iterable 与 source `for`

1. AST / parser 的 source `for`、一次验证的非可选 HIR `IterationCore` 与完整 HIR lowering 同一原子切片落地；`IterationCore` 固定 Iterator template、`next` exact slot、Option template、Some payload field 与 None variant。
2. 每个 source `for` 保存完整 `ForIterationPlan`：source temporary、唯一 iterator winner/result、exact `Iterator<T>`、conformance/boxing witness、specialized next slot、exact `Option<T>` refs、element type、binding plan 和 `LoopId`。
3. binding plan 首次持久进入 Export HIR / meta 前，必须以私有 checked constructor 或覆盖 local/type/mutability、shape/application/field、action 数据流与 component winner/call 的完整边界 validator 封闭跨字段不变量；reader 不能接收任意 public-field 组合。
4. source 与 iterator 各求值一次；`iterator()` 可以 suspend，`next()` 固定为 ordinary；每轮只通过 typed `next()` 和 Option primitive 分支，不按名称重查协议。
5. 把 4.2 的 binding planner 接到每轮 Some payload；refutable `for` pattern 继续为编译错误。
6. generic body 保存已选 typed witness，concretization 只替换类型，不重新做 operator / protocol resolution。
7. 每轮创建 fresh binding scope；lambda / local callable capture 必须指向该轮独立 binding，不复用上一轮存储。
8. 加入 Array / MutableArray 的普通 Iterable conformance与端到端 fixture。

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
- 新 binding fixture 最初复现了“32 字节 tuple 作为 struct 构造器参数时运行值损坏”；独立 ABI 审计进一步证明根因是一般 Scoop aggregate 参数分类缺口而不是 32 字节阈值：嵌套 24 字节也可失败，平坦 32 字节也可通过。4.3 必须用 caller / callee 共用的 typed ABI classification 修复，不能接受错误输出为 golden。
- 4.2a 的 plan 当前只由 HIR lowering 内部 builder 构造并在 val / var owner 内立即消费；公开数据模型尚不能独自排除所有跨字段非法组合。它不阻塞本切片，但在 4.5 持久进入 Export HIR / meta 前必须以 checked construction 或完整 validator 封闭。
- `for` 当前仍在 parser 阶段明确拒绝；不能在 LoopId、cleanup、IterationCore 未结构化完成前先做仅语法可用的半实现。
- coroutine CFG 审计发现仅靠 LIR 自然回边识别 header poll 在 resume edge 参与时不可靠；4.4 必须先加入由 MIR 显式携带并穿过 coroutine 变换的 header poll marker，再开放 `break` / `continue` parser。
- runtime spec 仅在 ABI、对象模型、GC 或 runtime function contract 变化时修改。binding 名称分流和 compile-time transaction 复用既有 EH / coroutine / root 契约，不单独增加 runtime 规则。

## 7. 更新记录

- 2026-09-06：建立本计划。记录 copy update 已收窄为 exact struct-only；enum 在编译期拒绝且不生成状态检查或异常路径。
- 2026-09-06：binding / match 原子语义规范已提交为 `efed112`；binding 裸名分流、Unit shorthand 与 pattern transaction 正在完成最终回归。
- 2026-09-06：记录新 fixture 暴露的既有 32 字节聚合参数 ABI 问题，列为 M22 收口前必须单独处理的风险，不在 binding fixture 中接受错误输出。
- 2026-09-06：根据独立计划复核消歧 binding plan 生命周期与 LoopId 边界；`for` parser 延后到完整协议切片；把 32 字节 aggregate 参数 ABI 提升为独立任务，并补齐逐提交卫生门。
- 2026-09-06：完成 binding 裸名分流与 pattern transaction，提交 `a324559`。验证通过：`cargo fmt --all -- --check`；全 workspace clippy（`-D warnings`）；全 workspace test（含 HIR lowering 670 项、parser 394 项及 full-pipeline fixtures 4/4）；额外 strict fixture replay 4/4；`git diff --check`；无 `.snap.new`。独立审查未发现 correctness blocker。该切片只改变编译期名称分类与事务提交，不改变 runtime ABI / 对象模型 / GC / runtime function contract，因此无需修改 runtime spec。
- 2026-09-06：4.2 进入设计复核；重点是 val / lambda 在 Export HIR 前展开、generic `for` 的 plan 生命周期、class component effect / suspend / GC 完成门，以及每个 owner 至多一次 transaction。
- 2026-09-06：提交 `487244a`，明确 `IrrefutableBindingPlan` 由 Export HIR crate 拥有；val / lambda 在 Export HIR 前消费，generic source `for` 持久保存并在 LocalConcrete HIR 结束前展开。同步明确单一根 binding 的 storage coalescing 与 coroutine ordinary liveness 边界；4.2a 开始实现。
- 2026-09-06：完成 4.2a val / var tuple / struct planner，提交 `0049933`。plan 的 declaration-order shape 与 source-order depth-first actions 分离，subject / projection 为 immutable hidden local，`var` 只影响用户叶，struct field 保留 exact application identity；val / var 在 Export HIR 前展开为 binding-only statement。验证通过：格式检查；全 workspace clippy（`-D warnings`）；HIR lowering 671/671；snapshot refresh 后 strict fixture 4/4；全 workspace tests；`git diff --check`；无 `.snap.new`。独立审查未发现 correctness blocker。该切片不改变 runtime ABI、对象模型、GC 或 runtime function contract，因此无需修改 runtime spec。
- 2026-09-06：根据独立 loop / ABI 审计更新后续顺序。aggregate 问题改名为一般 Scoop ABI classification 缺口并前移到 4.3；loop 采用 outcome → typed target / cleanup → coroutine / explicit header poll → parser 开放的 downstream-first 顺序。记录 `Component` action 接入前校验与 binding plan 持久化前封闭构造为后续硬门。
- 2026-09-06：完成 4.2b class component、lambda、effect / EH、suspend 恢复与 moving-GC，提交 `dde7063`。最终审查发现并修复 extension component receiver 被合法拓宽为父类 / 接口后错误要求 exact type 的断言，并补两类正向回归。验证通过：`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test -p scoop-hir-lower`（679/679）；snapshot refresh；`INSTA_UPDATE=no cargo test --workspace`（端到端 fixture 4/4，324.84 秒，全部 doctest 通过）；手工 baseline 与 `SCOOP_GC_STRESS_MOVE=1` 输出一致；`git diff --check`；无 `.snap.new` 或新增 `TODO` / `unimplemented!`。三路独立审查最终均无 correctness blocker。该切片只复用既有 typed call、EH、coroutine frame 与 moving-GC root / relocation 契约，不改变 runtime ABI、对象模型、GC 或 runtime function contract，因此无需修改 runtime spec。下一执行切片为 4.3 Scoop aggregate 参数 ABI classification。
- 2026-09-06：4.3 开始。并行审计三条路径：LIR logical / physical signature ownership，32 / 24 字节及平坦 / 嵌套 aggregate 的 full-pipeline 复现矩阵，Darwin/AArch64 codegen definition / direct / dispatch / statepoint / Scoop extern artifact。当前已确认 `Function.params`、call signature 与 `LirFunctionType` 尚无参数 classification sum，codegen仍从各自的 `LirType`列表构造物理参数；先闭合共同 target classifier，再改 storage/root，不做单 shape 阈值补丁。
- 2026-09-06：完成 4.3 的 LIR ownership 与 codegen / root 只读审计收敛。确认 ABI ownership 在 MIR → LIR，签名使用单一 `ScoopAbiSignature` / Abi sum，参数物理顺序由 logical entry 机械派生；首版所有非空 aggregate 间接传递，声明与 callsite 发射 typed `byval / sret / align`，显式 statepoint 使用 `5 + physical_index`。记录 indirect storage 在 ordinary / invoke / native-borrowed 中的 root / relocation 硬门，并把 definition 与 caller 一致性定为不可拆分的行为提交边界。
- 2026-09-06：复现矩阵进一步定位 physical mismatch。原始 32-byte 嵌套 tuple 构造参数期望 `3/0/4/0`、实际 `3/0/0/1`；24-byte `(Signal, Long)` 构造参数期望 `3/4/55`、实际 `3/4/4294967296`，同 shape 经 NoGC 自由函数与 generic specialization 也失败，而 flat 32-byte 对照通过。LLVM IR 两端都写 raw by-value aggregate，但 AArch64 caller 与 callee 对溢出 stack 叶的拆分 offset 不一致；这直接验证了 exact indirect storage 的修复方向。
