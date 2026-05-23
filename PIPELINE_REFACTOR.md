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

### per-cone build artifact

把 cone 当作编译单元的真正含义，是它必须能以单元为粒度被「编译完一次、产物落地、下游复用」。在不强行复活 `.cone` archive 的前提下，正确的工程化形态应是：

1. 每个 cone 在 `build/<profile>/cones/<cone-name>@<version>/` 下输出一份本 cone 自己的产物。
2. 产物的内容必须能让下游 cone 完成它本来要从上游源码里推出来的所有事情：HIR 屏障的声明事实、MIR 屏障的实例事实、effect/control 屏障的合同、LIR 屏障的 callable/dispatch/init contract、本 cone emit 的 `.o` 与 native obj。
3. 下游 cone 通过反序列化 fact 注入自己的 `Index` / `TypeEnv`，而不再 parse 上游源文件；只有在跨 cone 单态化跨过 generic 边界时，才需要回到上游 generic 的 HIR/MIR 模板（这是后续 milestone 的范围，本轮接受下游 re-lower generic 的成本）。
4. 产物之间通过 `inputs.fingerprint` / `outputs.fingerprint` 链表达失效传播：上游 outputs 不变 → 下游 inputs 不变 → 下游跳过整段 stage。

这个形态依赖于本设计基线里若干已经固定的事实：

1. fact 的 `FactIdentity` 已经携带 `StableConeKey`，按 cone 切片本身就是 schema 内置能力。
2. `LirFacts.global_init.cone_init_routines` / `final_entry_init_order` 已经按 cone 标记 init routine 入口符号；下游链接时直接消费这些符号即可。
3. stage / fact crate 边界一旦由 `cargo build` 强制（见 §crate 划分），artifact serialization 只需要给已有 fact 加 wire format，而不必重新设计跨阶段数据。

唯一需要补足的工程能力是 `TypeId` 的 cross-process stable wire format：当前 `TypeId(u32)` 是进程内 `TypeStore` 索引，序列化后下游进程无法解释。要么把 fact / LIR 中的 type 字段替换为 stable text key（`scoopc_ids::CanonicalTextKey`），要么给 `TypeStore` 设计 portable serialization + 反序列化重映射。这是落地 per-cone artifact 的硬前置。P7-T05 已在 LIR stable dump / `LirFacts.type_context` 中永久记录推迟决策，并用 dependency gate 防止 LLVM handoff/emit/reachability 回退到上游 wrapper 或 HIR/MIR 扫描；wire-format owner 为 P8/per-cone build artifact serialization；P7 LLVM handoff 不持久化 `TypeId`，只校验同进程 `TypeStore` owner。

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

这一步很重要，因为历史优化逻辑曾分散在：

1. 旧 comptime / const 语义
2. HIR lowering 里的去虚化开关
3. MIR materialization 尾部顺手跑的 pass
4. LIR 的窄后处理
5. codegen 里的局部去虚化和 LLVM 自己的 target pass

P0/P3 已清理前 3 类主线 owner；codegen/reachability residual 继续归入 P7 backend cleanup。

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

历史上 HIR lowering 里存在 `devirtualize_dispatch_calls` 开关，并会在 lowering 时尝试把 virtual/interface call 直接改成 direct call；P3 已删除该 owner，HIR 现在保留动态 dispatch 语义与 dispatch contract。

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

当前 `mir/inline.rs` 是显式 MIR pass pipeline 中的 summary-driven inlining step：summary-driven、保守、基于 canonical pass artifacts 改写 body。

这项能力已经从“materialization 尾部顺手跑一下”迁入正式 MIR pass pipeline。

也就是说：

1. 它应该有清晰的输入：canonical materialized MIR + MIR facts/pass artifacts。
2. 它应该有清晰的输出：rewritten canonical materialized MIR / pass artifacts。
3. 它不应只是 `MirInstanceMaterializer::run()` 尾部的一个布尔开关。

#### D. MIR devirtualization

历史上 MIR 去虚化主要内嵌在 `mir/materialize/rewrite.rs` 里：重写 `CallKind::Virtual` / `Interface` 时直接尝试变成 `CallKind::Direct`。

P3 已把这项能力抽成正式 MIR pass；materialization rewrite 只保留 substitution / instance discovery / target fact recording，不再直接做优化改写。

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

P3 当前结果：

1. escape analysis 是 always-on 的 MIR analysis/facts step。
2. closure simplification 继续作为 MIR opt，按 opt level gate。
3. 若简化后影响 facts，由显式 MIR pass pipeline 记录 refresh 顺序，而不是在 materializer 尾部临时再跑一遍。

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

1. **已删除 / 已迁移**
   - 现有 comptime/const 相关语义路径
   - HIR 层 `devirtualize_dispatch_calls`
   - `mir/inline.rs` 已成为正式 MIR pass
   - `devirtualize.rs` 算法已迁到显式 MIR pass 语境
   - `mir/escape.rs` 已按 MIR analysis / MIR facts step 调度

2. **仍需删除**
   - codegen 层 dispatch 去虚化
   - reachability 层临时去虚化推断

3. **仍需重定位**
   - `effect_lowered::opt`：保留，改定位为 `LIR opt`
   - `llvm/codegen/main/const_eval.rs`：若未来仍保留极窄的全局静态数据初始化 helper，应改名并视为 backend data initializer helper，而不是 optimization pass

## 各阶段应发布什么

下面的“应发布的 facts”只讨论每个阶段自己拥有的职责，不包含把上游整包暴露给下游的临时包装。

### AST stage

#### 应发布的 IR/output

AST stage 应发布：

1. 解析完成的 AST compilation unit。
2. 每个 AST 文件的稳定 source identity、文件顺序和 compilation-unit membership。

这里的重点是“compilation unit”，而不是单文件 worker 包装。P1 已保留 `AstStageOutput` 作为 dump/helper worker，并新增 `AstCompilationUnitOutput` 作为 cone-level AST handoff；P2 已在后续 HIR stage handoff 中固定 `HirStageOutput = { hir, hir_facts }`。

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

#### P1/P2 后仍需补齐什么

当前最明显的问题是：

1. `AstCompilationUnitOutput` 已能表达 cone-level AST handoff，但它还没有发布独立 parser-owned facts。
2. production frontend 仍让 resolver/typecheck/HIR 过渡路径消费 build closure；P2 已收口 HIR barrier 与 `hir_facts`，后续若要完全物理拆分 cone-level orchestration，需要在后续阶段单独处理。

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

#### P2 收口结果

当前 HIR stage 已经发布独立 `scoopc_hir_facts::HirFacts`，公开 handoff 形状为
`HirStageOutput = { hir, hir_facts }`。

P2 已完成以下收口：

1. `LoweredHir` 不再携带 canonical MIR snapshot / pass view。
2. `TypedHirEffectContracts` 与 `ProgramFacts` 已从生产代码删除，不再作为并行 fact owner。
3. declaration/entity/global/native facts 与 source-site typed contracts 都由 `HirFacts` 发布和校验。
4. `MirLoweringFacts` 从 `HirFacts` 派生输入，不再保留 typed/fallback 双轨。
5. `@CallingConvention` non-generic、global roots monomorphic、top-level `var` storage policy gate 已固定在 HIR/typecheck 屏障内。

仍暂留的 `LoweredHir` side table 属于 HIR body inventory、type context、lowering helper 或 LLVM compatibility scaffold；它们不能被当作跨阶段 authoritative semantic facts。P7 backend cleanup 前，LLVM 仍可读取这类 scaffold，但共享源码语义查询必须走 `HirFacts`。

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

#### P3 收口结果与剩余边界

P3 的入口已经不再有 HIR typed-contract 泄漏；P3 完成后，`MirStageOutput` 发布的是 MIR-owned handoff：direct-style MIR、必选 canonical materialized snapshot，以及 `scoopc_mir_facts::MirFacts`。

`MirFacts` 当前承载：

1. direct-style root inventories
2. materialized snapshot binding
3. instance/callable family inventory
4. pass artifact metadata
5. MIR pass pipeline metadata

ordinary dispatch devirtualization、summary-driven inlining、escape analysis、closure simplification 和必要 refresh 已由显式 MIR pass pipeline 调度。`materialized_pass_view()` 仍是下游读取 canonical pass artifacts 的 MIR-owned query surface；它不再代表 HIR fallback 或 optional snapshot gap。

P4 已把 effect facts output 收口为只读窄产物，P5 已把 LIR output / LIR facts 收口为正式 handoff；P7 仍需继续清理 backend 的 HIR compatibility scaffold 与 raw MIR/effect residual。不能把这些后续问题伪装成 P3 未完成项，也不能重新引入重复 MIR owner。

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

#### P4 收口结果

当前实现已经采用纯分析阶段形状：`input = &MirStageOutput`，`output = EffectFactsStageOutput = { effect_facts }`。

P4 固定的边界是：

1. `EffectFactsStageOutput` 只保存 `MaterializedEffectFacts`，不嵌套整份 `MirStageOutput`，也不暴露 `materialized_pass_view()` / `materialized_mir()` / `mir_facts()` / `types()` 等上游查询面。
2. `MaterializedEffectFactsBuilder` 只读借用 `&MaterializedMir`，并通过显式 `EffectOwnedTypeContext` 保存 runtime-error effect、tuple carrier、step schema 与 continuation schema 等 effect-owned additions。
3. `scoopc_effect_facts` 发布 stage-independent effect/control facts 数据产品，只依赖基础 crate，不依赖 `scoopc` facade、MIR/LIR stage、backend crate 或其它 fact crate。
4. P5 输入必须显式分开 `MirStageOutput` 与 `EffectFactsStageOutput`；需要 MIR pass view 或 MIR facts 时从 MIR handoff 读取，不能通过 P4 output 回看。

因此，effect facts stage 已达到本节定义的“纯分析阶段”目标。若下一阶段 LIR 还需要某些 MIR-derived 但非 effect-owned 的 global facts，这些 facts 的 owner 应在 P5 的 LIR facts / query layer 中固定，不能重新把 P4 output 扩展成上游整包入口。

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

#### P5 收口结果与剩余边界

P5 已把 late-lowered handoff 收口为正式 `LirStageOutput = { lir, lir_facts }`：

1. `EffectLoweredStageOutput` 现在只是 `LirStageOutput` 的迁移别名，不再公开上游 `EffectFactsStageOutput` / `MirStageOutput` wrapper，也不提供 `materialized_pass_view()` / `effect_facts()` / `mir_facts()` 等上游整包 accessor。
2. `scoopc_lir_facts` 发布 P5-owned backend-neutral contract，包括 plain callable ABI/source/call-site、本地 effect/control contract、effect-step state/frame/boundary/resume query、dynamic invoke、dispatch owner/slot、continuation object、surface resume dispatch、resume packing 与 LIR opt metadata。
3. Program ABI materialization 的 logical 输入已经切到 `LateLoweredProgram + LirFacts + TypeStore`；LLVM-specific `ProgramAbiQuery` 仍是 backend-private physical layout 结果，不是 LIR facts。
4. LIR opt family 已固定为 effect/control LIR-owned 窄优化，并通过 pipeline metadata / verifier 与 post-opt LIR 对齐；它不承担普通调用图、全程序 inlining 或 backend-specific 优化。

剩余边界进入 TODO-6/P6-P8：global init/storage/entry init order、LLVM HIR compatibility scaffold、`LlvmCodegenStageOutput` / `StageEmitInput` 对 P5 handoff 的 backend wrapper、crate-private MIR pass-view residual、LLVM physical ABI/layout、backend reachability 和多 `TypeStore` 桥接仍需要继续收口，不能描述为 P5 已完成范围。

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

1. `LoweredHir` 去掉 `materialized_mir`。（P2 已完成）
2. `MirStageOutput` 不再泄漏 HIR source-site contracts。（P2 已完成）
3. `MirStageOutput` 收口为 `{ mir, mir_facts }`，把 optional materialized snapshot / pass artifacts 变成 MIR-owned handoff。（P3 已完成）
4. `EffectFactsStageOutput` 去掉对整份 `MirStageOutput` 的长期嵌套。（P4 已完成）
5. `EffectLoweredStageOutput` 去掉对整份 `EffectFactsStageOutput` 的长期嵌套。（P5 已完成）

这一步的目标是先让 stage output 长成正确的“外形”。即使内部还有旧实现，也不能继续把上一阶段整包暴露给下游。

### 第三阶段：收口 facts 的 owner

1. HIR declaration/source-site facts 已重新分配到 `HirFacts`。（P2 已完成）
2. `MirLoweringFacts` 的 source-site input 已合并成从 `HirFacts` 派生的单一路径。（P2 已完成）
3. 把当前在 LIR stage 里重算的 MIR-derived global facts 挪回 MIR stage 或其 fact crate；P5 已把 codegen-neutral ABI/query owner 收口到 LIR facts，TODO-6/P7 若发现新的 backend-neutral 缺口必须继续补到明确 owner，不能回退到上游整包回看。

这一步的目标是让“每一类事实只在一个地方定义和验证”。

### 第四阶段：补齐 LIR

1. 让 plain callable surface 不再依赖 materialized MIR 回看。（P5 已完成）
2. 让 dynamic-invoke / dispatch contract 不再依赖 MIR/HIR 扫描。（P5 已完成）
3. 发布 backend-neutral 的 `lir_facts` / `lir_query` 层。（P5 已完成）

这一步已为后端只依赖 LIR 奠定 backend-neutral 输入基础；真正删除 LLVM backend 的 HIR/raw MIR/effect residual 仍属于 P7。

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

当前实现离最终目标还剩 P8 最终清场与后续 P9/P10 crate/artifact 拆分，但 P2 已完成 HIR barrier 与 `hir_facts` 收口，P3 已完成 MIR-owned handoff、`mir_facts` 与 MIR pass pipeline 收口，P4 已完成 effect facts purity 与窄输出收口，P5 已完成正式 `LirStageOutput = { lir, lir_facts }`、backend-neutral LIR facts/query owner 与 LIR opt family 收口，P6 已完成 global init/storage/final-entry owner 收口，P7-T05 已完成 LLVM backend cleanup 清场、stage handoff 与 physical ABI/layout 合并验证：

1. `LlvmCodegenStageOutput` / `StageEmitInput` 已收窄为 `LIR + LIR facts + LlvmStageBaseContext`，`LirStageOutput` 不再携带 LLVM residual accessor。
2. LLVM entry/global、reachability、production body emission 与 physical ABI/layout 已迁到 LIR/LIR facts/base context；integer literal 范围诊断已从 LLVM HIR body precheck 上移到 typecheck，dependency gate 已覆盖 LLVM handoff/emit/reachability 的 forbidden residual 搜索。
3. `TypeId` cross-process stable wire format 显式推迟到 P8/per-cone build artifact serialization；P7 同进程 LLVM handoff 只要求 TypeStore owner/verifier 一致。
