# Scoop：Stable ID 落地计划

> 生成时间：2026-05-13  
> 设计基线：[`STABLE_ID.md`](./STABLE_ID.md)  
> 格式参考：`docs/archive/plans/PLAN.md`、`docs/archive/plans/PLAN-9.md`、`docs/archive/plans/PLAN-effect-refactor.md`  
> 本轮主题：按 `STABLE_ID.md` 的 identity 规则，把 `dump/fixture`、`.cone`/cache/RTTI、LLVM IR/object/linker visible surface 从 dense id / path / raw `Debug` / pretty text 中解耦，建立统一 stable key / mangling / dump rendering / linkage 规则。  
> 重要边界：本轮默认是“identity / naming / serialization / linkage 卫生治理”，不是语言语义重构；除 snapshot / symbol / RTTI id / JSON identity 字段 / linkage 等外部 surface 外，不接受功能漂移。  
> 行号说明：下文中的代码位置以 `STABLE_ID.md` 生成时引用的行号为准；若后续行号漂移，优先按文件路径和函数名定位。

## 0. 工作原则

- [`STABLE_ID.md`](./STABLE_ID.md) 是本轮唯一设计基线。若实现过程中要改变 stable key、mangling、hash、linkage 或“什么算对外 surface”的主张，必须先回写 `STABLE_ID.md`，再继续实现。
- 本轮不以“替换所有内部索引”为目标。`TypeId`、`ClosureId`、`StepSchemaId`、`BasicBlockId` 等 dense id 仍可继续作为内部 handle 使用；整改目标是禁止它们直接进入外部协议，见 `STABLE_ID.md` §0.2、§5.1。
- 本轮默认不得引入语言语义或运行时功能漂移。以下内容必须保持等价：
  - typecheck 结论
  - 程序运行结果
  - effect / continuation 语义
  - GC / runtime 行为
  - callable ABI 语义合同
- 本轮允许发生变化的只有 identity surface：
  - `dump-*` 文本与 fixture expect
  - `dump-rtti` 文本与 RTTI id
  - `.cone` / cache / JSON 中的 identity 字段
  - LLVM IR / object / linker 可见符号名
  - compiler-private helper 的 linkage
- `main`、`@Extern` 指定的 native symbol、以及宿主 / 平台强制要求的固定名字属于显式例外；不得被统一 mangler 误改，见 `STABLE_ID.md` §5.1.10、§7.4。
- 绝对禁止继续把 raw `Debug` 当作对外协议。`Debug` 可以继续服务内部调试，但 CLI dump、fixture、JSON、RTTI 不得直接 `format!("{:#?}")` 出去，见 `STABLE_ID.md` §3.2、§5.1.2-§5.1.3。
- 绝对禁止继续让 `sanitize_llvm_ident()`、`TypeStore::display()`、`source_path + decl_span` 这类“可读文本”承担唯一性来源。它们只能作为可读前缀或 display 文本，不能再承担 identity 责任，见 `STABLE_ID.md` §3.4.6、§3.4.7、§7.1-§7.3。
- 仓库里已经健康的 active schema 不应被重写为另一套格式。以下四类应视为基线并只做防回归审计，不做无谓 schema churn，见 `STABLE_ID.md` §3.1：
  - `crates/scoopc/src/cone/scoopir/schema.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/cone/visibility.rs`
  - `crates/scoopc/src/cone/annotations.rs`
- 实现顺序必须先打通统一 stable-id 基础设施，再处理 linker 风险最高的 external namespace 污染，再迁移导出 ABI naming，最后再重写 dump/fixture。禁止重新走“哪里漂了就在字符串上补 canonicalize”的路线，见 `STABLE_ID.md` §4、§8、§9。

## 1. 当前判断

- 当前仓库已经有两类实践并存，见 `STABLE_ID.md` §12：
  - `.cone` active schema 等部分已经基本按语义键工作。
  - `dump-*`、RTTI closure env、LLVM helper symbol 等部分仍大量把 allocator id、源码路径、pretty-printer 文本直接外化。
- 当前最大风险不是“文字不好看”，而是 compiler-private helper 以 external linkage 进入 module / object / linker namespace，见 `STABLE_ID.md` §3.4.1、§3.4.4、§3.4.5、§8.6。
- 当前 generic / overload / instance 的 exported naming 还不是 ABI-grade stable id：`stable_template_symbol_suffix()` 仍把 `source_path + decl_span` 混进 hash 输入，`instance_fqn()` 仍把 pretty type text 直接拼进名字，见 `STABLE_ID.md` §3.4.2、§8.3。
- 当前 closure 相关命名是最典型的 dense-id 外泄点之一：
  - `scoop.lambda$<ClosureId>`
  - `scoop.lambda_resume$<ClosureId>`
  - `scoop.lambda_env$<ClosureId>`
  见 `STABLE_ID.md` §3.4.3。
- 当前 effect helper 相关命名是另一类高风险外泄点：
  - `__schema<StepSchemaId>`
  - `__case<CaseTag>`
  - `__k<ContinuationSchemaId>`
  - `t<TypeId>__<pretty text>`
  见 `STABLE_ID.md` §3.4.4、§3.4.6。
- 当前 dump surface 中最典型的问题不是“个别 label 不稳定”，而是协议层直接建立在 raw `Debug` 之上：
  - HIR：`crates/scoop/src/fixtures/mod.rs:1373-1380`、`crates/scoopc/src/pipeline/hir_stage.rs:1217-1223`
  - MIR：`crates/scoop/src/fixtures/mod.rs:1402-1411`、`crates/scoopc/src/pipeline/mir_stage.rs:142-174`
  - IR：`crates/scoop/src/commands/dump_ir.rs:14-18`
  - effect facts：`crates/scoopc/src/effect_facts/dump.rs`
  - effect lowered：`crates/scoopc/src/effect_lowered/dump.rs`
- 当前 RTTI 处于“hash 手段对、输入源错”的状态：`TypeDesc.type_id` / `InterfaceDesc.interface_id` 已经使用 hash，但 closure env 仍从 `ClosureId` 取 canonical name 和 `type_id`，见 `STABLE_ID.md` §3.3。

## 2. 代码入口总表

| 主题 | 入口文件 / 位置 | 当前问题 | 目标状态 |
|---|---|---|---|
| shared hash helper | `crates/scoopc/src/rtti/mod.rs:819`、`crates/scoopc/src/rtti/type_desc.rs:1742`、`crates/scoopc/src/llvm/codegen/mod.rs:8505`、`crates/scoopc/src/itable.rs:970` | 仓库里有多份 `stable_hash64` 实现，版本前缀与输入规范不统一 | 收口到共享 `stable_id` 模块；同一 surface 使用同一前缀和截断规则 |
| stable-id 基础设施接入点 | `crates/scoopc/src/lib.rs` | 当前没有共享 stable-id 模块 | 新增 `pub mod stable_id;`，统一承载 key / encoder / mangler / label helper |
| HIR dump / fixture | `crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/commands/dump_hir.rs` | raw `Debug` 泄漏 `TypeId`、`SymbolId`、`ClosureId` | 改为稳定 renderer；本地 label 来自 stable local key |
| MIR dump / fixture | `crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/commands/dump_mir.rs` | 仍靠字符串补丁稳定化；泄漏 `bbN/lN/siteN/TypeId` | 改为稳定 renderer；去掉字符串后处理 |
| materialized IR dump | `crates/scoop/src/commands/dump_ir.rs`、`crates/scoopc/src/mir/materialize.rs` | 直接 `Debug`；实例参数显示成 `tN` | 改为 stable renderer；实例名基于 `StableInstanceKey` |
| effect facts dump | `crates/scoopc/src/effect_facts/dump.rs` | `step_schema#N`、`continuation_schema#N`、`case#N`、`bbN`、`siteN` 直接对外 | schema / case / site / block 全改用语义或 stable local label |
| effect lowered dump | `crates/scoopc/src/effect_lowered/dump.rs`、`crates/scoopc/src/effect_lowered/ir.rs` | `t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 全部 allocator-derived | 改为 semantic / stable local label |
| generic / overload naming | `crates/scoopc/src/mir/materialize.rs:8505-8647`、`crates/scoopc/src/hir/lower/util.rs:3721-3769` | `instance_fqn()`、`stable_template_symbol_suffix()` 仍依赖 path/span / pretty text | 分离 display 名和 ABI 名；hash 输入改为 canonical semantic key |
| closure symbol naming | `crates/scoopc/src/llvm/codegen/closure/mod.rs`、`crates/scoopc/src/llvm/codegen/ordinary_callee.rs`、`crates/scoopc/src/llvm/codegen/gc.rs` | 直接使用 `ClosureId` 生成函数 / resume / env / RTTI 名 | 改为 `StableClosureKey` + `PrivateSymbolMangler` |
| effect helper naming | `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` | 直接使用 `StepSchemaId` / `ContinuationSchemaId` / `CaseTag` / `TypeId` | 改为 `StableEffectSchemaKey` / `StableContinuationSchemaKey` / canonical type hash |
| top-level / object helper linkage | `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/object_init.rs` | compiler-private helper 仍默认 external | 统一 internal/private；名字走 `PrivateSymbolMangler` |
| RTTI closure env | `crates/scoopc/src/rtti/type_desc.rs:323-328` | closure env `name` / `type_id` 仍由 `ClosureId` 决定 | 改为 stable closure canonical name + shared hash helper |
| object-level 验证基线 | `crates/scoopc/src/llvm/tests.rs:2995`、`3033`、`3136` | 已有 object 解析能力，但尚未用于 stable-id 审计 | 复用 `object::File` 解析 external symbol 集，做命名 / linkage / 路径稳定性回归 |
| 现有 IR 名字断言 | `crates/scoopc/src/llvm/tests.rs:1436`、`2402`、`2434`、`3403` | 仍有旧命名形状断言，如 `scoop.lambda$0`、`__scoop_object_init__...` | 改成稳定命名 / linkage / 可见性断言，而不是锁死旧字符串 |

## 3. 顺序总览

1. P0：冻结基线与审计脚手架，先把“什么该变、什么不该变”锁住。
2. P1：建立统一 `stable_id` 基础设施，定义 key / encoder / hash / mangler / label 生成器。
3. P2：先处理 linker 风险最大的 external namespace 污染，把 compiler-private helper 从 external linkage 收回。
4. P3：迁移 closure / effect helper / transport type 的私有命名源，彻底移除 `ClosureId` / `StepSchemaId` / `ContinuationSchemaId` / `TypeId` 对 private LLVM symbol 的控制。
5. P4：迁移 generic / overload / exported callable naming，把导出 ABI surface 从 path/span/pretty text 中解耦。
6. P5：重写 dump / fixture renderer，把对外文本协议从 raw `Debug` 迁走。
7. P6：收尾 RTTI / JSON / shared hash helper，做 `.cone` 基线防回归而不是重写 schema。
8. P7：做 full audit、fixture refresh、路径稳定性验证和无功能漂移回归。

依赖说明：

- P1 必须先于 P2-P6，因为后续所有命名 / hash / renderer 都需要共享 stable-id API。
- P2 必须先于 P3-P4，因为先把 compiler-private helper 收回 internal/private，能立即降低 linker 冲突风险；即使名字暂时还不够漂亮，也先把污染面收窄。
- P3 必须先于 P6 中的 RTTI closure env 收尾，因为 RTTI 的 closure canonical name 应复用同一份 `StableClosureKey`。
- P4 必须晚于 P1，且应在 P5 之前完成；否则 dump renderer 仍会被旧 `instance_fqn()` / overload suffix 牵着走。
- P5 不得在 P1-P4 未收口前先做大规模 snapshot 刷新，否则会把“identity 基础设施未定型”包装成 fixture churn。

## 4. 分阶段计划

### P0. 冻结基线与审计脚手架

参考：[`STABLE_ID.md`](./STABLE_ID.md) §1、§3、§10、§11、§12。

目标：

- 在开始改命名规则前，先把“哪些 surface 允许变化、哪些行为不允许变化”锁死。
- 给后续每个阶段提供统一的 grep / IR / object / fixture 审计入口，避免改动过程中只能靠人工读 IR 猜是否回归。
- 明确把健康的 `.cone` active schema 视为基线，而不是重写对象。

必须实现的内容：

1. 在 `crates/scoopc/src/llvm/tests.rs` 复用现有 object 解析能力，建立 stable-id 专用的 external symbol 审计测试骨架。
   - 直接复用已有 `object::File::parse(...)` 路径，见 `crates/scoopc/src/llvm/tests.rs:2995`、`3033`、`3136`。
   - 基线样例至少覆盖：
     - source-level top-level function
     - materialized generic callable
     - closure body / closure resume / closure env
     - effect helper shell / continuation outcome helper
     - object init bridge / object init function / top-level init bridge
2. 在测试层明确声明“本轮允许变化的 surface”和“本轮不允许变化的行为”。
   - 允许变化：symbol 文本、RTTI id、dump 文本、fixture expect、linkage。
   - 不允许变化：程序语义、运行结果、typecheck、effect / continuation / GC 行为。
3. 把 `STABLE_ID.md` §11 的 grep 清单转成实现期常驻审计表；至少固定以下搜索域：
   - `crates/scoop/src`
   - `crates/scoopc/src`
   - `tests/fixtures`
4. 对 `STABLE_ID.md` §3.1 提到的健康 schema 补防回归断言，而不是准备重写：
   - `api.scoopir`
   - `PRE_SPECIALIZE.json`
   - `SYMBOL_VISIBILITY.json`
   - `ANNOTATION_CLASSES.json`
5. 复核并记录现有字符串断言中哪些是在锁定“旧命名形状”，哪些是在锁定“真实语义”。
   - 当前已知需要迁移的例子：`crates/scoopc/src/llvm/tests.rs:3403` 对 `scoop.lambda$0` / `a.main.$lambda0` 的断言。

必须遵从的约束：

- P0 不得提前重写 symbol 命名规则，不得进行大规模 fixture refresh。
- P0 只建立审计地基，不借机重写 `.cone` schema。
- P0 中新增的测试必须围绕“外部 identity 是否来自稳定语义键”建模，而不是围绕今天的旧名字拼写建模。

阶段输出：

- 一组长期可复用的 stable-id 审计入口。
- 一份固定的“允许变化 / 不允许变化”边界。
- 一组明确标记为“健康基线”的 `.cone` / JSON surface。

验证：

1. `cargo test -p scoopc`
2. 对以下搜索点执行代码审计：
   - `TypeId\(`
   - `ClosureId\(`
   - `module\.add_function\(.*None\)`
   - `stable_template_symbol_suffix`
   - `source_path.*decl_span`
   - `scoop\.lambda\$[0-9]+`
   - `__schema[0-9]+`
3. 对 `.cone` / JSON 相关测试做一次基线复核，确保没有因为 P0 的审计骨架引入格式漂移。

完成条件：

- 后续阶段可以基于稳定的审计入口推进，而不再靠 ad hoc grep 和手工读 IR。

依赖：无

### P1. 建立统一 `stable_id` 基础设施

参考：[`STABLE_ID.md`](./STABLE_ID.md) §5、§6、§7、§8.1、§9 Phase 1。

目标：

- 把 stable key、canonical encoder、hash helper、mangler、dump label 生成器集中到一处，杜绝各模块自行拼字符串和自带 hash helper。
- 为后续 P2-P6 提供单一 authoritative API，避免多份“看起来类似但输入不同”的 stable-id 逻辑继续扩散。

必须实现的内容：

1. 在 `crates/scoopc/src/lib.rs` 增加 `pub mod stable_id;`。
2. 新增 `crates/scoopc/src/stable_id.rs`，集中承载以下能力：
   - `StableConeKey`
   - `StableDefKey`
   - `StableTemplateKey`
   - `StableInstanceKey`
   - `StableClosureKey`
   - `StableCallSiteKey`
   - `StableEffectSchemaKey`
   - `StableContinuationSchemaKey`
   - `StableBoundaryKey`
   - `StableStateKey`
   - `StableFrameSlotKey`
   - canonical type / effect encoder
   - shared hash helper
   - `AbiMangler`
   - `PrivateSymbolMangler`
   - dump label 生成器
3. canonical encoder 必须落地 `STABLE_ID.md` §7.1 的规则，而不是复用当前 pretty-printer：
   - nominal：`N(pkg.Name<...>)`
   - builtin value / ref：`V(Unit)`、`R(String)`
   - type param：`P(<owner-def-key>#<index>)`
   - function：`F(recv?; params... -> ret / row)`
   - tuple：`T(...)`
   - union：`U(...)`
   - effect row：按排序去重后的 term 编码
4. hash 规则必须统一到 `STABLE_ID.md` §7.2：
   - 输入必须是 fixed canonical text
   - 必须带版本前缀，例如 `abi0:`、`priv0:`、`rtti0:`
   - 算法统一为 `SHA-256`
   - linker-visible symbol 截断为 128 bit hex
   - 只有 runtime 结构已固定要求 64 bit id 时才允许截断为 64 bit
5. 统一并删除仓库中四份 `stable_hash64` 分叉实现；当前已知位置：
   - `crates/scoopc/src/rtti/mod.rs:819`
   - `crates/scoopc/src/rtti/type_desc.rs:1742`
   - `crates/scoopc/src/llvm/codegen/mod.rs:8505`
   - `crates/scoopc/src/itable.rs:970`
6. `StableConeKey` 的最小实现必须直接来源于 cone 元信息，而不是 `ConeId`。
   - 输入来源见 `STABLE_ID.md` §6.1。
   - 当前索引期 `ConeId` 的现场编号语义见 `crates/scoopc/src/frontend.rs:378-401`；它只能继续做内部 handle，不能再外泄。
7. `StableDefKey` / `StableTemplateKey` / `StableInstanceKey` 的实现必须把“源级语义身份”和“显示文本”分开。
   - 尤其不能继续把 `TemplateKey { fqn, source_path, decl_span }` 直接升级成 exported key，见 `crates/scoopc/src/mir/materialize.rs:53-76`、`STABLE_ID.md` §6.3。
8. 为 `stable_id` 模块补齐单元测试，覆盖：
   - 相同语义输入在不同顺序下输出一致
   - 不同 surface 前缀生成的 hash 不冲突
   - `sanitize_llvm_ident()` 只影响可读前缀，不影响真正 hash 主体

必须遵从的约束：

- P1 只能建立共享基础设施，不要在每个旧调用点里边写边改出一套新的局部 helper。
- 除了把旧 hash helper 收口到共享模块，不要在 P1 大规模改 symbol / dump surface。
- 若某个调用方还没迁移，允许暂时保留旧行为，但必须新增 `stable_id` API 作为唯一后续接入点；禁止继续新增新的 `stable_*suffix` / `hash64` 私有实现。

阶段输出：

- 一个可被 LLVM / RTTI / MIR materialization / dump renderer 共用的 `stable_id` 模块。
- 一套可直接复用到 ABI / private symbol / RTTI / dump label 的 canonical 编码与 hash 规则。

验证：

1. `cargo test -p scoopc`
2. 精确搜索以下点，确认后续新增逻辑已经开始向共享模块收口：
   - `fn stable_hash64`
   - `Sha256::digest`
   - `stable_template_symbol_suffix`
3. 对 `stable_id` 模块新增单元测试，覆盖 key 构造、canonical encoding、hash 前缀和 mangling 结果。

完成条件：

- 后续 P2-P6 在实现命名 / linkage / renderer 时，无需再自行定义 hash / mangling / label 规则。

依赖：P0

### P2. 收紧 linkage，先处理 external namespace 污染

参考：[`STABLE_ID.md`](./STABLE_ID.md) §3.4.1、§3.4.4、§3.4.5、§7.4、§8.5、§8.6、§9 Phase 2。

目标：

- 在不等待所有命名改造完成的情况下，先把 compiler-private helper 从 external namespace 收回 internal/private，尽快降低 object / linker 冲突风险。
- 把“真正导出 ABI symbol”“runtime / native import”“compiler-private helper”三类函数声明路径明确分类。

必须实现的内容：

1. 审计 `crates/scoopc/src/llvm/**` 中所有 `module.add_function(name, fn_ty, None)` 调用点，按三类归档：
   - 真正导出 ABI symbol
   - runtime / native import
   - compiler-private helper
2. 优先处理 `STABLE_ID.md` 已明确指出的高风险 external helper：
   - source-level top-level function declaration：`crates/scoopc/src/llvm/codegen/mod.rs:2321-2423`
   - materialized plain callable declaration：`crates/scoopc/src/llvm/codegen/mir_body.rs:306-365`
   - effect helper declaration：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6190-6193`
   - effect helper bodies中 `module.add_function(..., None)`：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:1641-2856`、`8078`
   - object init bridge / object init function：`crates/scoopc/src/llvm/codegen/object_init.rs:101-182`
   - top-level init bridge：`crates/scoopc/src/llvm/codegen/mod.rs:3702-3718`、`8305-8306`
   - closure body declaration：`crates/scoopc/src/llvm/codegen/closure/mod.rs:156`
3. 对 compiler-private helper 显式设置 `InternalLinkage` 或 `PrivateLinkage`；不得再使用默认 external。
4. 对 runtime imports 和宿主固定符号保留 external：
   - `main`
   - `malloc`
   - `exit`
   - `runtime_abi.rs` 中声明的 runtime entry
   - `@Extern` 指定 symbol
5. 对当前已经正确设置 internal linkage 的 global 只做复核，不重复改写：
   - `ensure_struct_anchor()` / `ensure_case_tag_constant()`：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6196-6214`
   - 多个 top-level / object / global descriptor：`crates/scoopc/src/llvm/codegen/mod.rs:932`、`948`、`2853`、`3577`、`3656`
6. 若为减少重复而需要抽 helper，helper 的职责只能是“声明时分类并应用 linkage”，不能把 exported / private 业务逻辑揉进同一个难懂分支里。

必须遵从的约束：

- P2 的目标是先处理 linkage 卫生，不要求在这一阶段把旧 symbol 文本全部改成最终形态。
- 不得把真正应导出的 ABI symbol 错误 internalize。
- 不得把 runtime import / `@Extern` symbol 接到 `PrivateSymbolMangler`。

阶段输出：

- 一个显式分类的 LLVM function declaration 策略。
- compiler-private helper 不再默认处于 external namespace。

验证：

1. `cargo test -p scoopc`
2. 基于 `crates/scoopc/src/llvm/tests.rs` 的 object 解析测试，验证 compiler-private helper 不再进入 external symbol 集。
3. 精确搜索 `module\.add_function\(.*None\)`，确认剩余命中只对应真正 external / import 场景。

完成条件：

- 即使名字文本尚未全部迁移，compiler-private helper 也已不再污染外部符号空间。

依赖：P1

### P3. 迁移 closure / effect helper / transport type 的私有命名源

参考：[`STABLE_ID.md`](./STABLE_ID.md) §3.4.3、§3.4.4、§3.4.6、§6.5-§6.8、§8.5、§9 Phase 3。

目标：

- 把 private LLVM helper 命名的 identity 来源从 `ClosureId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId` 中移除。
- 对 closure、effect helper、transport type 全面接入 `PrivateSymbolMangler`。

必须实现的内容：

1. 迁移 closure 相关命名，统一改用 `StableClosureKey`：
   - closure body：`crates/scoopc/src/llvm/codegen/closure/mod.rs:79`、`156`
   - closure current callable fqn / ordinary callee plan：`crates/scoopc/src/llvm/codegen/closure/mod.rs:508`、`523`、`crates/scoopc/src/llvm/codegen/ordinary_callee.rs:365-394`
   - closure resume entry：`crates/scoopc/src/llvm/codegen/closure/mod.rs:767-770`
   - closure env type name：`crates/scoopc/src/llvm/codegen/closure/mod.rs:697-733`
   - closure env descriptor canonical name：`crates/scoopc/src/llvm/codegen/gc.rs:1722-1726`
2. `StableClosureKey` 不得再来自 `ClosureId`；必须来自：
   - owner callable 的 `StableDefKey` 或 `StableInstanceKey`
   - lambda 在 owner 内的语义路径
   见 `STABLE_ID.md` §6.5。
3. 迁移 effect helper naming，统一改用 `StableEffectSchemaKey` / `StableContinuationSchemaKey`：
   - effect callable stem：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:144-156`
   - resume helper：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:569-572`
   - dynamic / direct invoke shell：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1137-1140`
   - closure / vtable / itable carrier entry shell：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:2514-2580`
   - continuation outcome / driver helper：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:100-105`、`2766-2830`
   - task transport resume adapter：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:7947-7955`
4. 迁移 transport box / type name，移除 `TypeId` 和 pretty-printer 文本作为唯一性来源：
   - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:10885-10906`
5. 清理 closure carrier alias 兼容层，避免继续把 direct HIR closure alias 映射到 `scoop.lambda$<n>`：
   - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6980-6985`
   - `crates/scoopc/src/llvm/codegen/mod.rs:783-819`
6. 若某些 helper 仍需要可读前缀，只允许通过 `sanitize_llvm_ident()` 生成 prefix，再拼接 `PrivateSymbolMangler` 的 hash 主体；不得把 prefix 当唯一键。

必须遵从的约束：

- P3 只改 private naming source，不得借机改变 callable ABI 语义、step contract 或 runtime 协议。
- P3 中所有新 private name 都必须与 P2 的 internal/private linkage 配套；禁止出现“名字已经 stable 了，但仍 external”的半成品。
- `StableEffectSchemaKey` / `StableContinuationSchemaKey` 的输入必须来自 authoritative semantic contents，不能把 `StepSchemaId` / `ContinuationSchemaId` 包一层 hash 后继续使用，见 `STABLE_ID.md` §6.7。

阶段输出：

- closure / effect helper / transport type 的 private LLVM names 不再受 allocator 顺序支配。
- `ClosureId` / `StepSchemaId` / `ContinuationSchemaId` / `CaseTag` / `TypeId` 不再决定 private helper symbol 文本。

验证：

1. `cargo test -p scoopc`
2. 精确搜索以下字符串，确认它们不再出现在 linker-visible naming 路径：
   - `scoop\.lambda\$[0-9]+`
   - `scoop\.lambda_resume\$[0-9]+`
   - `scoop\.lambda_env\$[0-9]+`
   - `__schema[0-9]+`
   - `__k[0-9]+`
   - `t[0-9]+__`
3. 更新 `crates/scoopc/src/llvm/tests.rs` 中围绕旧字符串的 IR / symbol 断言，改成对“private name 不外泄、hash 主体稳定、linkage 正确”的断言。

完成条件：

- private LLVM helper 的名字已经脱离 dense id / pretty text，且不再进入 external namespace。

依赖：P2

### P4. 迁移 generic / overload / exported ABI naming

参考：[`STABLE_ID.md`](./STABLE_ID.md) §3.4.2、§5.2、§6.1-§6.4、§7.1-§7.4、§8.3、§8.5、§9 Phase 4。

目标：

- 把导出 ABI symbol 的 identity 来源改成 `StableConeKey + StableDefKey + StableInstanceKey`。
- 把 `instance_fqn()` 从“既是显示名又是导出名”的混合职责中拆开。
- 把 overload suffix 从 `source_path + decl_span` 驱动改成 canonical signature key 驱动。

必须实现的内容：

1. 重写 overload-aware symbol suffix 的输入来源：
   - 当前入口：`crates/scoopc/src/mir/materialize.rs:8638-8647`
   - 当前同类逻辑：`crates/scoopc/src/hir/lower/util.rs:3721-3769`
   - 目标：改为 `StableDefKey + canonical signature key`，不得再输入 `source_path` / `decl_span`。
2. 分离 `instance_fqn()` 的职责：
   - 当前实现：`crates/scoopc/src/mir/materialize.rs:8505-8525`
   - 目标：
     - 保留 display 名用于人类可读 dump / debug
     - 新增 ABI symbol 路径走 `AbiMangler`
3. 处理 `TemplateKey` / `InstanceKey` 与导出 naming 的关系：
   - `TemplateKey { fqn, source_path, decl_span }` 可继续做内部实现键，但不得直接承担导出 ABI identity，见 `crates/scoopc/src/mir/materialize.rs:53-76`。
   - `InstanceKey { template, type_args, eff_args }` 的 exported identity 必须通过 `StableInstanceKey` 派生，不能继续直接消费 `TypeId`，见 `crates/scoopc/src/mir/materialize.rs:78-134`。
4. 在 LLVM 声明路径中引入导出 ABI naming 分类：
   - source-level top-level function：`crates/scoopc/src/llvm/codegen/mod.rs:2321-2423`
   - materialized plain callable：`crates/scoopc/src/llvm/codegen/mir_body.rs:306-365`
   - effect-lowered plain callable：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1219-1265`
5. 为 `StableConeKey` 接入 cone 名称 / 版本信息；若构建层发现两个 artifact 会映射到同一个 `StableConeKey`，应在构建层直接拒绝，而不是让 codegen 继续碰撞，见 `STABLE_ID.md` §6.1。
6. 对显式例外保留固定名字：
   - `main`
   - `@Extern` 指定 symbol
   - runtime / native 约定的固定入口
7. 在实现层明确区分“语义 FQN”和“linker-visible symbol”。
   - 这是本阶段的关键约束：不要把现有 `fqn` 字段原地整体改造成 mangled symbol；下游诸多分析和 dump 仍需要源级可读 FQN。
   - 若需要新增字段或 accessor，应新增并行的 `symbol_name` / `export_symbol` 概念，而不是覆写所有 `fqn` 语义。

必须遵从的约束：

- 不得再让 `source_path`、`decl_span`、checkout 路径、`TypeStore::display()` 或 `sanitize_llvm_ident()` 成为导出 ABI 名唯一性来源。
- 不得为了兼容旧 symbol 额外保留长期 alias，除非确认已有外部消费者依赖且用户明确要求兼容层。
- P4 中的 exported naming 改造不得改变 callable 的可见性决策本身；它只改变命名来源和 namespace 规则。

阶段输出：

- 导出 ABI symbol 有统一的 cone-aware、versioned、canonical-key 驱动的命名规则。
- generic / overload / materialized instance 的 exported naming 脱离 path/span 和 pretty text。

验证：

1. `cargo test -p scoopc`
2. 在 `crates/scoopc/src/llvm/tests.rs` 新增或扩展路径稳定性测试：
   - 同一源码复制到两个不同 checkout 根目录下编译，导出的 external symbol 集必须一致。
3. 扩展 multi-cone / generic / overload 样例，确认即使两个 cone 内部 closure / site / schema 编号都从 0 开始，也不会因 exported / private name 冲突而在链接阶段碰撞。
4. 精确搜索：
   - `stable_template_symbol_suffix`
   - `source_path.*decl_span`
   - `instance_fqn\(`
   确认 exported naming 已迁离旧逻辑。

完成条件：

- 同一份输入在不同 checkout 路径下，导出的 external symbol 集合不再变化。

依赖：P1、P2、P3

### P5. 重写 dump / fixture renderer

参考：[`STABLE_ID.md`](./STABLE_ID.md) §3.2、§5.1、§5.2、§8.2、§9 Phase 5。

目标：

- 把 `dump-hir`、`dump-mir`、`dump-ir`、`dump-effect-facts`、`dump-effect-lowered` 及其 fixture 协议从 raw `Debug` 迁到稳定 renderer。
- 彻底停止“先 `Debug`，再用字符串正则补丁稳定化”的路线。

必须实现的内容：

1. HIR dump 改造：
   - `crates/scoopc/src/pipeline/hir_stage.rs:1217-1223`
   - `crates/scoop/src/fixtures/mod.rs:1373-1380`
   - `crates/scoop/src/commands/dump_hir.rs`
   - 目标：不再直接 snapshot `format!("{:#?}\n", lowered.file)`；symbol / closure / type label 改用 stable local key 或 canonical text。
2. MIR dump 改造：
   - `crates/scoopc/src/pipeline/mir_stage.rs:142-174`
   - `crates/scoop/src/fixtures/mod.rs:1402-1411`
   - `crates/scoop/src/commands/dump_mir.rs`
   - 目标：不再依赖 raw `Debug` + `TypeId` canonicalize 字符串补丁；`bb` / `local` / `site` label 改由稳定 key 派生。
3. materialized IR dump 改造：
   - `crates/scoop/src/commands/dump_ir.rs:14-18`
   - `crates/scoopc/src/mir/materialize.rs`
   - 目标：实例显示名来自 `StableInstanceKey` 派生的可读 label，而不是 `tN`。
4. effect facts dump 改造：
   - `crates/scoopc/src/effect_facts/dump.rs:77-170`、`306-374`、`665-711`、`756-765`
   - 目标：`step_schema#N`、`continuation_schema#N`、`case#N`、`bbN`、`siteN` 全部改为语义 label 或 stable local label。
5. effect lowered dump 改造：
   - `crates/scoopc/src/effect_lowered/dump.rs:116-216`、`565-665`、`1066-1145`、`1425-1718`、`1782-1936`
   - `crates/scoopc/src/effect_lowered/ir.rs:155`
   - 目标：`t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 全量从 allocator id 迁出。
6. renderer 层必须自己负责排序与 label 分配，不得再依赖内部 `IndexMap` / `Vec` 的自然遍历顺序来碰巧稳定。
7. 保留内部 `Debug` impl 作为开发期调试手段是允许的，但 CLI / fixture surface 必须彻底与其脱钩。

必须遵从的约束：

- P5 不得通过修改 `Debug` impl 的语义来“顺带修好 dump”；必须引入明确的稳定 renderer。
- 不得继续做字符串后处理式 canonicalize；renderer 必须在 identity 层上直接选择正确来源。
- dump label 可以使用 `StableLocalEntityKey` 或语义文本，但不得再直接使用 allocator 顺序，见 `STABLE_ID.md` §5.2。

阶段输出：

- 五类 dump / fixture surface 都有稳定 renderer。
- `tests/fixtures/hir/**`、`tests/fixtures/mir/**`、`tests/fixtures/mir_refactor/**` 以及相关 effect dump 期待值被一次性迁移到新的 stable protocol。

验证：

1. `cargo test -p scoopc`
2. `cargo run -p scoop -- test`
3. 精确搜索 fixture 和 dump 输出中不再包含以下 allocator-derived 文本：
   - `TypeId(`
   - `S0`
   - `C0`
   - `bb0`
   - `site0`
   - `step_schema#0`
   - `k0`
   - `ri0`
   - `ko0`
   - `st0`
   - `bd0`
   - `fs0`
4. 对 `stable_dump()` 相关测试做定向复核；当前主要入口：
   - `crates/scoopc/src/pipeline/hir_stage.rs`
   - `crates/scoopc/src/pipeline/mir_stage.rs`
   - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
   - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`

完成条件：

- 所有 active dump / fixture surface 都不再直接消费 raw `Debug` 协议。

依赖：P1、P3、P4

### P6. 收尾 RTTI / JSON / shared hash helper

参考：[`STABLE_ID.md`](./STABLE_ID.md) §3.1、§3.3、§5.2、§8.4、§9 Phase 6。

目标：

- 统一 RTTI / interface id / type id 的 hash helper 和输入规范。
- 修掉 closure env 仍由 `ClosureId` 生成 canonical name / `type_id` 的最后一处显式 dense-id 外泄。
- 对 `.cone` / cache / JSON 保持“以健康基线为主、以防回归为主”的策略，不做额外 schema churn。

必须实现的内容：

1. 迁移 `dump-rtti` closure env 的 canonical name 和 `type_id` 生成逻辑：
   - 当前入口：`crates/scoopc/src/rtti/type_desc.rs:323-328`
   - 目标：改为 `StableClosureKey` -> canonical name -> shared hash helper
2. 把 RTTI 中的本地 `stable_hash64` 迁移到共享 `stable_id` 模块：
   - `crates/scoopc/src/rtti/type_desc.rs:1742-1745`
   - `crates/scoopc/src/rtti/mod.rs:819`
3. 复核 interface / runtime match 相关 id 生成点，统一输入前缀与 helper：
   - `crates/scoopc/src/rtti/type_desc.rs`
   - `crates/scoopc/src/itable.rs`
4. 对 `.cone` / JSON active schema 做防回归审计：
   - `crates/scoopc/src/cone/scoopir/schema.rs:16-155`
   - `crates/scoopc/src/cone/pre_specialize.rs:44-84`
   - `crates/scoopc/src/cone/visibility.rs:70-100`
   - `crates/scoopc/src/cone/annotations.rs:36-64`
   - 目标不是重写这些 schema，而是确认它们仍未引入 dense id、绝对路径或机器本地 identity。
5. 若实现中需要调整 `dump-rtti` 文本协议，应仅限于 identity 字段的稳定化，不要引入新的 unrelated JSON 结构改动。

必须遵从的约束：

- P6 不做无必要的 `.cone` / JSON schema 版本升级。
- 不得把 closure env 的 `ClosureId` 简单包一层 hash 后继续使用；必须先改 canonical name 来源。
- 同一类 RTTI / interface id 必须共用同一份 shared helper；不得保留各模块自带 digest 版本。

阶段输出：

- RTTI / interface id / type id 的 hash 规则统一。
- closure env 的 `name` / `type_id` 脱离 `ClosureId`。
- `.cone` / JSON surface 进入“防回归而非重写”状态。

验证：

1. `cargo test -p scoopc`
2. 重点复核 `crates/scoopc/src/rtti/type_desc.rs` 中现有 `dump_rtti_*` 测试。
3. 精确搜索：
   - `fn stable_hash64`
   - `ClosureId`
   - `scoop.lambda_env$`
   确认 RTTI 路径不再依赖旧 closure env naming。

完成条件：

- `dump-rtti` 不再把 closure env 的 identity 绑定到 `ClosureId` 分配顺序。

依赖：P1、P3、P4

### P7. Full Audit、fixture refresh 与无功能漂移验收

参考：[`STABLE_ID.md`](./STABLE_ID.md) §10、§11、§12。

目标：

- 在所有 stable-id 路径完成后，做一次完整的“identity 已稳定、语义未漂移”收口。
- 刷新所有受影响 snapshot / fixture expect，并确认没有把行为变化伪装成文本变化。

必须实现的内容：

1. 运行并记录 `STABLE_ID.md` §11 的 grep 审计清单；对每个残余命中给出分类：
   - 合法内部实现 handle
   - 仍需整改的外部 surface 泄漏
   - 误报 / 调试代码 / 测试数据
2. 刷新并复核所有受影响的 fixture / snapshot：
   - `tests/fixtures/hir/**`
   - `tests/fixtures/mir/**`
   - `tests/fixtures/mir_refactor/**`
   - 与 effect dump / RTTI dump 对应的快照测试
3. 跑“路径稳定性”验证：同一输入在两个不同 checkout 根路径下编译，external symbol 集必须一致。
4. 跑“局部编号冲突”验证：两个 cone 即使内部 closure / site / schema 编号都从 0 开始，链接时也不应因为 helper 名字冲突而失败。
5. 跑“无功能漂移”验证：
   - HIR / MIR / effect / RTTI 相关单元测试
   - LLVM codegen / object 相关测试
   - 端到端 fixture 测试
   - 若工作树和环境允许，最终执行 `cargo test --all`
6. 清理过程中若留下过渡期 helper、双轨 name builder、旧 alias 兼容层，必须在本阶段删净。

必须遵从的约束：

- P7 中刷新 fixture 不能只看“文本变了”；必须逐项确认这些变化仅来自 identity surface，而非真实语义漂移。
- 若某个测试失败暴露出 callable ABI、runtime 行为、effect 语义变化，必须回退到对应阶段修正，而不是用新 snapshot 吞掉行为漂移。

阶段输出：

- 一组完成记录，能够明确回答：
  - 哪些 surface 发生了预期中的 identity 变化
  - 哪些语言 / runtime 行为保持不变
  - 哪些旧 helper / 命名路径已经被彻底删除

验证：

1. `cargo test -p scoopc`
2. `cargo test -p scoop_runtime`
3. `cargo run -p scoop -- test`
4. 若环境允许，`cargo test --all`
5. 对 `STABLE_ID.md` §11 的 grep 清单做最终审计。

完成条件：

- `STABLE_ID.md` §10 的验收标准全部可被明确陈述为已成立。

依赖：P1-P6

## 5. 主要风险与应对

### 5.1 `fqn` 既承担语义标签又承担 symbol 名的历史耦合

- 风险：当前 `instance_fqn()` 和若干 callable `fqn` 字段同时承担“人类可读语义名”和“linker-visible symbol 名”两种职责。若直接原地改写，极易把下游排序、owner key、dump 文本、错误信息一并打乱。
- 应对：P4 必须显式分离 display 名与 symbol 名；需要导出 ABI name 的地方新增并行 accessor 或字段，不要整仓把 `fqn` 全部替换为 mangled symbol。

### 5.2 `module.add_function(..., None)` 里混有 external import 与 private helper

- 风险：若 P2 粗暴地把所有 `None` 都改成 internal/private，可能会误伤 `main`、runtime import、`@Extern` symbol。
- 应对：先分类，再改 linkage；`runtime_abi.rs`、`main`、`malloc`、`exit`、`@Extern` 等显式例外要在分类规则里写死。

### 5.3 renderer 重写可能掩盖真实行为漂移

- 风险：P5 会引发大规模 fixture 刷新；如果没有 P0 的“允许变化 / 不允许变化”边界，很容易把语义回归误当成纯文本差异吞掉。
- 应对：先完成 P0 审计脚手架；P7 刷新 snapshot 时必须附带行为层验证，不得只看 dump 文本。

### 5.4 path-stable 目标容易被绝对路径、临时目录、pretty-printer 文本重新污染

- 风险：即便 exported name 已迁移，若还有测试或辅助代码把绝对路径、临时目录、pretty-printer 文本掺进 hash 输入，仍会出现 checkout-path 漂移。
- 应对：P1 必须先建立 canonical encoder 和版本化 hash helper；P4 / P7 必须做实际的双 checkout 路径对比，而不是只做代码审阅。

### 5.5 `.cone` active schema 容易被“顺手统一风格”带偏

- 风险：实现者看到 shared `stable_id` 模块后，可能倾向于把本已健康的 schema 也大改一遍，造成不必要的 JSON churn。
- 应对：P0 和 P6 都明确把 `api.scoopir`、`PRE_SPECIALIZE.json`、`SYMBOL_VISIBILITY.json`、`ANNOTATION_CLASSES.json` 视为基线；除发现真实 dense-id 泄漏外，不做格式改造。

## 6. 完成标准

本轮完成时，必须能够明确陈述以下结论全部成立：

1. active 外部 surface 中不再直接渲染或 hash 以下类型：
   - `TypeId`
   - `SourceId`
   - `ConeId`
   - `SymbolId`
   - `ClosureId`
   - `BasicBlockId`
   - `LocalId`
   - `SiteId`
   - `StepSchemaId`
   - `ContinuationSchemaId`
   - `CaseTag`
   - `ResumeInterfaceId`
   - `ContinuationObjectId`
   - `StateId`
   - `BoundaryId`
   - `FrameSlotId`
2. 所有只在本模块内部使用的 compiler-generated function / global 都显式使用 `InternalLinkage` 或 `PrivateLinkage`。
3. 同一份输入在不同 checkout 路径下，导出的 LLVM symbol 集合不发生变化。
4. 两个 cone 即使内部 closure / site / schema 分配号都从 0 开始，也不会在链接阶段因为内部 helper 名字碰撞。
5. `dump-hir`、`dump-mir`、`dump-ir`、`dump-effect-facts`、`dump-effect-lowered` 及其 fixtures 不再包含 raw `TypeId(`、`S0/C0`、`bb0/site0`、`step_schema#0`、`k0/ri0/ko0/st0/bd0/fs0` 这类 allocator-derived 文本。
6. `dump-rtti` 的 closure env `name` 和 `type_id` 不再依赖 `ClosureId` 分配顺序。
7. `sanitize_llvm_ident()` 只出现在“可读前缀”路径里，不再承担唯一性责任。
8. 本轮所有变更均只作用于 identity / naming / serialization / linkage surface，没有引入语言语义、运行结果、effect / continuation / GC 行为漂移。

## 7. 常驻审计清单

实现过程中应长期保留以下 grep 审计点；它们不是最终验证本身，但能快速暴露“又把内部 dense id 渗回外部 surface 了”的回归：

```text
TypeId\(
SymbolId\(
ClosureId\(
SourceId\(
ConeId\(
BasicBlockId\(
LocalId\(
SiteId\(
StepSchemaId\(
ContinuationSchemaId\(
CaseTag\(
ResumeInterfaceId\(
ContinuationObjectId\(
StateId\(
BoundaryId\(
FrameSlotId\(

module\.add_function\(.*None\)
stable_template_symbol_suffix
source_path.*decl_span
scoop\.lambda\$[0-9]+
scoop\.lambda_resume\$[0-9]+
scoop\.lambda_env\$[0-9]+
__schema[0-9]+
__k[0-9]+
t[0-9]+__
```

## 8. 结语

- 这轮整改的关键不是“把所有 `N` 改成别的字符”，而是把 identity 的 source of truth 明确化。
- dense id 可以继续作为内部实现工具存在；问题从来不是“用了数字”，而是“把 allocator 顺序当成了外部协议”。
- 真正的收口标准不是“IR 看起来更漂亮”，而是：
  - 导出 ABI name 来自统一语义键
  - compiler-private helper 统一 internal/private
  - dump / fixture 不再建立在 raw `Debug` 上
  - RTTI / JSON / symbol / object surface 都不再受 path / span / dense id / pretty text 漂移影响
- 只有按本计划的顺序把基础设施、linkage、private naming、exported naming、renderer、RTTI 和最终审计依次收口，`STABLE_ID.md` 想解决的 fixture 漂移、多 cone 冲突、closure 命名泄漏、object/linker 风险才会一起消失，而不是在不同 surface 上轮流复发。
