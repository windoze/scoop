# Stable ID 设计与整改方案

> 生成时间：2026-05-12  
> 状态：设计草案  
> 范围：`dump/fixture`、`.cone`/cache/RTTI、LLVM IR/object/linker 可见符号。  
> 核心结论：编译器内部 dense index 可以继续存在，但它们不得再直接决定任何外部可见 surface 的文本、JSON、RTTI id、LLVM type/function/global 名字或 linker 可见符号。

## 0. 目标与非目标

### 0.1 目标

1. 把“同样的实体在适用场合应有同样的 id”变成明确规则，而不是靠各处 ad hoc canonicalize。
2. 消除所有外部可见 surface 对内部 dense index/id 的直接依赖。
3. 明确区分三类名字：导出 ABI 名字、模块私有 helper 名字、纯文本 dump label。
4. 停止把 raw `Debug` 输出当作协议或 golden baseline。
5. 让相同输入在不同遍历顺序、不同 intern 顺序、不同 checkout 路径下，仍能产生相同的外部 identity。

### 0.2 非目标

1. 本文不要求“源代码任意改动后 id 仍不变”。函数改名、移动、拆分 lambda 后，identity 改变是允许的。
2. 本文不要求把所有 `Vec`/`IndexMap`/`u32` 索引都替换掉。dense index 仍然是内部实现的合理工具。
3. 本文不承诺立刻发布一个长期兼容的公共 ABI，只定义当前仓库重构期需要遵守的 identity 规则。
4. 本文不禁止一切数字。像 enum 判别值、vtable/itable slot、字段顺序、handler arm ordinal 这类具有语义的序号仍然可以存在。

## 1. 什么算“外部可见”

| Surface | 具体载体 | 当前入口 | 是否必须去 dense id |
|---|---|---|---|
| 文本 dump / fixture | `dump-hir`、`dump-mir`、`dump-ir`、`dump-effect-facts`、`dump-effect-lowered`、`tests/fixtures/**` | `crates/scoop/src/commands/*`、`crates/scoop/src/fixtures/mod.rs` | 必须 |
| 序列化格式 | `api.scoopir`、`PRE_SPECIALIZE.json`、`SYMBOL_VISIBILITY.json`、`ANNOTATION_CLASSES.json`、`dump-rtti` JSON | `crates/scoopc/src/cone/**`、`crates/scoop/src/commands/dump_rtti.rs` | 必须 |
| LLVM/module/object/linker | LLVM module 中的 function/global/type 名字、进入 object 符号表的名字、通过函数指针/descriptor/runtime lookup 间接可观测的名字 | `crates/scoopc/src/llvm/**` | 必须 |
| 诊断与 ICE 文本 | `miette!`、frontend/codegen 错误、debug 日志 | `crates/scoopc/src/**` | 应尽量去 dense id，但优先级低于前三类 |

本文里的“对外”默认覆盖上表前三类。尤其是 LLVM/object/linker surface，不能因为“这是编译器自己合成的 helper”就不算对外；只要它进入了模块全局命名空间或 object 符号表，就必须按外部 identity 面对。

## 2. 当前仓库里的 dense id 清单

### 2.1 明确属于“局部实现索引”的 id

| 类型 | 定义位置 | 设计作用域 | 备注 |
|---|---|---|---|
| `TypeId` | `crates/scoopc/src/ty/mod.rs:17-29` | 单个 `TypeStore` | 只是 intern 表索引，不是外部类型身份。 |
| `SourceId` | `crates/scoopc/src/source.rs:93-106` | 单个 `SourceMap` | 注释已经明确“只在当前 `SourceMap` 实例内有效”。 |
| `ConeId` | `crates/scoopc/src/resolve/mod.rs:193-208` | 单次 `Index` 构建 | `frontend` 里当前是 `0=sysroot, 1=consumer, 2..=deps` 的现场编号，见 `crates/scoopc/src/frontend.rs:378-401`。 |
| `SymbolId` | `crates/scoopc/src/hir/mod.rs:52-68` | 单次 HIR lowering | 当前注释称“稳定标识”，但只在单次 lowering 的 HIR 中稳定。 |
| `ClosureId` | `crates/scoopc/src/hir/mod.rs:76-93` | 单次 HIR lowering | 当前注释也明确只在 HIR/dump 场景里稳定。 |
| `BasicBlockId` | `crates/scoopc/src/mir/mod.rs:1465-1487` | 单个 MIR `Body` | `Debug` 直接打印 `bbN`。 |
| `LocalId` | `crates/scoopc/src/mir/mod.rs:1489-1507` | 单个 MIR `Body` | `Debug` 直接打印 `lN`。 |
| `SiteId` | `crates/scoopc/src/mir/mod.rs:1509-1533` | 单个 MIR `Body` | `Debug` 直接打印 `siteN`。 |
| `StepSchemaId` | `crates/scoopc/src/effect_facts/schema.rs:4-16` | 单个 `MaterializedEffectFacts` | 当前名字里写着 “stable identity”，但稳定范围只是 effect facts 实例内部。 |
| `ContinuationSchemaId` | `crates/scoopc/src/effect_facts/schema.rs:18-30` | 单个 `MaterializedEffectFacts` | 同上。 |
| `CaseTag` | `crates/scoopc/src/effect_facts/schema.rs:32-44` | 单个 `StepSchema` 内部 | 语义上只在所属 schema 内唯一。 |
| `ResumeInterfaceId` | `crates/scoopc/src/effect_lowered/ir.rs:2132-2144` | 单个 `LateLoweredProgram` | 当前对外 dump 打印为 `riN`。 |
| `ContinuationObjectId` | `crates/scoopc/src/effect_lowered/ir.rs:2246-2256` | 单个 `LateLoweredProgram` | 当前对外 dump 打印为 `koN`。 |
| `StateId` | `crates/scoopc/src/effect_lowered/ir.rs:2514-2526` | 单个 callable version | 当前对外 dump 打印为 `stN`。 |
| `BoundaryId` | `crates/scoopc/src/effect_lowered/ir.rs:2528-2540` | 单个 callable version | 当前对外 dump 打印为 `bdN`。 |
| `FrameSlotId` | `crates/scoopc/src/effect_lowered/ir.rs:2542-2553` | 单个 frame schema | 当前对外 dump 打印为 `fsN` / `slotN`。 |

上表这些 id 的共同点是：它们都适合作为内部数组索引或 pass-local handle，但不适合作为外部协议。

### 2.2 当前仓库里已经存在的“更像语义键”的结构

| 结构 | 定义位置 | 价值 | 问题 |
|---|---|---|---|
| `CallSite { source_path, span }` | `crates/scoopc/src/hir/mod.rs:914-933` | 已经是跨文件 side table 的稳定位置键 | 适合 dump/local label，不适合 ABI/exported symbol。 |
| `TemplateKey { fqn, source_path, decl_span }` | `crates/scoopc/src/mir/materialize.rs:53-76` | 比纯数字 id 更接近“声明身份” | 仍含 `source_path + decl_span`，不能直接当导出 ABI key。 |
| `InstanceKey { template, type_args, eff_args }` | `crates/scoopc/src/mir/materialize.rs:78-134` | 已经是实例语义的好骨架 | 当前 `TemplateKey` 仍 path/span 绑定；`type_args` 仍是 `TypeId`。 |
| `.cone` schema 中的 `fqn`/`type_args` | `crates/scoopc/src/cone/scoopir/schema.rs:16-155`、`crates/scoopc/src/cone/pre_specialize.rs:44-84` | 当前 active JSON 基本已经是语义键 | 这些 schema 是好的基线，应尽量复用其思路。 |

## 3. 当前外部 surface 审计

### 3.1 当前已经比较健康的序列化 surface

以下 active schema 当前没有发现明显的 dense id 泄漏，应该视为基线而不是重写对象：

1. `api.scoopir` / `ScoopIrFile`：`crates/scoopc/src/cone/scoopir/schema.rs:16-155`
2. `PRE_SPECIALIZE.json`：`crates/scoopc/src/cone/pre_specialize.rs:44-84`
3. `SYMBOL_VISIBILITY.json`：`crates/scoopc/src/cone/visibility.rs:70-100`
4. `ANNOTATION_CLASSES.json`：`crates/scoopc/src/cone/annotations.rs:36-64`

这些格式主要用 `fqn`、`type_args`、`visibility` 之类语义字段，说明仓库已经有一部分地方在按正确方向工作。

### 3.2 `dump-*` / fixture 仍然直接泄漏内部 dense id

#### 3.2.1 HIR

1. `hir_fixture()` 仍然直接 snapshot `format!("{:#?}\n", lowered.file)`：`crates/scoop/src/fixtures/mod.rs:1373-1380`
2. `TypedHirStageOutput::stable_dump()` 先打印 `format!("{:#?}\n", self.hir_file())`：`crates/scoopc/src/pipeline/hir_stage.rs:1217-1223`
3. `tests/fixtures/hir/**` 当前广泛包含 `TypeId(...)`、`S0`、`C0` 这类值。

这说明 HIR 目前仍在把 raw `Debug` 当作对外协议。

#### 3.2.2 MIR

1. 旧 `mir_fixture()` 仍然直接 snapshot `format!("{:#?}\n", lowered.file)`：`crates/scoop/src/fixtures/mod.rs:1402-1411`
2. `MirStageOutput::stable_dump()` 仍然是 raw `Debug`，只额外做了 `TypeId` canonicalize 和 path normalization：`crates/scoopc/src/pipeline/mir_stage.rs:142-174`
3. `BasicBlockId` / `LocalId` / `SiteId` 的 `Debug` 输出就是 `bbN` / `lN` / `siteN`：`crates/scoopc/src/mir/mod.rs:1483-1533`

当前 `dump-mir` 的“稳定化”本质上还是字符串补丁，不是 identity 层的治理。

#### 3.2.3 `dump-ir`

1. `dump-ir` 直接 `format!("{materialized:#?}")`：`crates/scoop/src/commands/dump_ir.rs:14-18`
2. `MaterializedMir` 的 `Debug` 会把实例类型参数渲染成 `tN`：`crates/scoopc/src/mir/materialize.rs:96-134`

这里把 materialized instance 的外部视图直接绑定到了 `TypeId`。

#### 3.2.4 `dump-effect-facts`

`effect_facts::dump` 已经是自定义 renderer，但仍然把内部 id 直接暴露给外部：

1. `step_schema#N`：`crates/scoopc/src/effect_facts/dump.rs:77-123`、`756-758`
2. `continuation_schema#N`：`crates/scoopc/src/effect_facts/dump.rs:138-170`、`760-762`
3. `case#N`：`crates/scoopc/src/effect_facts/dump.rs:665-711`、`764-765`
4. `bbN` / `siteN`：`crates/scoopc/src/effect_facts/dump.rs:306-374`

#### 3.2.5 `dump-effect-lowered`

`effect_lowered::dump` 现在几乎把整套内部 allocator id 都做成了外部文本协议：

1. `tN`：`crates/scoopc/src/effect_lowered/dump.rs:1921-1936`
2. `sN` / `kN` / `cN`：`crates/scoopc/src/effect_lowered/dump.rs:116-165`、`167-216`、`1901-1919`
3. `riN` / `koN`：`crates/scoopc/src/effect_lowered/dump.rs:167-216`、`1573-1585`
4. `stN` / `bdN` / `fsN` / `slotN`：`crates/scoopc/src/effect_lowered/dump.rs:640-665`、`1066-1145`、`1782-1787`
5. `localN` / `bbN` / `siteN`：`crates/scoopc/src/effect_lowered/dump.rs:565-638`、`1425-1455`、`1614-1718`

### 3.3 RTTI 处于“部分正确、部分错误”状态

`dump-rtti` 不是 fixture，但它是正式的对外调试 surface。

1. `TypeDesc.type_id` 和 `InterfaceDesc.interface_id` 当前走 `stable_hash64(name)`，方向是对的：
   `crates/scoopc/src/rtti/type_desc.rs:61-66`
   `crates/scoopc/src/rtti/type_desc.rs:155-159`
   `crates/scoopc/src/rtti/type_desc.rs:1742-1745`
2. 但 closure env 例外。它的 canonical name 和 `type_id` 都来自 `ClosureId`：
   `crates/scoopc/src/rtti/type_desc.rs:323-328`

也就是说，RTTI 这里已经用了 hash，但 hash 的输入仍然是内部 dense id。

### 3.4 LLVM IR / object / linker surface 仍然暴露 dense id、路径、pretty text

#### 3.4.1 顶层函数和 materialized callable 目前默认处在 external namespace

1. source-level 顶层函数：`declare_top_level_fun_with_symbol()` 对非 `@Extern` 函数使用 `module.add_function(llvm_name, fn_ty, None)`；`None` 在这里不是“内部”，而是默认 external：
   `crates/scoopc/src/llvm/codegen/mod.rs:2321-2423`
2. materialized plain MIR callable：`declare_materialized_mir_plain_fun_with_symbol()` 同样用 `module.add_function(llvm_name, fn_ty, None)`：
   `crates/scoopc/src/llvm/codegen/mir_body.rs:306-365`
3. effect-lowered plain callable 最终也会把 symbol name 写回 `RefactorPlainCallableLayout`，并默认进入 module 全局命名空间：
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1219-1265`

这意味着“名字怎么生成”不是纯粹调试问题，而是真正的 linker surface 问题。

#### 3.4.2 generic instance / overload 名字当前不是 ABI-grade stable id

1. `instance_fqn()` 当前把实例名字拼成 `fqn::<type args, eff args>{symbol_suffix}`：
   `crates/scoopc/src/mir/materialize.rs:8505-8525`
2. 同名 overload 的后缀来自 `stable_template_symbol_suffix()`，但它的 hash 输入包含：
   `template.source_path`
   `template.decl_span`
   `signature_key`

   见：
   `crates/scoopc/src/mir/materialize.rs:8638-8647`
   `crates/scoopc/src/hir/lower/util.rs:3721-3769`

3. 这意味着相同源码在不同 checkout 路径下，导出的实例/overload 符号就可能不同。

#### 3.4.3 closure 相关符号直接由 `ClosureId` 决定

1. closure 函数本体：`scoop.lambda$<ClosureId>`
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:79`
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:156`
2. closure current callable fqn / ordinary callee plan 也复用了同样的名字：
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:508`
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:523`
   `crates/scoopc/src/llvm/codegen/ordinary_callee.rs:365-394`
3. closure resume entry：`scoop.lambda_resume$<ClosureId>`
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:767-770`
4. closure env type name：`scoop.lambda_env$<ClosureId>`
   `crates/scoopc/src/llvm/codegen/closure/mod.rs:697-733`
5. closure env descriptor canonical name：同样是 `scoop.lambda_env$<ClosureId>`
   `crates/scoopc/src/llvm/codegen/gc.rs:1722-1726`

这些名字不只是 dump 可见，而且已经进入 LLVM module 的全局命名空间和 RTTI。

#### 3.4.4 effect helper 符号大量由 `StepSchemaId` / `ContinuationSchemaId` / `CaseTag` 驱动

1. effect callable stem 当前用 `sanitize_llvm_ident(root_fqn)`，若一个 `root_fqn` 对应多个 callable version，则追加 `__schema<StepSchemaId>`：
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:144-156`
2. resume helper：`__scoop_refactor_resume__{stem}__case{CaseTag}`
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:569-572`
3. dynamic/direct invoke shell：`__scoop_refactor_dynamic_invoke__{stem}` / `__scoop_refactor_direct_invoke__{stem}`
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1137-1140`
4. closure/vtable/itable carrier entry shell：
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:2514-2580`
5. continuation outcome/driver helper：名字直接含 `k{ContinuationSchemaId}`
   `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:100-105`
   `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:2766-2771`
   `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:2825-2830`
6. task transport resume adapter：名字直接含 `s{StepSchemaId}`
   `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:7947-7955`
7. 这些 helper 壳函数大多通过 `ensure_declared_function()` 或直接 `module.add_function(..., None)` 创建，没有再改成 internal/private：
   `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6190-6193`

这意味着 effect 内部 schema id 今天已经实打实地成为 object/linker 层面的名字来源。

#### 3.4.5 object/top-level init bridge 虽然不是用户 ABI，但也在外部 namespace 里

1. object init bridge：
   `__scoop_refactor_hidden_object_init_bridge__{object_fqn}`
   `crates/scoopc/src/llvm/codegen/object_init.rs:101-117`
   `crates/scoopc/src/llvm/codegen/object_init.rs:488-489`
2. object init function：
   `__scoop_object_init__{object_fqn}`
   `crates/scoopc/src/llvm/codegen/object_init.rs:165-182`
   `crates/scoopc/src/llvm/codegen/object_init.rs:484-485`
3. top-level immutable init bridge：
   `__scoop_refactor_hidden_top_level_init_bridge__{value_fqn}`
   `crates/scoopc/src/llvm/codegen/mod.rs:3702-3718`
   `crates/scoopc/src/llvm/codegen/mod.rs:8305-8306`

这些名字虽然没有用 dense id，但它们是 compiler-private helper，不应该默认处在 external linkage 的全局符号空间里。

#### 3.4.6 `TypeId` 和 pretty-printer 文本也会进入 LLVM-visible 名字

effect transport box 的布局名当前是：

`t<TypeId>__<sanitize(types.display(source_ty))>`

见：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:10885-10906`

这同时暴露了：

1. 内部 `TypeId`
2. `TypeStore::display()` 的 pretty-printer 文本

#### 3.4.7 `sanitize_llvm_ident()` 只能做转义，不能做 identity

`sanitize_llvm_ident()` 只是把非 `[A-Za-z0-9_]` 变成 `_`：

`crates/scoopc/src/llvm/codegen/mod.rs:8385-8394`

它不能保证不同语义键不会碰撞，因此只能作为可读前缀，不能作为唯一性来源。

### 3.5 不是主要 linker 风险的部分

以下名字虽然也用了数字，但它们主要是 LLVM IR 内部 label 或临时 SSA name，不直接进入 object 符号表，因此不是本文的第一优先级：

1. `append_basic_block(function, "mir.bb{idx}")`：`crates/scoopc/src/llvm/codegen/mir_body.rs:429`
2. `append_basic_block(function, "plain.bb{idx}")`：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:828`
3. 各类 `struct_field_{idx}`、`tmp_name_{idx}` 这类 IR builder 局部名字

这些 label 仍然可以在后续做清理，但它们不是 object/linker 冲突的主因。

## 4. 根因总结

1. 很多对外 surface 直接把 raw `Debug` 当成协议。
2. 仓库里缺少一层统一的 stable key / stable mangling 基础设施，所以各阶段都就地拿 allocator id 顶上。
3. 很多 compiler-private LLVM helper 用了默认 external linkage，而不是 internal/private linkage。
4. 当前“稳定 hash”有不少是拿 path/span/dense id 做输入，属于“包装后的不稳定”。
5. `TypeStore::display()`、`sanitize_llvm_ident()` 这类人类可读文本被拿来承担 identity 责任。
6. 当前导出名字没有 cone/package-aware namespace；一旦多 cone、多版本、separate compilation 真正接通，只有 `fqn` 不够。

## 5. 目标状态与强制规则

### 5.1 强制规则

1. `TypeId`、`SourceId`、`SymbolId`、`ClosureId`、`ConeId`、`BasicBlockId`、`LocalId`、`SiteId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`ResumeInterfaceId`、`ContinuationObjectId`、`StateId`、`BoundaryId`、`FrameSlotId` 只能作为内部实现 handle，不得直接出现在外部协议里。
2. 所有外部 surface 必须显式选择 identity 来源，而不是“先 `Debug`，再补一个字符串 canonicalize”。
3. CLI dump、fixture、JSON、RTTI 不能再直接使用 `format!("{:#?}")` 作为最终输出协议。
4. 所有跨 object / 跨 cone / 跨编译单元可见的名字必须由统一 `AbiMangler` 生成。
5. 所有 compiler-private LLVM function/global/type 名字必须由统一 `PrivateSymbolMangler` 生成，并显式使用 `InternalLinkage` 或 `PrivateLinkage`。
6. 导出 ABI 名字不得依赖 `source_path`、`decl_span`、allocator 序号、raw pretty-printer 文本。
7. `sanitize_llvm_ident()` 只能做可读前缀，不能作为唯一性来源。
8. 模块私有 dump/local label 可以使用“归一化的相对路径 + span”作为原始锚点，但必须先变成稳定 key，再由 renderer 决定是否显示成短标签。
9. `ConeId` 绝不能再被外部 surface 直接使用；导出名字必须改用 `StableConeKey`。
10. 平台/宿主强制要求的固定名字可以保留，例如真正的入口点 `main` 或 `@Extern` 指定的 native symbol，但它们应作为显式例外处理，而不是默认规则。

### 5.2 不同 surface 允许使用的 identity 来源

| Surface | 允许来源 | 禁止来源 |
|---|---|---|
| 导出 ABI 符号 | `StableConeKey + StableDefKey + StableInstanceKey` | `ClosureId`、`TypeId`、`StepSchemaId`、`source_path`、`decl_span` |
| compiler-private LLVM helper | `StableLocalEntityKey` + `PrivateSymbolMangler` | 任何 allocator id；默认 external linkage |
| 文本 dump label | `StableLocalEntityKey` 或语义文本 | raw `Debug`、allocator 顺序 |
| `.cone`/cache/JSON | `fqn`、canonical type/effect text、版本化 hash | dense id、机器本地绝对路径 |
| RTTI id | canonical name 的固定 hash | `ClosureId`、`TypeId`、path/span |

## 6. 建议引入的 stable key 模型

### 6.1 `StableConeKey`

建议字段：

1. `cone.name`
2. `cone.version`
3. 必要时加上 artifact kind/version 前缀

当前最小可行实现可以直接从 `Cone.toml` 的 `[cone]` 段读取名称和版本。当前仓库如果出现“两个不同依赖 artifact 解析到同一个 `StableConeKey`”的情况，应由构建层直接拒绝，而不是让后端继续碰撞。

### 6.2 `StableDefKey`

建议覆盖：

1. `StableConeKey`
2. namespace：`type`、`value`、`fun`、`property_getter`、`property_setter`、`object_init`、`top_level_init`、`extern_global` 等
3. owner path / FQN
4. callable kind 或 declaration kind
5. overload signature key

这里的 overload signature key 应该来自声明的语义 surface，而不是 `source_path + span`。可以复用当前 `signature_key` 思路，但输入必须切换到 canonical type/effect text。

### 6.3 `StableTemplateKey`

它应该替代今天 export 语义上的 `TemplateKey { fqn, source_path, decl_span }`：

`crates/scoopc/src/mir/materialize.rs:53-76`

也就是说，`TemplateKey` 仍可保留给内部实现，但导出/符号层必须改成 `StableDefKey` 派生的模板键。

### 6.4 `StableInstanceKey`

建议字段：

1. `StableTemplateKey`
2. canonical type args
3. canonical effect args

它是 generic specialization、materialized callable、LLVM exported symbol 的基本身份。

### 6.5 `StableClosureKey`

它应替代 `ClosureId` 进入 LLVM/RTTI。建议字段：

1. owner callable 的 `StableDefKey` 或 `StableInstanceKey`
2. lambda 在 owner 内的语义路径

当前仓库里已经存在 `$lambda0` 家族这一类“词法路径”概念，例如 direct HIR callable 的 root fqn 会出现 `a.main.$lambda0`。这类路径比 `ClosureId` 更适合作为 closure 语义身份的来源。

### 6.6 `StableCallSiteKey`

建议字段：

1. owner semantic key
2. 归一化的相对 `source_path`
3. 本地 `Span`
4. site kind

仓库里已有 `CallSite { source_path, span }` 可以直接复用其思想：`crates/scoopc/src/hir/mod.rs:914-933`

### 6.7 `StableEffectSchemaKey` / `StableContinuationSchemaKey`

这两类 key 不能再来自分配序号。建议字段：

1. owner callable version key
2. schema role
3. schema 的 authoritative semantic contents

对 `StepSchema`，语义内容至少应包括：

1. invoke args tuple type 的 canonical text
2. complete type 的 canonical text
3. 全部 concrete op case 的 canonical key
4. 每个 case 的 continuation contract

对 `ContinuationSchema`，语义内容至少应包括：

1. resume tuple type
2. answer type
3. out step schema key
4. surface type

### 6.8 `StableBoundaryKey` / `StableStateKey` / `StableFrameSlotKey`

这些 key 主要服务两类场景：

1. `dump-effect-facts` / `dump-effect-lowered` 的稳定 label
2. compiler-private LLVM helper 的私有命名

建议字段：

1. owner callable version key
2. structural role
3. source anchor 或 binder role

这里不要求它们成为跨 cone ABI 的一部分，但要求它们不再依赖 allocator 顺序。

## 7. canonical 编码与 mangling 规则

### 7.1 canonical type/effect text

导出名字和外部 hash 不能直接用 `TypeId`，也不能无条件复用当前 pretty-printer。

原因：

1. `TypeId` 本身只是 intern 索引：`crates/scoopc/src/ty/mod.rs:17-29`
2. `TypeParamType` 目前用 `(decl_file, decl_span)` 区分不同 `T`：`crates/scoopc/src/ty/mod.rs:171-180`

因此建议新增一个专用 canonical encoder，至少覆盖：

1. nominal：`N(pkg.Name<...>)`
2. builtin value/ref：`V(Unit)`、`R(String)` 这类固定文本
3. type param：`P(<owner-def-key>#<index>)`
4. function：`F(recv?; params... -> ret / row)`
5. tuple：`T(...)`
6. union：`U(...)`
7. effect row：按已排序去重后的 term 列表编码

### 7.2 hash 算法

建议统一为：

1. 所有 ABI/exported/private symbol 使用固定 canonical text 输入
2. 使用固定版本前缀，例如 `abi0:`、`priv0:`、`rtti0:`
3. hash 算法使用 `SHA-256`
4. linker-visible 符号截断为 128 bit hex
5. 只有在 runtime 结构已经固定要求 64 bit id 的场景，才允许截断为 64 bit

不能再使用：

1. Rust 默认 `Hash`
2. raw `Debug` 字符串
3. `source_path + span + ...` 作为 exported ABI hash 输入

### 7.3 名字模式建议

建议统一成以下模式：

1. 导出函数：`__scoop_abi0_fun__<escaped-fqn>__h<hash128>`
2. 导出全局：`__scoop_abi0_global__<escaped-fqn>__h<hash128>`
3. 导出类型相关 metadata：`__scoop_abi0_type__<escaped-fqn>__h<hash128>`
4. compiler-private function/global/type：`__scoop_priv0__<role>__h<hash128>`

其中：

1. `<escaped-fqn>` 只负责可读性
2. 真正唯一性来自 `h<hash128>`
3. `sanitize_llvm_ident()` 仅用于生成可读前缀

### 7.4 linkage 规则

1. 真正的导出 ABI symbol：允许 external
2. `@Extern` import/native symbol：沿用用户指定 external symbol
3. compiler-private helper：必须显式 `InternalLinkage` 或 `PrivateLinkage`
4. “虽然名字基于 FQN，但只在本模块内部调用”的 init bridge、closure body、effect helper 也必须 internal/private

## 8. 具体修改方案

### 8.1 先新增统一基础设施

建议新增一个集中模块，例如：

`crates/scoopc/src/stable_id.rs`

集中放置：

1. canonical type/effect encoder
2. shared hash helper
3. `AbiMangler`
4. `PrivateSymbolMangler`
5. dump label 生成器

这样可以避免 `mir/materialize.rs`、`hir/lower/util.rs`、`rtti/type_desc.rs`、`llvm/codegen/**` 各自维护一套字符串拼接逻辑。

### 8.2 dump / fixture 相关改造

| 组件 | 主要文件 | 当前问题 | 需要的改动 |
|---|---|---|---|
| HIR dump | `crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/commands/dump_hir.rs` | raw `Debug` 泄漏 `TypeId`、`SymbolId`、`ClosureId` | 新增自定义 renderer；不再直接 `format!("{:#?}")`；本地 symbol/closure 使用 `StableLocalEntityKey` 派生的 label。 |
| MIR dump | `crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/commands/dump_mir.rs` | raw `Debug` 加字符串补丁；仍泄漏 `bbN/lN/siteN/TypeId` | 新增自定义 renderer；移除 `TypeId` 文本 canonicalize 补丁；block/local/site label 来自稳定 key。 |
| Materialized MIR dump | `crates/scoop/src/commands/dump_ir.rs`、`crates/scoopc/src/mir/materialize.rs` | 直接 `Debug`；实例参数仍是 `tN` | 增加 stable renderer；实例名改用 `StableInstanceKey`；输出不再依赖 `TypeId`。 |
| Effect facts dump | `crates/scoopc/src/effect_facts/dump.rs` | `step_schema#N`、`continuation_schema#N`、`case#N`、`bbN`、`siteN` | schema/site/case/block label 全部改为语义 label 或 stable local label。 |
| Effect lowered dump | `crates/scoopc/src/effect_lowered/dump.rs` | `t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 全泄漏 | 全量切换到 semantic/stable local label；去掉 allocator id。 |

### 8.3 generic / overload / instance identity 改造

| 组件 | 主要文件 | 当前问题 | 需要的改动 |
|---|---|---|---|
| overload-aware symbol suffix | `crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/hir/lower/util.rs` | `$overload$<hash>` 的 hash 输入含 `source_path + decl_span` | 改成基于 `StableDefKey + canonical signature key` 的 hash。 |
| `instance_fqn()` | `crates/scoopc/src/mir/materialize.rs:8505-8525` | 名字直接拼 pretty type text 和 path/span-derived suffix | 分离“调试显示名”和“ABI/exported 名”；前者可读，后者走 mangler。 |

### 8.4 RTTI 改造

| 组件 | 主要文件 | 当前问题 | 需要的改动 |
|---|---|---|---|
| `dump-rtti` closure env | `crates/scoopc/src/rtti/type_desc.rs` | closure env `name` 和 `type_id` 仍来自 `ClosureId` | 改为从 `StableClosureKey` 生成 canonical name，再对该 name hash。 |
| shared hash helper | `crates/scoopc/src/rtti/type_desc.rs` | 本地自带 `stable_hash64`，与其它 surface 分离 | 移到共享 `stable_id` 模块，统一版本和输入规范。 |

### 8.5 LLVM / object / linker 改造

| 组件 | 主要文件 | 当前问题 | 需要的改动 |
|---|---|---|---|
| source-level top-level function symbol | `crates/scoopc/src/llvm/codegen/mod.rs:2321-2423` | 非 `@Extern` 函数默认 external namespace；仅用 `fun.fqn` | 把“导出 ABI”与“模块私有实现”分离；需要 external 的函数走 `AbiMangler`，其余显式 internal/private。 |
| materialized plain callable symbol | `crates/scoopc/src/llvm/codegen/mir_body.rs:306-365` | `mir_fun.fqn` 默认 external | 同上，改成导出策略 + linkage 策略。 |
| closure function/resume/env name | `crates/scoopc/src/llvm/codegen/closure/mod.rs`、`crates/scoopc/src/llvm/codegen/ordinary_callee.rs`、`crates/scoopc/src/llvm/codegen/gc.rs` | 直接来自 `ClosureId` | 改成 `StableClosureKey` + `PrivateSymbolMangler`；默认 internal/private。 |
| effect helper shell/trampoline | `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` | 直接来自 `StepSchemaId` / `ContinuationSchemaId` / `CaseTag`，且默认 external | 改成基于 `StableEffectSchemaKey` / `StableContinuationSchemaKey` 的 private name；统一 internal/private linkage。 |
| object/top-level init bridge | `crates/scoopc/src/llvm/codegen/object_init.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` | compiler-private helper 名字进入全局 namespace | 继续保留 FQN 作为可读前缀也可以，但必须走 private mangler，并显式 internal/private。 |
| effect transport box/type name | `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:10885-10906` | 名字含 `TypeId` 和 pretty type text | 改为 private name，唯一性来自 canonical type hash。 |
| carrier alias 兼容层 | `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6980-6985`、`crates/scoopc/src/llvm/codegen/mod.rs:783-819` | 仍把 direct HIR closure carrier alias 映射到 `scoop.lambda$<n>` | 迁移 closure naming 后删除或改成新的 stable closure alias。 |

### 8.6 linkage 卫生改造

当前仓库里有不少 global 已经正确设成 `InternalLinkage`，例如：

1. `ensure_struct_anchor()` / `ensure_case_tag_constant()`：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6196-6214`
2. 多个 top-level/object/global descriptor：`crates/scoopc/src/llvm/codegen/mod.rs:932`、`948`、`2853`、`3577`、`3656`

真正需要补的是 function。优先级最高的检查点是所有 `module.add_function(name, fn_ty, None)`：

1. 如果函数是 compiler-private helper，就必须显式 internal/private。
2. 如果函数是导出 ABI，就必须由统一 `AbiMangler` 生成名字，而不是裸 `fqn` 或 `ClosureId` 名字。

## 9. 推荐实施顺序

### Phase 1：先建立公共基础设施

1. 增加 `stable_id` 共享模块。
2. 定义 canonical type/effect encoder。
3. 定义 `AbiMangler` / `PrivateSymbolMangler`。
4. 定义 dump 用的 stable label 生成器。

### Phase 2：先处理 linker 风险最大的部分

1. 审计并修改所有 compiler-private helper function 的 linkage。
2. 先把 closure、effect helper、object/top-level init bridge 从 external namespace 收回 internal/private。

这样即使外部名字还没全部变漂亮，也能先消除最危险的符号表污染和潜在链接冲突。

### Phase 3：替换 closure / effect / helper 命名源

1. 把 `ClosureId` 从 LLVM/RTTI 命名中移除。
2. 把 `StepSchemaId` / `ContinuationSchemaId` / `CaseTag` 从 helper 符号名中移除。
3. 把 `TypeId` 从 LLVM type/global/helper 名字中移除。

### Phase 4：替换 generic / overload 的 exported naming

1. 重写 `$overload$<hash>` 的 hash 输入。
2. 把 `instance_fqn()` 从 ABI/exported symbol 语义里拆出来。
3. 引入 cone/package-aware exported namespace。

### Phase 5：重写 dump/fixture renderer

1. 先 HIR/MIR/`dump-ir`
2. 再 effect facts / effect lowered

这样可以在基础 identity 已经就位后，一次性把 fixture surface 从 raw `Debug` 迁走，而不是继续做字符串补丁。

### Phase 6：收尾 RTTI 与审计

1. 统一 RTTI hash helper。
2. 清除 `dump-rtti` 中 closure env 的 dense-id 依赖。
3. 更新 LLVM tests / fixture / grep-based audit。

## 10. 验收标准

1. active 外部 surface 中不再直接渲染或 hash 以下类型：
   `TypeId`
   `SourceId`
   `ConeId`
   `SymbolId`
   `ClosureId`
   `BasicBlockId`
   `LocalId`
   `SiteId`
   `StepSchemaId`
   `ContinuationSchemaId`
   `CaseTag`
   `ResumeInterfaceId`
   `ContinuationObjectId`
   `StateId`
   `BoundaryId`
   `FrameSlotId`
2. 所有只在本模块内部使用的 compiler-generated function/global 都显式使用 `InternalLinkage` 或 `PrivateLinkage`。
3. 同一份输入在不同 checkout 路径下，导出的 LLVM symbol 集合不发生变化。
4. 两个 cone 即使内部 closure/site/schema 分配号都从 0 开始，也不会在链接阶段因为内部 helper 名字碰撞。
5. `dump-hir`、`dump-mir`、`dump-ir`、`dump-effect-facts`、`dump-effect-lowered` 及其 fixtures 不再包含 raw `TypeId(`、`S0/C0`、`bb0/site0`、`step_schema#0`、`k0/ri0/ko0/st0/bd0/fs0` 这类 allocator-derived id。
6. `dump-rtti` 的 closure env `name` 和 `type_id` 不再依赖 `ClosureId` 分配顺序。
7. `sanitize_llvm_ident()` 只出现在“可读前缀”路径里，不再承担唯一性责任。

## 11. 审计 grep 清单

下面这些搜索点在实现过程中应长期保留为审计清单：

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

这些 grep 不是最终验证本身，但它们能快速暴露“又把内部 dense id 渗回外部 surface 了”的回归。

## 12. 结论

当前仓库已经有两类实践并存：

1. `.cone` active schema 这类地方，基本按语义键工作。
2. `dump-*`、RTTI closure env、LLVM helper symbol 这类地方，仍然大量把内部 allocator id、源码路径、pretty-printer 文本直接外化。

后续整改不能继续走“看到哪里漂就给哪里加 canonicalize”的路线，而应把整个 identity 层明确化：

1. 内部 dense id 继续只做内部实现 handle。
2. 对外协议一律走 semantic key、stable local key、或 private/exported mangler。
3. compiler-private helper 一律 internal/private。
4. exported ABI symbol 一律由统一 mangler 生成。

只有这样，fixture 漂移、多 cone、公私有可见性、RTTI，以及 LLVM/object/linker 风险才能一起收敛，而不是此起彼伏地冒出来。
