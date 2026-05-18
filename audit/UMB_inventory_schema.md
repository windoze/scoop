# UMB Inventory Schema

生成时间：2026-05-18  
数据文件：[`audit/UMB_inventory.csv`](./UMB_inventory.csv)  
冻结基线：[`audit/UMB_inventory_initial.csv`](./UMB_inventory_initial.csv)
退场 ledger：[`audit/UMB_retired.csv`](./UMB_retired.csv)
生成/对账入口：`cargo run -p scoopc --bin umb-audit -- diff`

## 目标

`audit/UMB_inventory.csv` 是当前 active `LlvmEmitError::UnsupportedMainBody` 治理主表。每一行对应 `crates/scoopc/src/llvm/codegen/**/*.rs` 中一个尚未 retired 的 `UnsupportedMainBody { ... }` constructor，并为后续 bucket 文档、策略、fixture 和 baseline test 提供稳定引用。`audit/UMB_inventory_initial.csv` 保留 P7 开始前的 1,284 行 immutable baseline；`audit/UMB_retired.csv` 记录已经由 production 修复退场的 initial ID。

## 排序与稳定 ID

- 扫描范围固定为 `crates/scoopc/src/llvm/codegen/**/*.rs`。
- `audit/UMB_inventory_initial.csv` 的 `id` 是 P7 immutable baseline，格式为 `UMB-NNNN`，从 `UMB-0001` 到 `UMB-1284` 连续编号。
- `audit/UMB_inventory.csv` 继承 initial baseline ID；删除源码 row 后，不得按当前扫描顺序重排剩余 ID。
- stable ID 匹配策略依次为：exact `(file,line,kind)`；唯一 `(file,kind,bucket,expected_class,surface)`；同组按 initial `old_line` 与当前 source line 升序配对。
- 无法唯一匹配的 row 必须报错，禁止自动生成新 ID 或重号。
- active IDs 与 retired IDs 必须互斥，二者并集必须等于 initial baseline 的 1,284 个 ID。

## CSV 格式

表头必须严格为：

```csv
id,file,line,kind,route,surface,bucket,expected_class,spec_anchor,upstream_gate,existing_fixture,notes
```

CSV escaping 规则：

- 字段包含逗号、双引号、换行或回车时，必须用双引号包裹。
- 字段内部的双引号必须写成两个双引号。
- 字段不包含上述字符时不加引号。
- 行尾使用 `\n`。
- 不允许额外列、缺失列或重复表头。

## 字段定义

| 字段 | 取值域 | 合法性规则 | 对账规则 |
|---|---|---|---|
| `id` | `UMB-NNNN` | active CSV 内必须唯一；initial baseline 内必须连续、4 位数字 | `umb-audit diff` 通过 initial/retired 继承 stable ID 后比较 |
| `file` | 仓库相对路径 | 必须位于 `crates/scoopc/src/llvm/codegen/` 下 | 必须与扫描到的 constructor 文件一致 |
| `line` | 1-based 正整数 | 必须是 constructor 起始行 | line drift 由 `umb-audit diff` 报告 |
| `kind` | `kind:` 字面量或 `DYNAMIC:<expr>` | 不得为空；动态/转发式字段必须以 `DYNAMIC:` 标注 | literal kind 出现次数必须与源码 constructor-scoped 扫描一致 |
| `route` | `RawMirLlvm` / `EffectLoweredLlvm` / `Helper` | 由源码路径推导；不得为空 | field drift 由 `umb-audit diff` 报告 |
| `surface` | `stmt` / `rvalue` / `terminator` / `type` / `builder` / `intrinsic` | 表示触发点表达层；不得为空 | field drift 由 `umb-audit diff` 报告 |
| `bucket` | `B-01` 到 `B-36` | 必须唯一归属一个 bucket；禁止 `TBD` | `umb-audit stats` 输出每 bucket 数量 |
| `expected_class` | `FrontendReject` / `InternalBugSentinel` / `RealImpl` | 禁止 `TBD`；helper invariant 必须是 `InternalBugSentinel` | `umb-audit stats` 输出每 class 数量 |
| `spec_anchor` | spec 锚列表或 `N/A:helper-invariant` | 非 helper entry 必须非空；多值用 `;` 分隔；禁止 `TBD` | `umb-audit stats` 输出缺失数量 |
| `upstream_gate` | 明确 gate 描述 | 必须非空；B 类 entry 必须写真实上游 gate；禁止 `TBD` | `umb-audit stats` 输出缺失数量 |
| `existing_fixture` | fixture 路径或空字符串 | 已有覆盖 fixture 才填写；多 fixture 后续用 `;` 分隔 | U5/U6 与 fixture index 对账 |
| `notes` | 短备注 | 可含 `bucket_rule=...`、`legacy_gap=...`、第二候选说明；禁止用 `TBD` 代替决策 | field drift 由 `umb-audit diff` 报告 |

## 合法 Bucket 表

| Bucket | 名称 | 一级类 |
|---|---|---|
| B-01 | inkwell builder bookkeeping | A |
| B-02 | MIR local / member 类型推断不完整 | B |
| B-03 | MIR direct/closure/funptr 调用 ABI 漂移 | B |
| B-04 | MIR 函数签名 / 参数 / 返回类型缺失 | B |
| B-05 | MIR CFG / start block / goto target 异常 | B |
| B-06 | MIR struct/tuple/enum 字面量 schema 漂移 | B |
| B-07 | MIR pattern 子句 schema 漂移 | B |
| B-08 | MIR 成员存取 / 赋值合法性 | B |
| B-09 | Cross-TypeStore equivalence 不闭合 | B/C |
| B-10 | Effect-typed callable adapter / ABI routing | C |
| B-11 | Pure / plain statement 边界路由 | B |
| B-12 | Closure / lambda / capture 表达 | C |
| B-13 | 数组 / 复合 transport metadata | C |
| B-14 | Cast / TypeCheck (`as`/`as?`/`is`) | B/C |
| B-15 | When / 模式匹配用户面 | B |
| B-16 | 控制流 outside-of-context | B |
| B-17 | Coercion / 标量运算 | A/B |
| B-18 | 字面量与字符串 | B |
| B-19 | Top-level / object init / extern global | B |
| B-20 | Class ctor / property / 字段访问 | B |
| B-21 | Struct literal / 字段层 | B |
| B-22 | Enum 布局 / niche / Option | B |
| B-23 | Member access - 通用 | B |
| B-24 | Reflection / comptime intrinsic | C/D |
| B-25 | Platform / RTTI intrinsic | B/C |
| B-26 | atomic intrinsic 系列 | B |
| B-27 | sync intrinsic 系列 | B |
| B-28 | thread intrinsic 系列 | B |
| B-29 | GC intrinsic 系列 | B |
| B-30 | named / unsafe / FunPtr intrinsic | B |
| B-31 | 标量扩展方法 (Float/Int/Char/Bool/String) | B |
| B-32 | print / panic / sysroot 桥接 | B |
| B-33 | Extern global / FunPtr 顶层 | B |
| B-34 | RuntimeError / try-catch-finally | B |
| B-35 | unsafe / NoGC / 边界 | B/C |
| B-36 | 未定义/暂未支持的 spec surface | D |

## Retired Ledger 格式

`audit/UMB_retired.csv` 表头必须严格为：

```csv
id,bucket,expected_class,file,old_line,kind,retired_by,retired_reason,retired_at_notes
```

- `id,bucket,expected_class,file,old_line,kind` 必须与 `audit/UMB_inventory_initial.csv` 中对应 initial row 一致。
- `retired_by` 记录退场任务或提交；`retired_reason` 记录 production 修复原因。
- 当前 P7-0-T01 状态不退场 production row，因此 ledger 只有 header，active=1,284、retired=0、initial=1,284。

拆分或合并流程：

- 不得只修改 CSV。任何 bucket 拆分、合并、重命名或一级类变更，必须同步更新 `PLAN.md` §1.4/§3.2 与 `UnsupportedMainBody_FIX.md` §3.2。
- 修改后必须重新运行 `cargo run -p scoopc --bin umb-audit -- diff` 和 `cargo run -p scoopc --bin umb-audit -- stats`。
- 影响已有 `UMB-NNNN` 的变更必须在对应任务完成记录中说明原因。

## 对账命令

- `cargo run -p scoopc --bin umb-audit -- list --bucket B-02`：列出指定 bucket 的 entry。也支持 `--file PATH` 与 `--class CLASS`。
- `cargo run -p scoopc --bin umb-audit -- diff`：重扫源码并与 `audit/UMB_inventory.csv` 比较；报告新增、删除、line drift、kind drift、field drift；无漂移时退出码为 0。
- `cargo run -p scoopc --bin umb-audit -- stats`：输出每 bucket、每 class、每 file 的 entry 数，以及缺失 `spec_anchor` / `upstream_gate` 数。
