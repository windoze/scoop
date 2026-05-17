# UnsupportedMainBody 修复总计划

> 本计划为 **doc-and-test only** 计划：本阶段不进行任何 production 代码修复，
> 只产出 (1) 详细的成因分析文档、(2) 完整的 fixture / testcase 集合、
> (3) 可机器消费的 inventory 数据。所有 production 修复必须在后续阶段
> 按本计划提供的测试用例驱动完成。
>
> 计划目标（终态）：当本计划下定义的所有 fixture 全部 PASS 时，
> `LlvmEmitError::UnsupportedMainBody` 在 production 代码路径上不可达，
> `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 总数降为 0，
> `crates/scoopc/src/llvm/codegen/` 不再以"兜底报错"形式出现该错误，
> 编译器对 `docs/spec/language_spec-part{1..6}.md` 中描述的所有语法特性
> （不含标准库扩展）做到"合法输入产出有效输出，非法输入明确报错"。

---

## 0. 背景与定位

### 0.1 当前状态（实测，2026-05-18 基线）

- `LlvmEmitError::UnsupportedMainBody` 在 `crates/scoopc/src/llvm/codegen/` 下
  共出现 **1 277 处**，覆盖 60 个文件；其中 production 路径
  （`STALE_UNSUPPORTED_MAIN_BODY_COUNTS`）**冻结为 637 处**。
- `kind:` 字面量共 **964 个不同标签**，其中 **825 个只出现一次** ——
  目前事实上是"一处一个 ad hoc bug 标签"的状态。
- 错误本身 (`crates/scoopc/src/llvm/mod.rs:165-169`) 的设计语义是
  **"编译器内部不变量被打破（compiler bug）：LLVM 主 codegen 收到本不应
  抵达的节点（这表示上游 contract drift，不是合法语言特性）"**；
  但代码现状是把它当作"上游各阶段的兜底层"在用。

### 0.2 计划遵循的硬约束

1. **不允许"兜底报错"** ——
   每一处 `UnsupportedMainBody` 必须归属于以下三类之一：
   - **Frontend Reject**（前端早拒）：非法输入，必须在前端 / typecheck /
     HIR / MIR 阶段以稳定 diagnostic 拒绝；
   - **Internal Bug Sentinel**（不可达断言）：合法输入下绝不可能命中，
     由 `unreachable!` / `expect("...")` 表达，命中即编译器 bug；
   - **Real Implementation**（真正实现）：当前 codegen 缺失该形状支持，
     需要补全实现使其转为合法路径。
2. **本阶段不动 production 代码**：所有产出物必须是 `.md`、`.scoop`
   fixture、`.stdout` expected output、`.csv`/`.json` 数据表，
   或仅由这些数据驱动的 baseline 单元测试。
3. **Spec 是覆盖完备性的最终标准**：fixture set 必须能与
   `docs/spec/language_spec-part{1..6}.md` 章节做双向回链。
4. **既有 baseline 不破坏**：`pipeline_user_visible_failure_policy_*` 系列
   测试目前冻结了 637 这个数字；本阶段产出的新增 fixture 不得使该计数
   "意外漂移"，所有刻意引入的对照样本必须明确归类。

### 0.3 关键参考文件

- `crates/scoopc/src/llvm/mod.rs:135-169` — 错误枚举定义。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` — 当前
  failure 分类基线 + 冻结计数。
- `crates/scoopc/src/llvm/codegen_gap_inventory.rs` — codegen-stage gap
  inventory（21 entries）。
- `docs/archive/designs/PIPELINE_GAPS.md` — 历史 gap ledger。
- `docs/spec/language_spec-part{1..6}.md` — 语言规范。
- `tests/fixtures/{run-pass,typecheck,mir,hir,parse,resolve,...}/` — 既有
  fixture 布局。

---

## 1. 计划阶段总览

| Phase | 名称 | 主要产出 | 是否本计划范围 |
| --- | --- | --- | --- |
| P0 | 计划立项（本文档） | `UnsupportedMainBody_FIX.md` | ✅ |
| P1 | Inventory 快照 | `audit/UMB_inventory.csv` + 索引脚本 | ✅ |
| P2 | 成因分析 | `audit/UMB_categories/*.md` | ✅ |
| P3 | Spec 覆盖矩阵 | `audit/spec_coverage_matrix.md` | ✅ |
| P4 | 修复策略 | 每个 bucket 一份策略草案 `audit/strategies/*.md` | ✅ |
| P5 | Fixture 集合 | `tests/fixtures/umb_fix/**` | ✅ |
| P6 | Baseline 测试 | inventory / spec / fixture 三向交叉验证测试 | ✅ |
| P7 | Production 修复 | 实际删除 / 改写 `UnsupportedMainBody` 的代码改动 | ❌ 不在本计划 |
| P8 | 退场审计 | 计数归零 + variant 在 production 不可达 | ❌ 不在本计划 |

> 备注：P5、P6 中所有 fixture / 测试都允许暂时标注为 `IGNORE-UNTIL-FIX:
> <bucket-id>`（见 §5.4），用于在 production 修复落地前与 CI 共存。

---

## 2. P1 — Inventory 快照

### 2.1 目标

为后续所有阶段提供**可机器消费、可对账**的全量基线，使任意一处
`UnsupportedMainBody` 都能被唯一寻址、归档、回溯。

### 2.2 产出位置

- `audit/UMB_inventory.csv` — 主表。
- `audit/UMB_inventory_schema.md` — 表头规范。
- `crates/scoopc/src/audit/umb_inventory.rs` — 自动生成 + 校验脚本
  （仅 `#[cfg(test)]` baseline，禁止参与 production）。

### 2.3 主表 schema（`audit/UMB_inventory.csv`）

| 字段 | 含义 | 示例 |
| --- | --- | --- |
| `id` | 稳定编号 `UMB-<NNNN>`，按 file+line 排序生成 | `UMB-0042` |
| `file` | 仓库相对路径 | `crates/scoopc/src/llvm/codegen/mir_body/types.rs` |
| `line` | 1-based 行号 | `74` |
| `kind` | `kind:` 字面量原文 | `pass MIR local type` |
| `route` | `RawMirLlvm` / `EffectLoweredLlvm` / `Both` / `Helper` | `Both` |
| `surface` | 触发点表达层（`stmt`、`rvalue`、`terminator`、`type`、`builder`、`intrinsic`） | `type` |
| `bucket` | 见 §3 分类编号 `B-XX` | `B-02` |
| `expected_class` | `FrontendReject` / `InternalBugSentinel` / `RealImpl` | `InternalBugSentinel` |
| `spec_anchor` | 关联的 spec 章节锚（可多值 `;` 分隔） | `part2#9-泛型;part3#4-调用` |
| `upstream_gate` | 期望由谁保证不可达；缺则 `TBD` | `MIR strict verifier: type completeness` |
| `existing_fixture` | 已有覆盖 fixture（如有） | `tests/fixtures/run-pass/closure_env_basic.scoop` |
| `notes` | 短备注 | `cross-TypeStore equivalent fallback` |

### 2.4 自动化要求

- 索引脚本必须从源码 grep 重建 CSV，且与磁盘上 CSV 比较；不一致时
  baseline 测试失败。
- 索引脚本必须验证：
  - 每条 entry 的 `bucket` ∈ §3 分类表；
  - 每条 entry 的 `expected_class` ∈ 三类之一；
  - 每个 `kind` 字面量在 CSV 中出现次数 == 实际源码中出现次数；
  - CSV 总行数 == 源码 `UnsupportedMainBody {` 出现总数。
- 输出 `cargo run -p scoopc --bin umb-audit -- list/diff/stats` 三个子命令，
  便于在 P7 阶段对账。

### 2.5 验收标准

- CSV 行数 == 当前实测的 1 277（基线值随 baseline 测试一并冻结，
  不允许偷偷增长，需增长须改 baseline 数字并写明原因）。
- 每个 `kind` 标签都有归属 bucket，无 `bucket=TBD` 残留。
- 每条 entry 的 `spec_anchor` 不为空，除非 `expected_class =
  InternalBugSentinel` 且明确标注 `spec_anchor=N/A:helper-invariant`。

---

## 3. P2 — 成因分析与 Bucket 分类

### 3.1 一级分类（按"应该由谁保证不可达"）

| 一级类 | 期望治理路径 |
| --- | --- |
| **A. Helper invariant** | 由 codegen 内部 helper 直接 `expect`/`unreachable!` |
| **B. Upstream contract** | 由前端 / typecheck / HIR / MIR strict verifier 保证 |
| **C. Real implementation** | 当前 codegen 真未实现该合法形状，需要补全 |
| **D. Spec-uncovered** | 当前路径触及尚未规范化的语法（如 spec 中 "未定义"），需先收紧 spec |

### 3.2 二级 Bucket（候选清单，最终以 P1 inventory 为准）

> 以下 bucket 编号 `B-XX` 是本计划的稳定标识。每个 bucket 在
> `audit/UMB_categories/B-XX.md` 中独立成文。每条 inventory 记录都必须归到
> 恰好一个 bucket。

| Bucket | 名称 | 一级类 | 典型 `kind` 标签 |
| --- | --- | --- | --- |
| B-01 | inkwell builder bookkeeping | A | `builder has no insert block`、`builder has no parent function`、`function has no entry block`、`builder has no current function` |
| B-02 | MIR local / member 类型推断不完整 | B | `pass MIR local type`、`pass MIR local member field type drift`、`pass MIR member receiver type`、`pass MIR member receiver type drift` |
| B-03 | MIR direct/closure/funptr 调用 ABI 漂移 | B | `pass MIR direct call arity mismatch`、`pass MIR closure callee type`、`pass MIR FunPtr invoke arity mismatch` |
| B-04 | MIR 函数签名 / 参数 / 返回类型缺失 | B | `pass MIR plain param type`、`pass MIR closure return type`、`missing pass MIR llvm function sret param`、`pass MIR param arity` |
| B-05 | MIR CFG / start block / goto target 异常 | B | `pass MIR cfg`、`pass MIR start block`、`pass MIR goto target`、`pass MIR then target` |
| B-06 | MIR struct/tuple/enum 字面量 schema 漂移 | B | `pass MIR struct literal layout`、`pass MIR tuple arity mismatch`、`pass MIR enum payload schema`、`pass MIR struct literal missing field` |
| B-07 | MIR pattern 子句 schema 漂移 | B | `pass MIR pattern is subject type`、`pass MIR variant pattern arity`、`pass MIR tuple pattern element type` |
| B-08 | MIR 成员存取 / 赋值合法性 | B | `pass MIR member store target not writable`、`pass MIR member store value type`、`pass MIR immutable class member store` |
| B-09 | Cross-TypeStore equivalence 不闭合 | B/C | `equivalent_codegen_*` 系列返回 `None` 后被 `?` 兜底（`pass MIR closure param type`、`pass MIR closure env contract codegen type` 等） |
| B-10 | Effect-typed callable adapter / ABI routing | C | `effect-typed plain adapter sret payload type`、`effect-typed closure surface function type`、`plain closure call effect-typed surface requires adapter` |
| B-11 | Pure / plain statement 边界路由 | B | `pure statement effectful direct call requires boundary lowering`、`pure statement resume call requires boundary lowering`、`pure statement todo` |
| B-12 | Closure / lambda / capture 表达 | C | `closure env type`、`closure env capture type`、`mutable capture (not supported yet)`、`pass MIR closure env contract mismatch`、`capture local (non-scalar)` |
| B-13 | 数组 / 复合 transport metadata | C | `composed call replay block`、`task transport tuple type`、`pass MIR closure env capture schema arity` |
| B-14 | Cast / TypeCheck (`as`/`as?`/`is`) | B/C | `MIR \`as?\` cast result contract`、`as? operand type`、`type check operand value`、`cast target type` |
| B-15 | When / 模式匹配 用户面 | B | `when (no arms)`、`when missing enum arm`、`when arm type mismatch`、`when guard value`、`when subject type` |
| B-16 | 控制流 outside-of-context | B | `break outside loop`、`continue outside loop`、`return outside function with return context`、`\`return\` inside block expression` |
| B-17 | Coercion / 标量运算 | A/B | `coerce bool to u64 word`、`coerce composite value to u64 word`、`equality lhs`、`bool operator lhs` |
| B-18 | 字面量与字符串 | B | `int literal type`、`bool value`、`float value`、`pass MIR pattern string literal`、`source-backed literal slice` |
| B-19 | Top-level / object init / extern global | B | `top-level const without initializer`、`top-level immutable value init (missing metadata)`、`object init (missing metadata)`、`extern global initializer contract` |
| B-20 | Class ctor / property / 字段访问 | B | `class ctor selected ctor contract`、`class ctor delegation cycle`、`class field index out of bounds`、`object property without initializer` |
| B-21 | Struct literal / 字段层 | B | `pass MIR struct literal duplicate field`、`pass MIR struct literal missing field`、`unknown struct field` |
| B-22 | Enum 布局 / niche / Option | B | `Option niche payload type`、`Option niche pointer none_value (must be NULL)`、`niche enum variant arity`、`enum boxed payload field value` |
| B-23 | Member access — 通用 | B | `member access receiver type`、`member access target`、`pass MIR member target unresolved` |
| B-24 | Reflection / comptime intrinsic | C/D | `reflection intrinsic call binding`、`pass MIR descOf arg type`、`pass MIR kindOf arg type`、`pass MIR sizeOf arg type` |
| B-25 | Platform / RTTI intrinsic | B/C | `getPlatform intrinsic *`、`MIR runtime type descriptor`、`MIR runtime type target` |
| B-26 | atomic intrinsic 系列 | B | `atomicInt*`、`atomicIntCompareExchange*`、`atomicIntLoad*`、`atomicIntStore*` |
| B-27 | sync intrinsic 系列 | B | `sync.Mutex.*`、`sync.CondVar.*`、`sync.Once.*`、`sync.condVarCreate *`、`sync.destroy *` |
| B-28 | thread intrinsic 系列 | B | `thread.currentId *`、`thread.sleepMillis *`、`thread.threadSpawn *`、`thread.Thread.join *`、`thread.yield *` |
| B-29 | GC intrinsic 系列 | B | `GC.handleNew *`、`GC.handleGet *`、`GC.handleDrop *`、`GC.pin *`、`GC.unpin *` |
| B-30 | named / unsafe / FunPtr intrinsic | B | `named intrinsic *`、`named runtime intrinsic *`、`funptrToUIntPtr *`、`uintPtrToFunPtr *`、`stackmap statepoint smoke *` |
| B-31 | 标量扩展方法 (Float/Int/Char/Bool/String) | B | `Float.toInt receiver type`、`Float.toInt return value`、`String.byteLength load type`、`Int.hash receiver value`、`Char.hash receiver value` |
| B-32 | print / panic / sysroot 桥接 | B | `__scoop_print_string arg type`、`sysroot panic message value`、`sysroot print/println String arg value` |
| B-33 | Extern global / FunPtr 顶层 | B | `top-level FunPtr param type`、`top-level FunPtr value metadata`、`extern global type`、`atomicInt extern global type` |
| B-34 | RuntimeError / try-catch-finally | B | `RuntimeError enum layout`、`RuntimeError payload variant arity`、`completion payload target type` |
| B-35 | unsafe / NoGC / 边界 | B/C | `value primitive boundary payload requires published contract`、`single gc ptr explicit frame reload slot`、`stackmap *`、`spill slot/frame slot count mismatch` |
| B-36 | 未定义/暂未支持的 spec surface（async / generator / yield / class literal 运行期） | D | `pass MIR statement todo`、`mutable capture (not supported yet)`、其他 `*Todo*` 残留 |

### 3.3 每个 bucket 文档（`audit/UMB_categories/B-XX.md`）必须包含

1. **Symptom**：从 inventory 抽出该 bucket 下所有 `(file, line, kind)` 表，
   并附三个最具代表性的源码片段（含上下文 ±10 行）。
2. **Root cause hypothesis**：用一段话说明"上游哪个阶段的什么不变量
   缺失，使得 codegen 走到了这里"。
3. **Spec linkage**：引用所有相关的 `language_spec-partN#section`。
4. **Expected post-fix class**：`FrontendReject` / `InternalBugSentinel` /
   `RealImpl`；如果不同 entry 在 fix 后落到不同类，必须分行枚举。
5. **Fix strategy outline**：见 §4。
6. **Fixture set pointer**：指向 `tests/fixtures/umb_fix/B-XX/...`。
7. **Open questions**：未解决的设计/规范问题（不阻塞本计划，但需登记）。

### 3.4 验收标准

- 36 份 bucket 文档全部存在；
- inventory 中每条 entry 都有 bucket 归属，无 `B-??`/`TBD`；
- bucket 总数与 inventory 总条数对齐（CSV-side 与 MD-side 双向校验
  baseline 测试通过）。

---

## 4. P3 — Spec 覆盖矩阵

### 4.1 目标

将 inventory 的"按 codegen 表面分类"转换成"按语言特性分类"，确保：

- 每一个 spec 描述的语法特性都至少有一条 fixture 验证；
- 每一处 inventory entry 都能映射回 spec 章节（除 helper-invariant 外）；
- spec 中"未定义/暂未支持"的章节在 inventory 中不会以 codegen 兜底
  错误形式漏出，而是 spec 立场明确（要么前端拒绝、要么落到 `D`
  bucket 由本计划列入未来 spec 工作）。

### 4.2 产出 `audit/spec_coverage_matrix.md`

按 spec 6 个 part 编排，每节给出表格：

| Spec 锚 | 语法特性 | 现有正例 | 现有负例 | 新增正例（本计划） | 新增负例（本计划） | 关联 buckets | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `part1#5` | 词法 / 标识符 / 关键字 | … | … | … | … | … | … |
| … | … | … | … | … | … | … | … |

### 4.3 必须覆盖的 spec 区段（粗粒度，最终列表以 spec 实读为准）

- **Part 1**: 源文件结构、cone 包、顶层声明、词法、字面量
  （含字符 / 数字 / 字符串 / 插值）、名称作用域。
- **Part 2**: 类型总览、根/特殊类型、内建标量、引用类型、值类型、
  Nullable、装箱、`with` copy-update、泛型、类型别名、Function Type、
  GC-free 类型。
- **Part 3**: 表达式与语句、变量绑定与赋值、运算符优先级、
  调用/参数/重载、函数声明、Lambda 与函数值、Extension 函数、属性、
  控制流（if/while/for/return/break/continue）、When 与模式匹配、
  类型测试 / Smart Cast / Cast、数组字面量、Range / Progression、
  Struct literal / Closure / `do` 消歧、Operator overloading、
  Class literal、类型推断。
- **Part 4**: Effect 系统、Effect 声明、调用效果、Effect row、
  Effect polymorphism、Handler、动态 dispatch、Handler finally、
  try/catch/finally、RuntimeError、Async（spec 当前未定义 → bucket D）、
  Generator/Yield（同上）、Required effect 推断、程序边界 / Entry。
- **Part 5**: 编译期执行、`const fun`、`const val`、`comptime if`、
  `comptime for`、静态反射 intrinsic、编译期元数据、splice field、
  Platform、RTTI、Class literal、注解 / annotation class / 命名空间注解 /
  内建注解 / 编译期注解访问、`@Intrinsic` 与 sysroot 声明。
- **Part 6**: unsafe context、safe region、`@NoGC`、raw pointer、
  FFI、GC 互操作、低层边界。

### 4.4 验收标准

- 每个 spec section 的"现有 + 新增"两列至少有一条 fixture（否则在
  矩阵中显式标 `INTENTIONALLY-EMPTY: <reason>`，且 `<reason>` 必须引用
  spec 中明确说"未定义/未支持"的句子）。
- 矩阵中每个 `bucket` 链接均闭环（在 §3 表格里也存在）。
- 没有 inventory entry 找不到 spec 锚（除 helper-invariant）。

---

## 5. P4 — 修复策略（每 bucket 一份草案）

### 5.1 通用模板（所有 bucket 一致）

```
# B-XX 修复策略

## 上游契约
- 谁负责（typecheck / hir / mir / mir.materialize / strict verifier / codegen helper）
- 契约形式（强类型 invariant / impossible state / explicit gate）

## 落地路径
- A. helper invariant：
    - 引入 `MainCodegen::expect_<...>()` 之类 helper，集中 `unreachable!`/`expect`
    - 删除该 bucket 下所有 `UnsupportedMainBody { kind: ... }` 站点
- B. upstream contract：
    - 在上游引入显式拒绝 / 显式 invariant
    - 在 strict verifier 中加 baseline；
    - codegen 处改为 `unreachable!` + upstream gate 注释
- C. real implementation：
    - 引用 P5 fixture 集；按 fixture 驱动实现
    - 实现完成后 codegen 处直接走正常分支
- D. spec uncovered：
    - 立项 spec follow-up；
    - 当前阶段以 `FrontendReject` 形式拒绝，并写明 spec 缺口

## 验证锚
- 引用 §6 fixture 集 `tests/fixtures/umb_fix/B-XX/**`
- 列出该 bucket 通过的退场标准（counts、inventory diff、verifier baseline）
```

### 5.2 A 类（helper-invariant）特别说明

B-01、B-17（部分）等本质是机械重复，必须**先**完成统一 helper 抽取
（设计 doc 在 `audit/strategies/B-01.md`），避免每处单点替换造成大量
噪声。helper 命名建议：

- `MainCodegen::expect_insert_block(span) -> BasicBlock<'ctx>`
- `MainCodegen::expect_parent_function(span) -> FunctionValue<'ctx>`
- `MainCodegen::expect_entry_block(fn_, span) -> BasicBlock<'ctx>`
- `MainCodegen::expect_basic_value(call, span) -> BasicValueEnum<'ctx>`

### 5.3 B 类（upstream contract）特别说明

每条 entry 必须指定**唯一一个** `upstream_gate`，并在 P5 中至少有一条
**negative fixture** 验证该 gate 真的会拒绝畸形输入（否则 gate 就是
名义上的）。可选 gate 的位置：

- `crates/scoopc/src/typecheck/**`
- `crates/scoopc/src/hir/lower/**`
- `crates/scoopc/src/mir/lower.rs` / `crates/scoopc/src/mir/materialize/**`
- `crates/scoopc/src/mir/{verify,strict_verify}.rs`（如已存在；否则需
  在 P7 阶段创建，本计划只占位）

### 5.4 C 类（real impl）特别说明

每个 C 类 bucket 必须给出最小可行 fixture（"happy path"），且在该 fixture
通过之前，codegen 处的 `UnsupportedMainBody` **不允许**移除 ——
本计划负责把 fixture 标注为 `IGNORE-UNTIL-FIX: B-XX`，由 fixture runner
自动 skip / xfail。

### 5.5 D 类（spec uncovered）特别说明

D 类 bucket 不得在 P7 阶段进入 production 修复，必须先：

1. 在 `docs/spec/...` 增补该 surface 的 spec；
2. 在 `audit/spec_coverage_matrix.md` 中清掉对应的 `INTENTIONALLY-EMPTY`；
3. 然后才能从 D 类降级到 B/C 类继续走。

---

## 6. P5 — Fixture 集合

### 6.1 目录布局

```
tests/fixtures/umb_fix/
├── B-01-builder-invariant/
│   ├── _README.md
│   ├── pos_<spec_anchor>.scoop
│   ├── pos_<spec_anchor>.stdout
│   └── neg_<spec_anchor>.scoop      # EXPECT-ERROR 形式
├── B-02-mir-local-type/
│   └── ...
├── B-XX/
└── _index.csv
```

`_index.csv` schema：

| 字段 | 含义 |
| --- | --- |
| `fixture_path` | 相对仓库根 |
| `bucket` | B-XX |
| `kind` | positive / negative |
| `spec_anchor` | 关联 spec 锚（多值 `;` 分隔） |
| `umb_ids` | 该 fixture 旨在驱动消除的 inventory `id` 列表 |
| `status` | `active` / `ignore-until-fix:B-XX` |
| `notes` | 备注 |

### 6.2 每个 bucket 的 fixture 集最小要求

> "条件分支" 指 inventory 中**每一个不同的 `kind` 标签所属的 if/match
> 分支**。最终目标是让该 bucket 下每一个分支都至少被一条 fixture 命中。

- **Happy path（pos）**：覆盖该 bucket 对应 spec 特性的 1-N 个最小合法
  程序，每个程序要有 `.stdout` 锁定运行时输出（`run-pass` 一族）；
  对纯 typecheck 特性以 `tests/fixtures/typecheck/...` 加 `EXPECT: ok`
  形式。
- **Negative — frontend reject（neg）**：对 B 类 bucket，必须有至少
  一条 fixture 给出**会被前端 / typecheck / HIR / MIR verifier 拒绝**的
  畸形输入，并锁定 `EXPECT-ERROR-CODE` / `EXPECT-ERROR-AT` /
  `EXPECT-ERROR` 文案。
- **Negative — internal sentinel（synthetic）**：对应 A 类 bucket 的
  `unreachable!`，在 fixture 层面无法直接构造，改在 baseline 单元测试
  里（`#[should_panic(expected = "...")]` 或基于 IR 注入的 negative
  baseline）模拟，统一收敛在 `crates/scoopc/src/audit/sentinel_tests.rs`。
- **Branch 覆盖**：每个 fixture 的注释顶部必须列出 `// COVERS: UMB-0001,
  UMB-0042, UMB-0073`，由 `_index.csv` `umb_ids` 字段冗余记账。
  baseline 测试要求：每个 inventory `id` 至少出现在一条 fixture 的
  `umb_ids` 中（除 D 类 bucket，可标记 `D-pending`）。

### 6.3 通用 fixture 头部规范

正例：

```
// EXPECT: ok
// SPEC: part3#9-控制流; part3#10-When 与模式匹配
// COVERS: UMB-0123, UMB-0124
// BUCKETS: B-15
package fixtures.umb_fix.B_15
import scoop.core.*

fun main(): Unit / Pure! { ... }
```

负例（前端拒绝）：

```
// EXPECT: fail
// EXPECT-ERROR-CODE: scoop::typecheck::<...>
// EXPECT-ERROR-AT: <line:col>
// EXPECT-ERROR: <稳定文案前缀>
// SPEC: part3#10-When 与模式匹配
// COVERS: UMB-0789
// BUCKETS: B-15
// REASON: 当前 spec 不接受 when arm 缺失时回落到 codegen unsupported
package fixtures.umb_fix.B_15
...
```

### 6.4 必须新增的最小 fixture 集（按 spec part 列示）

> 下列条目是**计划层硬约束**：实际作者必须在 P5 阶段交付以下每一项；
> 不允许"看上去差不多"地省略。每条 fixture 要求 1 正例 + 至少 1 负例
> （除非 spec 明确说该形态总是合法）。

#### Part 1 — 词法、字面量、源结构
1. 整型 / 浮点 / 字符 / 字符串 / 字符串插值 / `\u{...}` 转义。
2. 包声明 + cone 边界（合法 / 跨包私有引用拒绝）。
3. 顶层 `val` / `var` / `const val` / `fun` / `class` / `enum` / `object` /
   `interface` / `extension` / `typealias` 各 1 正例。

#### Part 2 — 类型系统
4. 标量值类型 + 装箱往返。
5. 引用类型层级（Any / String / 自定义 class）。
6. 值类型 nominal struct / data class / enum。
7. `Option<T>` Niche / non-niche 对比。
8. 泛型函数 / 泛型类 / 多参 / variance / star projection。
9. 类型别名（普通 + 泛型 + 在 receiver 上）。
10. Function type （有 / 无 receiver、有 / 无 effect row、`Pure!`）。
11. GC-free 类型（标注 + 验证）。
12. `with` copy-update（合法 + 缺字段 / 多字段负例）。

#### Part 3 — 表达式 / 函数 / 模式
13. 运算符优先级（spec 表逐行）正反例。
14. 函数定义：默认参数、命名参数、可变参数（如 spec 支持）、
    返回类型推断、tail expression、early return、block expression。
15. Lambda：单参省略 / 闭包捕获不可变 / 闭包捕获可变 / receiver lambda /
    `it` 推断、显式 `this.<...>`。
16. Extension fun（含泛型、含 receiver constraint）。
17. 属性 getter/setter / backing field。
18. `if` / `while` / `for` / `return` / `break` / `continue` 各种作用域
    内外、嵌套 break / labeled break（如 spec 支持）。
19. `when`：表达式 / 语句、subject / no-subject、bool / int / char / string /
    enum / nullable / tuple / variant / guard / 缺 arm 检查。
20. `as` / `as?` / `is` 各 scalar / ref / nominal / function-type
    （function-type 强制 frontend reject）。
21. 数组字面量 + element type 推断 / 嵌套。
22. Range / Progression（`..` / `..<` / `step`）。
23. Struct literal / `do` 消歧：与花括号 lambda 冲突的所有 spec 行为。
24. Operator overloading（`+` / `==` / `compareTo` / `invoke` / `get`/`set`）。
25. Class literal（`Foo::class`）。
26. 类型推断（target-typed if、common super、function value）。

#### Part 4 — Effect 系统
27. `effect` 声明 + `eff` 调用。
28. Effect row 在函数签名中、effect polymorphism（`<E: Pure>`）。
29. Handler 静态 / 动态 dispatch、resume / abort / control transfer。
30. Handler `finally`。
31. `try` / `catch` / `finally` + RuntimeError 引发 / catch 子句类型分发。
32. Required effect 推断（自动收紧到调用方签名）。
33. Entry point：`main(): Unit / Pure!`、`main(): Int / Pure!`、
    `main(args: Array<String>): Unit / Pure!`、
    `main(args: Array<String>): Int / Pure!` 四个签名 + 重复 main /
    错误签名前端拒绝。
34. Async / Generator / Yield → 触发 spec 当前未定义的负例
    （`EXPECT-ERROR` 锁定文案，关联 D 类 bucket）。

#### Part 5 — 编译期 / 反射 / 注解
35. `const fun` / `const val` / 编译期纯度违规拒绝。
36. `comptime if` / `comptime for` 展开正反例。
37. `descOf` / `kindOf` / `sizeOf` / `alignOf` 反射 intrinsic。
38. Splice field `value.[field]`（合法 + 动态字段名拒绝）。
39. Platform introspection。
40. RTTI（`is` 在引用类型 / nominal / Any 上）。
41. Annotation 声明 + 使用 + 命名空间 + 内建注解 + 编译期访问。
42. `@Intrinsic` + sysroot 声明。

#### Part 6 — unsafe / FFI / GC
43. `unsafe { ... }` 块允许 vs 普通块禁止的全部操作。
44. Safe region。
45. `@NoGC` 允许 / 违规拒绝。
46. Raw pointer 操作 + funptr 互转。
47. FFI 声明 + 调用。
48. GC.handleNew / handleGet / handleDrop / pin / unpin 全形态。

#### Bucket-driven 直接对账（每个 B-XX 自身）
49. 每个 §3.2 中列出的 bucket 至少有一条**专门**针对 inventory 中该
    bucket 的代表 entry 的 fixture（即使在上面 1-48 中已经覆盖了，也要
    在 `_index.csv` 中显式登记 `umb_ids`）。

### 6.5 fixture 撰写禁区

- ❌ 不允许**新增** `UnsupportedMainBody { kind: "..." }` 站点；
- ❌ 不允许在 fixture 中使用 sysroot 之外尚未定义的库 API；
- ❌ 不允许 `EXPECT-ERROR` 文案出现 `FRONTEND_REJECT_FORBIDDEN_TERMS`
  禁词（"后端"、"backend"、"LLVM"、"codegen"、"UnsupportedMainBody"）；
- ❌ 不允许 fixture 间互相 import（保持单文件可读）。

---

## 7. P6 — Baseline 测试

### 7.1 必须新增的 baseline 测试（位置：`crates/scoopc/src/audit/`）

1. `umb_inventory_csv_in_sync` — CSV ⟷ 源码 grep 结果完全一致。
2. `umb_inventory_buckets_total` — 每个 bucket 的 entry 数 == bucket md
   表头声明的数；总和 == 1 277。
3. `umb_inventory_each_entry_has_spec_anchor_or_helper_marker`。
4. `umb_inventory_class_distribution` — 三类 entry 数与 bucket md 中
   `Expected post-fix class` 段的数字对账。
5. `umb_fix_fixture_index_in_sync` — `tests/fixtures/umb_fix/_index.csv`
   ⟷ 实际目录扫描结果一致。
6. `umb_fix_every_inventory_id_is_covered` — 每个 `UMB-XXXX` 至少出现
   在一条 fixture 的 `// COVERS:` 行（D 类例外）。
7. `umb_fix_every_bucket_has_at_least_one_pos_and_one_neg` — A 类 bucket
   只要求 sentinel test，不强制 negative fixture。
8. `umb_fix_spec_coverage_matrix_in_sync` — `audit/spec_coverage_matrix.md`
   每行的 fixture 引用都真实存在。
9. `umb_fix_no_forbidden_terms_in_neg_messages`。
10. `umb_fix_helper_invariant_sentinel_tests_present` —
    `crates/scoopc/src/audit/sentinel_tests.rs` 中每个 A 类 bucket
    至少一个 `#[should_panic]` 单测。

### 7.2 baseline 测试与 P7 production 修复的衔接

- 每完成一个 bucket 的 production 修复，对应 baseline 测试的 "数字"
  必须同步下调；
- `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 中的 `expected_count` 字段调整
  规则在本计划中显式定义：每删一个 production `UnsupportedMainBody`
  必须在同一 commit 中减 1（或转为 `unreachable!` 并从 inventory 删行）；
- `UMB inventory baseline test`（§7.1.1）会作为最终退场计数器：
  当其断言 `len == 0` 通过时，`UnsupportedMainBody` variant 即可在
  P8 阶段从 `LlvmEmitError` 中物理删除。

---

## 8. 文档 / 规范交付物清单

本计划阶段（P1-P6）必须交付以下文件，缺一不可：

```
UnsupportedMainBody_FIX.md                                # 本文件
audit/UMB_inventory.csv                                   # P1
audit/UMB_inventory_schema.md                             # P1
audit/UMB_categories/B-01.md ... B-36.md                  # P2
audit/spec_coverage_matrix.md                             # P3
audit/strategies/B-01.md ... B-36.md                      # P4
tests/fixtures/umb_fix/_index.csv                         # P5
tests/fixtures/umb_fix/B-01-builder-invariant/...         # P5
tests/fixtures/umb_fix/B-02-.../...                       # P5
... (per bucket)
crates/scoopc/src/audit/umb_inventory.rs                  # P6 (test-only)
crates/scoopc/src/audit/sentinel_tests.rs                 # P6 (test-only)
crates/scoopc/src/audit/spec_coverage.rs                  # P6 (test-only)
```

> 备注：`crates/scoopc/src/audit/` 整个 module 必须以 `#[cfg(test)]`
> 限定，禁止参与 production codegen 链路；如果 cargo 结构需要它单独
> 成 crate，则建立 `crates/scoopc-audit/` 并在 workspace 中显式标记
> `dev-dependency` only。

---

## 9. 退场标准（P7/P8 阶段使用，本计划只声明）

P7 production 修复完成的判据：

1. `audit/UMB_inventory.csv` 行数 == 0；
2. `git grep -n "UnsupportedMainBody" -- crates/scoopc/src/llvm/` 仅在
   `mod.rs` 的 enum 定义、`pipeline_user_visible_failure_policy.rs`
   的历史记录处出现（≤ 5 处），其它皆已清除；
3. `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 全部清空；
4. `pipeline_user_visible_failure_policy_tracks_stale_unsupportedmainbody_counts`
   断言 `total == 0`；
5. `tests/fixtures/umb_fix/**` 全部 active（无 `ignore-until-fix:`
   状态），`cargo run -p scoop -- test` 全部 PASS；
6. `audit/spec_coverage_matrix.md` 中所有 `INTENTIONALLY-EMPTY` 都引用
   仍然有效的 spec 立场，没有"未明示"的 D 类残留；
7. spec part 1-6 的每一节都有至少一条 fixture（自检脚本通过）。

P8 退场动作（一次性）：

1. 从 `LlvmEmitError` 删除 `UnsupportedMainBody` 变体；
2. 删除 `pipeline_user_visible_failure_policy.rs` 中相关常量；
3. 保留 `audit/UMB_inventory.csv` 历史快照在 `docs/archive/` 作为 ledger；
4. 在 `docs/archive/designs/PIPELINE_GAPS.md` 增加 "P8 终态" 段落，
   宣告本批 gap 收口。

---

## 10. 工作流程与责任划分

| Phase | 输入 | 任务步骤 | 产出 | 验证 |
| --- | --- | --- | --- | --- |
| P1 | 当前源码 | 写 inventory 脚本 → 跑 → 落 CSV → 写 schema md | CSV + schema | baseline test #1 |
| P2 | P1 CSV | 36 个 bucket 各写 1 份 md | bucket md | baseline test #2-#4 |
| P3 | spec 6 part + P1 CSV + P2 md | 编 spec 覆盖矩阵 | matrix md | baseline test #8 |
| P4 | P2 + P3 | 36 份策略草案 | strategy md | review-only |
| P5 | P3 + P4 | 写 fixture（pos + neg）| `tests/fixtures/umb_fix/**` | baseline test #5-#7, #9 |
| P6 | P1-P5 | 写 baseline test | `crates/scoopc/src/audit/**` (cfg(test)) | self |

> P1-P3 串行（后者依赖前者数据），P4-P5 可并行启动，P6 在 P1-P5
> 主体落地后串接（要等数据稳定）。

---

## 11. 已知风险与对策

1. **inventory 漂移**：源码改动会冲击 CSV。
   对策：baseline test #1 是强制 gate，PR diff 必须显式更新 CSV 才能合并。
2. **bucket 边界争议**：某些 entry 同时像 B-02 又像 B-09。
   对策：每条 entry 唯一 bucket；模糊归属优先归到"上游修复点更具体"
   的 bucket，并在 `notes` 字段记录另一候选。
3. **spec 缺口**：D 类 bucket 触及 spec 未定义。
   对策：P3 矩阵中 `INTENTIONALLY-EMPTY` 必须直接引用 spec 原句；
   遇到 spec 沉默时，P5 阶段写 frontend-reject 负例并把策略归到
   `BlockedOnSpec`。
4. **fixture 体量爆炸**：1 277 entry × 平均 2 fixture ≈ 上千文件。
   对策：每个 fixture 通过 `// COVERS:` 多对一覆盖 inventory id；
   只有当一条 fixture 无法物理同时触发多 entry 时才拆分。
5. **D 类阻塞**：async/generator 等 spec 工作量很大。
   对策：D 类不计入 P7 退场判据 #1 / #4 的"清零"约束（但仍要在 #6 中
   登记 `INTENTIONALLY-EMPTY`），允许独立 release。

---

## 12. 本计划自身的退场条件

当且仅当下列条件全部满足时，本计划文档可标注为 `[DONE]`：

- `audit/UMB_inventory.csv`、36 份 bucket md、36 份 strategy md、
  spec coverage matrix、`tests/fixtures/umb_fix/**` 与
  `crates/scoopc/src/audit/**` 全部存在；
- `cargo test -p scoopc audit::` 通过；
- `cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（允许带
  `IGNORE-UNTIL-FIX` 状态）；
- 本文件第 §9 节"退场标准"被另起一份 `UnsupportedMainBody_DONE.md`
  接手追踪（即本文件只负责到 P6 完成；P7/P8 由后续文件负责）。
