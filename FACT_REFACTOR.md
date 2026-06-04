# Fact 重构方案：阶段输出从「扁平 fact 袋」转为「阶段专属的引用闭合树」

> 状态：设计阶段（概念已闭环，尚未动代码）。本文先只谈**内存形态与不变量**，序列化/存储留到最后。

---

## 1. 目标（必须说清的部分）

**每个编译阶段产生单一的「树型结构输出」，使用阶段专属的类型；并且每个阶段在产出之前，必须自行确保输出的有效性、正确性与完整性——不能把校验工作交给下游。**

展开成可执行的判据：

1. **单一结构**：一个阶段的输出是**一个**结构化实体，不是一堆平行的 `Vec<XxxFact>`。
2. **树型 + 所有权**：结构的根是 `project`，往下是 `cone → package → class/interface/function/... → block → statement/expression/...`。**节点的 owner 是它的父结构**；节点之间可以「引用」，但引用不等于拥有。
3. **阶段专属类型**：HIR 树、MIR 树各有自己**独立的节点类型集合**（不是同一个泛型）。这样上一阶段的输出在**编译期**就无法被误喂给下一阶段。
4. **纯函数**：`X.S_n = lower_n(X.S_{n-1}, {依赖的接口面@n})`，同输入必产同输出。
5. **产出前保证三性（核心约束）**：
   - **有效性**：结构里没有悬空引用——每个引用（`TypeId`、callee、instance ref）都能解析到本 cone 拥有的子结构、或某依赖接口面的传递闭包之内。
   - **正确性**：结构满足该阶段的全部不变量（如候选集非空、ABI 与 schema 匹配）。
   - **完整性**：下游 walk 这棵树所需的一切都在树上，下游**不需要也不应该**再做「从 map 里盲查 + 查不到就报错/panic」。
6. **下游不做校验**：消费侧只允许 `walk 树 + 按 id 进 arena + 按 ref 进依赖接口面`，**不允许** `map.get(k).expect("上游应该填过 k")`。
7. **责任划分：HIR 是唯一的「前端拒绝」边界**。凡是能让程序非法的判断（类型检查、引用解析、可见性、效果合法性……），到 HIR 输出产出时**全部已决**。**MIR 起不再做任何类型检查 / 输入拒绝**，只对基本块做变换。
   - 强推论：**HIR 之后 A 类（应为诊断的）fail-fast 不复存在**——下游任一处 fail-fast 都只能是 B 类（内部不变量 / ICE）。于是有一条可机械检查的纪律：**MIR 及之后任何一处发出「用户诊断」，即责任划分被破坏的 bug**。
8. **下游对输入 never fail（硬纪律）**：每个已清理阶段的变换是**全函数**——
   - **无 `Result` 错误出口、无 `Todo`/`Unsupported`/`Unknown` 等占位/escape 变体、无 `Todo(_) => {}` 之类 no-op 容忍**。一个阶段要么接受输入并产出，要么该输入**早被上游拒绝**。
   - 任何「失败的可能」必须把**失败来源上移**为上游的**输出保证**（producer 担保 / consumer 全函数），使本阶段**结构上不可能失败**。
   - **真正的内部不变量违背（B 类 ICE）不用 `Result` 表达**，靠引用闭合让其**结构上不可达**（`unreachable!`/`debug_assert`），不是错误出口。
   - **绝不在代码里留 placeholder**：占位/escape/「以防万一的 `Result`」是**引诱**——会诱使后续 coding agent 不断增加占位、让纪律失效。
   - **报错沿 back-to-front 逐级上移**：清理第 K 级时，其失败可能移到第 K-1 级的输出保证；逐级上推，最终所有「预期失败」只在 **HIR** 汇成前端拒绝（P5 用 fixture 验证 + 补缺口）。

> 一句话：**把「结构有效性」从下游的运行期祈祷，提升为上游的构造期类型保证；并把「前端拒绝」这件事全部、且仅仅压在 HIR 这一站。**

---

## 2. 概念模型

### 2.1 两种关系：所有权 vs 引用

- **所有权（树脊，用来 walk）**：`cone ⊃ package ⊃ item ⊃ body ⊃ {block, site}`。一个节点**拥有**它的全部子数据。例如一个 call site 应当**拥有**它自己的 inventory / event / target / surface / boundary，而不是把这些拆成 5 张按 `SiteId` 平行的 map 让下游 join。
- **引用（图的边）**：一个 call site 指向**另一个** instance；一个节点引用某个 `TypeId`。引用是「一个保证能解析到闭合世界里某个已存在节点的、带类型的指针」。它的唯一契约：**构造出来时，referent 一定存在**。谁构造它，谁就在构造前负责确认目标存在，确认不了就地报「输入错误」，绝不把可能悬空的引用塞进输出。

### 2.2 三类数据，三种归属

| 数据 | 例子 | 归属 |
|---|---|---|
| **树节点（pass I/O 本体）** | function、block、site、签名、vtable、effect 事件 | 挂到它的 owner 节点上 |
| **被引用的共享子结构** | 类型（`List<List<Int>>` 这种递归 DAG）、符号表 | cone 拥有的 interned arena，节点按 `id` 引用 |
| **全局 side table** | schema 版本、编译参数、关心的环境变量、产出 provenance（哪些 pass 跑过） | 薄薄一层全局上下文 |

**判据**：一项数据能进 side table，当且仅当它同时满足「**全局有效**（对整棵结构是同一个值）」与「**是被查阅的环境信息、不是被某节点拥有/产出的数据**」。凡 per-node 拥有的，逻辑上一律归树。

> 注：这里的「归树」只关乎**逻辑归属**，不要求物理上也是树——物理可以是「每类一张扁平 arena」。「side table」也不止一种，精确的三分法与物理表示见 §2.7。

### 2.3 阶段专属树（不是泛型）

每个阶段定义自己的一套节点类型：

```
HirTree  ：body 是表达式树（block → statement → 嵌套 expression），已 desugar
MirTree  ：body 是 CFG（block → instruction* + terminator），已 monomorphize
```

- 上半身（`cone → package → item`）在各阶段都重现，但是**不同的 nominal 类型**（`HirCone`/`MirCone`、`HirFun`/`MirInstance`）——结构相似、类型不同，且**有意不同**。
- 下半身（函数体内部）各阶段形状本就不同，**不该硬捏成同一个类型**。
- 引用永远闭合在「单一阶段的类型宇宙」内：`MirCallee` 在类型上只能指向 `MirInstance`，不可能指向 `HirFun`。于是「纯函数 + 无悬空引用 + 阶段隔离」被**类型系统强制**，而非靠约定。

### 2.4 project 是根，依赖 = 接口面

- **`project` 是树根，children 是多个 cone。**
- **接口面（Interface Surface）必须是一等产物**，否则 cone 分离编译无法做：
  - 依赖者只消费 `依赖 D 的接口面@本阶段`，**看不到 D 的内部树** → D 可独立编译、缓存、分发，依赖者不必重编 D。
  - **接口面是重编译防火墙**：D 内部改了但接口面@n 没变 → 依赖者无需重建。增量编译的正确性挂在这一条上。
  - 接口面本身也服从全套原则（引用闭合、所有权即树、阶段专属、可序列化）。

- **依赖规则**：
  ```
  X.S_n  ←  X.S_{n-1}                              // 本 cone 上一阶段的完整内部树
         +  { D.Interface@n  for D in deps(X) }    // 每个依赖在【本阶段】的接口面（不是完整内部树）
  ```
- **闭合世界有界**：`X.S_n` 的闭合世界 = X 的内部树 + 依赖图里各 cone 接口面的**传递闭包**。

### 2.5 唯一会「往回够」的特例：跨 cone monomorphization

跨 cone 的 monomorphization 是 **demand-driven**——D 不可能预先知道 X 要哪个 `foo<Int>`。所以：

- **实例 `D::foo<Int>` 由实例化它的 cone X 拥有**，落在 `X.S_n` 内、闭合于 X 自己的宇宙。
- X 唯一的跨 cone 引用是 `foo<Int> → D 接口面里的【模板】`。
- 因此 **MIR 接口面 = 可实例化模板 + 公开的具体实例**；frontend 接口面 = 已解析的签名/类型。
- 答案：**不需要 D 的「完整上一阶段输出」**；需要的是 D 的**本阶段接口面**，而该接口面在 MIR 阶段必须包含 pre-mono 模板。

### 2.6 一个 cone 在阶段 n 的完整产物

```
StageArtifact@n(cone) {
    internal_tree : InternalTree@n,    // 私有；阶段专属节点；引用闭合于本阶段宇宙
    interface     : Interface@n,       // 公开给依赖者；阶段专属、引用闭合、可序列化（缓存单元）
    types         : TypeArena,         // cone 拥有，按 TypeId 引用
    symbols       : SymbolArena,       // cone 拥有，按 id 引用
    meta          : { schema_version, compiler_args, produced_by_passes, ... }  // 薄 side table
}
```

### 2.7 物理表示：逻辑树 ≠ 物理树

**逻辑上引用闭合的树，物理上完全可以是「每类节点一张扁平 arena，用 id 互相关联」（SoA）。** 二者不矛盾——只要关联机制对，物理扁平往往更好（cache 友好、克隆/扩容便宜、易挂派生数据）。

要害是区分**「好的扁平表」与「坏的扁平表」**——我们要消灭的从来不是物理扁平，而是「字符串语义 key + 可失败 join」：

| | 坏（要消灭） | 好（采用） |
|---|---|---|
| 关联方式 | `HashMap<String, Fact>`，字符串语义 key | `Idx<T>`，指向某张具体 arena 的**定型下标** |
| 解引用 | **可失败搜索**（key 可能不存在）→ `expect` | **不可失败 deref**（`arena[idx]`，下标铸造时即有效） |
| 父子关系 | 消费侧**运行期 join 重建** | 生产侧**构造时焊死**（父节点持有子节点的 `Idx`） |
| 类型安全 | 无 | `Idx<Expr>` 不能索引 `Stmt` 表 |

`Idx<T>` 带类型参数后，**阶段隔离也落到 id 层**：`HirExprId` 在类型上无法索引 MIR 的 arena。

**两种 id，分界线 = 接口面 / 增量复用边界**：

- **数组下标（`Idx` = u32）**：cone 内部、阶段内部、整棵树作为纯函数输出**整体重生**的引用。最便宜最快，可挂平行派生数组。代价：插入移位，**不跨 cone、不跨重编稳定**。
- **stable id**：凡要**暴露给下游 cone**（接口面）、或要作为**增量复用单元**的节点。从稳定输入（名字路径/签名）派生，过序列化、做失效判定。

> 分界线 = **「跨 cone 暴露」∪「增量复用粒度」**。**接口面是翻译层**：内部全用 `Idx`，到接口面把暴露子集翻成 stable id 发布；下游持 stable id，加载依赖时**一次性**解析成本地句柄——边界处一次可失败查询，之后全程不可失败 deref。

**统一身份模式（所有句柄层一律照此）**：每一层需要身份的实体（DefId §8、LirCallable §13/14、以及后续 MIR/effect 各层），其身份表示一律三件套：

- **密集下标**（`Idx`/`LocalDefId`/`LirCallableId` 等，u32）——artifact 内 live 引用，deref 不可失败、不比字符串；
- **紧凑 hash**（`DefPathHash`/`LirCallableHash` 等，定长）——跨 cone 引用、序列化、map key；
- **String**（canonical path / readable）——**仅调试**，不进 live 引用、不做 map key。

各层的「稳定身份语义」不同（定义 vs 实例 vs …），是**同纪律、不同 key 值**的并列方案，不共用同一 key。这条取代「字符串当 live key」的现状（如 `StableLirCallableKey{canonical_text, readable_path}` 满地当 BTreeMap key）。

**side table 的精确三分法**（判据是「presence 是否构造保证 + 关联机制」，不是物理是否分开）：

1. **per-node 自有数据**——存节点结构体里（AoS）**或**与节点 arena 同下标的平行数组里（SoA）。两者都是「节点拥有它」，presence 由 co-index 构造保证。**好**。（例：推断出的类型 = 与 `ExprId` 同下标的 `Vec<TypeId>`，**不是** `HashMap<Expr, TypeId>`。）
2. **全局环境 side table**——schema 版本 / 编译参数，全局唯一。**好**。
3. **可失败语义 map**——`HashMap<语义key, 数据>` 且 key 可能缺。**这才是要消灭的**。

即：§2.2「per-node 数据不准进 side table」精确化为「不准进第 3 类」；进第 1 类的平行数组完全 OK（只是 SoA，不是可失败 join）。

**唯一要守的不变量**：SoA 的平行数组必须与节点 arena **严格锁步分配**（插了节点就必须插对应槽，否则 desync）。但这是**集中在 arena 分配器一处的局部构造不变量**，不是跨阶段 join。引用闭合校验退化成廉价的 `idx < len && slot 活着`（可降级 debug assert）；唯一真正可失败的是跨 cone stable-id 解析，且只在加载点。

---

## 3. 调研结果（现状证据）

> 以下为针对当前代码库（`/Volumes/Data/home/chenxu/repos/scoop`）的调研，用来佐证「现状违反上述目标」以及「重构在哪些不变量上是安全的」。
>
> **更新（2026-06-03，HEAD `8f7a372d`）**：代码已推进一串 `T3-04` 提交（"Close fact fallback gaps"），正在**原地**落地本文档思路——把 codegen 侧「fact 缺失就回退 MIR 重算/猜」的 fallback 逐个关掉，改为由生产者 `crates/scoopc/src/pipeline/lir_facts_builder.rs`（~3800 行）强制发布对应 fact（如 `call/lowering.rs` 删掉数百行 fallback 重算）。这验证了下文思路、也使下文的「现状数字 / fact 组」需要按此更新（已更新）。注意：这是在**扁平 fact 表示内**做硬化，并非本文档主张的「树 + 句柄」重构。

### 3.1 callee 存在性已由 typecheck 保证（→ 下游可合法假定）

**结论：「callee 存在且可调用」是 typecheck 阶段已强制的上游不变量。**

- 直接调用：`crates/scoopc_hir/src/typecheck/expr/call/dispatch.rs:40-93` 收集 overload 候选；全部失败时发**诊断错误** `ExprTypeError::CalleeNotCallable`（`crates/scoopc_hir/src/typecheck/expr/error.rs:345-351`），**不是 panic**。
- 跨 cone 调用：同样在 typecheck 校验，经可见性过滤 `is_symbol_visible_from_source`（`crates/scoopc_hir/src/typecheck/expr/ops.rs:304-314`）；符号不存在或不可见 → 同样诊断错误。
- 下游不再 re-validate：MIR lowering（`crates/scoopc_mir/src/mir/lower/fn_lowering_call.rs:81-103`）直接消费携带已解析 FQN 的 `TypedCallSiteContract`，缺 contract 时 `return false`（跳过），并不当作错误重判；effect-facts（`crates/scoopc_lir/src/effect_facts/facts.rs:270,303`）直接 `.expect()`，假定 MIR 已建立目标存在性。

**对重构的意义**：MIR/effect 阶段里所有「找不到 call 目标」的运行期路径都是**纯 B 类（编译器内部 bug）**，应当转成 MIR 树/接口面的**构造期不变量**，而非运行期重新校验。这印证了 §2.1「引用的存在性应在构造引用时、由持有目标世界的那一方负责」。

### 3.2 「扁平 fact 袋 + 下游 join + verify 兜底」是全管线通病

**结论：四个 facts crate 都是同一个反模式。**

| 阶段 | 顶层形态 | 证据 | 下游 join / fail-fast | verify.rs |
|---|---|---|---|---|
| **HIR** | 扁平平行 `Vec` | `crates/scoopc_hir_facts/src/lib.rs:27-33`（`declarations`/`source_sites`/...，每条 fact 自带 owner FQN） | — | 收集所有 list 的 identity 查重；不强制树形 |
| **MIR** | 扁平 `Vec` + 待 join 的 key | `crates/scoopc_mir_facts/src/lib.rs:37-48`；`effects.rs:11-18`（`site_inventory`/`call_site_targets` 等各自带 `instance`/`body`/`site_id`） | `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs` 的 `MirFactIndex` 用 `HashMap<(InstanceKey,String), Bundle>` 重建树，每条 fact `instance_for_artifact(...)?`，缺 key 报 `UnknownMirFactInstance` | 查重 + 空候选集 + canonical snapshot 绑定（运行期兜底） |
| **Effect** | `BTreeMap` keyed，但 schema 内嵌 ID | `crates/scoopc_effect_facts/src/lib.rs:36-43` | `crates/scoopc_lir/src/effect_lowered/builder.rs:88,176-177` 链式 `.get().ok_or_else()` | 递归爬 step/continuation schema 检查引用存在（运行期） |
| **LIR** | `BTreeMap` keyed + summary 计数（组数已增，见 §12.3） | `crates/scoopc_lir_facts/src/lib.rs` | callable→step/packing、dynamic_invoke→dispatch 等 `.contains_key()`/`.get()` | **2007 行**（2026-06-03），含多处 count 一致性、global-init 依赖序、引用存在性 |

**关键观察**：LIR 的 `verify.rs` 已从 1331 涨到 **2007 行**（同期 MIR 316 / Effect 463 / HIR 395）——**verify 体量随「扁平程度」＋「硬化程度」上升**。T3-04 在关 fallback 的同时给 `LirFacts` 又新增多组发布 fact（§12.3），verify 随之继续增长：这反而**强化** §2 的论点——在扁平表示里靠「多发布 fact + 运行期 verify」硬化，成本只增不减；唯有把所有权做进类型（instance ⊃ body ⊃ site ⊃ target），这些运行期校验才会由构造保证而消失。

**两类 fail-fast 的区分**（贯穿全管线、且当前混在同一批 `expect("...")` 里无法区分）：
- **A 类**：本应是诊断的——非法输入绕过前端校验漏洞，到下游 `unwrap` 才崩（如 `canonical_call.rs` 的 `expect("known field name mapped to param")`）。修法：回到 resolve/typecheck 补诊断。
- **B 类**：真正的内部不变量（如「candidate set 为空」「step schema 无 shell」「materialized family 无 summary」）。修法：前移到产出方的**构造期类型保证**，下游 `expect` 随之变为逻辑上不可达。

### 3.3 依赖接口面只在 frontend 层建模，没有 per-stage 接口

**结论：今天的依赖「接口面」只有 frontend 一层；中间阶段没有阶段专属接口。**

- `CachedConeImport`（`crates/scoopc_hir/src/cone_import.rs:37-52`）只含 frontend 级签名：`CachedConeType`（`:56-65`，仅类型元数据，无 body）、`CachedConeFun`（`:69-74`，`FunSig` 签名，`has_body:false`）。
- 它在 frontend 一次性注入共享 `Index`/`TypeEnv`（`inject_cached_cone_imports`，`cone_import.rs:165`）；下游阶段经 `HirSemanticArtifact` 消费注入后的环境，**不再重放**（`crates/scoopc/src/frontend.rs:388-389` 注释明确说明）。
- 跨 cone 泛型：MIR materialization 从注入的 `Index`（`by_fqn → FunOverload.sig` 模板）**在消费 cone 本地实例化**（`crates/scoopc_mir/src/mir/materialize/inputs.rs:14-16`），实例由消费 cone 拥有——印证 §2.5。
- 另一端：codegen 有独立的 `CachedDepArtifactHandoff`（`crates/scoopc_codegen_llvm/src/llvm/handoff.rs:26-34`，含 `lir`/`lir_facts`/`type_store`/`object_files`），只在 codegen 阶段进入。

**对重构的意义**：现状是「frontend 接口面 + codegen LIR handoff」两个点，中间 MIR/effect 阶段**借用 frontend 形状**的接口（`UnknownMirFactInstance` 这类 join 错误正源于此——跨实例引用用字符串 key 表达、下游重新解析）。目标是把**接口面升成 per-stage、引用闭合、可序列化的一等产物**，让每个阶段的跨 cone 引用闭合在本阶段宇宙。

---

## 4. typecheck 阶段的信息组织（特殊一站）

typecheck 是把「未解析的实体引用」变为「指向确定实体的引用」的阶段，天然有大量查询需求。目标：**经过这一站后，下游不再有任何同类问题，所有操作都是树上的变换。**

### 4.1 它是唯一「可以有可失败查询」的阶段

因为**解析本身就是查询**——给定 scope 里的名字找它指代的实体，是对 scope/命名空间的可失败 lookup，做不成纯树变换（AST 里的引用是字符串）。所以不消灭 typecheck 里的查询，而是立**纪律**：

> 查询只跑在**临时脚手架**上；解析的产出，把每一次「可失败的名字查找」转换成「不可失败的句柄」，使**下游对引用零次可失败查询**。下游跟随引用 = 句柄 deref = 不可失败的边遍历，不是搜索。

### 4.2 杠杆：实体身份从「字符串」变成「铸造的句柄」

**现状（调研）**：实体身份就是 FQN 字符串——`Index.by_fqn: HashMap<String, NamespacedSymbols>`（`crates/scoopc_hir/src/resolve/mod.rs:550`），解析即按字符串查表。**字符串身份一路漏到下游**，每阶段都按字符串再查一次（`instance_by_callable_fqn`/`instance_for_stable_text`），`UnknownMirFactInstance` 这类 join 错误就是它的必然产物——**查询永远停不下来**。

**目标**：typecheck **铸造稳定句柄 `DefId`**（指向 def arena 的 id），HIR 输出里所有引用都是 `DefId` 边，不是字符串。`by_fqn` 字符串表是脚手架，用完即弃。跨 cone 用 `(ConeId, LocalDefId)`——**依赖接口面本质就是「该 cone 公开 def 的 arena」**（呼应 §2.4）。字符串→句柄的转换只发生一次，就在脚手架还在的 typecheck。

### 4.3 让解析成立的相位：身份优先（identity-first）

解析有依赖环（前向调用、互递归类型），不能单遍线性扫完。相位结构：

```
phase 1  整 cone 扫描建「检查树骨架」+ 给每个声明铸造身份（DefId）
         —— 此刻「有什么」已有身份，但引用/类型还是「洞」。这是 forward/循环引用成立的前提。
phase 2  per-body fill：解析引用 与 类型推断【交织】的不动点（不是先后两遍——
         重载解析依赖实参类型、实参类型又依赖推断），就地把洞填实。
phase 3  totality gate + lower：把填好的检查树塌缩成 hole-free 的 HIR 输出。
产出不变量：无未解析名字；每个引用是已解析句柄；每个表达式有具体 TypeId；全图引用闭合。
```

关键在 **phase 1 先铸造所有身份**：之后画引用边时目标身份必已存在，「谁先构建」的顺序问题消失——画的是「指向已铸造身份」的边，对应节点 body 可以还在填。

> 这三相的「两个类型 / 三终态 / totality gate / phase 参数化」详见 §5。

### 4.4 两个信息结构，严格分开

| | 脚手架（临时，查询优化） | 输出树（持久，walk 优化） |
|---|---|---|
| 内容 | `by_fqn` 索引、scope 树、命名空间、可见性、推断上下文 | 已解析 HIR；引用是不可失败句柄 |
| 查询性质 | **可失败** lookup（解析/推断在此） | **不可失败** deref（边遍历） |
| 生命周期 | typecheck 结束即弃 | 唯一交给下游的东西 |

这正是 §3.3 那个「漏」的修法：现在 `HirSemanticArtifact` 把 `Index`/`TypeEnv`（查询脚手架）一路下传；本模型里下游该拿到的是**已解析的树 + def arena**，可随树带一个**派生的 `DefId→node` arena**（构造即完整、deref 不可失败），但**那张可失败字符串索引不下传**。

> 统一观察：**「树」与「def arena」是同一份数据的两个视图**——所有权 walk 看到树，`DefId` 下标看到 arena。不是两份数据。（呼应 §2.7：arena 索引是结构的派生，不是结构本身。）

### 4.5 为什么「之后不再有同类问题」成立

- 跟随引用 = `DefId` deref = 不可失败 → 边遍历，不是搜索。
- monomorphization / devirtualization **创造新节点**（`foo<Int>`），但由**已解析引用 + 类型实参**驱动，是**纯变换**，不是可失败名字解析——不是反例，是「树→树」的一部分。

唯一的可失败查询被收敛进 typecheck 的 P1/P2 且结果焊进树；出了 typecheck，世界里只剩不可失败 deref。

### 4.6 决定

1. **DefId 表示 / def arena**：见 §8（两级 id：定型 `LocalDefId` + `StableDefKey`/`DefPathHash`；def arena = 一族 per-kind arena）。
2. **`Index`/`TypeEnv` 降为 typecheck 内部脚手架**：phase 2 结束即弃，**不下传**。HIR 输出 = 已解析树 + def arena（+ §2.7 的 interned arena + 薄 side table）。这同时修掉 §3.3「`HirSemanticArtifact` 把查询脚手架一路下传」的漏；现散落各处的 `by_fqn` 字符串 lookup 全部收敛到 §8.1 的 load/import 边界一处。

---

## 5. typecheck → HIR：三相与两个类型

§4.3 给了相位骨架，本节给落地机制。核心：**带洞的「检查树」与无洞的「HIR 输出」是两个不同的类型**，phase 3 是两者之间的全函数闸门。

### 5.1 三相

```
phase 1  扫描建检查树骨架 + 铸造所有声明身份。   （整 cone、sequential；引用/类型是洞）
phase 2  per-body fill：resolve↔infer 交织的不动点，就地填洞；错误恢复插「毒值」。
phase 3  totality gate + lower：检查树 → hole-free HIR 输出；顺带建 Idx arena（§2.7）、抽接口面（§2.4）。
```

### 5.2 两个类型，不是「一个类型带 flag」

- **检查树**：引用是 `Unresolved | Resolved | Error`，类型是 `Option<推断变量>`。允许洞。
- **HIR 输出**：引用**只有**裸句柄，类型**只有**具体 `TypeId`。**类型上根本表示不出洞。**

于是「输出完全确定」不靠 phase 3 记得检查，而是**类型系统强制**：

```
lower : CheckingTree → Result<HirTree, InputError>   // 唯一的塌缩闸门
```

### 5.3 洞有两种，输出 TypeStore 也无洞

- **引用洞**（名字未绑到句柄）、**类型洞**（推断变量 `?n`）。
- 输出 TypeStore 是另一个类型——检查期有 `TypeKind::InferVar`，输出**没有**。phase 3 的 "zonk"（回填变量）与「引用回填」是同一件事的两面；还有未解变量 = 「需要类型标注」输入错误。

### 5.4 三终态 + 全有或全无（错误恢复）

真实 typechecker 会插「毒节点」继续，以便一次报多个错。所以洞要区分三种终态：

| 洞终态 | 含义 | phase 3 动作 |
|---|---|---|
| `Resolved` | 填实 | 进输出 |
| `Error`（已诊断） | 用户错误、已报过 | **编译失败、不产出 HIR**；合法路径，**非 ICE**，不重复报 |
| 未填 且 未诊断 | 不该发生 | **ICE**（检查期 bug） |

推论：**HIR 输出全有或全无**——只要有一个 `Error`，就没有 HIR 输出。下游永远只在「完全有效的 HIR」上运行，错误一律卡在 typecheck 边界。

### 5.5 表示：phase 参数化 + Final 裸化

洞字段的类型由 phase 的**关联类型**决定，且 **Final 的关联类型是裸的已解析类型，不是 `Option`**（否则下游照样 unwrap）：

```rust
trait Phase { type Ref; type Ty; }

struct Checking;
impl Phase for Checking {
    type Ref = RefHole;          // enum { Unresolved(Name), Resolved(DefId), Error }
    type Ty  = Option<InferTy>;  // 洞 / 推断变量
}
struct Final;
impl Phase for Final {
    type Ref = DefId;            // 裸句柄——没有 Option、没有 enum
    type Ty  = TypeId;           // 裸
}

struct CallExpr<P: Phase> { callee: P::Ref, result_ty: P::Ty, /* ... */ }
```

`CallExpr<Final>.callee : DefId`——**类型上拿不出洞**，下游直接当句柄用。一条结构定义（`CallExpr<P>`）避免 drift，Final 裸化保证 totality。

### 5.6 关键纪律：区分「洞」与「真·可选」

「Final 不许有 `Option`」太强；精确判据是 **Final 不许有「洞」，但允许「真·可选」**：

| | 洞 | 真·可选 |
|---|---|---|
| `None` 含义 | 「还没填上」 | 「程序的合法终态」 |
| 例 | `callee` 未解析、表达式类型未推断 | 自由函数的 `receiver`、用户省略的返回标注 |
| Final 表示 | **经 phase 字段塌缩成裸类型** | **保留** `Option<T>`（两 phase 一致） |
| 下游 | 拿不出 | `match` 合法分类，**非 unwrap** |

判据：**`None` 在「完全检查过的程序」里是否一个有意义的答案？** 是→真·可选保留；否（=「没填上」）→洞必塌缩。落地要审计：**任何「洞性质」字段漏成共享 `Option<洞>`，就是一个下游 unwrap 漏点。**

### 5.7 判据：同构才参数化

> **两个表示结构同构、只差「字段是否已解析」→ phase 参数化（Final 裸化）；结构本身不同 → distinct 类型。**

- 单阶段内 checking↔final 同构 → 参数化。
- 跨阶段 HIR（表达式树）↔ MIR（基本块 CFG）**不同构**（HIR→MIR 是拍平表达式、引入 block/terminator/SSA 的真重构）→ distinct 类型；强行参数化＝逼出假同构，又会退化成「给阶段专属字段硬塞 Option」的病。

`Hir<Final>` 塌缩完即 §2.7 那套表示（`Idx` 句柄 + `TypeId` + arena）——它**就是** HIR 输出树。

---

## 6. HIR 树承载内容盘点（首次定型）

HIR 是「树」第一次定型，涉及面最广。本节基于三路调研，把当前散落在 `scoopc_hir_facts` / typecheck 脚手架里的东西，按 §2.7 / §4 的分类（A 树节点 / B interned / C 全局 side table / D 临时脚手架）整理一遍，确保重设计时无遗漏。

> 三个来源：① 当前 HIR 输出面 `crates/scoopc_hir_facts`；② typecheck 脚手架 `resolve::Index` / `TypeEnv` / `HirSemanticArtifact`（`crates/scoopc_hir/src/{resolve,typecheck,stage}.rs`）；③ MIR 对 HIR 输出的消费需求 `crates/scoopc_mir/src/mir/materialize/`。

### 5.1 实体域 → 树节点与所属数据（A）

逻辑所有权树的内容（物理上每类一张 arena，见 §2.7）：

| 域 | 树节点（owner） | 节点拥有的 per-node 数据 | 当前来源 fact |
|---|---|---|---|
| **声明：nominal** | class/struct/enum/object/interface/effect | kind、type_params(+variance)、字段表、变体表、companion、modifiers | `NominalDeclarationFact`、`type_kinds`、`object_types`、`companion_objects` |
| **声明：callable** | function/method | receiver?、params(名/类型/默认/vararg)、return、effect row（declared/inferred/published）、type_params、has_body、ABI | `CallableDeclarationFact`、`CallableSourceEffectFacts`、`FunctionEffectContract`、`FunOverload/FunSig` |
| **声明：field / 变体** | field、enum variant | 名、类型、tag、owner | `FieldDeclarationFact`、`EnumVariantDeclarationFact` |
| **泛型模板** | generic 模板（=带 type_params 的 callable/nominal 节点本身） | 模板参数名（owner/function 的 type/eff param）、signature、body? | `GenericTemplateFact`、`StableTypeParamFact` |
| **dispatch 表** | vtable / itable（挂在 class/interface 节点上） | slot：index、方法句柄、签名类型 | `DispatchTableFact`/`DispatchSlotFact` |
| **函数体** | block → statement/expression（desugar 后的表达式树） | 见 §5.2 | `SourceSiteFacts` 各 site |
| **全局/初始化** | top-level val/var、object singleton | kind、类型、storage policy、initializer、init 依赖序 | `GlobalRootFact`、`InitializerFact`、`TopLevelInitRootContract` |
| **native/extern** | extern fun、native callable、extern global、extern lib | symbol、calling convention、签名、effect、mutable | `ExternFunctionFact`、`NativeCallableFact`、`ExternGlobalFact/Contract`、`ExternLibraryFact` |

### 5.2 体内「site」：从 5+ 张按 SiteId 平行表 → 节点字段（核心 collapse）

当前函数体里的语义点被拆成**一堆按 `SourceSiteIdentity`（owner+SiteId+源位置）平行的 fact 表**，消费侧按 site 再 join：

- `CallSiteContract`、`ArgumentBindingContract`、`CallSiteInstanceFact`、`TemplateSiteBindingFact`、`DispatchCandidateFact`、`CanonicalSemanticOperationFact`（一个 call 表达式被这 6 张表共同描述！）
- `PerformSiteContract`、`HandleSiteContract`（含 arm）、`ContinuationResumeContract`
- `AssignmentContract`、`PatternBindingContract`、`WithUpdateContract`、`HiddenInitializerEffectFact`

**目标**：体里的**每个表达式/语句就是一个树节点，直接拥有它自己的全部契约**。一个 `CallExpr` 节点拥有：已解析 target 句柄、arg binding、type/eff 实参、（虚/接口调用的）dispatch 候选、surface effect row——**6 张平行表塌缩成一个节点的字段**，不再有 `(source_path, span)` / `SiteId` 的 join。这是 §2.7「好扁平 vs 坏扁平」在体内的直接应用。

### 5.3 引用（边）：字符串 / SiteId key → 解析句柄

当前体内/声明里大量「字符串 FQN / CanonicalTextKey / (path,span)」引用，按 §4.2 全部要变成**铸造的句柄**（内部 `Idx`，跨 cone `(ConeId, StableId)`）：

- callee：`FunctionTarget.fqn` / `MemberCallTarget.member_fqn` / `ConstructorCallTarget.owner_fqn` → callable/ctor 句柄
- 类型引用：已经是 `TypeId`（好），但 `TypeId` 指向的 TypeStore 要随树成为 cone 拥有的 interned arena（B）
- supertype / companion / dispatch slot 方法：`CanonicalTextKey` → nominal/method 句柄
- assignment place：`AssignPlaceKind`/`MemberRef` 里的 `symbol_id`/`fqn` → 局部 slot / 全局 / member 句柄
- 泛型实例/模板：`stable_template_key`/`stable_instance_key` → 模板/实例句柄

### 5.4 interned 共享子结构（B，cone 拥有、按 id 引用）

- **TypeStore / TypeId**（`crates/scoopc_types`）：真正的类型 interned arena，递归 DAG，被全树用 `TypeId` 引用。**这是 B 的主体**，随树成为 cone 拥有的 arena。
- **EffectRow / EffectRowTemplate**：效果行，被签名与 site 大量引用；同样 interned。
- **符号词汇**（`Symbol`/`FunOverload`/`FunSig`/`ConstructorOverload`/`ExtensionFun/PropertySymbol`/`TypeParamSig`）：当前是 resolve 的查询词汇。重设计后，**其「声明信息」并入对应树节点（A）**，**其「按名字查」的索引身份退场为脚手架（D）**——即词汇内容入树，查询入口即弃。

### 5.5 全局 side table（C，全局有效的环境信息）

- 编译配置：`target_platform`、entry points（`runtime_entry_point`、`export_entry_points`）、`prelude_required`
- cone 归属：`SourceConeFact`（source→cone）、`file_cones`/`cone_kinds`/`StableConeKey`
- deprecation：`deprecated_types/values/funs`（`@Deprecated` 元数据）
- 产出元信息：schema 版本、produced-by 等

### 5.6 临时脚手架（D，typecheck 结束即弃，**不下传**）

- `resolve::Index.by_fqn`（字符串符号表）、`NamespacedSymbols` 的查询入口
- `TypeEnv` 的 name→symbol 映射（`by_fqn`/`type_aliases`/`supertype_defs`/`file_ctx`）及其保留的 `sources`/`files`(AST)
- `BlockScopeChecker`（`scopes`/`this_context`/`where_bound_scopes`/`init_value_members`）等词法作用域——本就完全 transient
- **修法**：§3.3 的 `HirSemanticArtifact.{index,type_env}` 下传，改为下游只拿「已解析树 + def arena」；C 类配置单独以薄 side table 传，D 类不传。

### 5.7 接口面（跨 cone 暴露子集，§2.4）

当前 `CachedConeImport`（frontend 形状）暴露的 = 公开 nominal/callable 的**签名 + 泛型模板 + 可见性**。重设计后 HIR 接口面 = 该 cone 公开声明节点的 **stable-id arena**（签名、type_params、字段/变体面、dispatch 面、可实例化模板），内部用 `Idx`、边界翻 stable id。

### 5.8 MIR 消费需求 → HIR 必须暴露（demand checklist）

调研确认 MIR 构造依赖以下，重设计的 HIR 树/接口面**必须覆盖**：

1. 完整泛型模板清单（stable 身份 + 参数名 + 签名）——驱动 monomorphization
2. 每个 call site 已观测的 type/eff 实例绑定（`CallSiteInstanceFact`/`TemplateSiteBindingFact`）
3. 虚/接口调用的 dispatch 候选（`DispatchCandidateFact`）
4. 声明元数据（nominal/callable/field/variant）足以重建类型/效果上下文
5. callable 的 effect row（declared/inferred/published）
6. extern/native/initializer 索引（预建，不重生）
7. 每个 call site 的契约（target、receiver 类型、arg binding、ABI）——**改为挂在 call 节点上**

> 关键观察：MIR 现在几乎全靠「按 (path,span)/FQN 查预建 fact 表」消费 HIR。重设计后这些**要么变成树上 call 节点的自有字段（§5.2），要么变成解析句柄（§5.3）**；`is_export_entry_point(fqn)`、`native_callable_funs.contains_key(fqn)` 这类字符串判定改为节点标志位或 C 类配置。monomorphization 用的模板**体**仍走 HIR→MIR-template lowering（HIR 树须拥有函数体表达式树），HIR 只发布稳定实例身份与参数签名。

### 5.9 待决 / 风险

- **site 节点粒度**：体内是「表达式树 + 每节点 SiteId」还是「扁平 site arena + 树用 Idx 串」？倾向后者（§2.7 物理 arena），但要确定 SiteId 与 `Idx` 的关系。
- **泛型模板的体**：HIR 拥有 desugar 后的 body 树；模板实例化在 MIR。需确认 HIR 接口面是否要携带「可被依赖 cone 实例化的模板体」还是只携带签名（取决于跨 cone 泛型实例化由谁做，呼应 §2.5）。
- **effect row 的多版本**（declared/inferred/published）如何在节点上表达，避免「只在某状态有效」的字段。
- **符号词汇并入节点 vs 保留独立 def 表**：倾向 def arena = 声明节点表（§4.6 待决 2）。

---

## 7. HIR 节点目录与 desugar 边界

> 基于 §6 盘点 + 语言特性校准（与用户确认）收窄而成。`🕳` 标记「会随 phase 变」的字段：phase 1 是洞、phase 3 裸化。HIR 是 **desugared** 的：很多表面语法在 HIR 里没有独立节点。

### 7.1 三类「糖」与摊平时机

摊平时机由「是否依赖类型」决定——纯句法的能在 phase 1 摊；依赖类型的只能等推断完成（phase 3）。

| 类别 | 例子 | 摊平时机 | 最终 HIR 里 |
|---|---|---|---|
| 类型无关 desugar | `for`、操作符（`+`/`==`…）、`try/catch/finally` | **phase 1** | 不存在 |
| 类型相关、归约到 Option | `x!!`、`x ?? e`、`x?.m()` | **phase 3**（类型已知后摊成 Match/Call） | 不存在 |
| 真·类型原语 | `x as T` | —— | 保留 `Cast` 节点（kind 已解析） |

- `for (x in seq)` → `val it = seq.iterator(); while(true){ when(it.next()){ Some(x)->body; None->break } }`
- 操作符 → `Method`/`Intrinsic` 调用；`try/catch/finally` → 固定的 `handle/on/finally`。
- `!!`/`??`/`?.` 依赖 `x` 是 `Option<T>`、且 `??` 的 e 惰性、`?.` 链短路于单点——故只能 phase 3 摊。
- `as` 不归约到 Option，是带「转换 kind」（upcast/数值/checked-downcast…）的原语。

### 7.2 desugar 的卫生性：发「已解析句柄」，不发「名字」

core lib（`Iterator`/`Iterable`/`Option`/`panic`…）默认 import、永远可用。desugar 引入的 core 引用**必须铸成 `RefHole::Resolved(core 句柄)`，跳过名字解析**，否则会被用户自定义的 `Some`/`Option` 遮蔽。

- 机制：一张 **lang-items 注册表**（core 周知条目 → `DefId`），从 core lib 接口面（§2.4）解析一次得到；core 作为默认依赖，其接口面是用户 phase 1 的输入，故 desugar 时句柄已可用。
- 覆盖：类型（`Option`/`Iterator`）、变体（`Some`/`None`）、接口方法（`Iterable.iterator`/`Iterator.next`）、`panic`/`map` 等。
- 用户写的子表达式（`seq`/`body`/`x`）仍是洞，正常解析。「`seq` 是否实现 `Iterable`」是普通 phase 2 类型检查，失败即诊断。

### 7.3 节点目录

**脊 / 容器**：`Cone`（根，拥有 packages + 各 arena + 薄 side table）→ `Package` → `Item`（声明总和类型）。

**声明节点（带 `DefId`，进 def arena，接口面暴露子集）**
- `Nominal { kind: Struct|Class|Object|Interface|Effect|Enum, type_params(+variance), eff_param?, where, fields, variants, dispatch 面, companion, supertypes🕳 }`
- `Callable { receiver?(真·可选), params(名/ty🕳/默认/vararg), return_ty🕳, effect_row🕳(declared/inferred/published), type_params, eff_param?, where, body?(真·可选) }`
- `Constructor(primary|secondary)`、`Field`、`EnumVariant(tag, payload 字段)`、`EffectOp(payload/result 签名)`、`TypeAlias(rhs🕳)`
- `TopLevelVal | TopLevelVar(storage policy, init?, init 依赖序)`
- `ExternFun | ExternGlobal | NativeCallable | ExternLibrary`、`ExtensionFun | ExtensionProperty(receiver ty🕳)`

**类型 / 效果（interned arena，按 id 引用，非脊）**
- `Type`（`TypeId`）：nominal-applied / type-param / function-type / tuple / ref-kind / …；检查期 arena 多 `InferVar`，**输出 arena 无**。
- `EffectRow / EffectRowTerm`（`RowId`）；`TypeParam`（名/variance/bounds🕳）作声明子项。

**语句节点**（住在 block 里）：`Let(pattern, ty 标注?(真·可选), init)`、`ExprStmt`、`Assignment(place🕳, 可变性/write-barrier)`、`Return | Break | Continue`（break/continue 绑定**最近 enclosing while**）。

**表达式节点**（block 是表达式、带尾值）
- `Literal`、`Path(→def/local🕳)`
- **`Call`（主力）**：`{ callee: Callee🕳, args, arg_binding }`——普通调用/方法/构造/操作符/效果操作/resume 全归它
- `MemberAccess`（仅字段/属性**读**；方法调用的成员形被 Call 吸收）
- `Construct/StructLiteral`、`WithUpdate`（函数式更新 + 路径段）
- `Closure`、`Block(尾值)`、`If`、`Match`(`when`；arms: pattern + guard? + body)、`While`(unit)
- `Tuple / 集合字面量`、`Index`、`Cast{ expr, target: TypeId, kind🕳 }`
- `Handle`：protected body + arms(`on E.op(...)`，`NonResuming|EscapeContinuation`) + finally?（`try` desugar 落点）

**`Callee` 总和类型**（语法上「成员调用形状」统一，phase 2 解析区分）
```
Fn(DefId) | Value(ExprId) | Ctor(DefId) | Variant(VariantId)
| Method{ receiver: ExprId, method: DefId, dispatch: Direct|Virtual|Interface }
| Extension(DefId) | Intrinsic(IntrinsicId)
| EffectOp{ effect:🕳, op:🕳 }        // E.action(p)：与 Method 同形状，靠 receiver 是否 effect 类型区分
| Resume{ route: CallArg|MemberReceiver }
```
重载塌成单个（未塌=歧义诊断）；虚/接口 dispatch 候选集是**真·多目标**（非洞，§5.6）。

**模式节点**：`Binding(名, ty🕳)`、`Variant(变体🕳 + 子模式)`、`Tuple|Struct`、`Literal`、`Wildcard`。

**句柄词汇（边）**：`DefId`/`LocalId`/`TypeId`/`RowId`/`FieldId`/`VariantId`；cone 内裸 `Idx`、跨 cone `(ConeId, StableId)`（§2.7）。

### 7.4 洞 / 真·可选 标注汇总
- **洞🕳**（phase 3 必塌，残留即输入错误）：所有 `Path`/`callee`/`place` 的引用、每个表达式的 `ty`、`Cast.kind`、`EffectOp` 的 effect/op、重载选择。
- **真·可选**（保留 `Option`，非洞，§5.6）：`Callable.receiver`、省略的类型标注、`finally`、虚调用候选集。

---

## 8. 实体身份与句柄设计（DefId）

地基决定。复用现有 `StableDefKey`（`{ cone, namespace, owner_path, declaration_kind, overload_signature_key }`）作稳定身份，**补一层密集本地句柄**，职责分清。

### 8.1 两级 id

```
LocalDefId(u32)   树里所有 intra-cone 引用只用它；进 def arena 的密集下标；deref 不可失败、便宜；
                  不跨 cone、不跨重编稳定。
StableDefKey      稳定身份（含 cone + namespace + 重载消歧）；用于接口面、跨 cone、增量复用、
                  lang-items、调试；可序列化。
DefPathHash       StableDefKey 的紧凑定长哈希（rustc 风格 path-hash）；跨 cone 引用与 map key 用它，
                  owner_path: String 仅留调试。
```

`DefEntry` 同时携带：`{ stable: StableDefKey, origin: Local(声明节点) | Imported(ConeId, DefPathHash), kind payload }`。
- `LocalDefId → StableDefKey`：arena 下标，便宜。
- `StableDefKey/DefPathHash → LocalDefId`：phase 1/2 建好的 map，**全管线唯一一处「按 stable key 解析」的可失败查询，且只在 load/import 边界**（现散落各处的 `by_fqn` 字符串 lookup 收敛于此）。

### 8.2 每类 arena + 定型 id

```
CallableId | NominalId | FieldId | VariantId | GlobalId
| ExternFunId | ExternGlobalId | EffectOpId | TypeAliasId      // 皆为 LocalDefId 的定型 newtype，各自一张 arena
```

resolution 后引用都已定 kind，故 `Callee::Fn(CallableId)` 在类型上放不进 `NominalId`——「引用错种类」成编译错误（§5 非法状态不可表示）。`StableDefNamespace` 映射到这些 kind；稳定空间每 cone 统一，live 下标按 kind 分（对齐 §2.7 SoA；§4.6「def arena = 声明节点表」成立，只是物理上是一族 per-kind arena）。

### 8.3 树里引用统一 `LocalDefId`，跨 cone 性藏在条目里

引用依赖 cone 的 def 时，在本地 arena 分配一个 `Imported` 代理条目（内含 `(ConeId, DefPathHash)`）。于是树里所有引用都是裸定型 `LocalDefId`，deref 永远命中、无分支；「是否跨 cone」是 deref 后看 `origin` 才知道的属性。`(ConeId, DefPathHash)` 只出现在**序列化接口面**，不进 live tree。

### 8.4 局部绑定不是 Def

`val`/参数是 **body 作用域**的 `LocalId`（进某 body 的 locals 表），不进 def arena、不稳定、不暴露。`Path` 解析到局部 → `LocalId`，到 item → 定型 `DefId`。两套空间不混。

### 8.5 子实体

field / variant / type-param 的 owner 是 nominal/callable；id 取 `(owner DefId, 子下标)` 或独立 arena + 回指 owner（二选一）；稳定身份复用现有 `StableTypeParamKey` 等。

### 8.6 phasing

- **phase 1**：为本 cone 所有声明铸 `LocalDefId` + `StableDefKey`（identity-first，支持前向引用）。
- **phase 2**：解析到依赖 def 时**懒分配** `Imported` 代理（只投影被真正引用的子集 → 闭合世界有界，§2.4）。
- **lang-items**：core 周知 `StableDefKey` 预解析成 `Imported` 的 `LocalDefId`，desugar 直接发该句柄（§7.2）。

---

## 9. 类型骨架（Rust 草案）

> **草案、示意为主**：只取代表性节点，体现 §5（phase 参数化）/§7（节点目录）/§8（DefId）的合流，不求完备、不保证可编译。

```rust
// ===== Phase：单阶段内 checking↔final 参数化（§5.5）=====
trait Phase {
    type Ref<I>;     // 名字引用：Checking=带洞，Final=裸 id
    type Attr<T>;    // 检查期计算出的属性：Checking=洞，Final=裸值（如 cast kind、dispatch 选择）
    type Ty;         // 类型：Checking=洞/推断变量，Final=具体 TypeId
    type Callee;     // 调用目标：Checking=未分类(语法 callee 子表达式)，Final=已分类已解析
}
enum Checking {}
impl Phase for Checking {
    type Ref<I> = RefHole<I>;  type Attr<T> = Option<T>;
    type Ty = TyHole;          type Callee = ExprId;
}
enum Final {}
impl Phase for Final {          // ← 全部裸化：结构上拿不出洞（§5.5）
    type Ref<I> = I;           type Attr<T> = T;
    type Ty = TypeId;          type Callee = ResolvedCallee;
}
enum RefHole<I> { Unresolved(Name), Resolved(I), Error }  // Error=已诊断毒值（§5.4）
enum TyHole     { Infer(InferVar), Unknown, Error }

// ===== 句柄：每类一张 arena + 定型 id（§8.2）=====
struct CallableId(u32); struct NominalId(u32); struct FieldId(u32);
struct VariantId(u32);  struct GlobalId(u32);  struct EffectOpId(u32);
/* …ExternFunId / ExternGlobalId / TypeAliasId / TypeParamId… */
struct LocalId(u32);    // body 作用域绑定，≠ Def（§8.4）
struct ExprId(u32); struct StmtId(u32); struct PatId(u32); struct BodyId(u32); // body 内结构下标（非洞，两 phase 一致）
enum DefOrLocal { Callable(CallableId), Global(GlobalId), Nominal(NominalId), Local(LocalId), /* … */ }

// ===== def 条目：稳定身份 + 来源（§8.1/8.3）=====
struct DefMeta { stable: StableDefKey, origin: DefOrigin }   // 与各 kind arena co-index（SoA）
enum DefOrigin { Local, Imported { cone: ConeId, hash: DefPathHash } }

// ===== cone artifact（§2.6/2.7）=====
struct ConeHir<P: Phase> {
    nominals:  Arena<NominalId,  (DefMeta, Nominal<P>)>,
    callables: Arena<CallableId, (DefMeta, Callable<P>)>,
    /* fields / variants / globals / extern* / effect_ops / type_aliases … */
    bodies:    Arena<BodyId, Body<P>>,
    types:     TypeArena<P>,    // Checking 有 InferVar，Final 无（不同类型，§5.3）
    rows:      EffectRowArena,
    meta:      ConeMeta,        // 薄 side table（§2.6）；接口面 Final 才发布（§2.4）
}

// ===== 声明节点示例（§7.3）=====
struct Callable<P: Phase> {
    receiver:    Option<P::Ty>,       // 真·可选：自由函数无 receiver（§5.6）
    params:      Vec<Param<P>>,
    return_ty:   P::Ty,               // 洞 → TypeId
    effects:     EffectRowId,
    type_params: Vec<TypeParamId>,
    body:        Option<BodyId>,      // None = abstract/extern/imported（由 origin 区分）
}

// ===== body 内：表达式（代表性变体，§7.3）=====
struct Body<P: Phase> {
    exprs: Arena<ExprId, Expr<P>>, stmts: Arena<StmtId, Stmt<P>>,
    pats:  Arena<PatId, Pat<P>>,   locals: Arena<LocalId, LocalBinding<P>>,
    root:  ExprId,                    // block 是表达式（带尾值）
}
enum Expr<P: Phase> {
    Literal(Lit),
    Path(P::Ref<DefOrLocal>),
    Call(Call<P>),
    Member { recv: ExprId, field: P::Ref<FieldId>, ty: P::Ty },
    Block  { stmts: Vec<StmtId>, tail: Option<ExprId> },
    If     { cond: ExprId, then_: ExprId, else_: Option<ExprId> },
    Match  { scrut: ExprId, arms: Vec<Arm<P>> },
    While  { cond: ExprId, body: ExprId },
    Cast   { expr: ExprId, target: P::Ty, kind: P::Attr<CastKind> }, // as：kind 洞→已解析
    Handle(Handle<P>),
    /* Closure / Tuple / Index / Construct / WithUpdate … */
}
struct Call<P: Phase> {
    callee:      P::Callee,            // Checking=ExprId（语法 callee），Final=ResolvedCallee
    args:        Vec<ExprId>,
    arg_binding: ArgBinding,
    result_ty:   P::Ty,
}
enum ResolvedCallee {                 // Final 专属：已分类、全裸、无洞
    Fn(CallableId), Value(ExprId), Ctor(NominalId), Variant(VariantId),
    Method { recv: ExprId, method: CallableId, dispatch: Dispatch },
    Extension(CallableId), Intrinsic(IntrinsicId),
    EffectOp { effect: NominalId, op: EffectOpId }, Resume { route: ResumeRoute },
}

// ===== phase 3 闸门（§5.2）：逐字段塌缩洞；残留=输入错误（已诊断则失败、不产出）=====
fn lower(t: ConeHir<Checking>) -> Result<ConeHir<Final>, CompileAborted> { /* … */ }
```

要点回顾：`Final` 的关联类型全裸（`Ref<I>=I`、`Attr<T>=T`、`Ty=TypeId`、`Callee=ResolvedCallee`）→ `ConeHir<Final>` 结构上不可能有洞（§5.5）；引用统一 `LocalDefId` 定型句柄、跨 cone 性藏在 `DefMeta.origin`（§8.3）；body 内 `ExprId/…` 是结构下标不是洞；`lower` 是唯一塌缩闸门。

---

## 10. 现状 → 目标 的差距小结

| 维度 | 现状 | 目标 |
|---|---|---|
| 输出形态 | 平行 `Vec<Fact>` / keyed map + 内嵌 ID | 单一引用闭合树（所有权即树） |
| 跨节点关系 | 字符串 key，下游 join | 带类型的已解析句柄，构造期保证有效 |
| per-site 数据 | 5 张按 `SiteId` 平行的 map | 一个 `Site` 节点拥有全部 |
| 类型/符号 | 散落 + side-table 化 | cone 拥有的 interned arena，按 id 引用 |
| side table | 几乎整个 fact 袋 | 仅 schema 版本 / 编译参数 / 产出 provenance |
| 阶段隔离 | 同套 fact 概念跨阶段复用 | 阶段专属节点类型，编译期不可混用 |
| 依赖接口 | 仅 frontend 一层，中间阶段借用 | per-stage 一等接口面（MIR 接口 = 模板 + 公开实例） |
| 校验 | 下游 `expect`/`?` + 运行期 verify 兜底 | 产出前构造期保证；verify 降级为「防漏网」的 debug 断言 |

---

## 11. 落地策略（已定）

**方案：并行另开一条路（strangler），从后往前推进。**

- **不大爆炸**：不截断现有管线从头重写（那会有一段无 oracle 的长红期 + 最后一次性集成调试，总成本最高）。改为在旁边逐阶段建新表示、用 shim 串回旧管线，**每打通一段立刻拿现有测试套件端到端验证**，bug 隔离在刚改的那一段，可增量提交。
- **方向：后 → 前。** 理由：(1) **消费者先于生产者**——先建下游，回头建上游时是照着「已存在、已被测试钉死」的具体需求生产，接口由消费者定义；(2) **把最高风险、最不稳定的设计（HIR）推到最后**，那时对全链需求了解最充分，正好对症「设计仍在演进」；(3) 先啃确定性高的后段，顺便在容易的地盘上跑通 arena/DefId/SoA 地基。
- **两端是固定锚点**：前锚点 = 源码/AST；后锚点 = LLVM IR/object（交外部，格式固定）。重构的是**中间的 IR + 阶段间接口**，两端不动。
- **codegen 几乎不重构**：它紧贴后锚点，输出已锁死；变的只是它的**输入读取**（从旧 `LirFacts`/`LateLoweredProgram` 改成走新 LIR 的 arena + 句柄），发射逻辑复用。
- **shim 方向 = 升级（旧→新）**，在前沿的「仍是旧」的上游侧。代价是它要从旧 flat facts 重建结构（正是要删的 join 逻辑）——**缓解：直接复用现有消费者的输入重建逻辑做这段 throwaway shim**，打通即弃。前沿同一时刻只需一个 shim：写一个、推进、退役。
- **首刀：`LIR ← codegen`**（契约见 §14）。先盘清 codegen 对 LIR 的真实需求（§12）→ 用它定义干净的新 LIR（arena + 定型句柄 + 引用闭合，§13）→ 抽出独立 LIR 阶段、codegen 改吃 `CodegenInput`（§14）+ 升级 shim 从旧 effect-lowering 喂入 → 跑全套 fixtures 验可执行行为。绿了，前沿上移到 effect-lowering。

> **进行中（2026-06-03，T3-04）**：首刀这个边界正被**原地**硬化——T3-04 在把 codegen 的「fact 缺失就回退重算」fallback 逐个关掉、改由 `lir_facts_builder.rs` 强制发布 fact（§3 更新、§12.3）。这是**利好且兼容**：它在现有扁平表示里把「codegen↔LIR 的契约」摸清并显式化，正好沉淀成新 LIR 的接口需求。新 LIR 不必推翻这批契约，只需把「扁平 + 字符串 key」换成「arena + 句柄」。建议：等 T3-04 fallback 关闭收口后再定型新 LIR，契约最稳。

> 通用纪律（§1.7 责任划分）：新段一律「构造期保证 + walk/deref，无可失败查询」；唯一可失败的「按 stable key 解析」收敛到 load/import 边界（§8.1）。
>
> 未决问题：阶段间/接口面共享基建（遍历/闭合校验/dump/寻址）走 **trait 抽象** 还是 **schema 驱动 codegen**——待首刀验证后定。

## 12. 首刀盘点：LLVM codegen 的输入需求（定义新 LIR 接口）

> 调研 `crates/scoopc_codegen_llvm`。codegen 输出固定，故这是「消费者先定义接口」的依据。新 LIR 必须**恰好**覆盖以下，且把现有的字符串/可失败 lookup 换成解析句柄。

### 12.1 stage 入口（`LlvmCodegenStageInput`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs:50-100`）
`lowered`(CodegenLoweringOutput)、`abi_visibility_lowered?`、`source_map`、`entry_source_id`、`entry_main_fqn?`、`opt_level`、`cached_dep_artifacts: Vec<CachedDepArtifactHandoff>`。

### 12.2 lowered program（`LateLoweredProgram`，`crates/scoopc_lir/src/effect_lowered/ir.rs`）
- `callables: Vec<LateLoweredCallable>`（plain ABI / effect-step ABI / has_control_body；codegen 在 `layout/*` 遍历）
- `step_types`、`continuation_objects`（状态机帧 / resume 帧形状）
- `class_ctor_init_bodies`、`stable_instance_keys`
- 注意：`call/abi.rs:172` 用 `root_fqn` **字符串扫描** callables 定位 ABI → 候选改句柄。

### 12.3 LirFacts（`crates/scoopc_lir_facts`；生产者 = `crates/scoopc/src/pipeline/lir_facts_builder.rs`，~3800 行）
> 2026-06-03 复核：fact 组随 T3-04「关 fallback」**显著增多**（每关一处 codegen fallback，就把对应 fact 升为强制发布）。当前顶层组：
- **summary** / **opt_pipeline** / **type_context**（新增；type_context 含 fingerprint 校验，§12.4）。
- **global_init**：`roots`(kind/ty/storage/initializer/dependencies)、`cone_init_routines`、init 顺序 DAG。
- **physical_layout**：`callable_symbols`(exported_symbol/kind/签名)、`abi_symbols`(符号→callable)、`classes`/`enums`/`class_vtables`/`interfaces`/`class_itables`（布局/分派）。
- **callables**：`LirCallableContract = Plain | EffectStep`（call sites / dynamic invokes / dispatch / 步骤状态机）。
- **dynamic_invokes**、**dispatches**（候选目标列表）、**step_types**、**continuation_objects**、**source_signatures**、**intrinsic_callables**、**source_call_sites**。
- **新增（T3-04 关 fallback 后发布）**：`class_ctor_call_sites`、`class_ctor_inits`、`reflection_call_sites`、`resume_packings`、`surface_resume_dispatches`——即原先 codegen 缺 fact 时回退重算的那些点，现已物化为 fact。
> 含义：codegen 的「按需重算」fallback 正被系统性消除 → codegen 越来越**纯消费已发布 fact**。这对新 LIR 是利好：demand 已被 T3-04 摸得更清楚（§12.8 仍成立，且更完整），新 LIR 只需把这些「扁平 + 字符串 key 的 fact」改成「arena + 句柄」即可。

### 12.4 类型（`TypeStore`/`TypeId`）
struct/enum 布局、字段偏移、vtable/itable 契约、callable ABI 签名；跨 cone 需可移植的 TypeId（现以 fingerprint 校验，`handoff.rs:313-327`）。

### 12.5 跨 cone（`CachedDepArtifactHandoff`，`src/llvm/handoff.rs:24-75`）
每个依赖 cone：`cone_id`、`stable_cone_key`、`lir`、`lir_facts`、`type_store`、`object_files`。消费者据 dep 的 `physical_layout.callable_symbols` 声明外部符号。

### 12.6 入口/根/ABI
`entry_main_fqn` + `entry_source_id` 决定入口；roots/export entry points/extern/native 决定哪些函数被发射；`callable_sources`(FQN→源位置) 供报错。

### 12.7 现状中「字符串 / 可失败」的引用点（新 LIR 要改成解析句柄）
- callable by `root_fqn`：~7 处（`reachability.rs`、`call/abi.rs:151`、`layout/lookup.rs:113`、`main/identity.rs:244`、`ordinary_callee.rs:256`…）
- global root by FQN：~3 处（`reachability.rs:80-98`）
- symbol by name：`main/identity.rs`、`gc.rs:281`
- dynamic invoke / dispatch by key：`lir_facts.{dynamic_invokes,dispatches}.get(key)`（key 内 owner_callable 仍是字符串派生）

### 12.8 新 LIR 接口需求清单（demand）
1. **预解析的 callable 句柄**——消灭所有按 `root_fqn` 的扫描/查找。
2. **完整 callable 清单 + ABI 契约**（plain / effect-step + 局部 effect 控制内嵌或索引）。
3. **dynamic-invoke / dispatch 的目标列表直接物化进 call-site 契约**（不再 key→get）。
4. **可移植 TypeId** 的类型 store（跨 cone 稳定）。
5. **global init DAG 显式定序**（roots + dependencies + cone-init 顺序，全物化）。
6. **布局事实按稳定 nominal 句柄索引**（classes/enums/interfaces）。
7. **入口选择元数据**解析到 LIR 内确定 callable。
8. **closure/合成 callable 的 owner 链**可经稳定句柄解析。
9. **依赖符号契约**经稳定 callable 句柄 + 符号名发布（替 string 索引的 `abi_symbols`）。

## 13. 新 LIR 类型草案

> **草案，示意为主**。LIR 在 monomorphization/effect-lowering 之后，**全已解析、无洞、无泛型**——故**不做 phase 参数化**，是单一具体类型（相当于 `Hir<Final>` 之后那层「全裸」表示）。

### 13.0 关键决策：LIR 自包含（own instructions）

**调研发现（2026-06-03 复核仍成立，`ir.rs:350` `LateLoweredSourceBody = crate::mir::Body`）**：当前 LIR 不自包含——`LateLoweredState` 存的是 `LateLoweredStateSlice{ block_id, start/end_statement_index }`，**指回 `mir::Body` 的语句切片**；真正的指令在 MIR 里。codegen 必须同时 walk LIR(状态机/boundary/frame) + MIR body。这是 §3.3 同款反模式（产物不自包含、reaching back）。T3-04 关 fallback 没有改变这一点（它动的是 fact 层，不是 body 表示）。

**决策（采 (b)）**：新 LIR **拥有自己的指令**（每 state 持有指令序列），codegen 只 walk LIR 一个结构。代价：首刀升级 shim 要把 MIR 语句 lift 进新 LIR body。此外 T3-04 新增的 fact 组（`class_ctor_call_sites`/`class_ctor_inits`/`reflection_call_sites`/`resume_packings`/`surface_resume_dispatches`，§12.3）也属于自包含 LIR 须拥有/索引的内容。

### 13.1 句柄与 arena（全裸定型，§2.7/§8）

```rust
struct LirCallableId(u32);  struct LirStateId(u32);  struct LirInstrId(u32);
struct LirLocalId(u32);     struct FrameSlotId(u32); struct BoundaryId(u32);
struct StepSchemaId(u32);   struct ContinuationObjectId(u32);
struct NominalLayoutId(u32);struct GlobalId(u32);
// 跨 cone 引用：(ConeId, StableLirCallableKey)（或其 hash）；live tree 内仍用 Imported 代理的 LirCallableId（§8.3）
```

### 13.2 顶层 artifact

```rust
struct LirProgram {
    callables:            Arena<LirCallableId, LirCallable>,
    step_schemas:         Arena<StepSchemaId, LirStepType>,
    continuation_objects: Arena<ContinuationObjectId, LirContinuationObject>,
    layouts:              LayoutTables,      // classes/enums/vtables/itables（§12.3 physical_layout），按 NominalLayoutId 索引
    types:                TypeStore,         // 可移植 TypeId（§12.4）
    global_init:          LirGlobalInit,     // roots + 依赖 DAG + cone-init 顺序（§12.3 全物化）
    entry:                LirEntry,          // 已解析 callable 句柄（§12.6，非 FQN）
    deps:                 Vec<LirDepInterface>, // 跨 cone 接口（§12.5）
    meta:                 LirMeta,           // schema 版本等薄 side table
}
```

### 13.3 callable + ABI

```rust
struct LirCallable {
    stable: StableLirCallableKey,            // 稳定身份（接口/跨 cone）
    origin: DefOrigin,                       // Local | Imported{cone, hash}（§8.3）
    symbol: SymbolInfo,                      // exported_symbol + linkage/kind（§12.3 callable_symbols）
    sig:    LirSignature,                    // param_tys / return_ty（TypeId）
    abi:    LirCallableAbi,
}
enum LirCallableAbi {
    Plain(LirBody),                          // 普通 CFG
    EffectStep(Box<LirEffectStep>),          // 效果状态机（invoke→Step_F）
}
```

### 13.4 body：自包含状态图（states 拥有指令）

```rust
struct LirBody {
    states: Arena<LirStateId, LirState>,
    instrs: Arena<LirInstrId, LirInstr>,     // 本 body 拥有的指令（不再 slice 回 MIR）
    locals: Arena<LirLocalId, LirLocal>,     // 类型化局部
    entry:  LirStateId,
}
struct LirState { role: StateRole, instrs: Vec<LirInstrId>, term: LirTerminator }
enum  StateRole { Entry, Segment, Resume, Complete, Cleanup, Drop }

enum LirInstr {                              // LIR 自有指令集（从 MIR lift；代表性）
    Assign { dst: LirLocalId, rv: LirRvalue },
    Call   { dst: Option<LirLocalId>, callee: LirCallee, args: Vec<LirOperand> },
    // Store/Load/FieldSet/Construct/... 视 MIR 指令集补齐
}
enum LirCallee {                            // 全解析句柄——消灭 §12.7 的 root_fqn 扫描
    Direct(LirCallableId),
    Dynamic  { boundary: BoundaryId, candidates: Vec<LirCallableId> },
    Dispatch { method_slot: u32, interface: Option<NominalLayoutId>, candidates: Vec<LirCallableId> },
    Intrinsic(IntrinsicId),
    Extern(LirCallableId),
}
enum LirOperand { Local(LirLocalId), Const(ConstValue) }
enum LirRvalue  { Use(LirOperand), Field { base: LirLocalId, field: u32 }, /* Construct/Index/... */ }

enum LirTerminator {                        // 保留 effect-lowered 控制集，句柄化
    Goto(LirStateId),
    Branch  { cond: LirLocalId, then_: LirStateId, else_: LirStateId },
    Return(LirCompletionPayload),
    Suspend { boundary: BoundaryId, resume: LirStateId, cleanup: Option<LirStateId>, drop: Option<LirStateId> },
    HandleDispatch { boundary: BoundaryId, body: LirStateId, arms: Vec<LirStateId>,
                     finally: Option<LirStateId>, exit: LirStateId },
    LocalRuntimeError { payload_ty: TypeId /* + terminal action */ },
    ResumeUnwind, Unreachable, Abandon,
}
```

### 13.5 effect-step 专属（状态机帧）

```rust
struct LirEffectStep {
    step_schema:          StepSchemaId,
    invoke_args_tuple_ty: TypeId,
    body:                 LirBody,           // 同上自包含状态图
    frame:                LirFrameSchema,    // lift 后的局部/系统槽（arena）
    boundaries:           Arena<BoundaryId, LirBoundary>,   // Call/Perform/Resume/Handle/...（操作数契约已解析）
    resume_state_map:     Vec<(BoundaryId, LirStateId)>,
    continuation_object:  ContinuationObjectId,
    resume_packings:      Vec<ResumeInterfaceId>,
}
struct LirFrameSchema { slots: Arena<FrameSlotId, LirFrameSlot> }
struct LirFrameSlot { kind: FrameSlotKind, ty: TypeId, writes: Vec<LirStateId>, reads: Vec<LirStateId> }
```

### 13.6 global init / layout / 跨 cone

```rust
struct LirGlobalInit {
    roots: Arena<GlobalId, LirGlobalRoot>,   // kind/ty/storage/initializer body
    deps:  Vec<(GlobalId, GlobalId)>,        // 依赖边（DAG，显式定序）
    cone_init_order: Vec<GlobalId>,          // 最终 init 顺序（替字符串 key 查找）
}
struct LirDepInterface {                     // §12.5：依赖 cone 的接口面
    cone: ConeId, stable_cone_key: StableConeKey,
    callables: Vec<LirImportedCallable>,     // 稳定句柄 + 符号名 + 签名 + 布局（替 string 索引 abi_symbols）
    types: TypeStore,                        // 可移植 TypeId
    object_files: Vec<PathBuf>,
}
```

### 13.7 与 demand（§12.8）对应核对

| demand | 草案落点 |
|---|---|
| 1 预解析 callable 句柄 | `LirCallee::Direct/Extern(LirCallableId)`、`entry`、`global_init` 全句柄 |
| 2 callable+ABI | `LirCallable.abi = Plain|EffectStep` |
| 3 invoke/dispatch 目标物化 | `LirCallee::Dynamic/Dispatch{candidates: Vec<LirCallableId>}`，不再 key→get |
| 4 可移植 TypeId | `LirProgram.types` + `LirDepInterface.types` |
| 5 global init DAG 定序 | `LirGlobalInit{roots, deps, cone_init_order}` |
| 6 布局按句柄索引 | `LayoutTables` 按 `NominalLayoutId` |
| 7 入口元数据 | `LirEntry`（句柄） |
| 8 closure owner 链 | `LirCallable.stable/origin` + 句柄 |
| 9 依赖符号契约 | `LirDepInterface.callables`（稳定句柄+符号名） |

> **已定**：
> - **身份**：复用 §8 的**两级纪律**——artifact 内 live 引用用密集 `LirCallableId`(arena 下标)；跨 cone/序列化/`Imported` 代理用紧凑 `LirCallableHash`（= `DefPathHash` 的 LIR 版，从 `canonical_text` 派生）；现有 `StableLirCallableKey{canonical_text, readable_path}` 的 `readable_path` 降为仅调试。注意 LIR callable 是 **monomorphized 实例身份**，与 §8 的 `StableDefKey`（定义身份）是**同纪律、不同 key 值**的两层。
> - **`LirInstr` 指令集 / `LirBoundary` 操作数契约 / `LayoutTables` 字段**：结构已定（指令从 `mir::Body` lift 且引用全句柄化；boundary 用 `BoundaryId`、layout 用 `NominalLayoutId`），具体字段在「照着当前 `mir::Body` / `LateLoweredBoundaryLowering` / `physical_layout` 起草真实类型」时逐条落，不在此凭空拍。

---

## 14. LIR↔codegen handoff 契约（结构化修补第一刀）

> 决定：放弃继续手工堵 fallback（§3 更新所示 13 轮 review 仍未收口、`dependency_gate.py` 已 2460 行黑名单），转入结构化修补。本节定义首刀的落地契约。

### 14.1 现状：handoff 纠缠且不对称（要替换的基线）

- **主 cone**：`LlvmCodegenStageInput.lowered = CodegenLoweringOutput { lowered_hir, materialized_mir, frontend_index, type_env }`（`crates/scoopc/src/frontend.rs`）——给 codegen 的是 **MIR + HIR + 前端 Index/TypeEnv，没有 LIR**。codegen 阶段内部（`llvm_codegen_stage.rs:324` `build_lir_stage_output_from_stage_outputs`）才把 MIR 降成 LIR + 建 facts + 发射。即 **codegen 把「effect-lowering + facts + emit」揉在一起，并一路 reaching back 到前端 `Index`/`TypeEnv`（§3.3 的漏直达后端）**。
- **依赖 cone**：`CachedDepArtifactHandoff { cone_id, stable_cone_key, lir, lir_facts, type_store, object_files }`（`handoff.rs`）——这才是预先降好的 LIR。

### 14.2 决定：抽出独立 LIR 阶段

把 effect-lowering 从 codegen 阶段**抽出**，成为独立的 LIR 阶段，产出自包含的 `LirArtifact`。codegen 收缩为纯粹「walk `LirArtifact` → emit LLVM」。这样主 cone 与依赖 cone 拿到的是**同一种现成产物**，handoff 才对称、自包含。（符合 §11 后→前「先固化后段接口」。）

### 14.3 目标 handoff 类型

```rust
// codegen 的全部输入：只有 LIR + 极少诊断/入口/选项
struct CodegenInput {
    program:    LirArtifact,            // 当前 cone 的自包含 LIR
    abi_shell:  Option<LirArtifact>,    // 原 abi_visibility_lowered（ABI/visibility shell）
    deps:       Vec<LirArtifact>,       // 依赖 cone——与 main【同一类型】
    entry:      Option<LirCallableId>,  // 已解析入口句柄（替 entry_source_id + entry_main_fqn 字符串）
    source_map: SourceMap,              // 仅诊断锚点
    opt_level:  OptLevel,
}

// 一个 cone 在 LIR 阶段的自包含产物（= §13 LirProgram + 链接产物）
struct LirArtifact {
    cone:         StableConeKey,
    program:      LirProgram,           // §13：callables/states/instrs/layouts/types/global_init…（LirFacts 已折叠）
    object_files: Vec<PathBuf>,         // 依赖的链接产物；main cone 为空
}
```

### 14.4 四个「拉直」动作（相对现状）

1. **删除 `CodegenLoweringOutput`**（`lowered_hir`/`materialized_mir`/`frontend_index`/`type_env`）——codegen 不再看见 HIR/MIR/Index/TypeEnv，斩断 §3.3 在后端的漏。
2. **主/依赖 cone 同类型 `LirArtifact`**：`CachedDepArtifactHandoff` 的 `lir`+`lir_facts`→`program`、`type_store`→`program.types`、`cone_id`/`stable_cone_key`→`cone`。
3. **entry 用句柄 `LirCallableId`** 替 `entry_source_id + entry_main_fqn` 字符串。
4. **`LateLoweredProgram` + `LirFacts` 合并为单一 `LirProgram`**（§13.0：facts 折叠进结构，无平行 flat facts）。

### 14.5 迁移（strangler，§11）

1. 定义 `LirArtifact` / `CodegenInput`（本节）+ 落 §13 的 `LirProgram` 真实类型。
2. 改写 codegen 消费 `CodegenInput`（walk + emit，删内部 effect-lowering 调用）。
3. **升级 shim**：把现有内部 effect-lowering 产物（`LateLoweredProgram`+`LirFacts`+`TypeStore`）打包成 `LirArtifact`；依赖侧 `CachedDepArtifactHandoff`→`LirArtifact`。复用现有 lowering，打通即弃。
4. 跑全套 fixtures 验可执行行为。绿后，把 effect-lowering 正式抽成独立 LIR 阶段（§14.2），前沿上移。
