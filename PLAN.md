# PLAN — scoop2 分阶段编译管线：HIR/MIR/LIR archive 化

> 状态：计划（设计经评审定型，未动代码）。
> 范围：`crates/scoop2_*`（旧 `scoopc_*` 管线不动）。
> 关联文档：`HIR-REDESIGN.md`（HIR 完整性/无 panic/目标 C）、`FACT_REFACTOR.md`（旧管线
> 「扁平 fact 袋」教训与树/句柄/接口面方法论）、`NEW-HIR-FIX.md`（已完成的前置硬化）。

## 0. 动机

当前 scoop2 管线是一次性内存直通：MIR 遍历 AST + 查 `TypedHir` 侧表，LIR 直接读
`&hir`，阶段之间没有可落地的产出物边界。本计划把管线改造成**分阶段编译**：每个阶段
（HIR/MIR/LIR）的输出是 per-cone 的 archive 文件集合，下游阶段只消费上一阶段的
archive collection（+ 编译选项等全局参数），源文件在 HIR collection 生成后即可删除。
这既是架构目标（分离编译、缓存、二进制发布的地基），也是可测试性目标：每个阶段可以
独立落地、独立从上一阶段文件驱动、独立验证。

## 1. 已锁定约束（评审结论，实施中不可妥协）

- **C1 单一产出物边界**：阶段输入 = 上一阶段 archive collection + 全局参数白名单。
  MIR **只读 HIR 的产出**（过渡期：`TypedHir`；终态：HIR archive collection），
  不读 AST / 源码 / parser 输出等**任何其他上游信息**；LIR 只读 MIR 的产出，不读
  HIR。白名单：target platform、entry 选择、opt level、编译开关、project-model
  （declared deps）。白名单之外一切（interner、类型表、lang-items、span）都是
  archive 数据。
- **C2 per-cone archive + per-cone arena**：每 cone 一份 HIR/MIR/LIR archive；符号 /
  类型 / 节点 id 空间 per-cone；跨 cone 引用 = `(ConeId, 稳定 id)`；ConeId 从包名等
  稳定输入派生，**禁止** intern 顺序（现状 `resolve/index.rs:73` 即顺序分配，要改）。
- **C3 id 三层**：跨 cone 稳定 id（从名字路径 + 签名派生，含**重载消歧 key**——现状
  重载集按声明序无稳定身份，是缺口）/ cone 内 dense id（u32 arena 下标，live 引用）
  / body 内 local id（`LocalId`/`ExprId`，非 def、不外泄）。closure 用 cone 内匿名
  def id（代码体要独立 lower、sibling 提升，不能只放 scope-local 层）。
- **C4 desugar 全在 HIR**：两档时机——类型无关（for、运算符、InfixCall、index）早摊；
  类型相关（`!!`、`?.`、Elvis、f-string）在推断后摊。合成引用一律发 **lang-items 已
  解析句柄**（不走名字解析，防用户遮蔽 `Some`/`iterator`；现状 for-desugar 生成
  `__it`/`Some` 裸名走正常解析 + 优先级启发，是卫生缺口）。不摊清单：`as`/`is`、
  `handle`/`perform`、`TypeApply`、cast kind——它们是类型原语不是糖。
- **C5 诊断责任边界**：HIR 是唯一「前端拒绝」边界；MIR/LIR 零用户诊断（只有 ICE/bug
  通道）；之后唯一允许的用户错误是 **linking error 白名单**（入口缺失、符号重复、
  archive 缺员），归 driver/link 阶段。
- **C6 MIR 是加工器，不是搬运工**：MIR 完成全部泛型实例化（含 effect 行实参），产出
  无泛型；call site 携带**实例句柄**（不是「模板 + 实参」形态）+ 最终
  direct/virtual/interface 分类 + vtable/itable slot + 候选集；符号 mangling 在 MIR
  定稿；LIR 不重建分派表、不占位回填（现状 `scoop2_lir/src/lib.rs:709,718` slot 0
  占位 + step 2/8 重建回填，即「猜」的真身，要删）。
- **C7 确定性与版本**：archive 字节级确定（禁 HashMap 迭代序泄漏进序列化；element
  序一律声明序）；版本头（compiler + schema，不兼容即拒，不做迁移）；**输入集合
  指纹**（参与的 archive 稳定 key + entry + target）作为缓存失效键——MIR 的可达性
  决策与去虚化依赖全集，新增 cone 声明子类可使旧 MIR archive 的去虚化结论失效，
  指纹是唯一干净的失效机制，必须进格式第一版。
- **C8 可测试性（本批重点）**：每阶段输出落地文件；下游阶段从上一阶段落地文件产出；
  oracle = 「内存直通管线」与「分阶段落地→重载管线」输出**字节一致**；staged 模式
  下删除源文件仍可完成 MIR/LIR。
- **C9 非法状态不可表示（上游类型即契约）**：下游**不做防御性设计**——不写「如果
  上游给了 X 该怎么办」的分支；上游产出的**类型**从结构上杜绝下游看到未加工 /
  不可能信息的可能。四条落地机制：
  1. **封闭词汇表**：每阶段节点枚举只含该阶段语义的变体。HIR 的 `Expr`/`Stmt`
     变体清单**就是** desugar 完成性的证明——不含 `For`/`?.`/运算符糖/插值字符串
     等变体，MIR 的穷尽 match 里就写不出「处理 for loop」的分支。现存反例（按本
     约束删除）：MIR 为已 desugar 的 For 保留的「不应到达」防御分支
     （`scoop2_mir/src/mir/lower/stmt.rs:51-57`）、为已移除特性保留的
     `splice_field_removed` 拒绝路径。
  2. **裸类型 + 定型句柄**：输出类型无「洞」（判据 FACT_REFACTOR §5.6：`None` 在
     完全检查过的程序里是否是有意义的答案——是则真·可选保留，否则塌缩为裸类型）；
     引用一律为 arena 铸造的定型 id（`InstanceId` 等），构造时已保证存在，下游
     deref 不可失败、不用字符串比较。
  3. **唯一构造入口**：阶段输出类型只经该阶段 builder 构造（枚举变体可见性收口在
     crate 内），错误恢复的毒值只存在于 Checking 中间态（两类型分离，
     FACT_REFACTOR §5），不进入 Final 产出。
  4. **穷尽即验证**：下游对上游枚举的 match 必须穷尽且**无 `_ =>` 兜底臂**——
     Rust 穷尽性检查取代运行期校验；MIR/LIR 的 verify 降级为 debug 断言 / ICE，
     不进生产控制流。
  合法的「可失败」只剩两处：**装配/加载边界**（I/O 校验：版本、损坏、闭合性——
  即 §2 的唯一可失败解析点）与 **HIR 拒绝边界**（错误程序全有或全无，C5）。

## 2. 目标形态

```
source cone(s)
   │  hir build（parse → resolve → typecheck → HIR 构造[desugar] → 完整性 gate）
   ▼
<cone>.hirarch ──┐
                 ├─ collection.manifest（成员、指纹、版本）
   │  mir build（装配 → lower → 实例化[单态化] → 去虚化 → 分派定稿 → 无泛型 gate）
   ▼
<cone>.mirarch ──┐
                 ├─ collection.manifest
   │  lir build（布局/ABI/GC/effect-step/全局 init——纯消费 MIR archive）
   ▼
<cone>.lirarch → codegen → <cone>.o
```

- **装配器（collection assembly）**：加载各 archive → 合并 arena（符号按文本、类型按
  结构 hash-cons，`TypeStore::extend_from` 已有先例）→ 校验（传递闭包齐全、跨 cone
  FQN 冲突、可见性、版本）→ stable key → 本地 dense id。**全管线唯一可失败解析点**
  在这里；MIR/LIR 拿到的集合结构上无悬空引用。
- **archive 容器**（每 cone 每阶段一个文件）：版本头 / 输入指纹 / 符号段 / 类型段 /
  element 段 / body 段 / 元数据段（lang-items 标记、span、可见性、init 定序、可达集）。
  每层 archive **自包含**（类型/符号/span 逐层携带，LIR 不许引用回 HIR archive）。
- **CLI**：`scoop2c hir build <src...> -o <dir>`、`scoop2c mir build <manifest>`、
  `scoop2c lir build <manifest>`、dump 子命令基于 archive；`build`/`run` 保持 one-shot
  模式（内部复用同一装配代码），staged 是增量能力。
- **MIR archive 的身份体系**：实例身份（哪个 `foo<Int>`），不是定义身份（哪个
  `foo`）；每实例带 origin 指回 HIR def（provenance）。HIR archive 是定义身份体系。
  两套 key 同纪律、不同值（对齐 FACT_REFACTOR §8/§13 的分层）。

## 3. 现状基线（证据锚点，避免重复调研）

| 事实 | 位置 |
|---|---|
| MIR 遍历 AST + 侧表 | `scoop2_mir/src/mir/lower/mod.rs:40`（`lower_module(files, hir, diags)`） |
| TypedHir 携带 AST 片段 | `scoop2_hir/src/hir/mod.rs:50,67` + `facts.rs:257`（`default_exprs`/`GenericBound`/`ResolvedCallArg::Default`） |
| HIR 不拥有函数体（仅 `expr_types`+`SemanticFacts` 侧表） | `hir/mod.rs:71-79` |
| MIR 10 个用户诊断码 | `scoop2_mir/src/diagnostics.rs:22-43` |
| typecheck 已有 break/continue 检查（MIR 同名码是重复防线） | `scoop2_hir/src/typecheck/expr.rs:1152,1158` |
| where 约束校验存在，但 `explicit_type_args` 路径有缺口 | `typecheck/expr.rs:1428`（缺口见 `:1452` 注释） |
| LIR 7 子系统全部读 `&hir` | `scoop2_lir/src/lib.rs:52-62`（layout/dispatch/abi/gc/effect/global_init/body） |
| LIR slot 0 占位 + backfill join | `scoop2_lir/src/lib.rs:709,718` + step2/step8 |
| materialize 已做类型 + effect 行替换 | `scoop2_mir/src/mir/materialize/mod.rs:676,810,815` |
| InstanceKey / BackendContracts 字符串键 | `materialize/mod.rs:27-40` 起 |
| stable key 编码在 MIR 层（旧管线在 HIR 层） | `scoop2_mir/src/mir/stable_id.rs` vs 旧 `scoopc_hir/src/stable_id.rs` |
| TypeStore 结构 hash-cons + 合并机制 | `scoop2_hir/src/ty.rs:475-608` |
| for-desugar 是 typecheck 内改写 AST 的 pre-pass | `typecheck/mod.rs:146` + `expr.rs:8697+`（`SynthCx` 裸名合成） |
| f-string desugar 在 MIR | `scoop2_mir/src/mir/lower/expr.rs:1662` |
| `<Class>.$init` 合成在 MIR | `scoop2_mir/src/mir/lower/builder.rs:1076` |
| 闭包捕获由 MIR 扫 AST ident 计算 | `scoop2_mir/src/mir/lower/expr.rs:1750`（`scan_stmt_idents`） |
| ConeId intern 顺序分配 | `scoop2_hir/src/resolve/index.rs:73` |
| type_infos 与 16 张旧表并存的中间态 | `hir/mod.rs:196` 注释 |
| driver 已有 has_errors 不进 MIR 的闸门 | `scoop2c/src/main.rs:580` |
| sysroot body 双态（`lower_sysroot_bodies`） | `typecheck/mod.rs:41-67` |
| MIR 为已 desugar 的 For 保留「不应到达」防御分支 | `scoop2_mir/src/mir/lower/stmt.rs:51-57` |

## 4. 里程碑

> 顺序原则：先用 v0 过渡格式把**落地机制 + oracle 安全网**建起来（最便宜地拿到可测试
> 性），再做最大件 HIR 结构重建；每步保持 fixtures 全绿。commit 前缀沿用 `[scoop2]`，
> 建议带里程碑标记（如 `[scoop2][M2-3]`）。

### M0 设计定型与地基（小）✅ 已完成

1. 稳定 id 规范：定义身份（`StableDefKey` 语义：cone + namespace + 名字路径 + 重载
   消歧 key）与实例身份（`StableInstanceKey` 语义）两套；`canonical_type_text` +
   hash 编码从 `scoop2_mir` 上移 `scoop2_hir`（MIR 复用）。
   → `STAGE-ARCHIVES.md` §1/§2/§4；实现：`scoop2_hir/src/stable_id.rs`（上移 +
   `StableDefKey` + `overload_disambiguation_key`）、`scoop2_mir` 原路径 re-export。
2. ConeId 派生规则（包名 → 稳定 key）+ 重载消歧 key 方案（签名 canonical 文本参与，
   解决「重载集按声明序无稳定身份」缺口）。
   → `StableConeKey`（base）+ `ConeInfo.stable_key`；ConeId 仍是会话内下标，
   跨构建身份一律用 stable key（重排 ConeId 留待 M2 per-cone 分区时一并处理）。
3. archive 容器格式规范：版本头 / 指纹字段 / 段布局 / 字节稳定规则（排序遍历、无
   HashMap 序）。→ `STAGE-ARCHIVES.md` §5（字节格式细节随 M1 定稿）。
4. `scoop2_base` 增加 id / 版本 / fingerprint 类型。
   → `scoop2_base/src/stable.rs`（scope 哈希 / `StableConeKey` / `archive_schema`
   / `compiler_version` / `ArchiveFingerprintBuilder`）。

验收记录：
- 规范文档：`STAGE-ARCHIVES.md`；地基类型编译通过。
- `cargo test -p scoop2_base -p scoop2_hir -p scoop2_mir -p scoop2_syntax -p scoop2_lir
  -p scoop2c` 全绿。
- 字节稳定回归：`dump-mir tests/fixtures/mir2/arithmetic.scoop` 改动前后（stash
  往返）逐字节一致——上移未改变任何 stable key。
- 备注：`cargo test --all` 被既有问题阻塞——`scoopc_hir/src/session/mod.rs` 自
  `dd19fe9d WIP` 起语法损坏（旧管线，与本里程碑无关）；fixture runner 同因无法
  构建旧 `scoopc`，mir2 子集暂不可跑。

### M1 落地机制 v0（可测试性优先，内容不改）✅ 已完成

**目的：最快建立「阶段产出落地 + 下游从文件驱动 + oracle」的机制。** v0 是显式
transitional 的前端捆绑（per-cone 序列化「AST + TypedHir 现状」），允许包含 AST 片段
与 collection 级共享 arena 段，M2/M3 替换为真 archive 后删除（打通即弃的升级 shim，
对齐 FACT_REFACTOR §11）。

1. serde 基建：`Symbol`/`NodeId`/`Span`/`FileId`/`Interner`（Vec<String> + 索引）、
   `TypeStore`（kinds Vec 序列化 + dedup 重建）、`NodeIdTable`（dense）。
2. per-cone 分区写出：element 归属按 `Index.file_cone`；共享 arena 进 collection 级
   段（v0-only 标记）。
3. 装配器：加载 + 合并 + 校验（闭合 / 重复 / 版本）；stable key → 本地 id 唯一可
   失败点。
4. CLI：`hir build` / `mir build` / collection manifest。
5. **oracle 测试**（常驻 CI）：one-shot vs staged 的 MIR dump 字节一致；staged 下删除
   源文件重跑 MIR 成功。
6. 确定性审计：同输入两次落地 hash 一致（提前暴露 HashMap 序泄漏）。

验收记录（2026-08-15）：
- serde 基建：base（Symbol/NodeId/Span/FileId derive + Interner 手动）、syntax
  （AST 85 类型 derive）、hir（ty/hir/facts/type_info derive + `TypedHir`/`TypeStore`
  手动 serde——HashMap 字段一律按 key 排序序列化）。
- 新 crate `scoop2_archive`：`pipeline`（胶水抽取，scoop2c 复用）+ `v0`（archive
  读写 + 装配 + `run_mir_and_dump` 共用核心）。
- CLI：`scoop2c hir-build <src> -o <dir>` / `scoop2c mir-build <dir>`；端到端验证：
  hir-build → 删除源文件 → mir-build 输出与 `dump-mir` **逐字节一致**。
- oracle 测试（`scoop2_archive/tests/oracle.rs`，4 个）：one-shot vs staged 字节
  一致 ✓；删源可跑 ✓；两次落地字节一致（确定性）✓；版本头篡改被拒（C7）✓。
- **确定性审计抓到两处真实 HashMap 序泄漏并已修复**（正是本项的目的）：
  (a) `into_typed_hir` 签名表 / `build_type_infos` 的 categories 迭代序影响
  effect-row / nominal TypeId 铸造序 → 全部改为按 Symbol 排序；
  (b) `direct_subtypes` 子类型列表来自 `supertypes_iter()` HashMap 迭代 → 构造后
  排序。修复后 `dump-mir` 输出字节不变（TypeId 序不影响文本渲染）。
- 与计划的有意偏差（记录）：v0 archive 仅归档**用户文件** AST（sysroot 视作
  预构建依赖，终态是 rlib——M6）；TypedHir element 表整体进共享段，未按 cone
  切分（切分需要 FQN→cone 映射，属内容变更，留 M2 element 体系一并做）。
- 测试：scoop2 全系 282 通过 0 失败；`cargo test --all --no-fail-fast` 共 50 个
  失败全部位于旧管线 crate（scoopc/scoopc_hir/scoopc_mir/effect_facts_stage
  单元测试 + scoop p7），已核实这些 crate 零 scoop2 依赖、源码与 HEAD 一致，
  为旧管线在当前环境下的既有失败（父提交 p7 失败更多：11 vs 5）；fixture 全套
  469/1643 失败同为旧管线基线（mir2 通道走旧 `scoopc` 子进程）。

### M2 HIR 结构重建（最大件）🚧 进行中（第一刀已落地）

> 进度（2026-08-15，第十二刀）：**双路径语料一致率 26/26**（mir2 + hir 两
> 目录，全部树支持函数字节一致）。修复五类差异：(a) 分配序——receiver → 实参
> → 结果 temp（镜像 emit_call_resolution）；(b) 方法 `<this>` 参数序 + fn_ty
> 排除隐式接收者；(c) `&&`/`||` 逐语句镜像 AST 历史形态（含 terminate 目标为
> then_bb 的 quirk——字节一致优先）；(d) `!!` 改为 NotNullAssert 控制流原语
> （CondBr + panic 路径，替代错误的 Option.unwrap 糖展开）；(e) `?.` 树 builder
> 实际产出 SafeMember 节点（null 短路镜像）。翻转语料扩至 hir 目录（62 函数、
> 26 支持、26 一致）。下一步：If/When/While/Ctor/Variant 支持集扩展 → 全语料
> 一致 → 切默认删 AST 路径。
> 第十一刀（M2-5 破冰）：**双路径翻转方法论端到端验证成功**。
> 新增 `scoop2_mir::mir::lower_tree`：从 `FnTree` 构造 MIR（复用 `FnLowering`
> 机器，只有遍历不同）+ `lower_tree_fun_decl` 脚手架（签名数据从 hir 表取）。
> oracle（`flip_oracle.rs`）：**arithmetic 双路径 dump 逐字节一致（2/2）**；
> mir2 语料 13 函数树支持、8 函数字节一致（余 5 处差异已定位为下一切片
> 工作项：temp 类型序 / 方法分派元数据）。顺带统一树实参约定：**Method 的
> args 不含接收者**（recv 独立携带；糖调用同）——与 MIR final_args 构造对齐。
> 第十刀：**TreeCallee 决议载荷补全**（M2-5 数据前置）：
> TopLevel/Method 携带 `type_args`（显式优先/推断兜底——与 MIR 合并规则一致）、
> `param_types`（overload_sig/stable key）、`is_virtual`/`is_interface`（CallKind
> 分派选择）；Ctor 携带 `secondary` 标记位（暂保守 primary，element 体系后续
> 显式化）。调研结论：MIR dump 的 local/block 标签是**索引派生 hash**
>（`hash("l{i}")`）——字节一致只需分配序一致，双路径按函数迁移方案成立。
> 第九刀：**树 Call 携带最终实参列表**（默认值填充 +
> 位置排序，消费 `resolved_call_args`——M2-4 的 AST 残留清除关键步：MIR 翻转后
> 不再做 resolution / default filling）。顺带修 typecheck 缺陷：`Default` 克隆的
> 默认值表达式未定型（现于 `fill_resolved_args` 内 walk——与 MIR 调用方上下文
> 语义一致）。语料 89/89 gap-free 保持。
> 第八刀：**class `$init` 合成上移 HIR**（M2-3 核心）：
> `tree::synthesize_class_init_tree` 复刻 MIR `lower_class_init_callable` 的
> Kotlin 顺序（super 委托 → 属性参数赋值（首个初始化体前）→ property 初始化器 /
> init 块源码序交错 → 无初始化体时末尾兜底）；词汇表新增 `TreeCallee::InitCall`
> （合成的 `<Super>.$init` callable 无 Symbol，按 FQN 文本携带）。继承链 fixture
> （A/B/C.$init）生成验证 gap-free。至此 M2-5 翻转的全部树前置就绪
> （顶层 fun / 方法 / val init / $init 全覆盖，89/89 语料 gap-free）。
> 第七刀：**树覆盖扩展到全部声明位**：方法体
> （`<OwnerFqn>.<method>`，嵌套类型递归）、object 方法、顶层 val/var 初始化器；
> `this` 作为隐式绑定进树的 local 作用域（类型 = owner 声明态 nominal，取自
> `type_infos`）。语料内含方法的文件（如 interface_dispatch 6 棵树）全部
> gap-free。剩余 M2-5 翻转前置：class `$init` 合成上移（property 初始化器 +
> init 块 + super 委托链的 HIR 化）。
> 第六刀：**gap 门禁语料拓宽**（mir2 全量 + run-pass/hir/
> infer 采样，LIMIT=120，`SCOOP2_GAP_DIRS` 可覆盖）→ 89/89 可编译文件树
> gap-free。词汇表新增：`LogicalAnd/Or`（短路原语）、`InterpolatedString`
>（与 MIR Rvalue 同构——注：scoop2 MIR 的 f-string 是原语而非调用链 desugar，
> PLAN 原「desugar 至调用链」表述按 scoop2 现实修正）、`SafeMember`（`?.` 原语）、
> `StructLit`（fqn 按 MIR resolve_struct_fqn 同规则解析）、struct 解构模式、
> TypeApply 透明展开、@Unsafe/@Safe 块折叠。修复 typecheck 泄漏：`?.` 的
> member_refs 未按 Option 内层解析（derive_member_ref_opt）。已知边界：树仍只
> 覆盖顶层 Fun——方法体 / 顶层 val init / class `$init` 的树构造是 M2-5 翻转的
> 直接前置（下一刀）。
> 第五刀（M2-1 起步）：**声明层 element 表落地**
> （`scoop2_hir::hir::element`）：`ElementTable`（排序 element 列表 + 派生
> `by_fqn` 索引，索引不序列化、加载重建）；`Element` 携带 fqn/name/种类化载荷
> （Fun/Method/Ctor/EnumVariant/Global/Field/Type）/`decl_span`/`decl_file`/
> `overload_key`（复用 M0 的 `overload_disambiguation_key`）。装配在
> `into_typed_hir` 尾部（确定性排序）。顺带：顶层函数注册的 `decl_span` 从
> 恒 `0..0` 修正为 `d.name.span`（成员方法本就正确），多重载函数的树构造从
> 「单重载直取」升级为「按声明 span 精确匹配」。已知粗粒度：variant/global/
> field/type element 的 span 暂为默认值（声明位登记待后续从 collect 阶段透出）；
> String 形态 FQN 句柄化（StableDefKey）待 archive v1。
> 第四刀：**mir2 15/15 fixture 全部 typecheck 通过且树
> gap-free**（零跳过零缺口）。修复 effect_resuming 的根因链：(a)
> `effect_op_resume_type` 对用户自定义 effect 的裸名 arm 路径查不到 FQN——补包
> 前缀候选（Continuation 的 Resume 实参由此正确定型为 Int，先前的 receiver 实参
> 替换机制本身无恙）；(b) escape continuation binder（`, k ->`）的合成
> Continuation 类型从 typecheck 中间态落入新侧表 `handle_escape_binders`，树的
> Handle arm 补 `escape_cont` 消费。此前登记的「owner-param 注册重构」经查证
> **不成立**（注册层 ParamId 化是正确的），已从遗留清单撤销。
> 第三刀：**可编译 fixture 14/14 树 gap-free**。修复 3 个
> 既有 typecheck 缺陷：(a) 限定名 enum variant 构造 `Color.Red(42)`——checker 与
> derive 各补一条与裸名等价的路径（enum_when / variant_destructure 归零）；
> (b) 参数位 lambda 的类型未写 expr_types（closure 归零）；词汇表新增
> `Lambda`（参数类型按位取自函数类型，隐式 `it`）、`Destructure` 语句（模式解构
> 绑定）、`it` 注入绑定的作用域回退查找。已知遗留：effect_resuming 的
> `k.resume(41)`——owner 类型参数在签名注册时被降级为 Unit（信息丢失），正确
> 修复需重构签名注册（owner-param 上下文），与 M2-1 element 体系一并做。
> 第二刀：**mir2 可编译 fixture 11/11 树 gap-free**（新增
> when+全模式/is/as/with/true-false 字面量/Handle（non-resuming binder，类型取自
> ascription 的 expr_types；escape-continuation arm 仍 gap））。顺带修复 typecheck
> 真实缺陷：`derive_call_resolution` 的 MemberAccess 分支 `expr_ty(receiver)?` 在
> effect-op 判定前短路——`Effect.op(...)` perform 调用永远无决议（MIR 只能兜底
> UnresolvedName 且 verify 报错）；调序后 5 个 effect/cast/with fixture 的
> one-shot dump-mir 从失败恢复为正常输出。gap 普查测试升级为硬回归门（凡
> typecheck 通过的 fixture 树必须 gap-free）。
> 第一刀（2026-08-15）：`scoop2_hir::hir::tree`：desugar 后封闭
> 节点词汇表（`TreeExprKind` 无糖变体：Binary/Unary/InfixCall/`!!`/Index 构造时
> 展开为 `Call`，决议内联、类型裸非 Option）+ `build_fn_tree` builder（作用域栈、
> 未覆盖构造记 `gaps` 不静默）+ `TypedFile.trees` 接线（v0 archive 往返保真）。
> 覆盖集：字面量/Local/TopLevelVal/Call（全 callee 形态）/Member/Block/If/While/
> Tuple/ArrayLit/Annotated/LocalVal(Name)/Assign(place)/Return/Break/Continue。
> 验收：arithmetic 树 gap-free + 糖展开断言 + archive 往返（2 测试）；oracle 持续
> 绿。已知缺口（gaps 记录中）：when/lambda/handle/cast/is/TypeApply/StructLit/
> WithUpdate/f-string/`?.`/Unsafe 块/模式绑定/下标赋值/FunVal；`TypedSignature.
> decl_span` 未填充（多重载函数暂跳过构树——重载消歧缺口的又一体现）。

1. **声明层 element 体系**：吸收 `TypedHir` 16 张散表 + `type_infos` 中间态（收口
   `hir/mod.rs:196` 注释的并存状态）。种类齐全：type（class/interface/struct/enum/
   effect/object，含 field/ctor/method 子列表 + init block）、function、method、
   closure、val/var、enum variant、effect op、typealias、extension、extern/@Intrinsic。
   metadata 含 span、可见性、泛型参数、声明序（Vec，杜绝 HashMap 序）、init 定序。
   「type 是树」精确化为：element 拥有子 element 列表，类型引用一律 `TypeId`。
2. **HIR body 树**：自有 `Expr`/`Stmt`/`Pattern` 节点类型（不复用 AST）；决议内联为
   节点字段（推断类型、callee 句柄、arg binding + 默认值填充、effect row、place、
   pattern bindings、mutability）——`SemanticFacts` 六张平行侧表折叠进节点。局部
   绑定进 body 的 `LocalId` 空间。节点词汇表 = **desugar 后封闭集**（C9-1）：变体
   清单即 desugar 完成性证明，评审时逐变体核对（无 For/`?.`/运算符糖/
   InterpolatedString/SpliceField 等）。
3. **desugar 迁移**（C4）：早摊组（for——从「改写 AST pre-pass」改为 HIR 构造时展开、
   运算符/InfixCall/index→方法调用）；推断后摊组（`!!`/`?.`/Elvis/f-string——发已决议
   节点，lang-items 句柄）；上移组（`$init` 合成、super 构造链展开、闭包捕获计算）。
   lang-items 注册表：core 周知条目 → id，替代 `set_option_fqn` 系列字符串注入。
4. AST 残留清除：`default_exprs`/`ResolvedCallArg::Default`/`GenericBound` → HIR 化；
   `scoop2_hir` 的 `pub use scoop2_syntax as syntax` 从 hir 公开面收回。构造收口
   （C9-3）：HIR 输出类型经 `HirBuilder` 唯一入口构造，毒值只留在 typecheck
   中间态。
5. **MIR 输入翻转**：`lower_module(HirCollection)`；翻转时删除现存防御分支
   （For「不应到达」分支 `stmt.rs:51-57`、SpliceField 拒绝路径）与一切 `_ =>`
   兜底臂——对 HIR 封闭词汇表的穷尽 match 落空即编译错（C9-4）。验收 =
   `dump_module` 与翻转前**字节一致**（mir2 + run-pass 全绿作回归）。
6. 格式 v1：per-cone arena；lang-items 标记数据；删 v0 共享段与 AST 片段；
   completeness gate 平移到新结构（含 FrozenNodeIdTable 语义）。

验收：dump-hir 渲染新结构；oracle 绿；`cargo test --all` + fixtures 全绿。

### M3 MIR 加工收口 + MIR archive

1. 实例句柄化：call callee 从「模板 FQN + 实参」改为实例 id（**非 Option**——
   模板缺失/未实例化在类型上不可表示，C9-2）；`InstanceKey` /
   `BackendContracts` 字符串键 → 结构化 stable key。
2. mangling 定稿在 MIR（`mangle_symbol` 结果进 archive，LIR/codegen 纯读）。
3. 分派定稿：vtable/itable slot + 候选集写入 call site，为**定稿值**（非 Option、
   无占位回填——LIR 的 slot 0 + backfill 即违反 C9 的现存形态）；`BackendContracts`
   成为唯一分派表来源（LIR 侧平行重建退位）。
4. **无泛型出口 gate**（ICE 级）：archive 内无 `TypeParam`/eff 参数残留、所有 callee
   为实例句柄——泛型版 completeness gate。
5. 可达集 + 去虚化依赖进指纹（C7）。
6. MIR archive 落地 + `mir build` 子命令。

验收：mir fixtures 全绿；切换前后 LIR dump 一致（为 M4 基准）。

### M4 LIR 输入切换 + LIR archive

1. 7 子系统（layout/dispatch/abi/gc/effect/global_init/body 映射）改吃 MIR archive
   字段（`lib.rs:52-62` 的 7 处 `&hir` 逐一消除）。
2. slot 0 占位 + backfill join 删除（`lib.rs:709,718`、step2/8）。
3. LIR archive 落地（自包含：类型/符号/span 携带）。

验收：run-pass 全绿；LIR dump 一致性；`scoop2_lir` 不再依赖 `scoop2_hir`。

### M5 诊断边界收口

1. MIR 10 码处置：A 类直删（`break/continue_outside_loop`——typecheck 已有同款；
  `monomorph_error`/`monomorph_no_template`——前置：补齐 `explicit_type_args` 路径的
  where 约束校验 `expr.rs:1452` 缺口）；B 类上移（`splice_field_removed`→parser/
  typecheck 拒绝；`prelude_symbol_missing`→HIR lang-items 边界）；C 类转 ICE
  （`verify_cfg`/`verify_direct_style`/`verify_semantic` 走 bug 通道）；
  `lower_unresolved` 随 M2 翻转自然消灭。
2. linking 白名单定义 + driver 归属。
3. fixture 断言迁移：`EXPECT-ERROR-CODE: scoop::mir::*` → 对应 typecheck 码。
4. CI 审计（新脚本 `tools/audit_scoop2_stage_boundary.py`）：grep MIR/LIR 用户诊断码；
   Cargo 依赖边检查（`scoop2_mir` ⊅ `scoop2_syntax`；`scoop2_lir` ⊅ `scoop2_hir`）；
   archive 自包含检查（staged 全管线无源文件可跑，并入 oracle）；**穷尽性纪律**
   （C9-4）：下游对上游枚举的 match 无 `_ =>` 兜底臂（静态检查）；HIR 变体清单
   评审清单（无糖变体，C9-1）。

验收：审计零违规；fixtures 全绿。

### M6 远期占位（不在本批）

rlib 打包（cone 的 HIR+MIR+LIR+.o+manifest → 单文件，作 source cone 替代品做二进制
发布）；sysroot 预编译为第一个 rlib（消解 `lower_sysroot_bodies` 双态，消除每次重编
sysroot）；依赖接口面与 per-cone 独立生产（HIR 只看依赖接口面）；基于指纹的增量缓存。

## 5. 迁移安全网

- one-shot 路径全程保留为基准，staged 是增量能力；oracle（M1 建立后常驻）。
- 每个里程碑验收统一跑：`cargo test --all --all-targets`、
  `python3 tools/run_fixtures.py`、`python3 tools/spec_fixtures.py check`。
- 过渡期允许临时 `--legacy` 内部开关做回归二分，但删除点定在对应里程碑验收时，
  不留长期 fallback（对齐「无 fallback」纪律）。
- v0 → v1 序列化 churn 是预期成本：v0 的 AST 序列化投入明码标价「打通即弃」。

## 6. 风险与待决

| 风险 | 缓解 |
|---|---|
| M2 推断后 desugar 需直接构造「已决议节点」（不走名字解析），工作量大头 | lang-items 注册表先行（M0/M2-3 前置）；早摊组先走通模式 |
| 字节级确定性暴露既有 HashMap 序依赖（`member_order` 类问题在别处重演） | M1 确定性审计提前暴露，趁 v0 修 |
| MIR 输入翻转（M2-5）回归面大 | 以 `dump_module` 字节一致为硬验收 + mir2/run-pass fixtures |
| LIR effect 子系统（`prepare_effect_steps`）对 HIR 的读取深度未完全盘清 | M4 开工前补一轮定向调研（读哪些 hir 字段逐项登记） |
| `.o` 字节一致受 LLVM 版本影响 | oracle 对比限定同环境；`.o` 层只比符号表与段哈希 |
| 泛型类布局的实例化覆盖（`List<T>` 逐实例布局）未验证 | M3 无泛型 gate 顺带校验；缺口提前在 M3-1 一并处理 |
