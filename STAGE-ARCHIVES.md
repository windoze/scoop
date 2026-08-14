# STAGE-ARCHIVES — 稳定身份与阶段 archive 格式规范（M0）

> 状态：已实施（地基类型 + 编码上移落地；容器字节格式细节随 M1 序列化实现定稿）。
> 上游：`PLAN.md` C2/C3/C7/M0。本文是这些约束的**规范文本**，实现锚点见 §6。

## 1. 身份分层（C3）

| 层 | 形态 | 生命周期 | 用途 |
|---|---|---|---|
| 跨 cone 稳定 id | `StableDefKey` → `path_hash`（hex16） | 跨构建、可序列化 | 跨 cone 引用、序列化、增量复用、lang-items 注册表 |
| cone 内 dense id | `u32` arena 下标（定型 newtype） | 单次装配会话内 | live 引用（deref 不可失败、不比字符串） |
| body 内 local id | `LocalId` / `ExprId` | 单个函数体 | 局部绑定 / 表达式节点；**不是 def**，不外泄、不序列化为稳定 id |

规则：

- **稳定 id 只从稳定输入派生**（cone key + 名字路径 + 声明种类 + 重载消歧 key）。
  禁止全局计数器（编译顺序敏感）、禁止注册顺序（`intern_cone` 的 `ConeId` 是会话内
  dense 下标，**不**作跨构建身份——跨构建一律用 `ConeInfo.stable_key`）。
- 装配（load）是**唯一**「按稳定 key 解析」的可失败点：stable key → 本会话 dense id
  一次性解析，之后全程不可失败 deref（对齐 FACT_REFACTOR §8.1）。
- closure / 匿名合成 element：cone 内匿名 def id（owner def + 序号派生的稳定 key），
  **不放** body-local 层（代码体要独立 lower、sibling 提升需要跨函数引用）。

## 2. 两套身份：定义 vs 实例

同纪律、不同 key 值（不共用同一 key）：

### 2.1 定义身份（HIR 层，`scoop2_hir::stable_id::StableDefKey`）

指向「**哪个声明**」。canonical 形如：

```
def({cone}::{namespace}/{owner.}{name}[/{overload_key}]#{kind})
```

- `cone`：`StableConeKey`（§3）。
- `namespace`：三命名空间之一（`ty` / `val` / `fun`）。
- `owner`：成员声明为其类型 FQN 文本；顶层为空。
- `overload_key`：仅 Fun 命名空间；见 §2.3。
- `kind`：`DefKind`（fn/meth/ctor/variant/field/effop/global/tydecl/object/ext）。

紧凑形态 `path_hash(scope)`：scope 前缀 FNV-1a hex16。跨 cone 引用与 map key 用
hash；canonical 文本仅调试 / 诊断。

### 2.2 实例身份（MIR 层，`scoop2_mir` 的 `StableTemplateKey` / `StableInstanceKey`）

指向「**哪个模板的哪组实参实例**」。模板 key = `template(fqn;[type_params];sig=…)`；
实例 key = 模板 key + canonical 类型实参 + effect 行实参。M3 起 MIR archive 的
element 主键。

### 2.3 重载消歧

- **HIR 定义层**（`overload_disambiguation_key`）：`[p1,p2,…*]->ret/E(row)`
  （vararg 尾缀 `*`；参数 + 返回 + effect 行 + vararg 全参与）。
- **MIR 层**（`build_overload_sig`）：仅参数类型 canonical——为保既有 stable key
  字节稳定**不变更**。两层是不同 key，不混用。

## 3. Cone 稳定 key（C2）

- `StableConeKey::from_cone_name(name)`：包名 verbatim；空白名归一 `<root>`。
- cone 名来自 resolve 层（包声明前缀；无包声明的 fallback 名 `<user>` / `<sysroot>`）。
- 同名 cone 幂等注册；不同 cone 同名即冲突（装配期报错）。

## 4. canonical 类型文本

`scoop2_hir::stable_id::canonical_type_text`（自 scoop2_mir 上移，**编码字节不变**）：
`V(Int)` / `V(Option<…>)` / `T(…)` / `N(fqn<args;eff=…>)` / `F(recv;[params]->ret/row)` /
`P(name)` / `U(sorted)` / effect 行 `E(sorted)!` / `Pure`。

已知限制：深度 > 64 输出 `?depth` 占位（语言内递归只能经 nominal 浅编码，正常深度
远低于上限；保留是为与既有 key 字节兼容）。

## 5. archive 容器（字节格式在 M1 实现时定稿，本节为规范）

### 5.1 版本头

- magic + `schema_version`（`archive_schema::{V0,V1}`）+ `compiler_version()`
  （workspace 版本）+ `stage`（hir/mir/lir）+ cone 稳定 key。
- 加载方对比版本与 compiler 版本，**不匹配即拒绝，不做迁移**（C7）。
- `V0`：M1 过渡格式（per-cone 序列化「AST + TypedHir 现状」，允许 AST 片段与
  集合级共享 arena 段；M2/M3 后删除）。`V1`：正式 per-cone archive。

### 5.2 指纹（缓存失效键）

```
fingerprint = FNV( schema_version, compiler_version, stage, cone_key,
                   sorted{input_cone_keys}, sorted{params} )
```

- `input_cone_keys`：本阶段消费的上游 archive 成员稳定 key **排序集**（成员序无关）。
- `params`：影响产出的全局参数（entry、target、opt level 等）。
- 语义：**指纹相同 ⇒ 产出可复用；不同 ⇒ 缓存必须失效**。MIR 的可达性与去虚化
  依赖全集（新增 cone 可引入子类使旧去虚化结论失效），故集合构成一等成员。
- 所有字符串喂入带长度前缀（`"ab"+"c" ≠ "a"+"bc"`）。

### 5.3 段布局与字节稳定

- 段：符号（intern 表）/ 类型（arena）/ element / body / 元数据（lang-items 标记、
  span、可见性、init 定序、可达集）。
- **字节稳定规则**（C7）：序列化遍历一律按确定性顺序（element 声明序、集合按
  stable key 排序）；**禁止 HashMap 迭代序进入字节流**。同输入必须逐字节同输出。
- 每层 archive **自包含**（类型/符号/span 逐层携带；LIR 不引用回 HIR archive）。

## 6. 实现锚点（M0 落地）

| 规范条目 | 实现 |
|---|---|
| scope 哈希 / FNV 核心 | `scoop2_base::stable::{stable_hash, StableHashScope, fnv1a_*}` |
| `StableConeKey` | `scoop2_base::stable::StableConeKey`；`ConeInfo.stable_key`（resolve/symbol.rs） |
| 版本 / 指纹 | `scoop2_base::stable::{archive_schema, compiler_version, archive_fingerprint, ArchiveFingerprintBuilder, ArchiveStage}` |
| canonical 类型文本 | `scoop2_hir::stable_id::{canonical_type_text, canonical_effect_row_text, canonical_effect_row_closed_text}` |
| 定义身份 / 重载消歧 | `scoop2_hir::stable_id::{StableDefKey, DefNamespace, DefKind, overload_disambiguation_key}` |
| 实例身份（既有） | `scoop2_mir::mir::stable_id::{make_stable_template_key, make_stable_instance_key, build_overload_sig, compute_public_stable_keys}`（编码/哈希改为复用上述两层） |

## 7. 验证记录（M0 验收）

- `cargo test -p scoop2_base -p scoop2_hir -p scoop2_mir -p scoop2_syntax -p scoop2_lir -p scoop2c` 全绿。
- 字节稳定回归：`dump-mir tests/fixtures/mir2/arithmetic.scoop` 在改动前后
  （stash 往返）**逐字节一致**——canonical 编码与哈希上移未改变任何 stable key。
- `cargo test --all` 当前被**既有**问题阻塞：`scoopc_hir/src/session/mod.rs` 自
  `dd19fe9d WIP` 起有未闭合分隔符（旧管线，与本里程碑无关）。
