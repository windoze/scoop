# TODO（core / stdlib reshape）

> 生成时间：2026-05-17
> 计划基线：[`PLAN.md`](./PLAN.md)
> 设计基线：[`PLAN.md`](./PLAN.md) §0–§7
> 格式参考：`docs/archive/plans/TODO-managed-abi.md`
> 顺序约束：严格按 ID 顺序推进；不得跨条目并行实现。
> 当前状态：全部任务待开始。前一轮 `Managed ABI / native callable ABI 收口` 已完成；其 final state 见 [`docs/archive/plans/PLAN-managed-abi.md`](./docs/archive/plans/PLAN-managed-abi.md) 与 [`docs/archive/plans/TODO-managed-abi.md`](./docs/archive/plans/TODO-managed-abi.md)。

本文件是任务索引。每条任务的完整内容（参考 / 目标 / 当前实现入口 / 必须实现的内容 / 必须遵从的约束 / 验证 / 完成条件 / 依赖）位于对应的 `TODO-N.md` 文件。

## 任务索引

| ID | 状态 | 阶段 | 标题 | 详情 |
| --- | --- | --- | --- | --- |
| `P0-T01` | [DONE] | P0 | [DONE] 冻结 reshape baseline 与 fixture 三分类清单 | [TODO-1.md](./TODO-1.md) |
| `P1-T01` | [DONE] | P1 | [DONE] `scoop.lang.string` 空 cone 落地（package + sysroot file + loader 接入） | [TODO-1.md](./TODO-1.md) |
| `P1-T02` | [DONE] | P1 | [DONE] 自动 prelude：`scoop.core.*` + `scoop.lang.string.*` 注入 ImportTable | [TODO-1.md](./TODO-1.md) |
| `P2-T01` | [DONE] | P2 | [DONE] 反射 const fun 补全 `kindOf<T>` / `descOf<T>` + `ARRAY_ELEM_KIND_*` 常量 | [TODO-1.md](./TODO-1.md) |
| `P3-T01` | [DONE] | P3 | [DONE] runtime 端：`ScoopMutableArray` out-of-line layout + 单态 new/push/freeze 入口 | [TODO-1.md](./TODO-1.md) |
| `P3-T02` | [DONE] | P3 | [DONE] 编译器端：`array_size/get/set/data_ptr` 按 receiver layout 分流 | [TODO-1.md](./TODO-1.md) |
| `P3-T03` | [DONE] | P3 | [DONE] sysroot 泛型 wrapper：`mutableArrayNew<T>` / `MutableArray<T>.push` / `MutableArray<T>.freeze` | [TODO-1.md](./TODO-1.md) |
| `P4-T01` | [DONE] | P4 | [DONE] 数组字面量 HIR desugar 切换到 `mutableArrayNew + push + freeze` 路径 | [TODO-2.md](./TODO-2.md) |
| `P4-T02` | [DONE] | P4 | [DONE] 删除 `__scoop_array_builder_*` 整套（sysroot / runtime / 编译器 lowering） | [TODO-2.md](./TODO-2.md) |
| `P5-T01` | [DONE] | P5 | [DONE] runtime 端:三个 `scoop_string_from_*_array` 单态入口 | [TODO-2.md](./TODO-2.md) |
| `P5-T02` | [DONE] | P5 | [DONE] sysroot 端：`scoop.lang.string` cone 内三个 scoop ABI 声明 + StringBuilder | [TODO-2.md](./TODO-2.md) |
| `P5-T03` | [DONE] | P5 | [DONE] sysroot/string.scoop 中高级 String helper 迁入 `scoop.lang.string`，更新 `String.split` | [TODO-2.md](./TODO-2.md) |
| `P6-T01` | [DONE] | P6 | [DONE] f-string HIR desugar：改写为 `StringBuilder().add(...).toString()` 调用链 | [TODO-3.md](./TODO-3.md) |
| `P6-T02` | [DONE] | P6 | [DONE] 删除 LLVM 阶段 f-string codegen 后门 + sysroot 文件 f-string 使用 lint | [TODO-3.md](./TODO-3.md) |
| `P7-T01` | [DONE] | P7 | [DONE] sysroot：`String` body method / 标量 toString / print/println / panic 等转 `@Extern(abi = "scoop")` | [TODO-3.md](./TODO-3.md) |
| `P7-T02` | [DONE] | P7 | [DONE] 删除 `sysroot/scalar_string_bridge.scoop` + 编译器对应 audited bridge dispatch | [TODO-3.md](./TODO-3.md) |
| `P7-T03` | [DONE] | P7 | [DONE] runtime 端可能的符号改名（`scoop_print_string` → `scoop_print` 等） | [TODO-3.md](./TODO-3.md) |
| `P8-T01` | [DONE] | P8 | [DONE] 标量 operator behavioral baseline 短文 | [TODO-4.md](./TODO-4.md) |
| `P8-T02` | [DONE] | P8 | [DONE] 编译器 method-level intrinsic 表扩展：算术 / 位运算 / 比较 / 布尔 / Char 一组 entry | [TODO-4.md](./TODO-4.md) |
| `P8-T03` | [DONE] | P8 | [DONE] sysroot 标量 type body 内补 `@Intrinsic("...")` method 声明 | [TODO-4.md](./TODO-4.md) |
| `P8-T04a` | [DONE] | P8 | [DONE] runtime String helpers 写入正确 `type_desc`（修复 P6-T01 引入的 5 个 GC fixture 失败） | [TODO-4.md](./TODO-4.md) |
| `P8-T04b` | [DONE] | P8 | [DONE] 修正 `INTERNAL_BUG_SENTINEL_HITS` 行号 drift（P8-T03 漏更新的 audit baseline） | [TODO-4.md](./TODO-4.md) |
| `P8-T04c` | [DONE] | P8 | [DONE] synthetic member call canonicalization + class/static init reachability 修复 | [TODO-4.md](./TODO-4.md) |
| `P8-T04` | [DONE] | P8 | [DONE] HIR / typecheck：binary / unary operator 改写为 method call | [TODO-4.md](./TODO-4.md) |
| `P8-T05` | [DONE] | P8 | [DONE] 删除 `mir_body/op.rs` 按 `ast::BinaryOp` 直接 codegen 路径 | [TODO-4.md](./TODO-4.md) |
| `P8-T06` | [DONE] | P8 | [DONE] 算术 fixture 矩阵 + 边界值回归 | [TODO-4.md](./TODO-4.md) |
| `P9-T01` | [DONE] | P9 | [DONE] 把 `IntProgression.forEach` / `Int.rangeTo/downTo/until` 等 desugar 依赖从 stdlib 迁入 core | [TODO-5.md](./TODO-5.md) |
| `P9-T02` | [DONE] | P9 | [DONE] 按 P0-T01 三分类清单批量改写 / 合并 / 删除 fixture | [TODO-5.md](./TODO-5.md) |
| `P9-T03` | [DONE] | P9 | [DONE] 删除 `stdlib/` 目录与 frontend stdlib 注入路径 | [TODO-5.md](./TODO-5.md) |
| `P10-T01` | [TODO] | P10 | 把 `__AtomicInt` 系列从 core 迁到 `scoop.unsafe` | [TODO-5.md](./TODO-5.md) |
| `P10-T02` | [TODO] | P10 | 删除 `__scoop_thread_spawn_join_resume*` 与相关 runtime 入口 | [TODO-5.md](./TODO-5.md) |
| `P10-T03` | [TODO] | P10 | 验证 core / lang.string 不再隐式依赖 `scoop.thread` / `scoop.sync` | [TODO-5.md](./TODO-5.md) |
| `P11-T01` | [TODO] | P11 | 审查 `__scoop_stackmap_statepoint_smoke` / `__scoop_gc_debug_*` 实际使用方 | [TODO-5.md](./TODO-5.md) |
| `P11-T02` | [TODO] | P11 | 测试 helper 迁移到 test cone 或 C ABI extern 或删除 | [TODO-5.md](./TODO-5.md) |
| `P12-T01` | [TODO] | P12 | sysroot 全 file 审计：每个 method/fun 满足 body / `@Intrinsic` / `@Extern` 三选一 | [TODO-5.md](./TODO-5.md) |
| `P12-T02` | [TODO] | P12 | sysroot 物理目录按 cone FQN 重组（`sysroot/scoop.core/` / `sysroot/scoop.lang.string/` 等子目录） | [TODO-5.md](./TODO-5.md) |
| `P12-T03` | [TODO] | P12 | 取消 `signature_only_sysroot_ast` / `is_compilable_sysroot_file` 整套 AST stripping | [TODO-5.md](./TODO-5.md) |
| `P12-T04` | [TODO] | P12 | body 缺失策略统一：sysroot file 与用户 file 用同一规则 | [TODO-5.md](./TODO-5.md) |
| `P12-T05` | [TODO] | P12 | `is_sysroot()` 语义收窄：仅保留在 `@file:AllowIntrinsic` 自动开 gate 处使用 | [TODO-5.md](./TODO-5.md) |
| `P13-T01` | [TODO] | P13 | spec §10.3 删除 `var StringBuilder.lastChar` 示例 + 加入 `scoop.lang` 简介 + sysroot 目录组织约定 | [TODO-5.md](./TODO-5.md) |
| `P13-T02` | [TODO] | P13 | 更新 `MANAGED_ABI.md` §2.2 typical example 列表 | [TODO-5.md](./TODO-5.md) |
| `P13-T03` | [TODO] | P13 | 清理 sysroot 文件中的过期 TODO 注释（T0143 / T1317 / T1325 等历史工单引用） | [TODO-5.md](./TODO-5.md) |
| `P13-T04` | [TODO] | P13 | 最终 fixture 收尾：所有 fixture 必须通过 / 删除 / 改写，不允许留下任何 failing fixture | [TODO-5.md](./TODO-5.md) |

## 全局约束

- [`PLAN.md`](./PLAN.md) 是本轮唯一计划基线；本文件与 `TODO-N.md` 只负责把 `PLAN.md` 的 P0-P12 拆成严格顺序执行的任务。
- 仓库尚无已发布版本，**不保留任何前向兼容性**。runtime layout、runtime symbol 名、sysroot 声明、ABI surface 在本轮中都允许一次性改换；不得为"兼容"目的引入桥接层或别名。
- "intrinsic 是否保留"的判定标准：**编译器在该 callsite 是否生成实质代码**。"只调一个 runtime symbol 别的什么都不做"不是 intrinsic，必须改成 `@Extern(abi = "scoop")` 或 `@Extern(abi = "c") @NoGC`。
- runtime symbol 永远是单态的；需要泛型 surface 的 helper 由 sysroot 写普通 Scoop 泛型 wrapper，wrapper 内部用反射 const fun 把 type info materialize 成 const args，再调单态 runtime symbol。
- core 自身（`sysroot/core.scoop` 等）禁止使用 f-string，避免 desugar 链路自指。
- **fixture 终态铁律（贯穿整个 reshape）**：仓库内**不允许保留任何 failing fixture**。最终收尾（P13-T04）时，每一条仍存在的 fixture 都必须通过；过时或不再合适的 fixture 必须**删除或改写**。任何 P 阶段的中间状态如果出现暂时性 failing fixture，对应阶段任务必须把它列入完成记录的"待 P9-T02 / P13-T04 处理"清单——后续阶段必须接手处理，不允许"沉默地累积失败 fixture"。
- **fixture 删除标准（与上条配对的硬约束）**：删除 fixture 的**唯一**正当理由是它测试的功能 / API / 语义在本轮 reshape 中**真的不复存在了**（例如测试 `__scoop_array_builder_*` 内部行为、测试旧 stdlib `MutableArray<Int>.splice` API 等）。**不允许**因为"fixture 跑不通且改起来麻烦"就删——这种情况必须**改写** fixture 让它继续验证有意义的行为。简言之：
  - **改写**：fixture 的测试意图仍然有效（验证某项语言能力 / sysroot helper / 运行时行为），只是表面 API / import / 语法形式改了 —— 必须改写让它通过。
  - **删除**：fixture 的测试意图本身已经失效（被测对象已删除、且没有等价新对象需要被覆盖测试）—— 才允许删。
  - 对每条删除决定，完成记录中必须写明"被测对象 X 已不存在 / 已被 Y 替代且 Y 由 fixture Z 覆盖"——一句"fixture 已过时"不构成正当理由。
- 每个任务完成后，必须回写：
  - 改动范围；
  - 核心决策；
  - 验证结果；
  - 与 `PLAN.md` 的对应闭合；
  - 如该任务期间出现暂时性 failing fixture，列出清单并指明哪个后续任务负责处理。
- 任务完成后，把本文件索引表中对应行的状态从 `[TODO]` 改为 `[DONE]`。完成记录写在该任务所在的 `TODO-N.md` 文件中条目末尾。
- 前一轮 `Managed ABI` 留下的下列 surface 是本轮的可用基础设施，不得回退：
  - `ExternAbi::Scoop`（`@Extern(abi = "scoop")`）：顶层、`Pure`、允许 GC ref 进出、ordinary managed call 框架。
  - method-level `@Intrinsic("name")` + 可枚举 intrinsic 表（`crates/scoopc/src/intrinsics.rs::named_intrinsic_audit_entries`、`crates/scoopc/src/llvm/codegen/intrinsics/named.rs`），默认 IR-emission 模式。
  - `@Intrinsic struct/class` 内建类型作为一等 struct/class implementer（含 generic class）。
  - `Array<T>` / `MutableArray<T>` 的 IR-direct `array_size/get/set/data_ptr` 已落地。

已落地基线的具体清单（前一轮成果与现仓库快照）见 [TODO-1.md](./TODO-1.md) 顶部 "已落地基线" 一节。
