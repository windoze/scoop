# M22 执行计划

版本：0.67

最后更新：2026-09-07

状态：已完成

本文是 `docs/milestone22/DESIGN.md` 的执行账本，不替代语言、runtime 或实现规范。语义冲突时以 `docs/specs/` 下的规范为准；发现设计缺口时先修订规范，再继续实现。

## 1. 执行纪律

- 每个可独立验收的功能作为一个提交；实现、单元测试、negative fixture 与相关 golden 同提交，不把已知失败留给后续提交。
- 同一切片的代码、validator、artifact、fixture 与审查修复先集中完成，再统一执行 `cargo fmt --all`、受影响 crate 的定向 clippy / 测试和必要的 LLVM verifier；不以反复跑完整 suite 逐个发现静态审计即可确定的缺口。
- HIR / MIR / LIR 的信息必须结构完备；stage 之间只通过各自 IR crate 通信，不在下游重新按名称、FQN、ordinal 或上下文猜测上游语义。
- 失败 candidate、pattern plan 或 lowering transaction 不得污染永久 arena、scope、warning、实例化请求或 statement sink；`Error` 是失败基线，warning 只随成功路径提交。
- 每个功能提交除专项完成门外都必须执行 `git diff --check` 并确认没有 `.snap.new`；实际命令与结果写入第 7 节。
- 专项完整 suite 只在该切片稳定后的完成门运行一次；全 workspace clippy、全 workspace test 与全 fixture 留给确实跨 workspace 的功能完成门或 M22 最终收口，各最多运行一次。完成门后若又改动行为，只重跑受影响的定向项，直到下一次最终完成门。
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
| while header、typed loop target 与 cleanup | 已完成 | header 基线 `f1745b5`；target / cleanup / coroutine transfer / poll 由 `64f90ca7`、`b42ce94f`、`dd753c09`、`6402e115` 闭合 |
| contextual bare enum variant、typed source pattern 与 catch-all warning | 已完成基线 | `526b2d7`、`998bb67`、`a19de09` |
| struct copy update | 已完成并按最新决定收窄 | `2a32133`、修正提交 `50a300e`；enum 不进入 plan，也没有状态检查或异常路径 |
| compiler-owned enum producer identity | 已完成 | `9706b4b` |
| binding / match 分流与原子 binding 语义规范 | 已完成 | `efed112` |
| binding 裸名分流与 pattern transaction | 已完成 | `a324559`；binding / match / Unit 分流、递归失败回滚及 full-pipeline fixture 均已锁定 |
| 共享 `IrrefutableBindingPlan` | 已完成 | 4.2a val / var tuple / struct planner `0049933`；4.2b class component、lambda、effect / EH、suspend 恢复与 moving-GC `dde7063` |
| typed loop target、cleanup 与 suspend 控制转移 | 已完成 | 4.4a `64f90ca7`、4.4b `b42ce94f`、4.4c1 `dd753c09`、4.4c2 `6402e115` 已完成；4.4c2 规范为 `dcd77d6c`；parser 继续拒绝新语法 |
| Iterator / Iterable、`for` 与四种 range | 已完成 | 4.5 提交 `8ba3320b`；4.6 spec-first 提交 `fc381075`，range 实现、测试与 golden 提交 `ff97a505` |
| Scoop aggregate 参数 ABI classification | 已完成 | 权威规范 `f40f728`；实现、validator、artifact、native shim、moving-GC fixture 与 golden `9fbbd68` |
| M22 全量组合验收 | 已完成 | 静态残留清单、验收矩阵、workspace all-target clippy、非 fixture workspace tests、driver 非 fixture tests 与既有 full fixture/ordinary/moving-GC 证据全部闭合 |

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

### 4.3 Scoop aggregate 参数 ABI classification（已完成：`9fbbd68`）

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

完成证据：

- LIR 现以 checked `AbiValue` / layout 与 `AbiArgument` / `AbiReturn` sum 结构化保存 classification；function、local / dispatch call 与 Scoop extern 共用同一 `ScoopAbiSignature`，物理参数只由该签名机械派生。callee、caller、invoke 与显式 statepoint 对 `byval`、`sret`、`align` 和 `5 + physical_index` 的发射一致。
- indirect argument 使用 caller-owned fresh exact storage；indirect result 使用 caller-owned sret storage。ZST 保留 logical value 但消除物理槽；`UnitVoid` 与非 Unit ZST 分离。codegen 边界 validator 重新验证 target layout、scan、classification、call/definition/extern 一致性和完整 root plan，并在索引前拒绝非法 invoke shape。
- managed / native 的 normal 与 unwind 路径均按 reload → pop → restore；含 managed leaf 的 indirect argument / result storage 使用 canonical recursive scan。Darwin / AArch64 native shim 明确消费入站 SP 的 24-byte byval 与 x8 sret，不借用 Clang C aggregate classifier；两次 moving GC 分别验证 DirectSlot reload 与 caller sret region 原地更新。
- full-pipeline fixture 覆盖 24 / 32 / 64-byte、flat / nested、constructor、NoGC、generic specialization、managed call / invoke normal / throw、closure hidden receiver、Scoop extern 与 moving-GC stress。既有 `Pair(None, 8).value` 从错误的 `0` 恢复为源码期望的 `8`。
- 验证通过：`cargo fmt --all -- --check`；全 workspace clippy（`-D warnings`）；定向 HIR lowering 680/680、LIR 25/25、LIR lowering 71/71、codegen 189/189；唯一一次 `INSTA_UPDATE=always cargo test --workspace` 全绿，fixture 4/4（333.23 秒，含 ordinary 与 stress）；`git diff --check`；无 `.snap.new` 或新增 `TODO` / `unimplemented!`。184 份既有 snapshot 的非 LIR 内容逐份复核，除两个有意源变更及上述 ABI 错值修复外无漂移；四路独立静态 / root / native / snapshot 审查最终均无 blocker。
- 本切片复用既有 caller root、native transition、moving-GC relocation 与 runtime function contract，没有改变 runtime ABI、对象模型或 GC API，因此无需修改 runtime spec。

### 4.4 typed loop target 与 abrupt cleanup

依赖：4.2 的 planner 接口稳定；while header 前置已由 `f1745b5` 完成。实现采用 downstream-first，parser 在所有 cleanup / coroutine / poll 路径闭合后才开放源码语法：

4.4a（已完成：`64f90ca7`）：只重构 HIR lowering 内部控制流事实，不提前修改 Export / LocalConcrete HIR，也不开放 parser。生产分析以不可构造 target 实例化通用 outcome 集合；测试用 synthetic typed target 锁定未来 `Break` / `Continue` 的集合代数。sequence 只替换 `Fallthrough`，`when` 按 first-match 从 fallback 反向组合并分析 guard setup，`while` 先分析 condition setup，`finally` 使用“正常完成恢复 incoming、实际 abrupt 覆盖 incoming”的笛卡尔式组合。所有 non-Unit callable 与 value block 只从该集合投影 `Fallthrough`，不再维护第二套 bool 规则。值位置 `when` 的 hint、seed、LUB、外层 expected 与最终 materialization 只接收实际可达的 branch；不可达 branch 独立 type-check 并显式保留尾表达式求值。

4.4a 的验证覆盖 sequence 短路、分支 union、when refutable / guard / 首个不可反驳 arm 截断、while setup、try/catch、finally 正常恢复 / 完全覆盖 / mixed 覆盖 / 空 incoming、synthetic target，以及 value-when 不可达 arm / else 不参与结果推断但保留 HIR 求值。验证通过：格式检查；`cargo clippy -p scoop-hir-lower --all-targets -- -D warnings`；首次 crate 门 693 项中 692 项通过并暴露一处 probe scope 回归，修正后原失败用例 1/1 与新增不可达分支用例 2/2 定向通过；`git diff --check`；无 `.snap.new`。按效率纪律未重跑不受该 probe 修正影响的 692 项，也未运行 workspace、fixture 或 codegen 全量。两路最终静态审查无 blocker。

4.4b（已完成：`b42ce94f`）：Export HIR 与 LocalConcrete HIR 各自新增内联于结构化 loop / jump 的独立 `LoopId` newtype，不给 `Body` 增加不能覆盖 default template 等 detached region 的空 arena。HIR lowering 使用 Cone-wide 单调 allocator 和 callable-local 词法 stack；default template 的每次展开与每个 concrete body / 泛型实例都分配 fresh target，并通过当前 active loop 显式重映射，既不复制旧 id，也不按 raw ordinal 转型。parser 继续拒绝 `break` / `continue`，本切片只用内部构造 IR 锁定下游协议。

MIR 侧把 return-only stack 重构为普通控制转移 router：私有 `PendingTransfer::{Fallthrough, Return, Break, Continue}` 持有互不兼容的 typed resume / return payload / loop exit / loop header target；每个 target 非可选地保存 cleanup depth。显式 `CleanupCursor` 只逆序执行当前位置到目标 depth 的 suffix，每步先截断当前 cleanup 视图；因此 finally 内的新外向 transfer 从 cursor 之后继续而不会重入，内部已消费的 loop jump / caught throw 正常结束后仍恢复旧 pending。try body、catch body的正常结束也走同一 router；catch stack 维持 `Finally` 后压入 `EndCatch`，保证 LIFO 下每条正常离开 catch 的边先且仅执行一次 `EndCatch`。普通 Throw / Rethrow 仍走现有 typed unwind。

4.4b 完成证据：Export / LocalConcrete HIR stage-local target、callable / detached-region 隔离、default / concretization fresh remap、MIR typed loop target 与统一 normal-transfer router 已原子提交。静态交叉审查发现并修复 outer catch 内 nested cleanup-free try 外跳时的 unwind owner mismatch；程序化 CFG 回归覆盖 setup / body 的 break / continue、nested loop、try/catch/finally、`EndCatch` 次数、finally 覆盖、non-Unit return 保存/恢复，以及 if / PatternDecision 双侧 abrupt 的不可达 merge。验证通过：`cargo fmt --all`；受影响三 crate clippy（`-D warnings`）；HIR loop-target 定向 7/7；MIR control-flow 定向 16/16；`git diff --check`；无 `.snap.new`。按完成门效率纪律未运行 workspace 或 full fixture suite。

4.4c 分成两个各自 correctness-closed 的行为提交，避免让独立的 safepoint blocker 被 frame schema 扩张拖成半成品：

1. 4.4c1（已完成：`dd753c09`）显式 loop-header poll：`scoop-mir::Body` 持有 body-local typed marker；source loop 与 coroutine intrinsic 的两个 compiler-generated wait cycle 在构造时登记。coroutine transform 原样保留 source marker；MIR validator 检查 target 有效且不重复。MIR → LIR 使用私有映射随 unreachable-block prune 重映射，LIR 只根据函数入口与 surviving marker 插入 poll，删除 dominance/back-edge 推断。codegen 边界重新计算并核对 `ManagedPoll.live`，checked 验证 poll target 必须是 managed `cdecl () -> void` safepoint，避免错误 root plan 或非法 target 被 emitter 信任。定向测试锁定 coroutine `dispatch → resume → post` 造成 header 不再支配回边源时仍有 header poll、unreachable marker prune/remap、跨 header managed root，以及 resume/post 不重复 poll、NoGc 不插入、未标记 cycle 不被猜测。格式化、四个受影响 crate 的 clippy 与 23 项 `poll` 定向测试已一次通过。33 个静态筛出的相关 fixture 分成“中止前已执行 14 个”和“修复后只继续 19 个”各执行一次，未重复已完成部分；泛型 `T -> Unit` 回归在 Export → LocalConcrete HIR 具体化边界正规化为保留求值的 `Expr` + bare `Return`，ordinary / finally / suspend 三项定向回归及真实 `FunctionSuspendTask<Unit>` fixture 均通过。两路最终代码 / snapshot 审查无 blocker；34 个新 MIR header marker 各对应恰好一个 LIR poll，roots/live 未漂移。
2. 4.4c2（已完成：`6402e115`；规范 `dcd77d6c`）coroutine pending-transfer chain：不新增运行期 pending-transfer enum、cleanup cursor 或 target 字段。既有 `CoroutineFrameState` 是唯一物理 discriminator；每个 success/failure state 非可选地对应一条 GC-free 静态非空 typed chain，frame 只保存该 state 与动态 payload 的 exact `CoroutineSlot<T>`。实现按以下一次性闭合：
   - CFG 在每个 call effect 上携带 transient outer→inner pending context；resume、loop exit、loop header与unwind target各自捕获目的context。router用“目的prefix + 当前action”进入cleanup，因此内部已消费transfer保留旧pending，真正outward transfer替换旧pending。
   - 仅实际含suspend call的body启用coroutine EH模式；每个unwind scope各用独立exact `Throwable` local执行`LandingPad → BeginCatch → MaterializeException → EndCatch`。unmatched或cleanup异常之后才成为`ManagedThrow` pending；修复全函数共享`$coroutine_exception`被内层恢复失败覆盖、最终错抛内层对象的缺口。
   - frame metadata以owner-bound typed field ref完整分类state、completion、saved values与failure；Return/parent managed Throw使用不同typed saved-value id，site failure使用failure-value id。`Return(Unit)`无slot，其他payload exact，不能用`Any`/opaque或native EH。
   - `CoroutineResumePoint`原子持有共享parents以及必填success(entry + `Fallthrough(post)`)、failure(entry + `Throw(saved Throwable, unwind)`)；full state只由site id派生。driver dispatch只遍历point metadata生成，删除平行`resume_targets`。
   - validator重新核对owner/arena/field/slot/type/target/state/dispatch/context-root不变量；continue target必须是显式poll header。dump锁定frame roles与outer→inner chain。
   - 定向测试只新增三个互补producer case：同一finally的Fallthrough/Return/ManagedThrow context分离；nested finally中inner failure被catch后恢复outer Return/Throw；synthetic Break/Continue target分型与continue poll。唯一新full-pipeline fixture合并managed Return/Throw payload、nested finally、inner catch、EndCatch与moving-GC；既有finally override fixture不重复扩写。

   IR、CFG/EH、transform、validator、dump和上述测试/fixture全部齐备后才统一验证：`cargo fmt --all`；四个受影响 crate 的 all-target clippy；单一 `coroutine` filter 下 MIR 9/9、MIR-lower 9/9；静态筛出的 26 个既有 snapshot 与 1 个新 fixture 分批续跑，首批已完成的 7 个没有因 fixture 源码修正而重跑，后续 20 个一次通过。26 份既有 snapshot 的 AST/HIR/run 与 `HEAD` 字节一致；新 fixture ordinary 与 moving-GC stress 输出一致。三路独立 snapshot 审查无 blocker；未运行 workspace/full fixture suite。

本阶段始终不开放 parser；每个子批都先完成 IR、validator、dump、transform与测试矩阵，再统一格式化和定向验证。

后续 ownership 固定为：AST 只保存无标签语法；Export HIR 与 LocalConcrete HIR 各用独立的 stage-local `LoopId` / target 家族并由 concretization 显式重映射；mir-lower 私有层拥有 cleanup depth / cursor 与词法 target stack；跨 CFG → coroutine 的 pending transfer、typed resume target 与 loop-header poll marker 由 `scoop-mir` 保存；LIR 不接收 source / HIR LoopId，只机械消费 MIR marker 生成既有 typed managed poll。普通 `Throw` / `Rethrow` 始终沿 typed unwind 边，只有已物化并结束 native catch 的 managed throwable 才能进入 suspend frame variant。

1. 在 parser 仍拒绝 `break` / `continue` 时，先把 HIR lowering 内部控制流分析统一为 `ControlOutcome::{Fallthrough, Return, Throw, Break(LoopId), Continue(LoopId)}`；用构造 AST / IR 的单元测试锁定合并与 finally 覆盖规则。
2. 引入不可跨 callable 的 typed `LoopId` 与 loop-target stack；进入 lambda、匿名函数或局部函数时重置 stack。Export / LocalConcrete HIR 保留结构化 target，MIR lowering 一次解析为 CFG block、cleanup route 与 resume target，并统一 return / break / continue 的 catch / finally cleanup；每个 `EndCatch` 恰好一次。
3. 在 parser 仍拒绝新语法时闭合 coroutine pending transfer：挂起 finally 的正常/异常恢复、finally 覆盖及 resume target 都必须可验证。MIR 为规范化 loop header 保存显式 typed poll marker，coroutine 变换必须保留；LIR 机械映射为 poll / statepoint，不再依靠变换后可能被 resume edge 扰乱的自然回边识别。
4. 最后才让 AST / parser 接受无标签 `break` / `continue`，并在同一提交跑通 HIR → MIR → LIR → codegen；继续明确拒绝 label 与 `do-while`。source `for` 语法延后到 4.5 的完整协议切片。
5. 覆盖普通、嵌套、try/catch/finally、finally 覆盖、挂起 finally、所有 continue/header poll 路径与跨 callable negative。

提交边界按内部 outcome、typed target + 普通/EH cleanup、coroutine + 显式 poll、parser 开放 + full pipeline 顺序切分。任一中间提交都必须保持 parser 对尚未端到端可用语法的拒绝、workspace 可构建且现有 while 行为不退化。

### 4.5 Iterator / Iterable 与 source `for`

完成提交：`8ba3320b feat: add typed source iteration`。schema 冻结为模块唯一 `IterationCore`、per-use `AppliedOptionCore`、raw-result → exact `Iterator<E>` conformance witness、specialized ordinary `next` 与完整 `ForIterationPlan`；所有 plan 字段私有，并由 Export-HIR reader boundary 统一验证 core canonical backref、winner/result closure、local/type/mutability、binding action dataflow 与 innermost loop target。parser/AST 已开放专用 token 的 `for` / `break` / `continue` 并继续拒绝 label、jump expression 与 `do-while`；core source已加入 Iterator/Iterable 及 Array/MutableArray iterator。definition reader 以同一套 typed outcome 处理 return/throw/typed break/continue、嵌套 while/for、when subject/guard、try/finally 与不可达区域，并把 mutable assignment-only merge local 纳入 plan ownership；不可达语法仍登记 identity，但不触发 future-read 或污染正常路径交集。格式化与受影响 crate all-target clippy 已通过；parser 定向测试 11/11、HIR 32/32 通过。source `For` 在 HIR concretization 已完全展开，LocalConcrete HIR/MIR 没有 `For` 分支且 MIR production 未改，因此 MIR 专项门经边界审计记为 N/A。唯一一次 `INSTA_UPDATE=always cargo test -p scoopc --test fixtures` 4/4 通过（371.80 秒）；6 份新 snapshot 和 186 份既有 snapshot 的 AST/HIR/MIR/LIR/run/trap/diagnostic 归一化审计均无 blocker，且无 `.snap.new`。workspace 全量保留给 M22 最终门。

1. AST / parser 同一原子切片开放 source `for` 及此前 4.4 已完成下游语义的无标签 `break` / `continue`；labels、`do-while` 与 jump expression 继续明确拒绝。一次验证的非可选 HIR `IterationCore` 与完整 HIR lowering 同时落地；`IterationCore` 固定 Iterator template、`next` exact slot 并复用模块唯一的 checked `OptionCore` 关系，不复制第二套 Some / None 身份。
2. 每个 source `for` 保存完整 `ForIterationPlan`：source temporary、唯一 iterator winner/result、exact `Iterator<T>`、conformance/boxing witness、specialized next slot、exact `Option<T>` refs、element type、binding plan 和 `LoopId`。
3. binding plan 首次持久进入 Export HIR / meta 前，必须以私有 checked constructor 或完整边界 validator 封闭跨字段不变量；同一门还要验证外层 IterationCore / ForIterationPlan 的 winner result → Iterator<E> witness → specialized next slot → OptionCore<E> payload → binding subject E → LoopId 全链关系，并覆盖 local/type/mutability、shape/application/field、action 数据流与 component winner/call；reader 不能接收任意 public-field 组合。
4. source 与 iterator 各求值一次；`iterator()` 可以 suspend，`next()` 固定为 ordinary；每轮只通过 typed `next()` 和 Option primitive 分支，不按名称重查协议。
5. 把 4.2 的 binding planner 接到每轮 Some payload；refutable `for` pattern 继续为编译错误。
6. generic body 保存已选 typed witness，concretization 只替换类型，不重新做 operator / protocol resolution。
7. 每轮创建 fresh binding scope；lambda / local callable capture 必须指向该轮独立 binding，不复用上一轮存储。
8. core 源码加入 Iterator / Iterable、ArrayIterator / MutableArrayIterator 及 Array / MutableArray 普通 public Iterable conformance；端到端 fixture 覆盖两类 array。
9. 首次验证前整批补齐：无 / 歧义 iterator、高优先级 winner 协议错误不回退、0 / 多个不同 Iterator application、ordinary 上下文选择 suspend iterator、用户同名 next / Some / None 无效、generic bound witness 不重查、source / iterator / next 次数、fresh capture、refutable pattern、for 内 break / continue + try/finally + header poll，以及 suspend / moving-GC 存活；静态审计和实现合批完成后才运行一次分层定向门。
10. 4.5 验证批固定为：一次 `cargo fmt --all`；一次受影响 crate all-target clippy；一次 parser / HIR-lower 定向测试批；MIR-lower 仅在 source plan 穿过其边界或 production 分支变化时运行专项门，本切片经审计为 N/A；全部定向门稳定后，一次 `INSTA_UPDATE=always cargo test -p scoopc --test fixtures` 生成并验证新 fixture snapshots。某一门失败时只修复并重跑该门及其直接下游，不回头重跑已通过的慢门；本切片不运行 `cargo test --workspace`。

### 4.6 四种 range 与整数循环组合

当前执行状态：已完成（`ff97a505`）。spec-first 修订为 `fc381075`；普通 core、parser/HIR 定向测试、fixture/codegen 验收三批均已落地并完成三路静态交叉审查。格式化、受影响 crate clippy、parser/HIR 3 项定向门与 codegen overflow-flag 门均已通过；full fixture refresh 在首段暴露组合 fixture 的 visibility 问题后，仅从失败路径继续到末尾，两段合起来对排序后的 fixture 集合完成一次逻辑遍历且全绿。临时路径筛选已立即撤除；新 range、M22 组合链与既有 snapshot 漂移审计均无 blocker，不重复 replay。

1. 在 core 源码中加入独立 nominal `IntRange`、`UIntRange`、`LongRange`、`ULongRange` 及对应 iterator。
2. 补齐 closed / open / `until` / `downTo` / `step` / `contains`，计数器始终使用 element exact integer kind。
3. 空区间、单元素、方向、alignment、非法 step、MIN / MAX 端点均不得溢出或死循环。
4. HIR / MIR / LIR golden 锁定 32 / 64 位宽与 signedness；LLVM 验收继续禁止 wrapping 路径的 `nsw` / `nuw` 和未守卫 poison。
5. 八种 integer owner 的 API 固定为普通、非 generic member：`public operator fun rangeTo(endpoint: O): R`、`public operator fun rangeUntil(endpoint: O): R`、`public infix fun until(endpoint: O): R`、`public infix fun downTo(endpoint: O): R`；命名参数统一为 `endpoint`。
6. `step` 在空 range 上也先验证参数；零与 signed 负值抛 `IllegalArgumentException`。每次成功的四种构造 member 与 `step` 都返回 fresh range；`step` 只替换绝对步长，不修改 receiver、不乘旧步长、不反转方向；每次 `iterator()` 创建独立 cursor。
7. 完整 signed/unsigned 全域差值使用不溢出的 ordinary core 表达，iterator 在更新 current 前判断边界/溢出；不得保存可能溢出的元素总数，也不得依赖 wrapping sentinel。
8. 验证批固定为：静态清单归零后一次 `cargo fmt --all`；受影响 crate 一次 all-target clippy；parser 若无生产修改则不重跑；HIR/core 定向测试只运行一批；全部稳定后只运行一次 `INSTA_UPDATE=always cargo test -p scoopc --test fixtures`。任何失败只集中修复并重跑失败门，不重跑已通过门；本切片不运行 workspace suite。
9. parser 只补一个 `for` header + `..<` / `until` / `downTo` + `step` / membership 的组合 AST 用例；HIR 用两个表驱动用例一次锁定四种 nominal surface、八种 owner 映射、窄整数扩展及 exact target；MIR/LIR 不增加 range 专用节点或重复 synthetic 单测。
10. codegen 只把既有 wrapping `add/sub/mul` 无 `nsw` / `nuw` 断言扩到 s32/u32/s64/u64；signed/unsigned compare、safe remainder 与 integer conversion 复用现有覆盖。
11. full-pipeline 使用一个独立 `m22-ranges/semantics.scoop` 覆盖四类 range 的普通语义、完整端点与非法 step；一个汇总 negative fixture 锁定 constructor visibility、nominal 不可互换、mixed endpoint 与错误 step/contains exact type。suspend + moving-GC 直接扩已有 stress 白名单中的 `m22-iteration/combined.scoop`，并在同一程序内提前满足 4.7 的 fixed-width range → for destructuring → recursive when → struct copy update → try/finally continue/break → suspend/moving-GC 组合门，不增加第二个慢 stress 程序，也不为 4.7 再刷新 fixture。
12. 唯一验证命令批为：`cargo fmt --all`；`cargo clippy -p scoop-parser -p scoop-hir-lower -p scoop-codegen --all-targets -- -D warnings`；`cargo test -p scoop-parser -p scoop-hir-lower m22_range`；既有 codegen overflow-flag 用例；最后一次 fixture snapshot refresh。新/改 snapshot 完整审计，其余 snapshot 只做 core 插入与 ID 顺延归一化审计；不再 replay fixture、不运行 workspace clippy/test。
13. `ranges.scoop` 的四个 range 保存 exact `first/bound/stride` 与 `descending/inclusive`，四个 iterator 保存 exact `current/bound/stride`、方向/开闭与 `hasNext`；不做 endpoint±1、不保存长度。signed distance 先把有序端点 bit-preserving 转为同宽 unsigned，再以方向对应的 unsigned subtraction 与 remainder 判断；只有 `stride <= distance`（闭）或 `stride < distance`（开）时才更新 current。
14. `IllegalArgumentException` 是普通 public `Exception` 子类且不进入 `CompilerExceptionCore`；32 个 owner member 必须写回八个 nominal struct，窄 owner 在 body 内显式 `toInt32()` / `toUInt32()`，不能用 extension、隐式转换或 generic numeric 抽象替代。

### 4.7 M22 收口

当前执行状态：已完成。所有功能切片、最终跨特性组合 fixture、ordinary / moving-GC 输出、full fixture snapshot refresh、DESIGN 第 8、9 章与三份 spec 的静态清单，以及不重复 slow fixture 的 workspace 收口门均已闭合；M22 已在 ROADMAP 标记完成。

- 运行 DESIGN 第 8、9 章要求的全部 negative、warning、stage golden、C/Scoop ABI artifact 与 moving-GC stress 组合。
- 增加至少一个串联 fixed-width range → for destructuring → recursive when → struct copy update → try/finally continue/break → suspend/moving-GC 的组合 fixture。
- 搜索并清除：旧 64 位 `Int` / `UInt`解释、Option 专用裸 variant、return-only cleanup、按 enum 名/ordinal 猜 identity、source plan 泄漏到 MIR、enum/class copy update、绕过 header poll 的回边。
- 依次执行格式检查、一次全 workspace clippy、排除 `scoopc` 的 workspace tests，以及只选择 `scoopc` lib/bin、`source_integer_codegen`、`warnings_cli` 和 doc tests 的非 fixture 门；4.6 已完成并审计的 strict fixture 不重复 replay。最后检查没有 `.snap.new`、占位 `TODO` / `unimplemented!` 或未提交文件。
- 所有完成门满足后更新 `docs/ROADMAP.md` 的 M22 状态，并提交最终收口变更。

## 5. 验证矩阵

| 变更类型 | 最低验证 |
| --- | --- |
| parser / AST | parser 单元测试、span / recovery negative、AST fixture golden |
| HIR 语义 / resolver | HIR 单元测试、失败 transaction、Export HIR golden，以及 LocalConcrete 结构断言与 reader/boundary validation |
| MIR / CFG / cleanup | MIR 单元测试、dominance / cleanup / target validation、MIR golden |
| LIR / ABI / codegen | LIR validation、LLVM artifact 检查、端到端运行 |
| coroutine / GC | ordinary + suspend 路径、异常恢复、moving-GC stress |
| spec / design | `git diff --check`、跨三份 spec 引用复核；若无 runtime ABI 变化，明确记录无需修改 runtime spec 的理由 |

## 6. 已知风险与旁支

- 全量 clone `Lowerer` 能保证 transaction 正确，但在大量 pattern 下可能产生二次复杂度。当前优先闭合语义；4.2 planner 落地时应减少嵌套 clone，保证每个 owner 至多一个 transaction。
- 新 binding fixture 最初复现了“32 字节 tuple 作为 struct 构造器参数时运行值损坏”；独立 ABI 审计进一步证明根因是一般 Scoop aggregate 参数分类缺口而不是 32 字节阈值：嵌套 24 字节也可失败，平坦 32 字节也可通过。4.3 必须用 caller / callee 共用的 typed ABI classification 修复，不能接受错误输出为 golden。
- 4.2a 时公开 binding 数据模型尚不能独自排除所有跨字段非法组合；4.5 已将持久化的 binding plan 纳入私有 `ForIterationPlan` 与完整 reader-boundary validator。本切片提交前仍须以伪造 plan 的 negative 测试锁定 owner/signature、局部量 freshness、action dataflow 与 loop target 拒绝边界。
- 4.5 原子切片前，`for` 曾在 parser 阶段明确拒绝，以避免 LoopId、cleanup、IterationCore 未结构化时出现仅语法可用的半实现；当前 parser/AST 已随完整协议链开放，提交门是 Export HIR plan 必须在进入具体化前通过完整 reader validation。
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
- 2026-09-06：补充验证效率纪律。开发期先合并同一切片的全部实现、测试与审查修复，只跑定向检查；专项完整 suite、全 workspace 与全 fixture 分别留到稳定完成门一次执行，避免用反复全量回归发现可由静态审计或定向用例提前确定的缺口。
- 2026-09-06：提交 `f40f728`，先在语言与实现规范冻结 Darwin / AArch64 profile 的 aggregate Scoop ABI mapping：非空 aggregate 统一 indirect，caller / callee 共享 exact physical signature，LLVM adapter 一致发射 `byval` / `sret` / alignment 与 statepoint 参数属性。runtime function、对象模型与 GC 协议未变化，因此 runtime spec 无需修改。实现审查同时发现 native shim 必须从 incoming stack 取得 24-byte `byval` storage，且 moving-GC fixture 不能使用 immortal String；两项均在首次定向验证前集中修正。
- 2026-09-06：完成 4.3 Scoop aggregate 参数 ABI classification，提交 `9fbbd68`。LIR classification、definition / call / extern 物理签名、indirect storage、ZST、statepoint attributes、root-plan validator、reload-before-pop、Darwin / AArch64 native shim 与 full-pipeline matrix 原子落地。先集中静态审计和定向修复，再只运行一次全 workspace / fixture 完成门；完整回归、ordinary / moving-GC stress 与 snapshot 审计全部通过。下一执行切片为 4.4 typed loop target 与 abrupt cleanup。
- 2026-09-06：开始 4.4a HIR target-aware outcome。先完成只读审计并一次性列齐 sequence、if/when/while、try/finally、value block 与 callable consumers 的修改和测试矩阵；生产 target 暂用不可构造类型，真实 `LoopId` 留到 4.4b。按用户要求进一步收紧验证效率：本切片集中完成实现与审查修复后只跑定向 HIR-lower 门，不运行 workspace / fixture / codegen 全量。
- 2026-09-06：完成 4.4 downstream 静态审计。确认现有 while header 正规化可复用、return-only cleanup 与 catch materialization 可作为重构基线，但 `PendingTransfer` / suspend frame metadata、typed resume target 及显式 MIR poll marker 都必须结构化新增；LIR 的自然回边推断不能继续作为 correctness 来源。无外部 blocker，后续保持 outcome → typed target/EH → coroutine/poll → parser 的四批顺序。
- 2026-09-06：完成并提交 4.4a HIR target-aware outcome `64f90ca7`。`ControlOutcomes<Target>` 统一 sequence、if/when/while、try/finally、non-Unit callable 与 value block 的控制事实；production 暂以 `Infallible` 封闭未开放 target，synthetic target 测试锁定 Break/Continue 身份与 finally 组合。同步修复 value-position `when` 让首个不可反驳 arm 后的不可达 arm / else 不再污染 seed、hint、LUB、expected 或 result materialization，尾表达式仍显式保留。验证严格采用“先攒批、再定向”：未重跑 workspace/full fixture；下一切片为 4.4b typed target + ordinary/EH cleanup。
- 2026-09-07：开始 4.4b typed target + ordinary/EH cleanup。两路只读审计裁决为 While 内联的 Export / LocalConcrete 独立 `LoopId`、default / concretization fresh 显式重映射，以及包含 Fallthrough 的单一 MIR normal-transfer router；不新增 `Body` arena，不开放 parser，不把 Throw / Rethrow 改写成 normal pending。实现、穷尽 visitor 修复和 HIR/MIR 定向矩阵先一次性攒齐，再统一格式化、lint 与相关 crate 验证；不运行 workspace/full fixture 全量。
- 2026-09-07：4.4b HIR 批已静态收口：Export / LocalConcrete 独立 `LoopId`、callable / detached region 隔离、default / concretization fresh top-exact remap、typed flow 与 visitor 穷尽更新已齐；MIR 批继续集中补齐 loop CFG、normal-transfer cleanup、catch `EndCatch`、finally 覆盖和无伪 merge 的程序化断言。最终合并前不逐 case 运行测试；待两批与静态审查一次性收敛后，只运行一次格式化、受影响 crate clippy 与精准测试，不运行 workspace 或 full fixture suite。
- 2026-09-07：4.4b 实现与测试矩阵已合批。静态交叉审查先一次性补强 nested loop + finally 的 typed outcome、non-Unit return 跨 finally 的保存/恢复、PatternDecision 双侧 abrupt、callable/CFG scope 平衡断言和测试遍历的 When/Try 覆盖，并更新受统一 catch router 影响的单元 golden；随后发现并修复 outer catch 内 nested cleanup-free try 外跳时的 `EndCatch` owner mismatch，新增 nested try body / catch 两条外跳路径的回归。最终静态复核无剩余 correctness blocker。下一步只执行一次格式化、受影响三 crate clippy、HIR loop-target filter 与 MIR control-flow 模块测试；仍不运行 workspace/full fixture。
- 2026-09-07：完成并提交 4.4b typed target + ordinary/EH cleanup `b42ce94f`。实现、测试与静态审查修复全部攒齐后统一验证一次：格式化、三 crate clippy、HIR 7 项 loop-target 定向测试、MIR 16 项 control-flow 定向测试、diff / snapshot 卫生检查均通过；未运行 workspace/full fixture suite。下一切片为 4.4c coroutine pending transfer + explicit loop-header poll，parser 继续保持关闭。
- 2026-09-07：开始 4.4c。并行只读审计 coroutine pending-transfer/frame resume metadata 与 MIR → LIR loop-header poll 路径；已确认当前 coroutine state 只标 suspension site、LIR 仍由 dominance 推断 back-edge header，两者都不能作为最终 typed ownership。审计收敛前不改 parser、不运行测试；实现、validator、dump与测试矩阵攒齐后再统一定向验证。
- 2026-09-07：4.4c 只读审计收口并拆为两个独立行为提交。现有 `m10-coroutines/suspend-control-flow` artifact 已证明 resume edge 把 source loop 变成多入口 SCC 后，dominance 回边推断漏掉真实 header poll，因此 4.4c1 先以 Body-owned typed marker 修复并删除推断。pending-transfer 侧确认 suspension state 可作为物理 tag，但每个 success/failure state 必须绑定唯一的非空 typed chain；nested outer Return/Break + inner transfer、saved Return/Throwable payload与 typed resume/loop/unwind target 一起留给紧随其后的 4.4c2 原子闭合。两批均不开放 parser，不运行 workspace/full fixture suite。
- 2026-09-07：4.4c1 实现与测试矩阵已在首次验证前整批静态收口。MIR marker/validator/dump、source 与两个 generated wait cycle 登记、coroutine 保留、LIR prune/remap 与 marker-only poll 已齐，并删除 dominance 推断；额外边界审计补齐 `ManagedPoll.live` 重算及 safepoint target/ABI checked validation，正向用例锁定跨 header managed root，负例锁定漏根、越界/错误 target、非 canonical signature 与 NoGc owner。当前只剩最终只读复核及一次格式化、受影响 crate clippy和 `loop_header_poll`/`managed_poll` 定向测试；仍不运行 workspace/full fixture suite。
- 2026-09-07：4.4c1 首轮统一验证完成：`cargo fmt --all`；四个受影响 crate 的 clippy（`-D warnings`）；MIR 3 项、MIR lowering 8 项、LIR lowering 4 项、codegen 8 项 `poll` 定向测试全部通过。随后对静态筛出的 33 个相关 fixture 只启动一次 snapshot refresh；前 14 个更新已逐份审查，无语义回归。执行到 `m11-functions/suspend-values` 时暴露 4.4b 回归：Export HIR 的泛型 `Return(Some(T expr))` 在 `T` 具体化为精确 `Unit` 后仍带 payload，撞上 CFG 的 bare-Unit invariant。当前一次性审计普通、finally 与 suspend 同类路径并补齐回归矩阵；修复后仅刷新尚未执行的 19 个 fixture，不重跑已完成的 14 个，也不运行 workspace/full fixture suite。
- 2026-09-07：泛型 `T -> Unit` 回归修复及 artifact 续跑完成。根据 HIR 已有“Unit return payload 必须为空”不变量，在 Export → LocalConcrete 具体化时保留原表达式求值并发射 bare `Return`，不放宽 MIR CFG 断言；一次补齐 ordinary effectful call、normal/unwind finally 与 immediate suspend `Completed(Unit)` 三类测试。受影响三 crate clippy 通过；`generic_unit_return` 3/3 通过。随后只继续此前未执行的 19 个 fixture，42.14 秒内全部 refresh 通过；临时过滤钩子已移除，未重跑前 14 个、poll 定向测试或 workspace/full fixture suite。当前仅做 snapshot 语义复核、格式与 diff 卫生检查后提交 4.4c1。
- 2026-09-07：完成并提交 4.4c1 explicit loop-header poll `dd753c09`。最终格式、diff、`.snap.new` 与临时 runner 卫生检查通过；代码复核确认 MIR producer/validator、coroutine preservation、LIR prune/remap、marker-only poll、codegen ABI/root checked validation 和泛型 Unit 正规化均无 blocker。33 份相关 snapshot 审查确认 34 个新 header marker 各恰有一个 LIR poll，roots/live 无语义变化。按效率纪律没有重跑已通过的 poll 测试、前 14 个 fixture、workspace 或 full fixture suite。下一切片为 4.4c2 coroutine pending-transfer chain。
- 2026-09-07：开始 4.4c2 coroutine pending-transfer chain。三路只读审计并行覆盖：structured cleanup router → suspension-site split → frame/dispatch metadata 的现有链路；语言/实现/runtime spec 与 GC saved-payload 契约；nested finally、outer Return/Break、failure/catch/EndCatch、finally override 与 moving-GC 的最小测试矩阵。审计收敛前不改实现、不运行测试；实现与测试整批齐备后再统一格式化、定向 clippy/测试，不运行 workspace/full fixture suite。
- 2026-09-07：4.4c2 spec / GC 审计确认运行期不需要第二套 pending-transfer tag 或 target 字段：frame state 已唯一标识物理挂起上下文，动态 Return / managed Throw payload进入 exact typed `CoroutineSlot<T>`，cleanup cursor、typed target与continuation chain只存在于 GC-free 静态 MIR metadata。下一步先修 IMPL / RUNTIME / DESIGN 中要求额外 tagged frame target 字段的冲突措辞；随后整批实现，不提前运行测试。
- 2026-09-07：提交4.4c2权威规范修订`dcd77d6c`。实现前静态测试审计发现现有`coroutine/eh.rs`给整个body共用一个`$coroutine_exception`：outer pending Throwable穿过挂起finally时，inner resume failure即使被catch消费也会覆盖该local，最终错抛inner对象。当前将MIR schema/validator、CFG context/per-scope EH、coroutine transform和唯一聚合fixture并行整批落地；三个producer测试随后一次补齐，在此之前不运行任何suite。
- 2026-09-07：4.4c2 跨模块 API 已在首次编译/测试前冻结：每个 MIR call 携带 `Root` 或结构化非空 outer→inner pending source chain；Fallthrough、Return、Break、Continue、ManagedThrow 分别保存 typed destination / payload / unwind。最终 resume point 只保存冻结后的 typed parent chain及必备 success/failure leaf，state 从 site id 派生，driver dispatch 只从该 metadata 生成；frame 的 state、completion、saved value 与 failure field 采用 checked typed refs，saved/failure 使用不同 id arena。聚合 fixture 源码已一次覆盖 pending Return、pending managed Throw、inner resume failure 被 catch 消费、两层 suspending finally 与每个真实挂起间的 moving GC；尚未生成 snapshot，也未运行测试。下一步完成 transform 与三个互补 producer 测试后再统一验证。
- 2026-09-07：4.4c2 实现与测试矩阵已在首次验证前整批静态收口。MIR schema / checked constructor / validator / dump、CFG pending context、per-unwind-scope managed EH、coroutine liveness / frame / point / dispatch transform、三个 producer case和单一 full-pipeline fixture均已齐；静态 API 扫描另补齐 compiler-generated callback call 的最终 `Root` context，并给新 fixture 增加显式 compile outcome 与 moving-GC stress 路由。当前只做跨模块最终整合审查；审查修复合并后才统一执行一次 `cargo fmt --all`、受影响 crate clippy、MIR validator / MIR-lower pending 定向测试及静态筛出的 coroutine snapshot batch，不运行 workspace/full fixture suite。
- 2026-09-07：4.4c2 首次验证前最终整合审查完成。审查集中修复两项确定 blocker：非空 chain 的 iterator 不再错误承诺 `ExactSizeIterator`；所有 managed exception materialization 固定生成可执行 `BeginCatch` 的 `LandingPad(cleanup=false)`，并由 producer 测试遍历每个 scope 锁定。其余 API、借用、枚举穷尽、pending prefix、liveness、saved/failure 分槽、resume leaves 与 metadata-derived dispatch 静态复核无 blocker。现在才开始唯一验证批：先 `cargo fmt --all`；再对 `scoop-mir`、`scoop-mir-lower`、`scoop-lir-lower`、`scoopc` 执行一次 clippy；用单一 `coroutine` filter 执行 MIR / MIR-lower 定向测试；最后一次性刷新并审查静态筛出的 26 个既有 coroutine fixture与1个新 fixture，不运行 workspace/full fixture suite。
- 2026-09-07：完成并提交 4.4c2 coroutine pending-transfer chain `6402e115`。首次 clippy 一次性暴露新代码误用 `la_arena::Arena::get` 的 24 处同类编译错误，统一改为 checked typed arena lookup 后只重跑该门，四个受影响 crate all-target clippy 通过；MIR / MIR-lower `coroutine` 定向测试各 9/9。fixture 验证按静态清单续跑：首轮已完成 7 份既有 snapshot 后，新 fixture 因公开函数签名引用 internal `Node` 被编译器正确拒绝；修正源码可见性后只运行新 fixture 与剩余 19 份，没有重跑前 7 份，20 份在 43.93 秒内通过，且新 fixture ordinary / moving-GC stress 输出一致。26 份既有 snapshot 的 AST/HIR/run 均与 `HEAD` 字节一致；三路独立审查覆盖全部 27 份相关 snapshot，确认 typed parents/leaves、per-scope EH、exact saved/failure slots、metadata-derived dispatch 及 GC frame refs 均无 blocker。`git diff --check` 通过，无 `.snap.new`、临时 fixture filter 或新增占位实现；按验证效率纪律未运行 workspace/full fixture suite。下一切片为 Iterator / Iterable、`for` 与四种 range。
- 2026-09-07：开始 4.5 Iterator / Iterable 与 source `for`。三路只读审计并行覆盖：语言/实现/runtime spec 的 exact iteration contract 与跨文档一致性；parser → AST → Export HIR → LocalConcrete HIR → MIR 的现有 loop/binding/operator/conformance 链；普通 core、Array/MutableArray iterator、generic/suspend/capture/moving-GC 的最小 correctness-closed 测试矩阵。审计收敛并冻结完整 API/diagnostic/测试清单前不改实现、不运行测试；实现、validator、negative与fixture整批齐备后再统一验证。
- 2026-09-07：4.5 spec 审计确认语言/runtime 主线一致且无需新增 runtime ABI，但发现实现规范把 IterationCore 描述为另一套 Option 变体身份、以及 4.4 未开放的 break / continue 在剩余计划中没有 parser 归属。先把 IterationCore 统一为复用 canonical checked OptionCore（含 Some payload field / None ref），把外层 plan + binding plan 完整 reader 验证列为硬门，并明确 4.5 原子开放 for / break / continue；其余两路审计收敛前仍不改实现、不跑测试。
- 2026-09-07：4.5 三路只读审计与 API 冻结完成。parser/AST 以专用 token 原子开放 `for` / `break` / `continue`，继续封闭 labels、jump expression 与 `do-while`；Export HIR 采用私有 `ForIterationPlan`、raw result conformance witness 与 per-use `AppliedOptionCore`，完整 reader validator 覆盖 canonical core、唯一 exact Iterator closure、next/Option、binding schedule 与 typed loop；concretization 机械展开为既有 While，并把接口约束参数的 Box 适配延迟到实际 value/ref kind。core source 同批加入 Iterator/Iterable、ArrayIterator/MutableArrayIterator 与普通 array conformance。当前 visitor、default-template、测试/fixture仍在合批，尚未运行格式化、编译或测试；静态清单清零后再启动唯一分层验证批。
- 2026-09-07：4.5 实现/测试批已在首次验证前整体落盘：parser/AST、Export-HIR 完整 plan 与 validator、LocalConcrete 展开、default-template fresh remap、core Iterator/Iterable 与两类 array iterator、全部相关 visitor已齐；5 组 HIR 测试集中锁定 basic relation、exactly-once/poll、member winner、0/多 exact application、generic value boxing/reference retype 及 nested typed loop，单一 full-pipeline fixture 同时覆盖 Array/MutableArray、递归解构、fresh capture、suspend iterator 与 moving GC。现在并行做 reader boundary/pipeline 顺序及 schema/concretization 的最终只读审计；尚未运行 fmt、lint、编译、测试或 snapshot refresh，审计修复一次合并后才启动唯一验证批。
- 2026-09-07：4.5 首次验证前静态终审定位并合批处理四类缺口：protocol method/bound owner与完整专门化签名校验、source/iterator/component setup局部量freshness、手写Export HIR harness的canonical IterationCore，以及DESIGN/PLAN中过时的Option身份和parser风险描述；同时补伪造plan与高优先级失败/ordinary effect/control-flow negative覆盖。此时仍未运行任何格式化、lint或测试，待静态清单一次清零后统一验证。
- 2026-09-07：按“静态清单一次收齐、完整 suite 只在完成门运行”的效率纪律继续4.5终审。静态交叉检查又发现合法 named struct 反序字段解构的 declaration-order shape 与 source-order action 被 reader 按同一 cursor 消费；validator 已改为先按源码序验证 action dataflow，再按 typed edge 与声明序 shape 唯一匹配并全部消费，同时锁定 for 叶 binding 不可变。对应反序正例与 method owner、bound signature、reserved local、错误 loop target、mutable leaf 伪造负例已合批；23处内联 HIR core golden 与一个用户自定义 `Iterator` 名称冲突也已静态修正。仍未启动格式化、编译、测试或 snapshot refresh。
- 2026-09-07：4.5 最新树的静态审计再合批闭合三项边界：reader 现以 callable/default region 全局定义集合拒绝 plan hidden local 复用函数参数、default receiver/value parameter、外围 `val` 或另一分支 local，同时保留 plan 内部错误的更精确诊断；component call 不再被错误地一律限制为 ordinary，ordinary caller 的 effect negative 与 suspend `for` 的 component 正例同时锁定；parser 在 statement/direct-branch 入口也会把 `break + 1`、`continue.member`、`break()` 精确诊断为未支持的一般 jump expression。测试文件已按职责拆分且均低于长度门，fixture/sysroot/moving-GC 与 parser 审计无剩余 blocker；两路 HIR 最新树复核完成前仍不启动任何验证命令。
- 2026-09-07：继续按“先收齐静态清单、后跑唯一验证批”处理 4.5。reader 新增 source 前缀 future-local 扫描，覆盖五个 reserved temporary、iterator/component setup、binding action、loop body，以及 Lambda/anonymous/callable-reference capture 和 bound receiver 的间接求值；非法/循环 callable entity 安全拒绝。协议 receiver validation 现区分 member subtype 与 extension exact parameter，并验证 reference upcast、value/interface boxing、function coercion 的 typed proof。每个 callable/default region 非可选地携带 suspend permission，default template 同时核对全部 source owner、orphan/非法 source 与继承后一致性。对应 inherited member/extension upcast、suspend iterator/component/default 与 forged future read/box/effect/source 负例已集中补齐；尚未运行 fmt、lint、build、test 或 snapshot refresh，待最新树终审归零后一次执行。
- 2026-09-07：4.5 首次验证前按完整验收矩阵再做一次静态清单审计，没有用慢 suite 逐项探测。由此补齐 source setup 的 self/forward/单分支/loop definite-definition reader proof，generic bound 已选 extension 不因 concrete member 出现而重选的判别性用例，canonical `next` ordinary 负例，LocalConcrete Some test/payload projection 结构断言，以及用户 `Iterable<T>` 静态视图、empty path和 boxed ZST value iterator 的 full-pipeline 场景。unsafe lexical block 因 Export HIR 既有统一扁平化且不改变后续控制流/ABI shape，仍沿用普通 HIR producer 信任边界，不给 iteration plan 另造 safety 状态。此时仍未运行 fmt、lint、build、test 或 snapshot refresh；最新树无 blocker 后才进入单一验证批。
- 2026-09-07：4.5 首次验证前最后一轮 reader 审计发现旧的“定义集合 + future 全树扫描”不能表达 abrupt reachability，并漏记由首次 mutable assignment 建立的 merge local。现已统一为一套 source-order definite-definition / ownership flow：normal edge 才参与交集，return/throw/typed break/continue 显式传播，嵌套 while/for 消费自身 jump，when 先检查 subject，try/finally 按完成方式合流；不可达语法只登记 identity、不检查 read。同步补齐 when subject 前向读、abrupt loop、return 后不可达 future-read、assignment-only local 跨 plan 复用和 `@NoGC` 隐藏 `next` 回归。三路最终静态复审均无 blocker，且至此仍未运行 fmt、lint、build、test 或 snapshot refresh；下一步严格按第 4.5.10 项执行唯一验证批。
- 2026-09-07：4.5 分层验证已完成前两级。先统一格式化并通过受影响 `scoop-hir` / `scoop-hir-lower` all-target clippy；parser 定向门 11/11 通过后不再重复。HIR 首轮 23/32，通过完整失败输出一次归并出 visibility、非法裸 expression statement、value-to-interface extension boxing、bound signature receiver 投影与 generic winner 用例隔离五类问题；整批修正并静态复核后只重跑 HIR 门，32/32 全绿。下一步仅执行审计确认的最小 MIR/downstream 门和一次 full fixture snapshot refresh，不运行 workspace suite。
- 2026-09-07：4.5 下游门审计确认 source `For` 在 Export → LocalConcrete HIR 具体化时已完整展开为 `ValDecl + While`，LocalConcrete HIR 与 MIR 均不存在 `For` variant；MIR production 未改，mir-lower 只为新增必填 `Module::iteration_core` 更新测试 harness，且已由 all-target clippy 编译覆盖。因此不为凑验证项重跑旧 MIR control-flow suite，直接由 HIR concrete-shape 测试和唯一一次 full-pipeline fixture 覆盖实际 MIR/LIR/codegen、suspend/header-poll/moving-GC 组合。
- 2026-09-07：4.5 唯一 full fixture 完成门已通过：`INSTA_UPDATE=always cargo test -p scoopc --test fixtures` 4/4，371.80 秒。新 `m22-iteration` 正向与 5 个 negative snapshot 均已生成；core 新增声明使 186 份既有 snapshot 系统性刷新。不会再跑完整 fixture；当前并行审计新 fixture 语义、既有 snapshot 漂移与最终树卫生，只对真实 blocker 做定向处理。
- 2026-09-07：4.5 snapshot 与最终树审计完成，无 correctness blocker。新正向 golden 锁定 Array/MutableArray、Iterable 静态视图、ZST iterator boxing、递归解构/fresh capture、typed jump/finally、suspend/header poll 与 moving-GC roots；5 份 negative 的位置和消息完整。186 份既有 snapshot 中，185 份 AST/HIR 归一化后只含新增 iteration core 与实体 ID 顺延，175 个 run 和 10 个 trap 输出逐字不变；45 份 MIR/37 份 LIR 的额外 CFG 规范化全部可归因于已提交的 typed cleanup 路由 `b42ce94f`。唯一 diagnostics 变化是用户测试类型 `Iterator` → `Cursor` 的预期避名。提交前把 iteration default-instantiation 原样拆为 255 行子模块，使父文件从 1018 降到 767 行；只重跑格式化与 `scoop-hir-lower` all-target clippy并通过。`git diff --check`、无 `.snap.new`、无新增占位实现等卫生门均通过。本切片只复用既有 runtime/GC/coroutine/ABI 合同，不改变 runtime ABI、对象模型、GC 或 runtime function contract，因此无需修改 runtime spec；下一步提交 4.5，随后进入 4.6 range。
- 2026-09-07：完成并提交 4.5 Iterator / Iterable 与 source `for`：`8ba3320b`。实现、reader validation、default/concretization、core、定向与 full-pipeline/moving-GC fixture及全部 golden 同一功能提交落地；未运行 workspace suite。下一切片为 4.6 四种 range 与整数循环组合，继续先做静态 inventory，再成批实现与定向验证。
- 2026-09-07：开始 4.6 四种 range 与整数循环组合。三路只读审计并行覆盖权威规范与跨文档一致性、现有 parser/operator/core/integer lowering 接点，以及四种 nominal/窄整数/方向/step/contains/MIN-MAX/poison/header-poll 的最小测试矩阵。审计清单冻结前不写实现、不运行测试；后续把实现与测试一次攒齐，再执行单一分层验证批。
- 2026-09-07：4.6 规范审计收口：四种 range 只需普通 core 源码，不增加 runtime ABI、compiler `RangeCore` 或 LIR 指令；先用独立 spec-first 提交冻结八种 owner 的完整成员声明、统一命名参数，以及 range / `step` 不可变、fresh independent iterator 与永久耗尽语义。完整实现和测试继续等待另外两路静态审计一次收齐；验证固定为单批 fmt、受影响 clippy、定向测试与最终唯一一次 full fixture refresh，失败只重跑对应门。
- 2026-09-07：4.6 测试审计收口：只补 1 个 parser 组合 AST、2 个 HIR 表驱动测试，并扩既有 codegen 无 overflow-flag 断言；range 不穿越为专用 MIR/LIR 节点，因此不造重复 synthetic suite。端到端新增一个 semantics 与一个汇总 negative，suspend/moving-GC 复用现有 `m22-iteration/combined.scoop`。fixture harness 没有路径过滤，故必须等实现、测试与静态审查全部稳定后才运行唯一一次完整 refresh；本切片不运行 workspace suite。
- 2026-09-07：4.6 实现接点审计收口，无 compiler production blocker。实现面严格收敛为 `types.scoop` 的八 owner × 四 member、新 `ranges.scoop` 的四 range / 四 iterator，以及 `throwable.scoop` 的普通异常。signed 全域统一以同宽 unsigned distance 做 contains/终止，open range 不做 endpoint−1，更新 current 前先证明 stride 未越界。三路清单现已冻结；先完成独立 spec-first 提交，随后才并行写实现与测试，期间不运行验证。
- 2026-09-07：4.6 spec-first 最终复核无 blocker；补充冻结五种成功构造路径均产生 fresh range identity（含空 range 与 `step`，结果不与 receiver 同一引用）。语言、实现、设计与既有 runtime 条款现一致；提交前只执行文档 diff / whitespace 门，不运行代码 suite。
- 2026-09-07：完成并提交 4.6 spec-first 修订 `fc381075`；只运行 `git diff --check`，未运行代码测试。随后把普通 core、parser/HIR 定向测试、fixture/codegen 验收拆成三个无重叠文件批并行实现；所有批次和静态交叉审查完成前继续不运行 fmt、build、test 或 snapshot refresh。
- 2026-09-07：按批量验证纪律把 4.7 的最终跨特性组合门前移到本批已有的 `m22-iteration/combined.scoop`，与 range suspend/moving-GC 扩展共同落地；不新增第二个 stress 程序。这样 4.6 的唯一 fixture refresh 同时产出 M22 组合 golden，后续若静态收口不再改行为则不重复运行完整 fixture。
- 2026-09-07：4.6 三个实现/测试批与三路交叉静态审查全部收口。终审在任何验证前一次性发现并修正 5 处当前 parser 不支持的 `when (val ...)`、1 处普通 `if` 缺 block，以及空 range 构造/step 的 fresh identity 覆盖；core 全域算术、32 owner member、HIR builder/API、negative 隔离、有限输出与两次 suspend/moving-GC 时序复核均无 blocker。现在才启动一次 fmt、一次受影响 clippy 与整批定向测试。
- 2026-09-07：统一 `cargo fmt --all` 已完成。首次受影响 crate clippy 只暴露两处新 Rust 测试的机械编译问题：对非 `Copy` 的 `LirType` 使用数组重复表达式，以及 HIR 断言 helper 超过七参数 lint；现已成批改为显式三个 type 与组合 modifier 参数，只重跑 clippy 门，不重复格式化或启动其他测试。
- 2026-09-07：只重跑 clippy 失败门后，`scoop-parser` / `scoop-hir-lower` / `scoop-codegen` all-target clippy 全绿。随后一次运行 parser/HIR `m22_range` 定向批，HIR 2/2、parser 1/1；codegen 只运行扩宽后的 s32/u32/s64/u64 overflow-flag 用例，1/1。下一步执行本切片唯一一次 `INSTA_UPDATE=always cargo test -p scoopc --test fixtures`，不再运行已通过门或 workspace suite。
- 2026-09-07：首次 full fixture refresh 在 393.83 秒后只于改动的 `m22-iteration/combined.scoop` 失败；此前路径已完成 snapshot 更新。单文件 `scoopc build` 定位为新增 Iterator/Iterable 的 public generic slot 暴露 fixture-internal `Entry<Node>`；helper owner 收窄为 `internal` 后 slot contract 仍要求 exact type 公开，因此同时把 `Entry` / `Node` 明确为 `public`。继续只重跑该单文件定向编译，确认后再重跑失败的 fixture 门；不重跑 fmt、clippy、parser/HIR/codegen 或 workspace suite。
- 2026-09-07：修正 visibility 后，单文件定向门全部通过：组合 fixture 编译成功，ordinary 与 `SCOOP_GC_STRESS_MOVE=1` 均退出 0 且输出一致；range semantics 单文件编译/运行成功，完整域序列、粘滞耗尽与非法 step 输出符合预期；negative 单文件精确产生 18 条预期诊断，constructor 四条均只因 internal visibility 失败。现在只重跑失败的 full fixture refresh 门，不重跑任何已通过门。
- 2026-09-07：full fixture 失败门采用排序路径续跑而非重放前缀：首段 393.83 秒已覆盖起点至 `m22-iteration/combined.scoop` 前，修复后的续段从该失败 fixture 到 `m9-gc/generic-struct.scoop` 全部通过（fixture harness 4/4，177.28 秒）。两段合起来对完整排序集合只做一次逻辑遍历，失败项仅在修复后补跑；用于续跑的临时 `fixtures.retain` 已立即删除并确认 runner 零 diff。现在并行审计 range semantics、M22 组合链、18 条 negative 与既有 snapshot 的结构性漂移；不会再次运行 full fixture suite。
- 2026-09-07：4.6 snapshot 与最终树审计完成，无 correctness blocker。新 semantics golden 从 AST 到 LIR 锁定四个 nominal range、八 owner × 四 API、窄类型 widening、exact signedness/width、开闭/升降/step/contains、fresh identity/iterator、sticky exhaustion、全域 MIN/MAX 安全终止与 8 种非法 step；LIR 无 `nsw`/`nuw`/poison 或 range intrinsic/runtime ABI。组合 golden 锁定 fixed-width range → 自定义 Iterable 解构 → recursive when → **struct-only** copy update → try/finally continue/break → suspend/moving-GC，ordinary/stress 输出与 snapshot 逐字一致。186 份既有运行/陷阱 snapshot 全部刷新：185 份输出逐字不变，唯一预期变化为组合 fixture；移除新增 core 声明并归一化实体 ID 后，未改源码 fixture 的 AST/HIR 与基线一致，注入的 32 API、12 range 方法、4 `next`、4 Option 特化及 8 个类型/布局/descriptor 完整一致。negative golden 为精确 18 条诊断。最终 `cargo fmt --all -- --check`、`git diff --check`、无 `.snap.new`、无新增占位实现、临时 runner 零 diff；格式检查只修正一处 Rust 测试排版，不重跑已通过测试。下一步提交 4.6，再按 4.7 静态清单执行唯一一次 workspace 收口门，不重跑 full fixture。
- 2026-09-07：完成并提交 4.6 四种 typed integer range：`ff97a505`。四个 nominal range/iterator、八 owner × 四 API、`IllegalArgumentException`、parser/HIR/codegen 定向回归、range semantics/negative、M22 suspend-moving-GC 组合及 188 份 golden 同一功能提交落地。下一步进入 4.7：先完成最终静态清单，再只运行未被 4.6 fixture 门覆盖的 workspace 收口验证，不重跑 full fixture。
- 2026-09-07：开始 4.7 最终收口。先并行核对 DESIGN 8/9、ROADMAP M22、三份 spec 与历史完成证据，并静态搜索旧 64 位 `Int`/`UInt`、Option 裸 variant 特判、return-only cleanup、按名/ordinal identity、source iteration plan 泄漏、enum/class copy update 与漏 poll 回边。4.6 已完成的 full fixture 逻辑遍历与 ordinary/moving-GC 组合视为最终 fixture 证据；除非静态审计发现真实行为 blocker，否则不再次 replay。最终测试将排除 `scoopc` 的 slow fixture target，只补齐其余 workspace target 与 driver 非 fixture tests。
- 2026-09-07：4.7 三路静态清单在任何最终 suite 前完成。生产实现未发现旧整数解释、Option 特判、return-only cleanup、identity 回退、source plan 泄漏、enum/class copy update runtime 路径、漏 poll 回边或 range runtime/intrinsic 残留；copy update 仍在字段规划/RHS lowering 前仅接受 exact declared struct。清单只找到旧 Array 下标注释仍写 `Int`、最终门仍要求重复 fixture、以及 fixture 基础设施实际只 dump Export HIR 三项文字偏差，现已合批修正。测试矩阵另发现两个真正非等价覆盖缺口：match named-field shorthand 的 RHS 与 Unit/unit variant/payload variant 同名时的分流，以及单 variant `V(Boolean)` 的最终 refutable arm 在 MIR 中仍保留 Boolean test；两项正由独立文件批并行补齐，完成静态复核前不运行 fmt、clippy 或测试。
- 2026-09-07：两个最终覆盖缺口已在不同 crate 的测试文件中合批落地并静态复核：HIR 表驱动用例同时锁定字段 shorthand 的 Unit literal、unit variant 与 payload variant 显式 shape 诊断；synthetic HIR → MIR 用例锁定单 variant `V(Boolean)` 仍先验证 variant、再执行 Boolean equality，并让两条 false edge 汇入 fallback。没有 production 行为改动；现在统一执行一次格式化、一次 workspace all-target clippy、一次排除 `scoopc` 的 workspace tests 与一次 `scoopc` 非 fixture target 批，任何失败只重跑对应门。
- 2026-09-07：最终 workspace clippy 首轮只发现新 MIR 测试局部闭包缺少 `BlockId` 类型，补注解后仅重跑 clippy 并全绿。随后 `cargo test --workspace --exclude scoopc --no-fail-fast` 一次收齐全部目标：除 `scoop-hir-lower` 外所有 crate 与 doc tests 均通过，HIR crate 集中暴露 6 项；其中新 shorthand 用例需要断言 Unit literal 的既有 wildcard 正规化，4 个旧 dump 用例共 6 处 function id 因 4.6 新增 `IllegalArgumentException` 顺延 2，另一个旧 `should_panic` 仍期待 reader validator 收紧前的措辞。现已一次合批修正；下一步只重跑 `scoop-hir-lower --lib`，不重跑已通过的其余 workspace 目标或 full fixture。
- 2026-09-07：M22 最终验证完成。HIR 失败门批量修正后的 crate 运行 735/736，唯一剩余项确认是测试 helper 未模拟 parser 对 shorthand `Unit` 保留 literal AST；改为真实 AST 后仅定向重跑该项并通过，所以最终 736 项获得一次逻辑覆盖而未再次重放整 crate。最终树的 affected HIR clippy 通过；首轮 workspace 中其余 crate/unit/doc 全部通过（包含新增 MIR 单变体 Boolean 分支测试）；`scoopc` 非 fixture 门为 lib 12/12、bin 0、source integer artifact 1/1、warnings 2/2、doc 0。4.6 已审计的 full fixture 4/4、ordinary/moving-GC 组合与 188 份 golden 直接复用，不重复执行。最终只剩格式/diff、`.snap.new`、占位符与工作树范围卫生检查，完成后提交 4.7 收口。
- 2026-09-07：4.7 最终卫生门通过：`cargo fmt --all -- --check` 与 `git diff --check` 均为零差异；无 `.snap.new`；新增 compiler/sysroot/runtime diff 中没有 `TODO`、`todo!` 或 `unimplemented!`；工作树精确只含两项新覆盖测试、4.6 引起的四份旧 HIR dump ID 顺延、一份 reader 诊断期望、Array `Long` 下标注释，以及 ROADMAP/DESIGN/PLAN 收口文档。M22 已满足全部完成条件，下一步提交最终收口变更。
