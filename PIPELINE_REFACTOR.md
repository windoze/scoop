# Pipeline Refactor

## 定位

这份文档是本轮 pipeline 清理的第一个目标设计文档。

`PIPELINE-CLEANUP.md` 负责记录当前实现中的问题、耦合和历史包袱。
这份文档负责回答另一类问题：

1. 目标 pipeline 到底应该长成什么样。
2. crate 应该如何划分。
3. 每个阶段应该发布什么 IR 和什么 global facts。
4. 后续重构时，哪些依赖关系和输出形状是绝对不允许回退的。

本文采用一个明确前提：

1. 当前的 `effect_lowered` 被视为正式的 LIR，而不是 LLVM codegen 的临时子阶段。

## 目标

本轮重构的目标不是单纯把代码拆小，也不是先做新的 backend。
真正的目标是把编译器重新整理成一个单向、可验证、可替换后端的阶段图。

目标 pipeline 形状如下：

```text
AST -> HIR -> MIR -> effect facts -> LIR -> codegen
```

这条线的含义是：

1. 每个阶段都只负责把上一阶段的 authoritative 输出转换成自己的 authoritative 输出。
2. 每个阶段都可以额外发布属于自己职责边界的 global facts，供后续阶段消费。
3. 后续阶段可以依赖这些 facts，但不能回头重跑上游阶段，也不能把上游 stage output 当成自己的长期依赖。
4. `codegen` 的唯一 authoritative IR 输入是 LIR。若 codegen 仍需要直接依赖 MIR/HIR，说明 LIR 还不完整。

## 本轮范围裁剪：先移除现有 comptime

本轮的目标是重构编译 pipeline，而不是保留所有现有语言实验功能并试图原样迁移。

在这个前提下，现有 comptime 功能应被视为一个**前置清理目标**：

1. 先整体移除现有 comptime 语义路径。
2. 把类似能力留待后续单独重设计。

### 为什么要先移除

现有 comptime 的问题不是“功能还不够完整”，而是它天然倾向于跨越多个阶段边界：

1. 它会影响 AST 是否保留、裁剪或改写某些源码结构。
2. 它经常需要 resolve/typecheck 才能决定该如何展开。
3. 它很容易要求读取 cone/project 级上下文，而不是单一阶段输出。
4. 它会诱导后续阶段继续保留“为了 comptime 先放宽一下边界”的例外路径。

而本轮重构恰恰要建立的是：

1. 单向阶段图。
2. 单一 authoritative 输出。
3. 自包含 fact crate。
4. 不跨阶段回看的 backend。

在这种情况下，强行保留现有 comptime，只会让所有阶段边界持续带着特判和例外，导致 refactor 的基础假设始终不干净。

### 本轮的设计决策

因此，本轮设计里对 comptime 的处理原则应明确为：

1. **现有 comptime 全部退出正式 pipeline 设计范围。**
2. 这里说的“退出”不是保留 surface 再专门拒绝，而是直接移除 comptime surface 本身以及所有相关实现入口；这应包括现有 `const` 等 comptime 相关语义点。
3. `AST -> HIR -> MIR -> effect facts -> LIR -> codegen` 这条主线，在本轮不再承担兼容旧 comptime 的职责，也不再为它保留任何专门的信息通道。

这意味着：

1. 使用旧 comptime surface 的代码若在重构后失效，应作为普通 parse/resolve/typecheck 失败自然暴露。
2. 我们不应在代码中保留“这是旧 comptime，所以特意拒绝它”的专门逻辑。

### 这不等于永久删除 comptime

这只是一次范围裁剪，不是语言层面的永久否定。

更准确地说：

1. 现有 comptime 实现与 surface 一并移除。
2. 类似功能未来若要恢复，必须作为一个单独设计题重新定义。

那个未来设计至少要回答：

1. 它究竟是 parser-time feature、semantic-frontend feature，还是一个独立 transformation stage。
2. 它依赖哪些输入。
3. 它发布哪些独立 facts/outputs。
4. 它如何不破坏 stage/fact crate 的依赖 DAG。

在这些问题没有单独设计清楚之前，本轮 pipeline refactor 不应继续背着旧 comptime 一起演进。

## 编译单元定义

在这次重构里，必须先把“文件”、“cone”、“编译单元”和“整个 build 输入”区分开。

如果这几个概念不先分清，后面关于 stage output、fact owner 和 crate DAG 的约束都会变得含糊。

### 文件不是编译单元

单个 `.scoop` 文件不能作为正式的编译单元定义。

原因很简单：

1. 同一个 cone 内的多个文件允许互相引用。
2. 这些引用不仅可能是单向的，也可能形成递归/环式关系，例如跨文件的函数互调、类型互见、扩展方法互用。
3. 如果把“单文件”当编译单元，就必然会把“先编哪个文件”错误提升成语义问题。

而正确的语义应该是：

1. 同一个 cone 内的文件共同构成一个声明闭包。
2. 文件顺序最多影响实现上的遍历顺序和输出稳定性，不能影响程序是否可编译。

### cone 才是源码级编译单元

在 source-only 构建里，正式的编译单元应定义为：

1. 一个 cone 的全部 Scoop 源文件。

原因是：

1. 可见性边界在 cone 上成立，尤其是 `internal`。
2. 同 cone 内允许自由相互引用。
3. 不同 cone 之间通过 source-cone graph 形成依赖 DAG，这一层才有稳定的“无互相引用保证”。

所以更准确的定义应该是：

1. `source file`：一个物理文件。
2. `source cone`：一个包/可见性边界/源码分发单元。
3. `compilation unit`：一个 cone 的全部源码，加上它依赖 cone 暴露出的 API/facts。
4. `build graph`：当前 build 需要处理的整个 source-cone DAG。

### 整个 project build 不是一个 compilation unit

当前实现里，`ProjectInput::build_closure_sources()` 仍提供 sysroot、local dependency、consumer cone 的 build-closure source view。
同时，facade 已通过 `ProjectInput::compilation_units()` / `consumer_compilation_unit()` 暴露 cone-level unit 视图，AST stage 也有 `AstCompilationUnitOutput`。

这个 build-closure view 可以暂时服务现有 resolver/typecheck 过渡路径，但不能被理解成“一个大 compilation unit”。

更合理的理解是：

1. 这是一个 build closure。
2. 它由多个 cone-level compilation unit 组成。
3. 这些 unit 按 cone dependency DAG 拓扑排序后分别编译。

也就是说：

1. `SourceConeGraph` 是 build 输入。
2. cone node 才是 compilation unit。

### 虚拟单文件输入只是“单文件 cone”特例

当前 virtual-cone 模式本质上不是“单文件编译单元”，而是：

1. 一个只含单个源文件的 synthetic consumer cone。

见 `crates/scoopc/src/cone/graph.rs::load_source_cone_graph_for_virtual_consumer` 和 `crates/scoopc/src/frontend.rs::ProjectInput::consumer_compilation_unit`。

所以它不应该成为其它场景的定义依据；它只是“一个 cone 刚好只有一个文件”的特例。

## 编译顺序模型

一旦把 cone 定义为编译单元，编译顺序就应分成两层：

1. cone 之间的顺序。
2. cone 内部的阶段顺序。

### cone 之间：按 DAG 拓扑排序

不同 cone 之间的依赖顺序应由 `SourceConeGraph` 决定，而不是由文件系统路径或 frontend 遍历顺序决定。

规则应是：

1. 先编译依赖 cone。
2. 再编译 consumer cone。
3. 每个 cone 只消费依赖 cone 已发布的 API/IR/facts，而不是直接消费依赖 cone 的内部 stage 输出。

这意味着：

1. sysroot cone 是最前面的若干节点。
2. local dependency cone 在 consumer cone 之前。
3. consumer cone 最后。

目标上，cone 间不应通过“把所有源 flatten 到一个 giant frontend run”来模拟依赖关系。

### 同一 cone 内：不是文件拓扑，而是阶段屏障

同一个 cone 内，由于文件可以互相引用，所以不存在一个语义上可靠的“文件编译拓扑序”。

正确做法不是给文件排序，而是给阶段设屏障。

也就是说，cone 内的顺序应该是：

1. 先让某个阶段在整个 cone 的全部文件上完成。
2. 再进入下一个阶段。

文件遍历顺序只允许用于：

1. 产生稳定输出。
2. 让诊断顺序可重复。

它不应承担语义职责。

### 推荐的 cone 内阶段顺序

对一个 cone-level compilation unit，推荐的顺序如下：

1. Parse 全 cone所有文件。
2. 做 parser-owned / header-only gate。
3. 收集并建立全 cone 声明索引。
4. resolve headers。
5. resolve bodies。
6. typecheck headers / signatures / declarations。
7. typecheck expressions / bodies。
8. 产出 HIR + HIR facts。
9. 基于整个 cone 的 HIR 产出 MIR + MIR facts。
10. 基于 canonical MIR 产出 effect facts。
11. 基于 MIR + effect facts 构造 LIR + LIR facts。
12. 后端消费 LIR。

这里最关键的是：

1. “文件先后”只存在于每个阶段内部的实现遍历。
2. “阶段先后”才是真正的语义顺序。

### 为什么必须用阶段屏障

以当前 frontend 为例，已经能看到这种趋势：

1. 先 parse 所有源文件。
2. 再对所有文件做 header 检查。
3. 再建立全局 `Index`。
4. 再跑 `resolve::check_file_headers`。
5. 再跑 `resolve::check_file_bodies`。
6. 再扩展 `TypeEnv`。
7. 最后再做 typecheck 各项检查。

见 `crates/scoopc/src/frontend.rs:506-660`。

这已经说明：

1. 当前实现本质上也不是按“一个文件完整编完再编下一个文件”。
2. 它已经在用“全局索引 + 分阶段遍历”的方法处理同 cone 多文件互引。

重构要做的，不是发明新的文件顺序，而是把这种“阶段屏障”设计正式化、crate 化、fact 化。

### 文件顺序只负责稳定性，不负责语义

同 cone 内部仍然需要一个稳定的文件遍历顺序，但它的用途只能是工程性的：

1. 稳定 dump 输出。
2. 稳定诊断顺序。
3. 稳定 codegen 命名或测试快照。

推荐做法：

1. 对 cone 内源文件按规范化路径排序。
2. 所有“收集阶段”都按这个顺序遍历。
3. 但任何语义判断都不能依赖这个顺序。

### top-level 初始化顺序不是文件编译顺序

另一个容易混淆的点是：

1. 编译顺序。
2. 运行时 top-level 初始化顺序。

它们不是一回事。

即使同 cone 多个文件之间存在 top-level value / object / immutable init 的依赖，也不应该靠“先编哪个文件”解决。

正确做法应该是：

1. 在 HIR/HIR facts 层显式发布 top-level init dependency facts。
2. 在 MIR/LIR 层继续保留 initializer root / dependency contract。
3. 运行时初始化顺序由这些依赖合同决定，而不是由源码文件顺序决定。

## 全局初始化模型

把归档的 Sysroot R2 文档（`docs/archive/plans/PLAN-sysroot-reshape-r2.md`、`docs/archive/plans/TODO-sysroot-reshape-r2.md`、`docs/archive/designs/SYSROOT_RESHAPE_R2.md`）中现有的 object once、top-level eager init、per-cone init routine 和 storage policy 要求统一起来后，本轮设计稿应覆盖下面这组全局初始化语义。

### 哪些声明属于“全局初始化根”

在移除现有 comptime（包括 `const`）之后，本轮正式 pipeline 中的运行时全局初始化根只应包括：

1. `object` singleton
2. top-level `val`
3. annotated top-level `var`

其中：

1. `const` 已随现有 comptime 一起退出本轮设计范围，因此不再是运行时全局初始化模型的一部分。
2. `top-level var` 必须显式选择 storage policy，不能再保留“未标注也先让后端猜一个行为”的空间。

### object 与 top-level value 的语义不同

这三类根不能继续被一个模糊的“global init”概念混在一起，它们至少要区分为两条语义路线：

1. `object`
   - 语义上是 singleton identity。
   - 初始化模型是 **once**。
   - 不要求在 `main` 前把所有 object 都 eager 初始化。

2. top-level `val` / annotated top-level `var`
   - 语义上是 cone-level global storage roots。
   - 初始化模型是 **eager pre-main init**。
   - 必须在进入用户 `main` body 前完成初始化。

这两个模型不应再共享同一套 runtime helper 或“第一次访问时顺手初始化”的语义。

### object 初始化模型

`object` 的目标语义应固定为：

1. 单个 object 声明对应单一 singleton storage identity。
2. 初始化由 codegen 实现的 once 协议保护。
3. 其他线程不得观察到半初始化状态。
4. 同线程递归访问同一正在初始化的 object 是 fatal compiler/runtime contract violation，而不是普通用户 effect。
5. `object` 初始化中的普通 self access 不能被当作“已发布 singleton”读取路径。

更细一点说，本轮设计稿至少应覆盖：

1. `0/1/2` 一类的 guard state machine 语义。
2. 同线程递归/间接环检测需要 compiler-owned 语义结构，而不是依赖用户可见 sync primitive。
3. object 初始化期间，对“同 object 已完成初始化的前序 property”与“后序 property / self ordinary access”要有不同 contract。

### top-level `val` / `var` 初始化模型

top-level `val` 与 annotated top-level `var` 的目标语义应固定为：

1. 覆盖所有 linked source cones。
2. 在用户 `main` body 前 eager 初始化完成。
3. 不允许 lazy first-access。
4. 初始化顺序先按 source cone DAG，再在每个 cone 内按稳定顺序执行本 cone roots。

这意味着：

1. final system entry 必须先跑 linked cones 的 init routines，再进入用户 `main`。
2. per-cone init routine 只负责初始化本 cone roots，不递归替依赖 cone 干活。
3. top-level global root 的依赖顺序必须通过 facts/IR contract 表达，而不是隐藏在文件顺序里。

### top-level `var` 的 storage policy

top-level mutable storage 不允许保持模糊状态。

本轮设计应固定为：

1. `top-level var` 必须显式标注 `@Global` 或 `@ThreadLocal`。
2. `@Global var` 使用 global storage，并在 cone init routine 中初始化。
3. entry thread 的 `@ThreadLocal var` 使用 TLS storage，并在 cone init routine 中初始化。
4. 非 entry thread 的 TLS 初始化策略必须被单独定义；若本轮不支持，也应在语义上显式限制，而不是留给后端猜。

### global object/var/val 不能是 generic

这条约束需要明确加入本设计稿：

1. **任何发布 global singleton identity 或 global storage identity 的声明，都不能是 generic。**

这包括：

1. `object`
2. top-level `val`
3. top-level `var`

原因不是“当前实现不方便”，而是语义上就冲突：

1. global object/var/val 必须对应单一稳定的 storage/singleton identity。
2. generic 声明一旦参与 monomorphization，就会潜在地产生多个实例。
3. 多实例与“单一全局 identity / 单一 per-cone init root / 单一固定 storage cell”这三个属性不能同时成立。

所以这条规则应被视为前端语义约束，而不是后端命名或链接细节：

1. 如果某个声明会发布 global root，它就必须是 monomorphic。
2. 即使将来语法层允许出现看似“带 type param 的全局声明”，语义前端也必须稳定拒绝。

### 这些语义应由哪些阶段发布

从阶段职责看，这组语义应拆成几部分：

1. HIR/HIR facts
   - root 分类：object / top-level val / top-level var
   - top-level init dependency roots
   - storage policy（`@Global` / `@ThreadLocal`）
   - “global roots must be monomorphic” 这类前端拒绝约束

2. MIR/MIR facts
   - initializer root inventory
   - root-level dependency / ordering contract
   - 若需要供后续阶段复用的 per-root control/data metadata，也应在这里显式化

3. LIR/LIR facts
   - per-cone init routine contract
   - final system entry 调用顺序 contract
   - object once contract
   - top-level eager init contract

4. codegen
   - 只负责把这些 contract 物理化成目标后端产物

换句话说，后端不应再自己决定：

1. 哪些 global root 要初始化
2. 先后顺序是什么
3. 某个 global root 是否允许 generic

这些都应在更早阶段被显式决定。

## 错误收口边界

除了 link error 之外，我们确实应该追求一个明确的“错误收口阶段”。

但这个问题必须先分清楚“什么叫源码错误”。

### 可以追求的保证

对于下面这类错误，我们应该要求它们在某个确定阶段之前全部检测完毕：

1. 语法错误。
2. package/import/可见性/声明头错误。
3. resolve 错误。
4. type/effect/override/layout/where-clause/overload/exhaustiveness 错误。
5. 同 cone 内 top-level 初始化依赖中，能够静态判定的非法结构。
6. （若未来恢复类似能力）comptime surface 的静态/执行错误。
7. build graph / consumer entry / export entry point 等与源码 build contract 直接相关的错误。
8. 固定导出/native symbol name 与 generic/monomorphization 语义冲突的声明错误，例如 `@CallingConvention` generic 函数。
9. global object/var/val 的 monomorphic 约束违反，例如任何会发布 global singleton/storage root 的 generic 声明。

对这类错误，理想设计应保证：

1. 一旦某个 cone-level compilation unit 通过了语义前端屏障，后续阶段就不再以“用户源码错误”的名义失败。

### 不能无条件保证的部分

下面这些不能被归入“必须在某个静态阶段完全穷尽”的范围：

1. link error。
2. FFI/`@Extern` 声明与真实外部实现不一致导致的问题。
3. `unsafe` 承诺被用户违反导致的问题。
4. 语言故意保留到运行时的失败，例如某些 dynamic cast / runtime error path。
5. 宿主工具链、平台环境、LLVM/clang/runtime 缺失导致的问题。

所以更准确的说法不是：

1. “所有广义上的源码相关失败都能在编译早期穷尽”。

而是：

1. “所有**静态可判定的 Scoop 语义错误**都必须在某个确定阶段之前收口”。

### 这个阶段应该是什么

在目标架构里，我们应当把 `AST -> HIR` 直接定义成：

1. **cone-level 语义前端屏障**。

这里的 `AST -> HIR` 不是狭义的“把 AST 改写成另一棵树”，而是一个完整的前端阶段族，其最终产物是：

1. `HirStageOutput`
2. `HirFacts`
3. 与该 cone 直接相关的 build-contract validation 已全部完成

也就是说，在目标设计里：

1. parse
2. cone/project validation
3. resolve
4. typecheck
5. （若未来恢复）comptime
6. HIR completeness / HIR facts publish

应被视为同一条大阶段链条的内部步骤，而它们整体的“出口”就是 HIR。

因此更直接的目标表述应该是：

1. **绝大部分，理想情况下是全部静态可判定的 Scoop 源码错误，都应在 `AST -> HIR` 阶段收口。**

后续的 MIR / effect facts / LIR / codegen 都不应再承担“发现新的普通源码语义错误”的职责。

### 为什么错误收口点不应晚于 HIR

如果某个错误要到 MIR/effect facts/LIR/codegen 才作为“用户源码错误”暴露，通常意味着两种情况之一：

1. 前面的 HIR/facts 不完整，漏掉了本应静态检查的语义。
2. 后面的阶段在替前面的阶段兜底，边界设计错误。

在目标设计里：

1. MIR stage 应只负责 HIR -> MIR 的结构化翻译与 MIR-owned facts 发布。
2. effect facts stage 应只负责对 canonical MIR 做 effect/control 分析。
3. LIR stage 应只负责 MIR/effect facts -> LIR 的构造。
4. codegen 应只负责 LIR -> backend artifact。

这些阶段都不应继续承担“检查普通用户源码是否合法”这一职责。

换句话说，后续阶段最多只能：

1. 验证上游输出是否完整一致。
2. 报告编译器内部不变量被破坏。
3. 处理环境、工具链、链接或运行时层面的失败。

### 后续阶段允许出现什么错误

通过语义前端屏障之后，后续阶段允许出现的失败只应是：

1. 编译器内部 bug。
2. 阶段 output/fact drift verifier 失败。
3. 环境/toolchain/runtime 集成错误。
4. link error。
5. 运行时语义错误对应的显式 runtime path。

其中前 2 类应被明确视为：

1. compiler bug / impossible state sentinel。

而不是用户源码错误。

### 这对当前实现意味着什么

当前代码里已经能看到这种目标方向的影子：

1. `HirCompletenessVerifier` 明确在 typed HIR handoff 边界上拒绝 placeholder HIR。
   见 `crates/scoopc/src/pipeline/hir_completeness.rs:10-30`。
2. `hir_preflight` 在用 fixture 检查“合法 typed HIR 输出之后，MIR 不该再依赖 HIR-origin fallback”。
   见 `crates/scoopc/src/pipeline/hir_preflight.rs:10-128,181-198`。
3. `pipeline_user_visible_failure_policy` 也在冻结一个方向：
   用户可见错误应尽量前移到 frontend reject，后续阶段只保留 impossible-state guard。
   见 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:1-6,120-132`。

但这件事当前还没有完全做实，因为：

1. 仍有一批用户可见错误直到后续阶段才暴露。
2. 仍有不少后续阶段在用“缺合同就 panic/unsupported”的方式兜底。
3. 仍有不少事实表不完整，导致后续阶段还要继续补语义。

### 设计结论

因此，这个问题的目标答案应当是：

1. **能**，而且目标上应尽量把范围推进到“全部静态可判定的 Scoop 源码错误”。
2. 这个收口点就定义为 **`AST -> HIR` 的 cone-level semantic frontend barrier**。
3. 一旦某个 cone 通过这个屏障，后续阶段出现的失败就不再应被当成普通源码错误。

## 对当前实现的含义

如果采纳上面的定义，那么当前实现至少有两个需要明确标记为“过渡态”的地方。

### 1. frontend flatten 整个 source-cone graph

当前实现其实已经同时具备两层结构：

1. `ProjectInput` 已显式持有 `SourceConeGraph`、扁平化后的 `sources`、`source_cone_ids`、
   `source_cone_infos` 等 graph-level metadata。
   见 `crates/scoopc/src/frontend.rs:33-57`。
2. 但 `run_frontend(...)` 仍会把 sysroot、local dependency、consumer 的源都放到一次流程里。
   见 `crates/scoopc/src/frontend.rs:535-565`。

这在目标设计里不应继续被解释为“一个 compilation unit”。

它更像：

1. 临时的 whole-build orchestration 方式。
2. 用 source flatten 来模拟 cone 间依赖闭包。

长期应收敛成：

1. 先按 cone DAG 排序。
2. 再逐个 cone 编译。

### 2. resolve/typecheck 接口仍偏文件级

当前很多 API 还是 `check_file_*` 风格，例如：

1. `resolve::check_file_headers`
2. `resolve::check_file_bodies`
3. `typecheck::check_file_*`

这并不一定错误，但它们应被理解成：

1. 在某个 cone-level stage 内，对单个文件执行的 worker。
2. 而不是“单文件就是编译单元”。

后续重构时应逐步引入更显式的：

1. `check_cone_headers`
2. `build_cone_index`
3. `typecheck_cone`
4. `lower_cone_to_hir`
5. `lower_cone_to_mir`

这些 cone-level orchestrator，再在内部调度 `check_file_*` worker。

## 设计结论

关于“编译单元”和“编译顺序”，最终应固定为下面这组规则：

1. 编译单元 = 一个 cone 的全部 Scoop 源文件。
2. 整个 project/source-cone graph = 多个 compilation unit 组成的 build DAG。
3. cone 之间按 DAG 拓扑顺序编译。
4. cone 内不定义语义性的文件编译顺序，只定义阶段屏障。
5. 文件顺序只用于稳定性，不用于语义。
6. top-level 初始化依赖必须通过 facts/IR contract 表达，不能依赖文件编译顺序。

这组规则一旦固定下来，后面的 crate 拆分、fact owner 收口和 backend 解耦才会有稳定基础。

## 核心设计原则

### 单向阶段图

阶段图必须是单向 DAG，而不是“前后都能看”的网状结构。

允许的关系只有：

1. `StageN` 依赖 `StageN-1` 的输出。
2. `StageN` 依赖 `Fact0..FactN-1`。

禁止的关系包括：

1. `StageN` 依赖 `StageN-2` 或更早的 stage crate。
2. `StageN` 依赖 `StageN+1` 或任何后续阶段。
3. `StageN` 在执行过程中重新调用上游阶段来“补齐输入”。

### 单一 authoritative 输出

每个阶段都必须有一个单一的 authoritative 输出对象。

这意味着：

1. `HirStageOutput` 负责发布 HIR。
2. `MirStageOutput` 负责发布 MIR。
3. `EffectFactsStageOutput` 负责发布 effect facts。
4. `LirStageOutput` 负责发布 LIR。

如果一个阶段的对外输出长成“我自己的产物 + 上一阶段整包输出”，那它通常不是完整输出，而是一个过渡包装器。

### fact 的职责必须独立

fact 不是“下游需要什么就再搬一份”的缓存。

每个阶段发布的 fact 必须同时满足：

1. 有独立语义。
2. 有明确 owner stage。
3. 可独立校验。
4. 不与其它 fact table 真正重叠。
5. 不依赖“另一个 fact 也在描述同一件事，所以我缺一点信息也没关系”。

如果下游觉得“这个 fact table 有时可以替代那个 fact table”，那通常只有两种可能：

1. 某个 fact table 信息不全。
2. 下游设计错误，消费边界不清。

### fact crate 必须自包含

fact crate 不是阶段实现的附属模块，而是阶段产出的独立数据产品。

所以它必须满足：

1. 不依赖任何 stage crate。
2. 不依赖任何其它 fact crate。
3. 只依赖基础 crate。
4. 只使用稳定 ID、基础类型和自身定义的数据结构表达信息。

换句话说，fact crate 自己就应该能被：

1. 独立 dump。
2. 独立 snapshot。
3. 独立 verifier 校验。

### 真正跨全阶段共享的通常不是 facts，而是 base context

在做 crate 划分时，一个很容易犯的错误是：

1. 看到某类信息“很多阶段都会用”，就把它归类成 global fact。

但目标设计里必须区分两类东西：

1. **base context**
2. **stage-owned facts**

真正从 AST 一直到 codegen 都可能被使用的，通常不是某个阶段产出的 fact，而是基础上下文。

#### base context 包括什么

真正允许跨全阶段共享的内容应尽量只包括：

1. source context
   - `SourceId`
   - 路径与规范化路径
   - source text / span 映射
   - compilation-unit 内文件身份
   - 稳定文件顺序

2. build / project context
   - `SourceConeGraph`
   - `source -> cone` 映射
   - consumer cone 身份
   - cone dependency DAG
   - source trust / cone kind

3. compiler / session config
   - 语言 profile / edition
   - feature 开关
   - 诊断策略
   - 全局编译参数（例如 opt level）

这些信息应进入基础 crate，而不是任何 stage fact crate。

#### 什么不应被视为“全阶段共享 fact”

下面这些即使很多后续阶段都会用，也不应被归入“全阶段共享”这一层：

1. typed call-site contract
2. assign/update place contract
3. perform/resume/handle contract
4. MIR materialization summary / escape / pass facts
5. effect step schema / continuation schema
6. LIR callable/frame/boundary/resume contract
7. backend ABI/layout/query

原因是：

1. 它们都有明确 owner stage。
2. 它们只在某个阶段之后才有定义。
3. 它们不应提前泄漏到更早阶段。
4. 它们也不应被后续阶段再提升成“通用基础事实”。

#### 判断规则

如果某个“事实”满足下面任一条件，它通常就不该被放进“全阶段共享”的类别：

1. 它描述的是语义解释结果，而不是基础身份或环境。
2. 它只在某个阶段之后才有意义。
3. 它的正确性依赖某个具体阶段的分析结果。
4. 它有明确 owner stage。

反过来说，如果某个 semantic fact 也变成“所有阶段都依赖”的东西，通常意味着：

1. 这个 fact 放错层了。
2. 某个阶段没有把语义真正吃掉。
3. 后续阶段还在回头补前端语义。

#### 设计结论

因此，这个设计里应明确承认：

1. **应该几乎不存在‘所有阶段都依赖的 semantic global fact’。**
2. 真正跨全阶段共享的，原则上只能是 base context。
3. stage facts 应该尽量少共享、晚发布、只向后流动。

### 分析阶段必须只读输入

如果某个阶段被定义为 analysis/facts stage，那么它对输入阶段产物必须是只读的。

这条规则的含义很直接：

1. 它可以读取上一阶段输出。
2. 它可以基于输入推导新的 facts。
3. 它可以发布“经过该分析解释后的新输出”。
4. 但它不能反向修改输入阶段的输出对象本身。

否则这个阶段就不再是纯分析，而是在做 transformation 或 augmentation。

这条规则尤其重要，因为“分析阶段偷偷改输入”会立刻引入两类歧义：

1. 下游拿到的到底还是不是原来的 `MirStageOutput`。
2. 某些信息到底属于 MIR 自身，还是属于 effect analysis 的派生产物。

对本次 pipeline 来说，合理的形状只有两种：

1. `effect facts` 是纯分析阶段：
   `input = &MirStageOutput`，`output = EffectFactsStageOutput`。
2. 如果它确实需要在 MIR 之上增加新的分析解释层，那么它发布的也必须是新的输出产品，
   例如 `EffectAnnotatedMirOutput` 之类，而不是修改 `MirStageOutput` 本身。

换句话说：

1. “经过 effect 分析的 MIR”可以存在。
2. 但它必须是另外一组输出，而不是 MIR 输出被原地改写后的别名。

### LIR 是 codegen 的唯一 IR 输入

一旦 `effect_lowered` 被正式定义为 LIR，后端就不该再直接依赖：

1. HIR body。
2. raw MIR body。
3. HIR side table。
4. effect-facts 的原始 schema 构造逻辑。

后端如果还需要这些信息，只能说明：

1. LIR 本身缺少必要 contract。
2. LIR facts 没有发布完整。

### HIR 阶段可以使用内部 CFG，但这不意味着需要 MIR 阶段参与

有一类问题很容易把“需要 CFG”与“需要 MIR”混淆在一起，例如：

1. 函数是否在所有路径上满足返回契约。
2. `break` / `continue` / `return` / `raise` 混合出现时的正常完成性分析。
3. smart cast 这类 flow-sensitive typing。

这里必须明确区分两件事：

1. **分析需要 CFG 式信息**。
2. **分析必须依赖 MIR 阶段**。

这两件事不是同一件事。

目标设计里，我们应坚持下面的规则：

1. 所有静态可判定的源码语义错误仍然要在 `AST -> HIR` 屏障内收口。
2. 如果某类检查需要 CFG，那么 HIR 阶段可以在内部构建自己的 **semantic CFG**、control-flow summary 或 dataflow graph。
3. 这个 CFG 属于 HIR 阶段的内部实现，不属于 MIR 阶段，也不等于发布 MIR。

#### 为什么这类检查不应推迟到 MIR

以函数返回完整性为例：

```scoop
while (...) {
    if (...) {
        return ...
    } else {
        break
    }
}
// 这里可能没有 return
```

这个例子确实涉及复杂控制流，但它本质上仍然是**源码语义检查**，不是 MIR lowering 之后才拥有的语义。

HIR 阶段只要拥有足够的控制流模型，就可以得出：

1. `while` 可能一次都不执行。
2. `else { break }` 会导致循环正常退出并继续执行后继语句。
3. 因此函数体存在“正常落空而未返回值”的路径。

如果函数声明要求返回 `Int`，那这应当被视为 HIR 阶段错误，而不是 MIR 阶段错误。

同理，smart cast 需要的也是控制流和绑定稳定性信息，而不是 MIR 作为阶段边界产物。

#### 推荐的实现策略

HIR 阶段可以有两种实现路线，二者都不要求 MIR 参与：

1. **结构化完成性分析**
   对 `if` / `when` / `while` / `for` / `break` / `continue` / `return` / `raise` 等结构化语法，
   递归计算 completion summary、fallthrough、branch merge 和 flow facts。

2. **HIR-owned semantic CFG**
   在 HIR stage 内部，把结构化 HIR/AST 临时投影成一个只服务语义分析的 CFG，
   用它做：
   - 返回完整性检查
   - dead/unreachable path 判断
   - smart cast / narrowing dataflow
   - 绑定稳定性与失效传播

第二种方式在复杂控制流上通常更稳，但它仍然属于 HIR 阶段的内部实现。

#### 设计要求

如果选择在 HIR 阶段内部使用 semantic CFG，应遵守：

1. 该 CFG 不是 MIR crate 的类型。
2. HIR stage 不依赖 MIR crate。
3. 该 CFG 不作为“已发布 MIR”对外暴露。
4. 对外发布的仍然是 `HIR + HIR facts`。

也就是说：

1. **可以在 HIR 阶段内部有 CFG。**
2. **但不能因此把错误收口点推迟到 MIR 阶段。**

## crate 划分

下面用 `scoopc_*` 作为目标名字示意，重点是角色和依赖方向，而不是必须原样采用这些 crate 名。

### 基础 crate

这些 crate 不属于任何阶段，也不属于任何 fact，它们是整个 pipeline 的公共底座。

建议最少有：

1. `scoopc_span`
   负责 `Span`、位置信息、诊断坐标等基础类型。

2. `scoopc_source`
   负责 `SourceId`、`SourceMap`、source identity、编译单元内文件身份。

3. `scoopc_types`
   负责 `TypeId`、`TypeStore`、`EffectRow`、builtin type universe 和通用 type-level 数据结构。

4. `scoopc_ids`
   负责 `SiteId`、`BodyVersionKey`、stable hash/key primitives，以及后续可提升为基础层的 stage-independent callable/body identity。当前 MIR materialization 的 `TemplateKey` / `InstanceKey` 仍是 stage-owned internal key，不能被 fact crate 当作基础 ID 直接依赖。

5. `scoopc_project_model`
   负责 source-cone、project membership、source trust、compilation-unit membership 等工程模型。

这层的意义是：

1. stage crate 和 fact crate 都必须通过基础 crate 间接共享身份和类型。
2. 不允许 fact crate 直接依赖 stage crate 中定义的业务类型。

### stage crate

目标上的阶段 crate 可以拆成：

1. `scoopc_ast`
2. `scoopc_hir`
3. `scoopc_mir`
4. `scoopc_effect_facts_stage`
5. `scoopc_lir`
6. `scoopc_codegen_llvm`
7. `scoopc_codegen_c`

这里把 `effect facts` 分成两个层次：

1. `scoopc_effect_facts_stage` 是“如何从 MIR 计算 effect facts”的实现。
2. `scoopc_effect_facts` 是“effect facts 这个数据产品本身”。

这样可以避免把“算法”与“产物”混在一起。

### fact crate

目标上的 fact crate 可以拆成：

1. `scoopc_ast_facts`
2. `scoopc_hir_facts`
3. `scoopc_mir_facts`
4. `scoopc_effect_facts`
5. `scoopc_lir_facts`

注意：

1. 不是每个阶段都必须有很大的 fact crate。
2. 如果 AST 没有真正的 global facts，`scoopc_ast_facts` 可以极小，甚至不存在。
3. 但一旦某阶段需要发布全局事实，它就应进入单独 fact crate，而不是继续塞回 stage 实现 crate 或下一阶段 crate。

### facade crate

最终还需要一个薄的 facade/orchestrator crate，例如保留 `scoopc`。

它负责：

1. 串联阶段。
2. 暴露用户入口。
3. 配置 feature gate。
4. 协调 backend 选择。

它不应该再承载：

1. 某个阶段的核心实现。
2. 某个 fact 的结构定义。
3. backend-specific 的内部逻辑。

## 依赖规则

### stage crate 允许依赖什么

阶段 crate `StageN` 只允许依赖：

1. 基础 crate。
2. 它自己的 fact crate `FactN`。
3. 前一个阶段 crate `StageN-1`。
4. 所有更早阶段发布的 fact crate `Fact0..FactN-1`。

举例：

1. `scoopc_hir` 可以依赖 `scoopc_ast` 和可选的 `scoopc_ast_facts`。
2. `scoopc_mir` 可以依赖 `scoopc_hir`、`scoopc_hir_facts`、可选的 `scoopc_ast_facts`。
3. `scoopc_lir` 可以依赖 `scoopc_mir`、`scoopc_mir_facts`、`scoopc_effect_facts`、`scoopc_hir_facts`。
4. `scoopc_codegen_llvm` 原则上只应依赖 `scoopc_lir`、`scoopc_lir_facts` 和基础 crate。

### stage crate 禁止依赖什么

阶段 crate 明确禁止依赖：

1. 更早但非直接前驱的 stage crate。
2. 任何后续 stage crate。
3. 任何后续 fact crate。

例如：

1. `scoopc_lir` 不能依赖 `scoopc_hir`。
2. `scoopc_effect_facts_stage` 不能依赖 `scoopc_lir`。
3. `scoopc_codegen_llvm` 不能依赖 `scoopc_mir`，除非我们承认 LIR 尚未完成，并把这视为临时债务而不是合法设计。

### fact crate 允许依赖什么

fact crate `FactN` 只允许依赖基础 crate。

它可以使用：

1. `TypeId`
2. `EffectRow`
3. `SiteId`
4. stage-independent callable/body identity（例如后续正式提升的 stable instance/callable key；不是当前 MIR materialization 内部 `InstanceKey`）
5. `SourceId`

它不能使用：

1. 某个 stage crate 里定义的 AST/HIR/MIR/LIR node 类型。
2. 另一个 fact crate 里的事实结构。

### fact crate 禁止做什么

fact crate 明确禁止：

1. re-export 另一个 fact crate 的数据。
2. 把 stage output 原封不动嵌进去当字段。
3. 用“有一个外部 helper 才能解释我”的方式发布半成品事实。

如果某个 fact 需要另一个 fact 才能成立，那它们大概率不是两个独立 fact，而是职责划分错了。

## stage output wrapper 规则

每个阶段对外暴露的 `StageOutput` 只应该包装本阶段自己的产物。

推荐形状：

1. `AstStageOutput = { ast }`
2. `HirStageOutput = { hir, hir_facts }`
3. `MirStageOutput = { mir, mir_facts }`
4. `EffectFactsStageOutput = { effect_facts }`
5. `LirStageOutput = { lir, lir_facts }`

禁止形状：

1. `CurrentStageOutput { previous_stage_output, my_output }`
2. `CurrentStageOutput` 继续暴露 `previous_stage_output()` 这类上游整包访问器。
3. `CurrentStageOutput` 为了方便下游，重新导出上游 facts。

如果下游确实需要上游某类事实，那么正确做法不是 nested bundle，而是：

1. 把那类事实归属到正确的 fact crate。
2. 让下游显式依赖那个 fact crate。

## 优化框架

本轮除了要清理阶段边界，还需要顺手把“优化 pass 应该属于哪一层”收口清楚。

这一步很重要，因为当前优化逻辑分散在：

1. 旧 comptime / const 语义
2. HIR lowering 里的去虚化开关
3. MIR materialization 尾部顺手跑的 pass
4. LIR 的窄后处理
5. codegen 里的局部去虚化和 LLVM 自己的 target pass

如果不先把这些层次讲清楚，后续很容易继续在 HIR/MIR/codegen 各自加一点“优化性特殊处理”，又回到今天的混合状态。

### 优化 pass 的分层原则

目标设计里，优化 pass 应抽象成三层，而不是只围绕当前已有的几项实现：

1. **语义规范化（semantic normalization）**
2. **IR 级优化（IR optimization）**
3. **后端优化（backend optimization）**

它们的边界如下：

#### 1. 语义规范化

这类步骤不应被当成 optimization pass，而应被视为阶段本身的职责。

例如：

1. 语义 desugar
2. 类型/效果合同显式化
3. smart cast / flow-sensitive typing 结果固化
4. 控制流完成性检查
5. global init root 分类与 storage policy 决议

这类工作属于 `AST -> HIR` 屏障的一部分，不应下沉到 MIR/LIR/codegen。

#### 2. IR 级优化

IR 级优化是 backend-neutral 的优化，应明确挂在某个 IR 层之下。

在当前目标里：

1. `MIR` 承载普通调用图/控制流/实例级优化。
2. `LIR` 承载 effect/control 相关的窄优化。

换句话说：

1. 若某个 pass 需要理解普通调用、虚调用、实例化、逃逸、闭包等通用语义，它应该属于 MIR。
2. 若某个 pass 需要理解 state graph、boundary map、resume packing、frame schema 等 effect/control 语义，它应该属于 LIR。

#### 3. 后端优化

后端优化是 backend-specific 的优化。

例如 LLVM 里：

1. `sroa`
2. `instcombine`
3. `simplifycfg`
4. `rewrite-statepoints-for-gc`

这些属于目标后端的物理 IR/target 优化，不应和 MIR/LIR 层的语义优化混在一起。

### 本轮对现有优化逻辑的建议

#### A. 旧 comptime / const 相关 constant folding

这部分不应作为本轮优化框架的一部分保留。

原因是它现在不只是“常量折叠”，而是一整套跨阶段 comptime 语义：

1. package-level `comptime if` 裁剪出现在 frontend/sysroot/hir/materialize/effect_facts 等多处。
2. runtime comptime plan 进入了 HIR lowering。
3. codegen 里还有 `top_level_const_eval_stack`、`codegen_top_level_const_ref`、`const_eval_*` 一类路径。

这说明它不是一个干净的 HIR optimization pass，而是一套跨阶段语言机制。

在本轮设计下：

1. 现有 comptime 与 `const` 已整体退出范围。
2. 因此与它们绑定的 constant folding 路径一起删除。
3. 不为它们保留新的“优化层落点”。

需要注意的一点是：

1. 若未来仍需要把某些 `@Global var` 的纯 scalar 初值直接发成 `.data/.bss`，那应被视为后端数据初始化 helper，
   而不是 comptime optimization 的残留形式。

#### B. HIR 中的 dispatch 去虚化

当前 HIR lowering 里存在 `devirtualize_dispatch_calls` 开关，并会在 lowering 时尝试把 virtual/interface call 直接改成 direct call。

这在目标设计里应删除。

原因：

1. HIR 应表达源码语义，而不是实例级优化结果。
2. 去虚化依赖 receiver 精确类型、subclass facts、vtable/itable facts、materialized direct target identity，
   这些都已经明显属于 MIR 之后的世界。
3. 让 HIR 做这件事，只会把去虚化逻辑和 HIR 语义混在一起。

所以：

1. HIR 一律保留动态 dispatch 语义与 dispatch contract。
2. 去虚化统一迁移到显式 MIR pass。

#### C. MIR inline

当前 `mir/inline.rs` 已经是一个真正的 MIR pass：summary-driven、保守、基于 canonical pass artifacts 改写 body。

这项能力应保留，但要从“materialization 尾部顺手跑一下”改成正式的 MIR pass pipeline 一步。

也就是说：

1. 它应该有清晰的输入：canonical materialized MIR + MIR facts/pass artifacts。
2. 它应该有清晰的输出：rewritten canonical materialized MIR / pass artifacts。
3. 它不应只是 `MirInstanceMaterializer::run()` 尾部的一个布尔开关。

#### D. MIR devirtualization

当前 MIR 去虚化主要内嵌在 `mir/materialize/rewrite.rs` 里：重写 `CallKind::Virtual` / `Interface` 时直接尝试变成 `CallKind::Direct`。

这项能力也应保留，但要抽成正式 MIR pass。

这样做的好处是：

1. 它有独立 owner。
2. 它的输入/输出更清楚。
3. HIR 和 codegen 里的重复去虚化逻辑都可以删掉。

#### E. MIR escape analysis 与 closure simplification

这两项应该拆成：

1. `escape analysis`：MIR analysis / MIR facts
2. `closure simplification`：MIR optimization

原因：

1. 逃逸信息不只是优化提示，后续阶段也在消费它。
2. 所以 escape analysis 不应只是 “O1+ 才有的可选优化附属品”。
3. closure simplification 则显然是基于这些 facts 做的优化改写。

目标上建议：

1. escape analysis 改成 always-on 的 MIR analysis/facts。
2. closure simplification 继续作为 MIR opt，可按 opt level gate。
3. 若简化后影响 facts，需要明确 refresh 顺序，而不是在 materializer 尾部临时再跑一遍。

#### F. LIR opt

当前 `effect_lowered::opt` 的方向基本是对的，应保留。

但它需要重新定位：

1. 它不是“effect lowering 附属优化”。
2. 它应被正式定义为 `LIR opt`。

它应只做 LIR-owned 的窄优化，例如：

1. wrapper state folding
2. internal resume interface 去虚化
3. 消除掉能被局部消费的 state machine
4. 对简单 higher-order wrapper 做定向 inline/devirt，尤其是那些只是简单调用输入的 effectful function/closure 的包装函数
5. dead state / dead slot cleanup

不应再做：

1. 普通调用图层面的去虚化
2. 面向全程序的一般意义上的 inlining
3. 前端语义补全

这里要特别强调：

1. LIR 层**可以**保留一轮 post-state-machine 的定向 inline/devirt。
2. 但这轮优化的目标必须非常窄：它服务于 state-machine elimination 和简单 higher-order wrapper 消解，
   而不是把 LIR 重新变成一个通用调用图优化层。

#### G. codegen 里的去虚化和“优化性选择”

当前 codegen 层仍残留一些不该保留的语义优化逻辑：

1. `llvm/codegen/call/lowering.rs` 里接口调用前的去虚化 fast path。
2. `llvm/reachability.rs` 里 reachability 收集阶段的去虚化推断。

这两类逻辑在目标设计里应删除。

原因：

1. 去虚化只能有一个 authoritative owner。
2. 如果它已经在 MIR 完成，就不该再在 codegen 重做。
3. 如果 MIR 没完成，codegen 也不应自己兜底补做。

codegen 层应保留的“优化”只包括 backend-specific 的物理层优化，例如 LLVM 自己的 pass pipeline。

### 推荐的优化流水线

在目标设计里，可以把“优化 pass”抽象成下面的分层流水线，而不只局限于当前已有的几项：

1. `HIR semantic normalization`
   - 这不是 optimization pass，而是前端语义规范化。

2. `MIR analysis`
   - 例如 escape analysis、call graph facts、summary refresh 等。

3. `MIR optimization`
   - 例如 devirtualization、inlining、closure simplification，以及未来可能新增的 DCE、SCCP、普通 CFG cleanup 等。

4. `effect/control analysis`
   - 当前对应 effect facts stage。

5. `LIR construction`
   - MIR/effect facts -> LIR。

6. `LIR optimization`
   - 针对 state graph / frame / resume / wrapper 的窄优化；它应被理解为一个可迭代的 pass family，而不是单个 pass。

7. `backend optimization`
   - LLVM 或未来其它后端自己的物理 IR/target 优化。

这样做的意义是：

1. 后续要新增优化 pass 时，只需要先决定它属于哪一层。
2. 不再需要每次都临时讨论“这个 pass 是不是该塞进 lowering/codegen 里”。
3. 同一层内部可以随着需要扩展为多个 pass 的固定点迭代，而不破坏整体阶段边界。

### 对现有实现的处理结论

如果把上面的原则映射到当前代码，建议是：

1. **删除**
   - 现有 comptime/const 相关语义路径
   - HIR 层 `devirtualize_dispatch_calls`
   - codegen 层 dispatch 去虚化
   - reachability 层临时去虚化推断

2. **保留但迁移/重定位**
   - `mir/inline.rs`：保留，改成正式 MIR pass
   - `devirtualize.rs`：保留算法，迁到显式 MIR pass 语境
   - `effect_lowered::opt`：保留，改定位为 `LIR opt`

3. **重新分类**
   - `mir/escape.rs`：从“可选优化”偏向 `MIR analysis / MIR facts`
   - `llvm/codegen/main/const_eval.rs`：若未来仍保留极窄的全局静态数据初始化 helper，应改名并视为 backend data initializer helper，而不是 optimization pass

## 各阶段应发布什么

下面的“应发布的 facts”只讨论每个阶段自己拥有的职责，不包含把上游整包暴露给下游的临时包装。

### AST stage

#### 应发布的 IR/output

AST stage 应发布：

1. 解析完成的 AST compilation unit。
2. 每个 AST 文件的稳定 source identity、文件顺序和 compilation-unit membership。

这里的重点是“compilation unit”，而不是单文件 worker 包装。P1 已保留 `AstStageOutput` 作为 dump/helper worker，并新增 `AstCompilationUnitOutput` 作为 cone-level AST handoff；后续 P2 应在这个 handoff 上建立 HIR barrier 与 facts。

#### 应发布的 facts

AST stage 原则上不需要复杂的全局 semantic facts。

如果为了 project/multi-file pipeline 需要 facts，它们也只能是 parser-owned 的轻量 facts，例如：

1. package/import surface
2. 顶层声明 stub
3. 源文件所属 source-cone / project membership

这些 facts 的共同特征是：

1. 不依赖 typecheck。
2. 不依赖 name resolution 后的信息。
3. 只描述 parser 能确定的 header surface。

#### P1 后仍需补齐什么

当前最明显的问题是：

1. `AstCompilationUnitOutput` 已能表达 cone-level AST handoff，但它还没有发布独立 parser-owned facts。
2. production frontend 仍让 resolver/typecheck/HIR 过渡路径消费 build closure；P2 需要把 `AST -> HIR` 收口成正式 cone-level semantic barrier。

### HIR stage

#### 应发布的 IR/output

HIR stage 应发布：

1. resolved + typechecked 的 HIR compilation unit。
2. 与该 compilation unit 对齐的统一 `TypeStore` / builtins context。

HIR 的职责是把 AST surface 解释成 typed program，而不是顺手替 MIR 做 instance collection、materialization 或后端准备工作。

#### 应发布的 facts

HIR stage 应发布两类且互不重叠的 facts。

第一类是声明/实体 facts：

1. nominal kind/variance/direct supertypes
2. struct/enum/class/object/interface metadata
3. extern/native declaration metadata（包括 `@Extern` 与有 body 的 `@CallingConvention`）
4. top-level value metadata
5. source path -> owning cone metadata
6. top-level init root classification、storage policy 与 dependency roots

第二类是 source-site typed contracts：

1. direct/member/extension/virtual/interface/constructor call-site contracts
2. closure/fun value/funptr/intrinsic call-site contracts
3. perform/resume/handle typed contracts
4. arg binding、assign/update place contracts
5. extern global root/init root contracts

除此之外，HIR 阶段还应承担一类很关键的**声明合法性约束**：

1. 任何会发布**固定 object-level/export symbol name** 的函数，都不能是 generic 函数。

`@CallingConvention` 就是最典型的例子，因为它对应固定的 native/export symbol name。
而 generic 函数在 monomorphization 后会产生多个实例；如果多个实例共享同一个固定 symbol name，
就会出现 object-level 名字冲突。

因此，这类约束必须被视为 `AST -> HIR` 屏障内的前端语义错误，而不是：

1. MIR/materialization 之后的实例冲突问题。
2. codegen 命名细节。
3. link-time 才暴露的错误。

更抽象地说，HIR 阶段需要对“声明是否允许单态化出多个实例”和“声明是否要求单一固定导出符号名”这两类属性做一致性检查；它们不能同时成立。

如果下游分析仍需要 HIR 级共享事实，那它也应该由 HIR stage 正式发布一套不与上面重复的 `hir_facts`，而不是由更后面的阶段从 `LoweredHir` 现场再组装一次。

#### 当前完整输出还缺什么

当前 HIR stage 最缺的是统一、非重叠的 facts 输出面。

现在的事实被拆散在：

1. `LoweredHir` side table
2. `TypedHirEffectContracts`
3. 后续重建的 `ProgramFacts`

这意味着同一类职责被多处重复发布和复制，后续阶段自然会混用。

具体证据：

1. `crates/scoopc/src/hir/lower/types.rs:360-409`
2. `crates/scoopc/src/pipeline/hir_stage.rs:1233-1255,1301-1313`
3. `crates/scoopc/src/program_facts.rs:18-31,39-147`

另外，当前 `LoweredHir` 已经开始显式发布一部分本应进入稳定 HIR handoff 的全局上下文，例如：

1. `source_cones`
2. `native_callable_funs`

见 `crates/scoopc/src/hir/lower/types.rs:322-350`。

这说明当前代码已经在向“HIR 必须发布更完整的 cone/decl/global metadata”这个方向移动，但它们还没有被系统化收口成独立 `hir_facts`。

另外，`TypedHirStageOutput` 仍以单个 `source_path` 为锚，而不是显式的 compilation-unit context。
见 `crates/scoopc/src/pipeline/hir_stage.rs:1234-1238,1273-1275`。

最后，HIR stage 输出不应再携带 MIR 产物；当前 `LoweredHir` 仍挂着 `materialized_mir`。
见 `crates/scoopc/src/hir/lower/types.rs:329-337,421-467`。

### MIR stage

#### 应发布的 IR/output

MIR stage 应发布：

1. generic direct-style MIR compilation unit
2. canonical materialized MIR snapshot
3. canonical MIR pass view / pass artifacts query surface

这三者都属于 MIR stage 自己的输出，而不是“额外挂在 HIR 上给后续偷用的补充品”。

#### 应发布的 facts

MIR stage 应发布 MIR-owned facts，而不是继续转发 HIR typed contracts。

最重要的 MIR facts 至少包括：

1. callable body inventory
2. initializer root inventory
3. extern/global root inventory
4. metadata root inventory
5. materialization snapshot binding
6. instance family inventory
7. summary/escape/pass artifacts
8. 一切供后续阶段继续使用的 site metadata/query surface

如果某些 LIR stage 必定要用到、且能从 MIR compilation unit 稳定导出的全局信息，也应由 MIR stage 直接发布，例如 nominal direct supertype index 这类 MIR-derived facts。

#### 当前完整输出还缺什么

当前 MIR stage 最缺的是单一 authoritative MIR handoff。

现在的 `MirStageOutput` 仍混有：

1. `LoweredMir`
2. `Option<MaterializedMir>`
3. `TypedHirEffectContracts`

见 `crates/scoopc/src/pipeline/mir_stage.rs:13-27,29-37,68-84`。

这意味着 MIR stage 输出还不是“只发布 MIR 和 MIR facts”，而是在继续把 HIR facts 带给后面。

此外，当前还缺少正式的 stage-owned `materialized_pass_view()` / snapshot-binding facts crate；下游仍需要从 `MaterializedMir` 或更后阶段包装里回拿。

还有一类问题是：有些 MIR-derived facts 仍未作为 MIR stage 输出发布，而是在 LIR stage 里重算。
见 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:85-97,119-131`。

### effect facts stage

#### 应发布的 IR/output

effect facts stage 应发布一个绑定到 canonical MIR snapshot 的 effect-facts 产品。

它不是新的 IR，但它是一个正式的数据产品，拥有独立 crate 和独立 verifier。

如果未来发现这一步不仅需要 facts，还需要“带有 effect 分析解释层的 MIR 产品”，那么这也应被明确定义为
一个新的 stage output，例如：

1. `EffectFactsStageOutput = { effect_facts }`
2. 或 `EffectAnalyzedMirOutput = { mir_snapshot_id, effect_facts, analysis_owned_context }`

但无论采用哪种形式，都不应通过修改 `MirStageOutput` 本体来达成。

#### 应发布的 facts

effect facts stage 应发布：

1. `snapshot_binding`
2. `callable_facts`
3. `bodies`
4. `step_schemas`
5. `continuation_schemas`

这些 facts 的职责边界应该非常明确：

1. 它们解释 effect/control 语义。
2. 它们不重新承载 MIR structure。
3. 它们不提前承载后端物理 ABI。

#### 当前完整输出还缺什么

当前最缺的是一个窄的、stage-owned 的 MIR snapshot reference/context。

现在 `EffectFactsStageOutput` 是通过嵌套整份 `MirStageOutput` 来满足下游需要，见 `crates/scoopc/src/pipeline/effect_facts_stage.rs:22-59`。

这意味着：

1. effect facts stage 自己的输出还不够自足。
2. 下一阶段很容易继续把它当成“访问 MIR 的入口”，而不是只把它当 effect-facts 产品。

另外，如果下一阶段 LIR 还需要某些 MIR-derived 但非 effect-owned 的 global facts，这些 facts 的 owner 目前没有被明确下来，结果就会在 LIR stage 里现场重算。

当前还有一个更具体的问题：现有实现并不是严格的只读分析。

1. `MaterializedEffectFactsBuilder` 直接持有 `&mut MaterializedMir`。
   见 `crates/scoopc/src/effect_facts/builder.rs:35-40,2030-2040`。
2. builder 会对 `materialized.types` 做 intern/派生型变更，例如：
   `find_or_intern_raise_runtime_error_effect(&mut self.materialized.types)`，以及后续
   `schema_pool.intern_callable_step_schema(types, seed)` 等。
   见 `crates/scoopc/src/effect_facts/builder.rs:2053-2067,2087-2093`。
3. `collect_callable_seeds(...)` 也会继续通过 `&mut materialized.types` 收集和派生 effect 相关类型信息。
   见 `crates/scoopc/src/effect_facts/builder.rs:2625-2664`。

这说明当前 `effect facts` stage 还没有完全达到“纯分析阶段”的目标。

在目标设计下，这部分应收敛为两种选择之一：

1. 要么把这些新增/派生的 type context 明确定义为 `effect facts` 输出的一部分，而不是写回 MIR。
2. 要么承认这一步已经不是纯 facts stage，而是一个会生产新阶段产物的 transformation stage，并据此改名和改 output 形状。

### LIR stage（当前即 `effect_lowered`）

#### 应发布的 IR/output

LIR stage 应发布：

1. formal LIR program
2. 与该 LIR program 严格配套的 stage-owned context

这里的 context 不是上游 stage output 的嵌套包装，而是 LIR 自己定义的配套事实层。

#### 应发布的 facts

LIR facts 至少应包含：

1. callable inventory 与 body version identity
2. plain callable facts：ordinary ABI、source slices、本地 effect/control contract
3. effect-step callable facts：step schema、state graph、frame schema、boundary map、resume state map、source statement classifications、dynamic invoke entry
4. continuation/resume facts：continuation object、surface resume publication、resume packing inventory
5. global initialization facts：per-cone init routine contract、final system entry init order contract、object once contract、top-level eager init contract
6. backend-neutral query surface：callable surface signature、source-slice lowering contract、dynamic call carrier contract、dispatch owner/slot selection结果
7. 若 LIR 使用到上游 `TypeStore`，则这套 type context 也应成为 LIR output 的显式组成部分

这里最重要的一点是：LIR facts 不是 LLVM ABI query。它们应该是 backend-neutral 的；LLVM/C backend 再各自把它们映射到自己的物理 ABI。

#### 当前完整输出还缺什么

当前 LIR stage 最缺的是自足的、stage-owned output。

现在的 `EffectLoweredStageOutput` 本质上仍是 `EffectFactsStageOutput + LateLoweredProgram` 的包装，见 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:30-79`。

所以后续 codegen 仍然需要直接回看上游对象，最典型的例子有：

1. plain callable codegen 仍需要回读 materialized MIR signature/body，说明 LIR 没有把普通 callable surface 发布完整。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/callable.rs:333-345,418-447` 与 `crates/scoopc/src/llvm/codegen/effect_lowered/body/emitter.rs:13-18`。
2. dynamic-invoke / dispatch contract 仍需要扫描 MIR body 和 HIR declaration tables，说明这些 backend-neutral 合同尚未正式进入 LIR output。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/lookup.rs:56-225`。
3. codegen-neutral ABI/query facts 目前仍缺位；现在真正存在的是 LLVM-specific 的 `ProgramAbiQuery`，而不是所有 backend 都能复用的 LIR-to-backend contract layer。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/mod.rs:74-85` 与 `crates/scoopc/src/llvm/codegen/effect_lowered/types.rs:2221-2254`。

## 重构清单

下面的清单不是“想到什么改什么”，而是按依赖方向整理过的顺序。

### 前置阶段：移除现有 comptime

1. 从正式 pipeline 中移除当前 comptime 执行/展开路径。
2. 直接删除现有 comptime surface 本身，以及 parser / resolve / typecheck / lowering / codegen 中所有与之对应的专门分支与元数据；这里应包括现有 `const` 等 comptime 相关语言点。
3. 删除那些仅为了兼容现有 comptime 而存在的跨阶段特判、占位节点和绕路接口。
4. 将“未来是否恢复类似能力”明确延期到后续独立设计文档，而不是在本轮 refactor 中顺手兼容。

这一步的目标不是语言最终形态，而是先把 pipeline 的边界条件清空。只要旧 comptime 还在主线上，本轮 crate DAG 和 stage output 设计就很难真正收口。

### 第一阶段：先把物理层次固定下来

1. 定义基础 crate 边界。
2. 明确 stage crate 和 fact crate 的最终角色。
3. 在 workspace 里预留这些 crate 的位置，即使一开始只有空壳或 re-export 也可以。

这一步的目标不是立刻搬代码，而是先让后续重构有一个明确的落点，不再把所有实现继续堆回 `scoopc` 一个 crate 里。

### 第二阶段：让现有输出停止跨阶段嵌套

1. `LoweredHir` 去掉 `materialized_mir`。
2. `MirStageOutput` 去掉 `TypedHirEffectContracts`。
3. `EffectFactsStageOutput` 去掉对整份 `MirStageOutput` 的长期嵌套。
4. `EffectLoweredStageOutput` 去掉对整份 `EffectFactsStageOutput` 的长期嵌套。

这一步的目标是先让 stage output 长成正确的“外形”。即使内部还有旧实现，也不能继续把上一阶段整包暴露给下游。

### 第三阶段：收口 facts 的 owner

1. 把当前重叠在 `LoweredHir` / `TypedHirEffectContracts` / `ProgramFacts` 中的事实重新分配 owner。
2. 把当前 `MirLoweringFacts` 里的 typed/fallback 双轨合同合并成单一 authoritative 输入。
3. 把当前在 LIR stage 里重算的 MIR-derived global facts 挪回 MIR stage 或其 fact crate。

这一步的目标是让“每一类事实只在一个地方定义和验证”。

### 第四阶段：补齐 LIR

1. 让 plain callable surface 不再依赖 materialized MIR 回看。
2. 让 dynamic-invoke / dispatch contract 不再依赖 MIR/HIR 扫描。
3. 发布 backend-neutral 的 `lir_facts` / `lir_query` 层。

只有这一步做完，后端才能真正只依赖 LIR。

### 第五阶段：拆后端

1. `scoopc_codegen_llvm` 只消费 `scoopc_lir` + `scoopc_lir_facts`。
2. 新增 `scoopc_codegen_c` 时也遵守同一接口。

如果 LLVM backend 仍需要回头依赖 MIR/HIR，那么新增 C backend 只会复制一份当前的耦合，而不是复用同一条中间层。

## 验收标准

当下面这些条件都满足时，才能认为 pipeline refactor 达到阶段性完成：

1. 每个 stage crate 的依赖只指向前一阶段 crate、基础 crate、以及更早阶段的 fact crate。
2. 没有任何 fact crate 依赖其它 fact crate 或 stage crate。
3. 没有任何 `StageOutput` 嵌套上一阶段的完整输出。
4. 没有任何下游逻辑把两个 fact table 当成可替代输入。
5. codegen backend 只依赖 `LIR + LIR facts + 基础 crate`。
6. LLVM backend 和未来 C backend 共享同一套 LIR 输入边界。
7. 正式 pipeline 中不再保留现有 comptime 路径，也不保留现有 comptime surface；代码库中不再存在任何专门识别、携带或拒绝旧 comptime surface 的逻辑。

## 当前状态总结

当前实现离这个目标还有明显距离，但方向已经可以确定：

1. `effect_lowered` 继续被视为正式 LIR。
2. 接下来优先做的不是 LLVM 文件切分，而是阶段输出、fact owner 和 crate DAG 的收口。
3. 只要 stage output 还在嵌套上游整包、fact table 还在重叠发布、后端还在回看 HIR/MIR，这轮重构就还没有到可以稳定扩展 backend 的程度。
