# `@ReleaseHook` + `@NoGC` 强制 Pure TODO

> 生成时间：2026-05-30
> 计划基线：[`PLAN.md`](./PLAN.md)
> 格式参考：[`docs/archive/plans/TODO-gc-pacing-immortal.md`](./docs/archive/plans/TODO-gc-pacing-immortal.md)、[`docs/archive/plans/TODO-1-gc-pacing-immortal.md`](./docs/archive/plans/TODO-1-gc-pacing-immortal.md)
> 当前状态：任务已拆为 P0-P5 六阶段，全部 `[TODO]`。每个实现任务后紧跟一个独立 review 任务，编号为原任务 ID + `R`。
> 行号说明：下文行号以当前文件状态为准；实现时若漂移，优先按文件路径、函数名、fixture 名定位。

## 总原则

- `PLAN.md` 是当前执行计划基线；实现时若发现阶段边界或 contract 需要变化，必须先回写 `PLAN.md`，再调整 TODO。
- 任务按 P0 → P1 → P2 → P3 顺序推进，不跨阶段并行实现。**P0 是 P1 的硬前置**：`@ReleaseHook` 的安全性依赖 `@NoGC` 已被保证 Pure。
- 每个实现任务后必须紧跟一个独立 review 任务，复审完整变更、阶段目标与约束遵守情况；review 不是形式检查，发现未达标必须直接修正或阻塞下一任务。
- 任务完成后更新本文件中对应任务的状态（`[TODO]`→`[DONE]`）与完成记录。
- **安全闭环不可削弱**：释放函数只能是 `@NoGC` 或 `@Extern(abi="c")`；`args` 只能是 GC-free 字段；宿主只能是 non-generic + final class 且带 `@Experimental(feature="releaseHook")`。任何放宽都必须先回写 `PLAN.md` 并补安全论证。
- **best-effort 语义不可被误记为 finalizer**：退出时存活对象不回收是预期行为，不得为「保证退出时调用」而引入 atexit/teardown 清理。
- 所有 runtime/codegen 改动必须保持 `baseline` / `immix` / `hosted` / `minimal` 后端可编译可回归。

## 任务包划分

| 阶段 | 覆盖 PLAN 阶段 | 目标 |
| --- | --- | --- |
| P0 | §5 / P0 | `@NoGC` 强制 Pure（独立正确性前置修复） |
| P1 | §5 / P1 | `@ReleaseHook` 注解 surface 与 front-end/HIR 全套校验 |
| P2 | §5 / P2 | trampoline codegen + 填 `release_fn` |
| P3 | §5 / P3 | 验证矩阵、四后端/跨平台回归、demo 用例与文档/spec 回写 |
| P4 | §5 / P4 | `scoop.sync` 迁移到 `@ReleaseHook`，删除 `Once.run` `@Intrinsic` 与全部编译器硬编码 |
| P5 | §5 / P5 | `lazy`/`observable`/`vetoable` 降为普通库 class，删除属性委托全部 by-name 特判 |

## 具体任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| P0-T01 | [DONE] | 新增 `@NoGC` effect 契约校验并接入 fun decl 检查 |
| P0-T01R | [DONE] | Review P0-T01 `@NoGC` Pure 契约 |
| P0-T02 | [DONE] | `@NoGC` Pure typecheck fixtures + spec 回写 |
| P0-T02R | [DONE] | Review P0-T02 fixtures 与 spec |
| P1-T01 | [DONE] | `@ReleaseHook` 注解种类、识别与参数解析 |
| P1-T01R | [DONE] | Review P1-T01 注解 surface |
| P1-T02 | [DONE] | 宿主校验：class / non-generic / final / `@Experimental` |
| P1-T02R | [DONE] | Review P1-T02 宿主校验 |
| P1-T03 | [DONE] | 释放函数签名校验 + `args` 字段 GC-free/类型匹配 + HIR side table |
| P1-T03R | [DONE] | Review P1-T03 函数/字段校验 |
| P1-T04 | [DONE] | `@ReleaseHook` typecheck fixtures（错误面 + 正例） |
| P1-T04R | [DONE] | Review P1-T04 fixtures |
| P2-T01 | [DONE] | 生成 release trampoline（按字段读值并调用释放函数） |
| P2-T01R | [DONE] | Review P2-T01 trampoline |
| P2-T02 | [DONE] | 在 type descriptor 填 `release_fn` + IR fixtures |
| P2-T02R | [DONE] | Review P2-T02 descriptor 接线 |
| P3-T01 | [DONE] | run-pass 端到端 + 四后端 parity + 跨平台矩阵 |
| P3-T01R | [DONE] | Review P3-T01 验证矩阵 |
| P3-T02 | [DONE] | 最小 demo 用例 + spec/runtime 文档回写 |
| P3-T02R | [DONE] | Review P3-T02 用例与文档 |
| P4-T01 | [DONE] | 重写 `sync.scoop`：三类型降为 `@ReleaseHook` class，`Once.run` 纯 Scoop 化 |
| P4-T01R | [DONE] | Review P4-T01 sync 源改造 |
| P4-T02 | [DONE] | 收缩 `scoop_sync.c` 为只管 raw native handle |
| P4-T02R | [DONE] | Review P4-T02 runtime 收缩 |
| P4-T03 | [DONE] | 删除 `Once.run` intrinsic 全套 codegen/runtime 硬编码 |
| P4-T03R | [DONE] | Review P4-T03 intrinsic 删除 |
| P4-T04 | [DONE] | 删除/重指 effect-facts 白名单与其余 `scoop.sync` 特判（含 lazy 属性引用决策） |
| P4-T04R | [DONE] | Review P4-T04 特判清理 |
| P4-T05 | [DONE] | sync 全量回归 + 四后端/跨平台 + 零硬编码 grep 守卫 |
| P4-T05R | [DONE] | Review P4-T05 回归与守卫 |
| P5-T00 | [DONE] | 修复泛型 class 实现参数化 interface 的 itable stable type id |
| P5-T00R | [DONE] | Review P5-T00 itable 泛型 interface 修复 |
| P5-T01 | [DONE] | 在 `scoop.delegates` 写 lazy/observable/vetoable 库实现，降级顶层函数 |
| P5-T01R | [DONE] | Review P5-T01 委托库实现 |
| P5-T02 | [TODO] | 删除三者 by-name 合成、backing 字段注入与 `ParsedStdDelegateExpr` 分叉 |
| P5-T02R | [TODO] | Review P5-T02 特判删除 |
| P5-T03 | [TODO] | 委托回归（含 lazy 三模式）+ 守卫扩展到三者与 Mutex 注入点 |
| P5-T03R | [TODO] | Review P5-T03 回归与守卫 |

## 阶段间验收门禁

- 进入 P1 前：`@NoGC` 已强制 Pure（带 effect 的 `@NoGC` 函数被编译期拒绝），且与 `@Extern` 既有 Pure 检查无重复/冲突诊断；P0 fixtures 与 spec 已绿并通过 review。
- 进入 P2 前：`@ReleaseHook` 的所有非法形态在 typecheck 阶段被拒绝，正例通过；校验结果（目标函数 FQN + 有序字段名）已落入 HIR side table 供 codegen 消费。
- 进入 P3 前：带 `@ReleaseHook` 的类型 descriptor `release_fn` 非 null 且指向正确 trampoline，trampoline 以正确偏移/顺序/类型调用目标函数；无注解类型 `release_fn` 仍为 null（IR fixture 锁定）。
- 进入 P4 前：P3 已收口，`@ReleaseHook` 机制在四后端 + 双平台验证通过；demo 类型工作，spec/runtime 文档已同步。`scoop.sync` 改造必须建立在已验证机制之上。
- 完成 P3 后：端到端 + 四后端 + 双平台全绿；`@ReleaseHook` 与 `@NoGC` Pure 语义写入 spec 与 runtime 文档；最小 demo 类型以纯 Scoop + FFI 形式工作。
- 完成 P4 后：`Mutex`/`CondVar`/`Once` 为纯 Scoop final class + `@ReleaseHook` + `@Extern(abi="c")`；`Once.run` 无 `@Intrinsic`；编译器内无这三类型的实现性硬编码（消费侧引用按 P4-T04 决策处理并被守卫测试锁定）；sync 全量回归与四后端/跨平台绿，且语义与迁移前逐项一致。
- 进入 P5 前：P4 已收口，`Mutex`/`Once` 已是库类（线程安全委托要内部组合它们）。
- 完成 P5 后：`lazy`/`observable`/`vetoable` 为纯库 class，三个顶层函数无 `@Intrinsic`；编译器属性委托 lowering 只剩泛型路径；`impl_lowering.rs` 的 `SYNC_MUTEX_*` 常量与 `sugar.rs`/`decls.rs` 三者合成全部删除；语义（含 lazy 三模式）与迁移前逐项一致，回归与守卫绿。

---

## P0：`@NoGC` 强制 Pure

### [DONE] P0-T01：新增 `@NoGC` effect 契约校验并接入 fun decl 检查

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0、§1（`@NoGC` 缺陷）、§2（安全论证）
  - `crates/scoopc_hir/src/typecheck/annotations.rs`（`check_extern_fun_effect_contract:2509-2525`、`check_builtin_annotations_on_fun_decl:2301`、`AnnotationError` 变体约 :376）
- 目标：
  - 让 `abi = "scoop"` 的 `@NoGC` 函数禁止携带 effect（禁 `eff_param`、要求 `effects.terms` 为空），与 `@Extern` 一致。
- 必须检查的文件/位置：
  - `check_extern_fun_effect_contract`（镜像对象）
  - `check_builtin_annotations_on_fun_decl` 中 `@NoGC` 分支与 `@Extern` 分支（确认 extern 隐含 `@NoGC` 的现状，避免重复诊断）
  - `BuiltinAnnotationKind`（`builtin_annotations.rs:19-47`）确认 `NoGC` 变体
- 必须实现的内容：
  1. 新增 `check_nogc_fun_effect_contract(fun: &ast::FunDecl) -> Result<(), AnnotationError>`，结构镜像 extern 版：`eff_param` 存在则报错；`effects.terms` 非空则报错。
  2. 视诊断需要新增/复用 `AnnotationError` 变体（如 `NoGcFunEffParamNotAllowed` / `NoGcFunEffectsNotAllowed`，参考 `ExternFun*` 变体）。
  3. 在 `check_builtin_annotations_on_fun_decl` 里，当函数显式带 `@NoGC` 时调用该检查。
- 必须遵从的约束：
  - `@Extern(abi="c")` 隐含 `@NoGC`：不得对同一 extern 函数同时触发 extern 与 nogc 两套 effect 报错；如有重叠，复用 extern 路径或在调用点排除已由 extern 覆盖的情形。
  - 不得改变 `@NoGC` 已有的调用点 gate 语义（`gates.rs:214-240`）。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test --all --all-targets`
- 完成条件：
  - 带 effect 的 `@NoGC` 函数编译期被拒绝；既有 Pure 的 `@NoGC` 用例不受影响。
- 依赖：无
- 完成记录：
  - 2026-05-30：新增 `NoGcFunEffParamNotAllowed` / `NoGcFunEffectsNotAllowed` 诊断与 `check_nogc_fun_effect_contract`；显式 `@NoGC` 且非 `@Extern` 的函数现在禁止 `eff_param` 与非空 effect row，`@Extern` 仍沿用既有 effect 契约路径以避免重复诊断。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。

### [DONE] P0-T01R：Review P0-T01 `@NoGC` Pure 契约

- 参考：P0-T01 完成记录、`check_extern_fun_effect_contract`
- 目标：独立复核 `@NoGC` Pure 契约的正确性与无误伤。
- 必须实现的内容：
  1. 复核 `eff_param` 与非空 `effects.terms` 两条路径都被拒绝。
  2. 确认 `@Extern(abi="c")`（隐含 `@NoGC`）无重复/冲突诊断。
  3. 抽样既有 `@NoGC` 用例确认无误伤。
- 必须遵从的约束：发现未达标直接修正或阻塞 P0-T02。
- 验证：`cargo test --all --all-targets`
- 完成条件：契约准确、无误伤。
- 依赖：P0-T01
- 完成记录：
  - 2026-05-30：复核 P0-T01 实现：`check_nogc_fun_effect_contract` 同时拒绝 `eff_param` 与非空 `effects.terms`；该检查只在非 `@Extern` 的显式 `@NoGC` 函数上执行，`@Extern(abi="c")` 仍由 extern 契约与重复修饰符诊断覆盖，未发现重复/冲突诊断；抽样既有 Pure `@NoGC` 正例无误伤。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/unsafe_nogc/nogc_call_nogc_function_ok.scoop`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/extern_fun_c_abi_nogc_redundant_is_error.scoop`。

### [DONE] P0-T02：`@NoGC` Pure typecheck fixtures + spec 回写

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0-T03、P0-T04
  - `SCOOP_FULL_SPEC.md`（`@NoGC` 约 :2646-2700）
  - 既有 typecheck fixture 命名/结构（`tests/fixtures/typecheck/`）
- 必须实现的内容：
  1. 新增 `tests/fixtures/typecheck/nogc_fun_with_effect_is_error.scoop`。
  2. 新增 `tests/fixtures/typecheck/nogc_fun_with_eff_param_is_error.scoop`。
  3. 新增正例 `tests/fixtures/typecheck/nogc_fun_pure_ok.scoop`。
  4. 回写 `SCOOP_FULL_SPEC.md` `@NoGC` 章节：明确 `@NoGC` 蕴含 Pure。
- 必须遵从的约束：fixture 期望输出须与实际诊断一致；spec 措辞与实现 contract 对齐。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：错误/正例 fixture 全绿；spec 已同步。
- 依赖：P0-T01
- 完成记录：
  - 2026-05-30：新增 `nogc_fun_with_effect_is_error.scoop`、`nogc_fun_with_eff_param_is_error.scoop`、`nogc_fun_pure_ok.scoop`，覆盖显式非 Pure effect row、effect-row 参数与 Pure 正例；回写 `SCOOP_FULL_SPEC.md` §15.8，明确 `@NoGC` 在声明边界蕴含 Pure（禁止 effect-row 参数，effect row 只能省略或为 `Pure` / `Pure!`）。
  - 验证：`cargo build -p scoop -p scoopc`（确保 fixture runner 使用更新后的 compiler 二进制）；`python3 tools/run_fixtures.py tests/fixtures/typecheck/nogc_fun_with_effect_is_error.scoop`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/nogc_fun_with_eff_param_is_error.scoop`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/nogc_fun_pure_ok.scoop`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P0-T02R：Review P0-T02 fixtures 与 spec

- 参考：P0-T02 完成记录
- 必须实现的内容：复核 fixture 覆盖完整（effect / eff_param / 正例）、期望输出准确、spec 与实现一致。
- 必须遵从的约束：发现缺口直接补齐或阻塞 P1。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P0 收口，可进入 P1。
- 依赖：P0-T02
- 完成记录：
  - 2026-05-30：复核 P0-T02 新增 fixtures 与 `SCOOP_FULL_SPEC.md` §15.8：`nogc_fun_with_effect_is_error.scoop` 覆盖显式非 Pure effect row，`nogc_fun_with_eff_param_is_error.scoop` 覆盖 effect-row 参数，`nogc_fun_pure_ok.scoop` 同时覆盖显式 `/ Pure` 与省略 effect row 的正例；期望诊断 code、错误文本与 span 均与实现一致，spec 已明确 `@NoGC` 在声明边界蕴含 Pure，P0 可收口进入 P1。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

---

## P1：`@ReleaseHook` 注解 surface 与校验

### [DONE] P1-T01：`@ReleaseHook` 注解种类、识别与参数解析

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1-T01
  - `crates/scoopc_hir/src/typecheck/builtin_annotations.rs`（`BuiltinAnnotationKind:19-47`、`builtin_annotation_kind:~104`、`parse_experimental_annotation:~194`）
- 注解形态：`@ReleaseHook(name = "releaseFunctionFQN", args = ["field1", "field2", ...])`
- 必须实现的内容：
  1. `BuiltinAnnotationKind` 新增 `ReleaseHook`。
  2. `builtin_annotation_kind` 识别 `["ReleaseHook"]` 与 `["scoop","core","ReleaseHook"]`。
  3. 新增 `parse_release_hook_annotation`：解析 `name`（字符串字面量 FQN）与 `args`（字符串数组），对参数形状（缺字段、类型错误、多余 key）给出清晰诊断；结构参考 `parse_experimental_annotation`。
- 必须遵从的约束：本任务只做解析与 surface 识别，不做宿主/函数/字段语义校验（留给 P1-T02/T03）。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test --all --all-targets`
- 完成条件：注解被识别，`name`/`args` 能被正确解析出结构化值。
- 依赖：P0 完成
- 完成记录：
  - 2026-05-31：新增 `BuiltinAnnotationKind::ReleaseHook`，识别 `@ReleaseHook` 与 `@scoop.core.ReleaseHook`；新增 `ReleaseHookAnnotationInfo` 与 `parse_release_hook_annotation`，解析 `name` 字符串 FQN 与 `args` 字符串数组，并对缺少参数、位置参数、未知 key、重复 key、`name` 非字符串、`args` 非字符串数组、`args` 元素非字符串给出结构化诊断；type 声明路径现在会校验 `@ReleaseHook` 参数 surface，宿主/函数/字段语义仍留给 P1-T02/P1-T03。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。

### [DONE] P1-T01R：Review P1-T01 注解 surface

- 必须实现的内容：复核识别路径（短名 + FQN）、参数解析的错误形状覆盖。
- 验证：`cargo test --all --all-targets`
- 依赖：P1-T01
- 完成记录：
  - 2026-05-31：复核 P1-T01 注解 surface：`BuiltinAnnotationKind::ReleaseHook` 覆盖短名 `@ReleaseHook` 与 FQN `@scoop.core.ReleaseHook`，两条路径均可解析出 `name` 与 `args`；`parse_release_hook_annotation` 对缺少 `name`/`args`、位置参数、未知 key、重复 key、`name` 非字符串、`args` 非字符串数组、`args` 元素非字符串均有结构化错误。补齐 review 中发现的单元测试覆盖缺口：FQN 路径同时断言解析结果，并新增缺 `args`、位置参数、重复参数、`name` 非字符串测试。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。

### [DONE] P1-T02：宿主校验（class / non-generic / final / `@Experimental`）

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1-T02
  - `crates/scoopc_hir/src/typecheck/annotations.rs`（`check_builtin_annotations_on_type_decl`）
  - `crates/scoopc_ast/src/ast/mod.rs`（`TypeDecl.type_params`、`Modifier::{Open,Abstract,Sealed}:535-537`）
- 必须实现的内容：
  1. 仅允许宿主为 **class**（拒绝 struct/enum/interface/annotation class）。
  2. **non-generic**：`type_params.is_empty()`，否则报错。
  3. **final**：modifiers 不含 `Open` / `Abstract` / `Sealed`，否则报错。
  4. **必须同时带** `@Experimental(feature = "releaseHook")`，否则报错。
  5. 每条违例独立、清晰诊断。
- 必须遵从的约束：错误信息要能直接指导用户改正（指出缺哪个条件）。
- 验证：`cargo test --all --all-targets`
- 完成条件：四类宿主约束均被强制。
- 依赖：P1-T01
- 完成记录：
  - 2026-05-31：在 type 声明内建注解检查路径中为 `@ReleaseHook` 增加宿主校验：仅接受普通 `class`（拒绝 struct / enum / interface / annotation class）、拒绝泛型宿主、拒绝 `open` / `abstract` / `sealed` 非 final 宿主，并要求同一声明带 `@Experimental(feature = "releaseHook")`；每类违例都有独立诊断。补充 Rust 单元测试覆盖合法宿主、非 class、annotation class、generic、open/abstract/sealed 与缺少 releaseHook 实验开关。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。fixture 套件通过后仅收紧了 `#[cfg(test)]` 断言覆盖，编译器产物未变，因此未重复运行 fixture 套件。

### [DONE] P1-T02R：Review P1-T02 宿主校验

- 必须实现的内容：复核四个条件（class/non-generic/final/`@Experimental`）逐一被拒绝且诊断清晰。
- 验证：`cargo test --all --all-targets`
- 依赖：P1-T02
- 完成记录：
  - 2026-05-31：复核 P1-T02 宿主校验实现：`@ReleaseHook` type 声明路径先校验注解参数 surface，再强制普通 `class`、non-generic、final（拒绝 `open` / `abstract` / `sealed`）和 `@Experimental(feature = "releaseHook")`；诊断分别指出非 class 宿主、类型参数、非 final modifier 与缺少实验开关。补齐 review 中发现的非 class 测试缺口：`release_hook_host_rejects_non_class_type` 现在同时锁定 `struct` / `enum` / `interface`，既有 `annotation class`、generic、open/abstract/sealed 与缺少 releaseHook 实验开关测试继续覆盖其余条件。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。`python3 tools/run_fixtures.py` 未重跑，因为本次仅修改 `#[cfg(test)]` 单元测试与 TODO 记录，不改变编译器产物或 fixture 行为。

### [DONE] P1-T03：释放函数签名校验 + `args` 字段校验 + HIR side table

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1-T03、P1-T04、P1-T05、§2（安全论证）
  - `crates/scoopc_hir/src/typecheck/lower.rs`（`is_gc_free_value_type:3168`）
  - `crates/scoopc_hir/src/typecheck/expr/call/gates.rs`（`@NoGC` 判定）、`@Extern` ABI 判定
- 必须实现的内容：
  1. `name` 函数校验：FQN 可解析、可访问；必须是 `@NoGC` 或 `@Extern(abi="c")`；签名为 `void f(FieldType1, ...)`（返回 Unit；参数个数/顺序与 `args` 对应）。
  2. `args` 字段校验：每个名字是该 class 字段；每个字段类型 GC-free（复用 `is_gc_free_value_type`）；字段类型与释放函数对应参数类型精确匹配（按 `args` 顺序）。
  3. 把校验结果（目标函数 FQN + 有序字段名列表）存入 HIR side table，供 P2 codegen 消费（落点/命名参考现有 annotation→codegen 传递机制）。
- 必须遵从的约束：不得放宽到允许传 `self` 或任何 GC 引用；不得接受非 `@NoGC` 且非 `@Extern(c)` 的释放函数。
- 验证：
  1. `cargo clippy --all-targets -- -D warnings`
  2. `cargo test --all --all-targets`
- 完成条件：签名/字段全部校验通过的合法用例产出可供 codegen 消费的 side table 记录。
- 依赖：P1-T02
- 完成记录：
  - 2026-05-31：新增 `@ReleaseHook` 语义校验：释放函数 FQN 必须解析为唯一可见的无 receiver 非泛型函数，且必须是 `@NoGC` 或 `@Extern(abi="c")`；返回类型必须为 Unit，参数数量/顺序/类型必须与 `args` 字段精确匹配。`args` 现在必须引用宿主 class 的真实字段，字段类型必须通过 `is_gc_free_value_type` 判定为 GC-free，禁止传入 GC ref / self / member receiver 路径。新增 AST `ReleaseHookBinding` side table，并在 HIR lowering 中发布 `ReleaseHookIndex`（class FQN -> 目标函数 FQN + 有序字段名列表）供 P2 codegen 消费；补充单元测试覆盖正例记录、非 leaf 释放函数、非 GC-free 字段、字段/参数类型不匹配和 HIR side table handoff。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P1-T03R：Review P1-T03 函数/字段校验

- 必须实现的内容：复核签名匹配（个数/顺序/类型/返回 Unit）、`@NoGC`/`@Extern(c)` 限定、GC-free 判定、side table 内容正确。
- 验证：`cargo test --all --all-targets`
- 依赖：P1-T03
- 完成记录：
  - 2026-05-31：复核 P1-T03 实现：释放函数解析为唯一可见的无 receiver 非泛型 FQN；只接受显式 `@NoGC` 或 `@Extern(abi = "c")`，拒绝普通函数与 `@Extern(abi = "scoop")`；返回类型必须为 Unit；参数个数、顺序与类型按 `args` 字段精确匹配；字段必须存在且经 `is_gc_free_value_type` 判定为 GC-free；typecheck 成功后写入 AST `ReleaseHookBinding`，HIR lowering 汇总为 `ReleaseHookIndex` 供后端消费。补齐 review 覆盖缺口：新增单元测试锁定显式 `@NoGC` 正例、`@Extern(abi = "scoop")` 拒绝、非 Unit 返回拒绝、参数个数不匹配、按 `args` 顺序匹配与 side table 字段顺序。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。`python3 tools/run_fixtures.py` 未重跑，因为本次仅新增 `#[cfg(test)]` 单元测试与 TODO 记录，不改变编译器产物或 fixture 行为。

### [DONE] P1-T04：`@ReleaseHook` typecheck fixtures（错误面 + 正例）

- 参考：[`PLAN.md`](./PLAN.md) §5 / P1-T06
- 必须实现的内容（每个错误面各一个 fixture，正例一个）：
  1. 宿主错误：generic class、open/abstract/sealed class、缺 `@Experimental`、非 class 宿主。
  2. 函数错误：释放函数不存在 / 不可访问 / 非 `@NoGC` 且非 `@Extern(c)` / 返回非 Unit / 参数个数或类型不匹配。
  3. 字段错误：`args` 字段不存在 / 非 GC-free / 类型不匹配。
  4. 正例：final non-generic class + `Ptr<T>` handle 字段 + `@Extern(abi="c")` 释放函数 + `@Experimental(feature="releaseHook")`。
- 必须遵从的约束：fixture 期望输出与实际诊断一致。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：全部错误面被拒绝、正例通过。
- 依赖：P1-T03
- 完成记录：
  - 2026-05-31：新增 `@ReleaseHook` typecheck fixtures，覆盖宿主错误（generic、open、abstract、sealed、缺 `@Experimental(feature = "releaseHook")`、非 class）、释放函数错误（不存在、跨文件 private 不可见、非 `@NoGC`/`@Extern(abi="c")`、返回非 Unit、参数数量不匹配）、字段错误（字段不存在、非 GC-free、字段/参数类型不匹配）以及正例（final non-generic class + `Ptr<Int>` raw handle 字段 + `@Extern(abi="c")` 释放函数 + `@Experimental(feature="releaseHook")`）。补齐 `sysroot/lib/scoop.core/src/core.scoop` 的 `ReleaseHook` annotation class surface，避免用户源码在内建语义校验前报未解析注解；同步因该 sysroot nominal/field/span 漂移影响的 HIR、effect-lowered 与 MIR golden。
  - 验证：`cargo build -p scoop -p scoopc`；targeted 新增 release hook fixtures；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。完整 fixture 首轮发现 `runtime_gc/gc_language_cross_thread_ref_handoff.scoop` 一次 30s timeout，单独复跑该 fixture 与整个 `tests/fixtures/runtime_gc` 均通过；同步 golden/行号后完整 fixture suite 重跑通过（`fixtures: ok (1644)`）。

### [DONE] P1-T04R：Review P1-T04 fixtures

- 必须实现的内容：复核错误面覆盖完整、期望输出准确、正例确实通过。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P1 收口，可进入 P2。
- 依赖：P1-T04
- 完成记录：
  - 2026-05-31：复核 P1-T04 fixtures 覆盖面与期望输出：宿主错误覆盖 generic、open、abstract、sealed、缺 `@Experimental(feature = "releaseHook")` 与非 class；释放函数错误覆盖不存在、多文件 private 不可见、非 `@NoGC`/`@Extern(abi="c")`、返回非 Unit、参数数量不匹配；字段错误覆盖字段不存在、字段非 GC-free、字段/参数类型不匹配；正例覆盖 final non-generic class + `Ptr<Int>` 字段 + `@Extern(abi="c")` 释放函数 + `@Experimental(feature="releaseHook")`。未发现缺口，P1 可收口进入 P2。
  - 验证：逐项运行 `tests/fixtures/typecheck/*release_hook*.scoop` 与 `tests/fixtures/typecheck_multi/release_hook_function_not_visible`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/run_fixtures.py`。`cargo test --all --all-targets` 未重跑，因为本 review 未修改编译器/fixture/运行时代码，且 P1-T04 完成记录已有完整 Rust 测试绿灯。

---

## P2：trampoline codegen + 填 `release_fn`

### [DONE] P2-T01：生成 release trampoline

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2-T01、§1（对象指针布局）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/layout.rs`（`lookup_struct_field:192`、`codegen_class_field_ptr:287`）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs`（`offset_of_element` 用法 :920、header 类型 :827、payload 布局 :1157）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs`（`lir_callable_symbol_facts:267`）、`gc.rs`（`declare_dispatch_target_fun:6-52`）
- 必须实现的内容：
  1. 生成 `void __scoop_release_<TypeMangled>(void *object)`，签名匹配 `ScoopTypeReleaseFn`。
  2. 把 `object`（header 基址）按该 class **完整布局**（含 header，payload 从 index 1）GEP 出各 `args` 字段值。
  3. 解析释放函数符号并按 `args` 顺序生成调用。
  4. 确保被引用的释放函数不被 DCE（trampoline 引用即保活点，必要时显式标记）。
- 必须遵从的约束：字段偏移必须基于含 header 的完整对象布局；不得只按 payload 偏移计算。
- 验证：
  1. `cargo clippy --all-targets -- -D warnings`
  2. `cargo test --all --all-targets`
- 完成条件：trampoline 读取正确字段并以正确顺序/类型调用目标函数。
- 依赖：P1 完成
- 完成记录：
  - 2026-05-31：LLVM stage 现在接收 HIR `ReleaseHookIndex`，并为带 `@ReleaseHook` 的 non-generic class 生成 `void __scoop_release_<TypeMangled>(void *object)` 形态的 internal release trampoline；trampoline 从 runtime 传入的对象 header 指针出发，按完整 class object 布局（header + payload）GEP 到 `args` 字段，按注解顺序 load 字段值并直接调用已校验的释放函数。释放函数声明复用 LIR callable symbol facts，`@Extern(abi="c")` 目标在 release trampoline 内不插入普通 managed-call 的 `scoop_enter_native` / `scoop_leave_native` boundary；trampoline 内的 call 作为目标释放函数保活引用。同步补齐 `Ptr<T>` / `FunPtr<F>` token 到 pointer-sized unsigned codegen 表示的类型映射，使 `Ptr<T>` release args 可进入 LLVM class layout 与 call ABI。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T01R：Review P2-T01 trampoline

- 必须实现的内容：复核字段偏移（含 header）、调用顺序/类型、符号解析与 DCE 保活。
- 验证：`cargo test --all --all-targets`
- 依赖：P2-T01
- 完成记录：
  - 2026-05-31：复核 P2-T01 trampoline：字段读取路径从 runtime 传入的 object header 指针进入完整 class object layout，再经 payload GEP 读取 `args` 字段；调用参数按注解字段顺序传入，类型经既有 codegen ABI coercion 对齐；trampoline 对 `@Extern(abi="c")` 目标不插入普通 `scoop_enter_native` / `scoop_leave_native` boundary。review 发现未被普通调用引用的 `@Extern(abi="c")` release target 缺少 LIR callable symbol facts 时会被误声明为 exported Scoop callable，已修复为从 HIR `extern_funs` 回退取得 native symbol/import surface。新增 `tests/fixtures/build/release_hook_trampoline_emit_llvm.scoop`，锁定 trampoline internal 函数、header→payload 字段 GEP、参数顺序、native target 调用与无 native boundary。
  - 验证：`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/build/release_hook_trampoline_emit_llvm.scoop`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T02：在 type descriptor 填 `release_fn` + IR fixtures

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2-T02、P2-T03
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs`（`get_or_create_type_descriptor_global:1023`、release_fn 槽 :1080）
- 必须实现的内容：
  1. 在 `get_or_create_type_descriptor_global` 内，当该类型带 `@ReleaseHook`（查 P1 的 HIR side table）时，把 values 第 9 项（:1080）由 `const_null` 改为 trampoline 函数指针；其余类型保持 null。
  2. IR/build fixtures：断言带 `@ReleaseHook` 类型的 descriptor `release_fn` 非 null 且指向 trampoline；trampoline 字段偏移/调用正确；无注解类型 `release_fn` 仍为 null。
- 必须遵从的约束：不得改变无注解类型的 descriptor 输出。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：descriptor 正确接线，IR fixture 锁定。
- 依赖：P2-T01
- 完成记录：
  - 2026-05-31：`get_or_create_type_descriptor_global` 现在按 HIR `ReleaseHookIndex` 为带 `@ReleaseHook` 的 class descriptor 填入 release trampoline 函数指针，`release_fn` 槽位保持在 `trace_fn` 之后；无注解类型继续写入 null。补充 LLVM 单元断言与 build fixtures：`release_hook_trampoline_emit_llvm.scoop` 锁定 descriptor 指向 trampoline、trampoline 字段读取/调用顺序与无 native boundary；新增 `release_hook_descriptor_absent_without_annotation_llvm.scoop` 锁定无注解 class 不生成 trampoline 且 descriptor `release_fn` 保持 null。
  - 验证：`cargo fmt`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/build/release_hook_trampoline_emit_llvm.scoop`；`python3 tools/run_fixtures.py tests/fixtures/build/release_hook_descriptor_absent_without_annotation_llvm.scoop`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T02R：Review P2-T02 descriptor 接线

- 必须实现的内容：复核 release_fn 填充条件正确、无注解类型无回归、IR fixture 断言充分。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P2 收口，可进入 P3。
- 依赖：P2-T02
- 完成记录：
  - 2026-05-31：复核 P2-T02 descriptor 接线：`get_or_create_type_descriptor_global` 仅在 HIR `ReleaseHookIndex` 命中 class FQN 时通过 `type_descriptor_release_fn_ptr` 填入 release trampoline，未命中时保持 `release_fn` 为 null；descriptor field order 仍为 `trace_fn` 后紧跟 `release_fn`，与 runtime `ScoopTypeDescriptor` ABI 对齐。检查生成 IR 后确认 annotated class descriptor 为 `ptr null, ptr @__scoop_release_...`，无注解 class descriptor 为 `ptr null, ptr null`；现有 LLVM 单元断言与 build fixtures 覆盖了 trampoline 指向、槽位顺序、无 native boundary、无注解类型不生成 trampoline 且 `release_fn` 保持 null。未发现需要修正的实现或 fixture 缺口，P2 可收口进入 P3。
  - 验证：`python3 tools/run_fixtures.py tests/fixtures/build/release_hook_trampoline_emit_llvm.scoop`；`python3 tools/run_fixtures.py tests/fixtures/build/release_hook_descriptor_absent_without_annotation_llvm.scoop`；生成 IR 人工检查；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1646)`）。

---

## P3：验证矩阵、回归与文档收尾

### [DONE] P3-T01：run-pass 端到端 + 四后端 parity + 跨平台矩阵

- 参考：[`PLAN.md`](./PLAN.md) §5 / P3-T01、P3-T02、P3-T03、§6（风险）
- 必须实现的内容：
  1. run-pass fixture：final non-generic class 持有 `Ptr<T>` native handle，构造时经 `@Extern(abi="c")` FFI 创建资源，`@ReleaseHook` 指向销毁函数；制造对象不可达 + 触发 GC，用计数器/side-effect 探针断言释放函数被调用且字段值正确传入。
  2. 断言进程退出时存活对象**不**触发释放（验证 best-effort 边界）。
  3. 四后端 parity（baseline moving / baseline non-moving / immix / minimal / hosted）：release_fn 调用一致；immix minor 与 major reclaim 都正确，单对象不重复释放。
  4. 跨平台矩阵至少 `linux/amd64` + `macos/aarch64`。
- 必须遵从的约束：探针机制不得依赖会触发分配/effect 的路径（保持释放上下文约束）。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：端到端 + 四后端 + 双平台全绿。
- 依赖：P2 完成
- 完成记录：
  - 2026-05-31：新增 `@ReleaseHook` 端到端 run-pass fixtures，宿主为 final non-generic class 持 `Ptr<Int>` native handle，构造经 `@Extern(abi="c")` 探针创建资源，`@ReleaseHook` 指向销毁探针。fixture 制造对象不可达 + 显式 `__scoop_gc_collect()`/`__scoop_gc_collect_minor()` 触发 GC，用 test-only native handle 探针（`scoop_test.c` 内 `scoop_test_release_hook_probe_*`）断言释放被调用一次、字段值（裸 handle id）正确传入、live/duplicate/invalid 计数正确；`__scoop_release_hook_probe_expect_at_exit` 注册 `atexit` 断言进程退出时存活对象**不**触发释放（best-effort 边界）。四后端 parity 经 fixture header `// ENV: SCOOP_RUNTIME_GC_BACKEND=...` 选择后端：baseline moving（`SCOOP_GC_MOVE=1`）/ baseline non-moving / immix major / immix minor（`SCOOP_GC_IMMIX_NURSERY_BLOCKS=1` 走 minor reclaim）/ minimal / hosted，单对象不重复释放。为支持 fixture 选后端，`scoopld` 新增 `SCOOP_RUNTIME_GC_BACKEND` env → `RuntimeGcBackend` 解析并透传 `-DSCOOP_GC_BACKEND=`（默认 Immix，保持既有 driver 行为）；`runtime_test.scoop` 新增 `__scoop_gc_collect_minor` 与探针 FFI surface。
  - 跨平台矩阵：macos/aarch64 本地全绿；linux/amd64 在 nuc12（`/home/chenxu/repos/scoop-1`，LLVM 21.1.8 via linuxbrew）经未提交变更 patch 后构建并运行六个后端 fixture，全部 PASS（baseline moving / baseline non-moving / hosted / immix major / immix minor / minimal）。
  - 验证：`cargo build -p scoop -p scoopc`；本地 `python3 tools/run_fixtures.py tests/fixtures/runtime_gc/release_hook_e2e_*.scoop`（macos/aarch64）；nuc12 `python3 tools/run_fixtures.py tests/fixtures/runtime_gc/release_hook_e2e_{baseline_moving,baseline_nonmoving,hosted,immix_major,immix_minor,minimal}.scoop`（linux/amd64，全 PASS）。

### [DONE] P3-T01R：Review P3-T01 验证矩阵

- 必须实现的内容：复核端到端断言有效（确实观测到释放且字段正确）、退出不回收语义被验证、四后端与跨平台覆盖完整、无重复释放。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P3-T01
- 完成记录：
  - 2026-05-31：复核 P3-T01 端到端验证矩阵：`ReleaseHookProbe` 为 final non-generic class，持 `Ptr<Int>` raw handle，释放 trampoline 仅把中间字段 `raw` 传给 `@Extern(abi="c")` 探针；stdout 断言释放次数、last id、live、duplicate、invalid 计数，能观测字段值正确传入、不可达对象被释放一次、重复 collect 不重复释放。`scoop_test_release_hook_probe_expect_at_exit` 的 atexit 检查锁定进程退出时仍存活对象不触发 release，符合 best-effort 边界。复核四后端/模式 fixture 覆盖 baseline moving、baseline non-moving、hosted、immix major、immix minor、minimal；本次修正 review 中发现的环境隔离缺口：baseline non-moving 显式 `SCOOP_GC_MOVE=0`，immix major/minor 显式 `SCOOP_RUNTIME_GC_BACKEND=immix` 并固定 nursery env，避免矩阵受外部环境污染。跨平台记录已在 P3-T01 完成记录中覆盖 macos/aarch64 与 linux/amd64。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1652)`）。

### [DONE] P3-T02：真实用例 + spec/runtime 文档回写

- 参考：[`PLAN.md`](./PLAN.md) §5 / P3-T04、P3-T05
  - `runtime/c/include/scoop_runtime.h:38-43`、`SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md`（release callback 约 :2959-2972）
- 必须实现的内容：
  1. 用 `@ReleaseHook` 把一个真实用例（`Mutex` 或 `CondVar`）重写为纯 Scoop class + FFI，去掉对应 compiler intrinsic 依赖；若本轮 scope 仅验证机制，可先以最小 demo 类型代替，并把正式迁移列为后续 backlog（在完成记录中注明）。
  2. `SCOOP_FULL_SPEC.md` 新增 `@ReleaseHook` 章节：形态、约束（class/non-generic/final/`@Experimental`/`@NoGC`|`@Extern(c)`/GC-free args）、best-effort 语义、退出不回收、与 `@NoGC`/`@Extern` 的关系。
  3. `runtime/c/include/scoop_runtime.h` release callback 注释与 `SCOOP_RUNTIME.md` 与 `@ReleaseHook` 关联说明对齐。
- 必须遵从的约束：文档措辞必须与实际实现 contract 一致，明确标注「尽力而为、非确定性析构」。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：至少一个真实/演示类型以纯 Scoop + FFI 工作；spec 与 runtime 文档同步。
- 依赖：P3-T01
- 完成记录：
  - 2026-05-31：新增最小 tracer-bullet demo `tests/fixtures/run-pass/release_hook_native_handle_demo.scoop`，用 final non-generic `DemoNativeHandle` 持 `Ptr<Int>` raw native handle，构造经 test-only `@Extern(abi="c")` create 探针获得裸 handle，`@ReleaseHook` 指向 `@Extern(abi="c")` release 探针并用 stdout 断言 GC reclaim 后释放一次、字段值正确传入、live/duplicate/invalid 计数正确。真实 `Mutex` / `CondVar` / `Once` 迁移按既有 P4-T01/P4-T02 backlog 推进，本任务仅完成机制 demo 与文档收口。
  - 文档：`SCOOP_FULL_SPEC.md` 新增/替换 `@ReleaseHook` 用户可见章节，覆盖注解形态、宿主约束、释放函数约束、GC-free `args`、best-effort/退出不回收语义、与 `@NoGC` / `@Extern(abi="c")` 的安全关系和最小示例；同步 `docs/spec/language_spec-part6.md`；`SCOOP_RUNTIME.md` 与 `runtime/c/include/scoop_runtime.h` 的 `release_fn` / `ScoopTypeReleaseFn` 说明已与 `@ReleaseHook` trampoline、GC 受限上下文和非确定性释放语义对齐。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/release_hook_native_handle_demo.scoop`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P3-T02R：Review P3-T02 用例与文档

- 必须实现的内容：复核 demo 用例确实走纯 Scoop + `@ReleaseHook` 路径、spec/runtime 文档与实现一致、best-effort 语义表述准确。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P3 收口，可进入 P4。
- 依赖：P3-T02
- 完成记录：
  - 2026-05-31：复核 P3-T02 demo 与文档：`release_hook_native_handle_demo.scoop` 使用纯 Scoop final non-generic `DemoNativeHandle` 持 `Ptr<Int>` raw handle，配套 `@Experimental(feature = "releaseHook")` 与 `@ReleaseHook`，释放目标为 `scoop.runtime.test.__scoop_release_hook_probe_release` 的 `@Extern(abi="c")` 探针；fixture stdout 锁定 GC reclaim 后释放一次、字段值 `303` 正确传入、live/duplicate/invalid 计数归零。对照当前 typecheck/codegen/runtime 实现复核 `SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part6.md`、`SCOOP_RUNTIME.md` 与 `runtime/c/include/scoop_runtime.h`，确认宿主约束、释放函数约束、GC-free args、descriptor release trampoline、best-effort/非确定性释放与进程退出不释放语义一致，未发现需修正文档或实现的缺口。P3 可收口进入 P4。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/release_hook_native_handle_demo.scoop`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

---

## P4：`scoop.sync` 迁移到 `@ReleaseHook` 并清理 intrinsic

> 现状（已核实）：`Mutex`/`CondVar`/`Once` 当前是 opaque `public class`，op 为 `@Extern(abi="scoop")`，对象由 C 侧 `scoop_sync_*_create` + C 写死的 type descriptor（已带 `release_fn`）分配；只有 `__scoop_sync_once_run` 是 `@Intrinsic`。本阶段把它们收敛为「普通 final class + `Ptr<T>` handle + `@ReleaseHook` + `@Extern(abi="c")`」，并删光编译器硬编码。

### [DONE] P4-T01：重写 `sync.scoop` 三类型为 `@ReleaseHook` class，`Once.run` 纯 Scoop 化

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4-T01、§3.1
  - `sysroot/lib/scoop.sync/src/sync.scoop`（`Mutex:21`/`CondVar:24`/`Once:27`；op `:32-101,110-119`；`__scoop_sync_once_run` `:130-131`）
- 必须实现的内容：
  1. `Mutex`/`CondVar`/`Once` 改为 final non-generic class，持 `Ptr<T>` native handle 字段；构造经 `@Extern(abi="c")` create-native（返回裸 handle）填字段。
  2. 各 op（lock/unlock、wait/notifyOne/notifyAll、isDone）改为 method body 内解出 `self.handle` 调 `@Extern(abi="c")` 函数；删除旧 `@Extern(abi="scoop")` 包装。
  3. 每类型加 `@ReleaseHook(name = destroyNative, args = ["handle"])` + `@Experimental(feature = "releaseHook")`，释放函数为 `@Extern(abi="c")` 销毁。
  4. `Once.run` 用纯 Scoop 重写（基于已 class 化的 `Mutex`/`CondVar` + `isDone`），删除 `@Intrinsic`。
- 必须遵从的约束：
  - 可见语义（可重入性、condvar 原子 unlock-wait-relock、once 并发单次执行）必须与迁移前一致。
  - handle 字段类型须满足 `@ReleaseHook` 的 GC-free 约束（`Ptr<T>`）。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：三类型为纯 Scoop class，`Once.run` 无 `@Intrinsic`。
- 依赖：P3 完成
- 完成记录：
  - 2026-05-31：`sysroot/lib/scoop.sync/src/sync.scoop` 已将 `Mutex` / `CondVar` / `Once` 改为 final non-generic 普通 Scoop class，内部持 GC-free raw native handle 字段，并为三者加上 `@Experimental(feature = "releaseHook")` 与 `@ReleaseHook(..., args = ["rawHandle"])`。`mutexCreate` / `condVarCreate` / `onceCreate` 现在经 `@Extern(abi="c")` raw-handle create helper 构造；lock/unlock、condvar wait/notify/destroy、once isDone 均通过 Scoop extension 方法解出 raw handle 调 C ABI helper，旧 `@Extern(abi="scoop")` 包装已从 `sync.scoop` 删除。`Once.run` 已改为普通 Scoop 状态机，组合 class 化后的 `Mutex`/`CondVar` 与 raw once state helper；`sync.scoop` 中不再声明 `@Intrinsic` / `__scoop_sync_once_run`。为避免显式 `destroy()` 后 GC release hook 双重释放，`Mutex`/`CondVar` 的 raw handle 字段为可变字段，显式销毁后写为空 handle。
  - Native 侧新增 `scoop_sync_*_native_*` raw-handle helper 供 P4-T01 使用，并保留 legacy GC-object path 等待 P4-T02 删除，避免本任务越界清理 C descriptor。同步更新 sync GC release stdout（`Once` 现在由 Scoop 层组合内部 `Mutex`/`CondVar`，销毁计数包含这两个内部资源）与 LLVM ABI 单元测试的 native create symbol 断言。
  - 验证：`cargo build -p scoop -p scoopc`；targeted sync/typecheck/runtime/delegate concurrency fixtures；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。完整 fixture 首轮发现 `runtime_gc/gc_language_repeated_collect_shared_chain.scoop` 与 `runtime_gc/gc_language_parallel_alloc_shared_roots.scoop` timeout，两个 fixture 单独复跑均通过；修正 stale sync 负例期望后完整 fixture suite 重跑通过（`fixtures: ok (1653)`）。

### [DONE] P4-T01R：Review P4-T01 sync 源改造

- 必须实现的内容：复核三类型 class 形态、`@ReleaseHook`/`@Experimental` 正确、op 走 `@Extern(abi="c")`、`Once.run` 纯 Scoop 且语义不变。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P4-T01
- 完成记录：
  - 2026-05-31：复核 P4-T01 当前实现与最新提交：`Mutex` / `CondVar` / `Once` 已是普通 non-generic class（默认 final），均带 `@Experimental(feature = "releaseHook")` 与 `@ReleaseHook(..., args = ["rawHandle"])`；`lock` / `unlock`、`wait` / `notifyOne` / `notifyAll`、`isDone` 等 op 均通过 `@Extern(abi = "c")` raw native helper；`sync.scoop` 中已无 `@Extern(abi="scoop")` sync 包装、无 `@Intrinsic`、无 `__scoop_sync_once_run` 声明，`Once.run` 为普通 Scoop 函数体。编译器/runtime 中残留的 `scoop_sync_once_run` 专用路径已由后续 P4-T03 明确调度，本 review 不越界删除。
  - 验证过程中发现并修复两个未调度 runtime_gc fixture 超时：`gc_language_repeated_collect_shared_chain.scoop` 改为 worker 在 stop flag 置位前持续扩展本地链并显式 `yield()` safepoint，main 在 worker 活跃期间重复触发 GC；`gc_language_cross_thread_ref_handoff.scoop` 增加 `waiters` barrier，确保首次 GC 前 producer/consumer 都进入 phase wait 协议，并保持 producer 在 consumer GC 完成前不退出。两个多线程语言级 STW fixture 的 timeout 调整为 55s，仍低于单 fixture 1 分钟上限。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/runtime_gc/gc_language_repeated_collect_shared_chain.scoop`；`python3 tools/run_fixtures.py tests/fixtures/runtime_gc/gc_language_cross_thread_ref_handoff.scoop`；`python3 tools/run_fixtures.py tests/fixtures/runtime_gc`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。Rust 验证后仅修改 fixture / TODO / memory，未重复运行 Rust 测试。

### [DONE] P4-T02：收缩 `scoop_sync.c` 为只管 raw native handle

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4-T02
  - `sysroot/lib/scoop.sync/native/scoop_sync.c`（C 侧 type desc + `*_release` `:209-224/334-349/496-511`；handle 布局 `:173-182`；create/op/destroy 各段）
- 必须实现的内容：
  1. 删除 C 侧 GC 对象分配（`scoop_alloc_typed`）、C 写死的 `ScoopTypeDescriptor` 与 `scoop_sync_*_release` wrapper。
  2. create 改为只 malloc + 初始化 native struct，返回裸指针；destroy 接收裸指针释放；op 接收裸指针操作。
  3. 保留 `destroyed` 等幂等标志，使显式 destroy 与 `@ReleaseHook` 兜底不会双重释放。
- 必须遵从的约束：`baseline`/`immix`/`hosted`/`minimal` 均可编译可回归；不得保留任何 C 侧 type descriptor 路径。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：C runtime 只承担 raw native handle 生命周期，不再涉足 GC 对象/descriptor。
- 依赖：P4-T01
- 完成记录：
  - 2026-05-31：`sysroot/lib/scoop.sync/native/scoop_sync.c` 已收缩为只承担 raw native handle 生命周期与操作：删除 `Mutex` / `CondVar` / `Once` 的 C 侧 GC object wrapper、`scoop_alloc_typed` 分配路径、手写 `ScoopTypeDescriptor`、`scoop_sync_*_release` release wrapper 与旧 object-facing `scoop_sync_*` API；文件不再依赖 `scoop_runtime.h`。保留的 `scoop_sync_*_native_*` create/op/destroy 均以 malloc 出的 native struct 指针为 handle，`Mutex` / `CondVar` 继续用 `destroyed` / `initialized` 标志保护显式 destroy 与 `@ReleaseHook` 兜底路径，`Once` native handle 只保留纯 raw state / owner 数据。
  - 验证过程中完整 fixture 首轮暴露两个未调度 runtime_gc timeout：`gc_language_cross_thread_ref_handoff.scoop` 与 `gc_language_repeated_collect_shared_chain.scoop`。已按失败策略修复为确定性跨线程发布回归：线程先发布对象图并结束，主线程随后反复触发 GC 并读取；专门的 live-thread STW roots 场景仍由既有 `gc_stw_cross_thread_roots_basic.scoop` 与 `gc_stw_cross_thread_in_native_roots_basic.scoop` 覆盖。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；多轮 targeted `python3 tools/run_fixtures.py tests/fixtures/runtime_gc/gc_language_cross_thread_ref_handoff.scoop` / `gc_language_repeated_collect_shared_chain.scoop`；`python3 tools/run_fixtures.py tests/fixtures/runtime_gc`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T02R：Review P4-T02 runtime 收缩

- 必须实现的内容：复核 C 侧已无 GC 对象分配与 type descriptor、幂等释放标志有效、四后端编译通过。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P4-T02
- 完成记录：
  - 2026-05-31：复核 P4-T02 runtime 收缩：`sysroot/lib/scoop.sync/native/scoop_sync.c` 已无 `scoop_alloc_typed`、C 侧 `ScoopTypeDescriptor`、旧 object-facing `scoop_sync_*` API、`scoop_sync_*_release` wrapper 或 `scoop_runtime.h` 依赖；保留的 `scoop_sync_*_native_*` create/op/destroy 均只操作 malloc 出的 raw native handle。`Mutex` / `CondVar` native handle 仍通过 `destroyed` / `initialized` 标志配合 Scoop 层显式 `destroy()` 后置空 `rawHandle`，避免显式释放与 `@ReleaseHook` 兜底 double-destroy；`Once` 没有公开显式 destroy，native 侧只保留 raw state / owner，并通过 `@ReleaseHook` 回收。
  - 同步复核 `sysroot/lib/scoop.sync/src/sync.scoop`：三类型仍为普通 `@ReleaseHook` class，op 走 `@Extern(abi="c")` raw helper，未发现 sync 相关 `@Intrinsic` 或 `@Extern(abi="scoop")` 包装残留；`Once.run` intrinsic 的 compiler 侧旧硬编码仍由后续 P4-T03 明确处理，本 review 不越界删除。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T03：删除 `Once.run` intrinsic 全套硬编码

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4-T03、§3.1
  - `runtime_symbols.rs:26`、`runtime_abi.rs:266-283`、`call/lowering.rs:1558`、`intrinsics/sync.rs:6-92`、`effect_lowered/value.rs:1794,1925-1997`、`closure/mod.rs:691`
- 必须实现的内容：
  1. 删除 `SCOOP_SYNC_ONCE_RUN` 符号常量与 `declare_runtime_sync_once_run`。
  2. 删除 `call/lowering.rs` 与 `effect_lowered/value.rs` 对 `scoop.sync.__scoop_sync_once_run` 的 FQN dispatch 及 `codegen_sysroot_sync_once_run` / `lower_sync_intrinsic` handler。
  3. 移除 `closure/mod.rs` 中为 `Once.run` 准备的 `lookup_pure_unit_closure_type` 特例（若仅为此存在）。
- 必须遵从的约束：删除后 `Once.run`（已纯 Scoop 化）走常规 method/closure codegen；不得残留任何 sync 专用 codegen 分支。
- 验证：`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：编译器 codegen/runtime 层无 `Once.run` 专用路径。
- 依赖：P4-T01（纯 Scoop `Once.run` 必须先就位）
- 完成记录：
  - 2026-05-31：删除 `Once.run` intrinsic codegen/runtime 残留：移除 `SCOOP_SYNC_ONCE_RUN` runtime symbol、`declare_runtime_sync_once_run` ABI 声明、HIR direct-call lowering 中对 `scoop.sync.__scoop_sync_once_run` 的 FQN dispatch、effect-lowered lowering 中的 `lower_sync_intrinsic` / `sync_once_run` handler，以及仅服务该路径的 `intrinsics/sync.rs` 模块与 `lookup_pure_unit_closure_type` closure expected-type 特例。`Once.run` 现在只通过 `sysroot/lib/scoop.sync/src/sync.scoop` 的普通 Scoop method 进入常规 method/closure codegen；编译器 crate 内 grep 已无 `scoop_sync_once_run`、`__scoop_sync_once_run`、`codegen_sysroot_sync_once_run`、`declare_runtime_sync_once_run`、`lower_sync_intrinsic` 或 `lookup_pure_unit_closure_type` 残留。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T03R：Review P4-T03 intrinsic 删除

- 必须实现的内容：复核所有列出的硬编码点已删除、`Once.run` 走常规路径、无回归。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P4-T03
- 完成记录：
  - 2026-05-31：复核 P4-T03 最新提交与当前源码：`SCOOP_SYNC_ONCE_RUN` runtime symbol、`declare_runtime_sync_once_run` ABI 声明、HIR direct-call lowering 中的 `scoop.sync.__scoop_sync_once_run` 分派、effect-lowered lowering 中的 `lower_sync_intrinsic` / sync once handler、`intrinsics/sync.rs` 模块以及 `lookup_pure_unit_closure_type` closure expected-type 特例均已删除；精确 grep 确认旧 `scoop_sync_once_run` / `__scoop_sync_once_run` / `codegen_sysroot_sync_once_run` / `declare_runtime_sync_once_run` / `lower_sync_intrinsic` / `lookup_pure_unit_closure_type` 名称只剩 TODO/PLAN/归档文档引用，不在编译器、runtime、sysroot 或测试源中残留。`sysroot/lib/scoop.sync/src/sync.scoop` 中 `Once.run` 为普通 Scoop method，组合 `Mutex` / `CondVar` 与 `@Extern(abi="c")` raw helper，并通过普通 closure call `block()` 运行初始化逻辑；`std_sync_basic` / `std_sync_api_surface_ok` / UMB sync fixtures 覆盖该常规路径，完整 fixture 回归无退化。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T04：删除/重指 effect-facts 白名单与其余 `scoop.sync` 特判

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4-T04、§3.1、§6（lazy 属性同步风险）
  - `effect_facts/builder.rs:2857-2867`、`session/mod.rs:242,269`、`scoop_project_model/graph_loader.rs:668,682,710,767`、`pipeline/llvm_codegen_stage.rs:617`
  - 消费侧：`impl_lowering.rs:33-36`、`decls.rs:965,982,1008,1025`、`sugar.rs:326,336,656,666,778,788,914,926`
- 必须实现的内容：
  1. 删除 `effect_facts/builder.rs` 的 11 个 sync FQN 无 effect 白名单；确认 class 化后常规 effect 推导给出正确结果（必要时调整 sync op 的 effect 声明）。
  2. 评估并按需收敛 `session`/`project_model`/auto-import 对 `scoop.sync` 的特殊处理。
  3. **决策点（必须在完成记录写明结论与理由）**：lazy/delegate 属性同步注入的 `Mutex` 引用是消费侧依赖——二选一：(a) 保留为经普通名字解析的 stdlib 引用（精确保留这唯一允许点），或 (b) 把 lazy-property 加锁合成下放 sysroot helper 使编译器持零 sync FQN。
- 必须遵从的约束：lazy/Observable/Vetoable 属性的同步语义不得被破坏；删白名单后 effect 推导结果必须与迁移前对这些调用的可见 effect 行为一致。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：除 P4-T04 决策保留的唯一消费侧引用外，编译器无 `scoop.sync` 实现性特判。
- 依赖：P4-T01、P4-T03
- 完成记录：
  - 2026-05-31：删除 `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs` 中 11 个 `scoop.sync` public API 的 plain intrinsic / no-effect 白名单；class 化后的 `Mutex` / `CondVar` / `Once` 调用现在依赖 `sync.scoop` 普通函数声明与函数体 effect facts，完整回归确认可见 Pure 行为未变。收敛其余特判：`session` / `project_model` 中点名 `scoop.sync` 的命中点仅为旧测试夹具，已移除这些测试对 sync 名称的固化；LLVM 测试通用 `session_for_source` helper 不再根据源码文本自动注入 `scoop.sync`，唯一 sync ABI 单测改为显式声明自身需要该 sysroot cone。
  - 决策：选择 P4-T04(a)，暂保留 lazy / observable / vetoable 属性合成中的 `Mutex` 消费侧引用，精确定界为 `HirLowering::SYNC_MUTEX_*` 四个 FQN 常量及其 lazy/delegate lowering 使用点；理由是 P5 已明确负责把三者降为普通库 class 并删除该注入点，本任务若提前下放 sysroot helper 会与 P5 的委托库化重叠且扩大变更面。该保留点只生成普通 HIR top-level call / nominal field，不走 sync intrinsic、runtime descriptor 或 codegen 专用分支。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T04R：Review P4-T04 特判清理

- 必须实现的内容：复核白名单删除后 effect 推导正确、属性同步未被破坏、消费侧决策合理且被精确界定。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P4-T04
- 完成记录：
  - 2026-05-31：复核 P4-T04 最新提交与当前源码：`crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs` 中 11 个 `scoop.sync` public API 的 plain intrinsic / no-effect 白名单已删除，effect-facts builder 内不再有 sync FQN 特判；`Mutex` / `CondVar` / `Once` 的 public API 现在通过 `sysroot/lib/scoop.sync/src/sync.scoop` 普通函数声明与函数体参与常规 effect 推导，完整回归确认可见行为未退化。精确 grep `crates` 后，`scoop.sync` 残留仅有 LLVM ABI 单测里的显式 sysroot 依赖，以及 P4-T04 决策保留的 `HirLowering::SYNC_MUTEX_*` 四个 lazy / observable / vetoable 委托消费侧 FQN 常量；未发现 session / project_model / pipeline 中的 sync 实现性特判残留。
  - 复核消费侧决策：保留 `SYNC_MUTEX_*` 属于 P4-T04(a) 的普通 stdlib 引用边界，只用于标准委托 lowering 注入 per-property `Mutex` 与普通 `lock` / `unlock` top-level call，不走 sync intrinsic、runtime descriptor 或 codegen 专用分支；`delegated_property_*` 并发 fixtures 与完整 fixture suite 覆盖属性同步语义，P5 已排期删除该允许点。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1653)`）。

### [DONE] P4-T05：sync 全量回归 + 四后端/跨平台 + 零硬编码 grep 守卫

- 参考：[`PLAN.md`](./PLAN.md) §5 / P4-T05、§6（sync 双重释放 / 行为基线）
- 必须实现的内容：
  1. sync run-pass fixtures：lock/unlock、condvar wait/notify、once 单次执行 + 并发竞争；以迁移前语义为基线逐项对齐。
  2. 四后端（baseline moving/non-moving、immix、minimal、hosted）+ 跨平台（`linux/amd64` + `macos/aarch64`）parity。
  3. 新增「零编译器硬编码」grep 守卫测试：断言 `Mutex`/`CondVar`/`Once`/`scoop.sync` 相关 FQN 不再出现在编译器 crate（消费侧若选 P4-T04(a)，守卫精确排除该唯一允许点）。
  4. 删除/迁移已被取代的旧 sync fixtures。
- 必须遵从的约束：显式 `destroy()` 与 `@ReleaseHook` 共存路径必须验证不双重释放。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：sync 全量回归与四后端/跨平台绿；守卫测试锁定零硬编码（除允许点）。
- 依赖：P4-T02、P4-T03、P4-T04
- 完成记录：
  - 2026-05-31：新增 sync backend parity 回归矩阵 `tests/fixtures/runtime_gc/std_sync_backend_parity_{baseline_moving,baseline_nonmoving,hosted,immix_major,immix_minor,minimal}.scoop` 与共享 stdout golden，覆盖 `Mutex` lock/unlock、`CondVar.wait` + `notifyOne` / `notifyAll`、两个线程并发竞争 `Once.run` 且 block 只执行一次，以及显式 `destroy()` 后对象丢弃 + 显式 GC 不 double-destroy（sync destroy count 保持 `1 1 0`）。矩阵覆盖 baseline moving / baseline non-moving / hosted / immix major / immix minor / minimal；immix parity fixture 固定 backend/mode，但不再用 `SCOOP_GC_IMMIX_NURSERY_BYTES=0` 注入无关线程启动期 STW stress，release/double-destroy 仍在 worker join 后通过显式 GC 验证。同步迁移旧 `std_sync_basic` 与 retired UMB sync-intrinsics fixtures 的说明文字，使其定位为普通 sync library smoke/arity gate。
  - 新增 `crates/scoop/tests/p4_sync_hardcoding_guard.rs`，扫描生产编译器 crate 源码中的 `scoop.sync` / `scoop_sync_` / 旧 Once intrinsic 名称 / `SYNC_MUTEX_*` 命中；除 P4-T04(a) 决策保留的委托属性 `Mutex` 消费侧边界（`HirLowering::SYNC_MUTEX_*` 四个 FQN 常量及 `decls.rs` / `sugar.rs` 使用点）外全部禁止。守卫跳过 `tests` 源目录，避免把 LLVM ABI 单测中的显式 sysroot 依赖误判为生产硬编码。
  - 跨平台矩阵：macos/aarch64 本地完整回归全绿；linux/amd64 在 `nuc12` 的独立 worktree `/tmp/scoop-p4t05-aadaa9da`（LLVM 21 via `/home/linuxbrew/.linuxbrew/opt/llvm@21`）运行六个 `std_sync_backend_parity_*` fixtures，全部 PASS。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop --test p4_sync_hardcoding_guard`；逐项运行六个 `tests/fixtures/runtime_gc/std_sync_backend_parity_*.scoop`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1659)`）；nuc12/linux/amd64 逐项运行六个 `std_sync_backend_parity_*` fixtures。

### [DONE] P4-T05R：Review P4-T05 回归与守卫

- 必须实现的内容：复核语义对齐基线、并发用例有效、守卫测试覆盖完整且排除点精确、无双重释放。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P4 整体收口。
- 依赖：P4-T05
- 完成记录：
  - 2026-05-31：复核 P4-T05 regression matrix 与硬编码守卫：六个 `std_sync_backend_parity_*` fixture 覆盖 baseline moving / baseline non-moving / hosted / immix major / immix minor / minimal；stdout 锁定 `Mutex` lock/unlock、`CondVar.wait` + `notifyOne` / `notifyAll`、双线程竞争 `Once.run` 只执行一次、`Once.isDone()` 结果，以及显式 `destroy()` 后 `__scoop_gc_collect()` 不 double-destroy。守卫 `crates/scoop/tests/p4_sync_hardcoding_guard.rs` 扫描生产编译器 Rust 源码并跳过测试目录；禁止 `scoop.sync` / `scoop_sync_` / 旧 Once intrinsic / `SYNC_MUTEX_*` 等实现性硬编码，仅允许 P4-T04(a) 决策保留的 delegated-property `Mutex` 消费侧边界，排除点精确。
  - Review 中修正三个矩阵缺口：Immix major/minor fixtures 显式设置 `SCOOP_GC_IMMIX_NURSERY_BYTES=0`，避免外部 env 覆盖 `SCOOP_GC_IMMIX_NURSERY_BLOCKS`；double-destroy 检查移到线程启动前，确保 minimal 后端的 `__scoop_gc_collect()` 真正执行而不是多线程后 no-op；CondVar 测试新增独立 `report` 条件变量并锁定 `notify_one_count=1`，避免 worker 自身报告唤醒干扰 `notifyOne` / `notifyAll` 区分。
  - 验证过程中发现并修复一个直接阻塞 P4-T05R 的线程/GC 边界 bug：`scoop.thread` 的 `scoop_thread_spawn` 是 `abi="scoop"` native runtime 入口，`pthread_create` 期间未切到 `InNative`，新线程 attach/init 后也缺少启动 safepoint；Immix major 下 worker 触发 STW 时可能等待仍处于 Running 的启动线程。修复为新线程 attach/init 后执行 `scoop_gc_safepoint_poll()`，并在 `pthread_create` 期间把 `Thread` handle 作为 native root 后 enter/leave native。
  - 验证：直接运行 immix major sync fixture（复现并确认 STW timeout 消失）；`python3 tools/run_fixtures.py --fixtures tests/fixtures/runtime_gc/std_sync_backend_parity_immix_major.scoop`（重复通过）；`SCOOP_GC_IMMIX_NURSERY_BYTES=1048576` 污染下运行 immix major/minor parity fixtures；`python3 tools/run_fixtures.py --fixtures tests/fixtures/runtime_gc`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1659)`）。验证后仅补充 C 注释，未改变编译输出，未重跑。P4 整体收口，可进入 P5。

---

## P5：`lazy` / `observable` / `vetoable` 降为普通库 class

> 依据（已核实）：泛型委托路径已把委托对象存成宿主类普通字段、读写编译成 `getValue`/`setValue`（`tests/fixtures/hir/delegated_property_lowering.scoop`），这正是三者要走的路；普通 class 本就能自由持有/改写 `var` 字段（`sysroot/lib/scoop.core/src/core.scoop:1506` 的 `RefCell`/`Atomic`），`@InteriorMutable` 只是值类型后门（`structs.rs:37-44`），与 class 委托无关。本阶段**不需要任何新原语**，是纯减法 + 库重写。

### [DONE] P5-T00：修复泛型 class 实现参数化 interface 的 itable stable type id

- 参考：
  - `crates/scoopc_hir/src/itable.rs`（`collect_concrete_class_targets`、`build_precise_class_itable_entries`、`stable_runtime_type_id_for_lower`）
  - `crates/scoopc_hir/src/stable_id.rs`（`StableTypeParamResolver`、`NoTypeParamResolver`、`MissingTypeParamKey`）
  - 触发形态：`class Lazy<V>(...) : ReadOnlyProperty<Any, V>` / `class ObservableProperty<V>(...) : ReadWriteProperty<Any, V>`
- 必须实现的内容：
  1. 修复 runtime itable metadata 对泛型 class 模板或未替换 type param 的处理：不得用 `NoTypeParamResolver` 对含 `TypeParam` 的具体 interface 实例直接求 stable type id。
  2. 确保 `Lazy<Int> : ReadOnlyProperty<Any, Int>`、`ObservableProperty<Int> : ReadWriteProperty<Any, Int>` 等实例能生成稳定 itable metadata；必要时跳过非 ground 模板、或在 class 实例化后用正确 type-param substitution / stable key resolver 计算。
  3. 新增最小 run-pass 或 build fixture，覆盖泛型 class 实现参数化 interface 并触发 runtime itable metadata 生成，防止再次出现 `missing stable type parameter key for \`V\``。
- 必须遵从的约束：
  - 不得通过让委托类放弃实现 `ReadOnlyProperty` / `ReadWriteProperty`、改成非泛型类、改返回 `Any`、或关闭 itable metadata 来绕过。
  - 修复应覆盖泛型 class + 参数化 interface 的通用形态，不只针对 `scoop.delegates` 名称。
- 验证：`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：泛型 class 实现参数化 interface 的实例可完成 itable metadata 生成；P5-T01 的库委托类形状不再触发 stable type id 错误。
- 依赖：P4 完成
- 完成记录：
  - 2026-05-31：修复 runtime itable metadata 收集路径：`collect_concrete_class_targets` 与 `collect_concrete_interface_targets` 现在跳过仍含 `TypeKind::Param` 或泛型实参未完整替换的模板目标，只对 ground runtime class/interface 实例生成 stable type id；`stable_runtime_type_id_for_lower` 增加 ground-type guard，避免把含未替换 type param 的 runtime type 交给 `NoTypeParamResolver`。新增 run-pass fixture `generic_class_parameterized_interface_itable_stable_id.scoop`，覆盖 `Box<T> : Tagged<T>` 的 `Box<Int>` ground 实例、interface dispatch 与 `is Tagged<Int>` runtime match，锁定泛型 class 实现参数化 interface 时 itable metadata 可稳定生成。
  - 验证：`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/generic_class_parameterized_interface_itable_stable_id.scoop`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1660)`）。

### [DONE] P5-T00R：Review P5-T00 itable 泛型 interface 修复

- 必须实现的内容：复核修复覆盖的是通用泛型 class + 参数化 interface 问题，`NoTypeParamResolver` 不再误用于含未替换 type param 的 runtime interface 实例，新增 fixture 能真实触发 itable metadata。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P5-T00
- 完成记录：
  - 2026-05-31：复核 P5-T00 itable 修复：`collect_concrete_class_targets` 与 `collect_concrete_interface_targets` 现在只收集 ground runtime class/interface 实例，跳过仍含 `TypeKind::Param` 或泛型实参未完整替换的模板目标；`stable_runtime_type_id_for_lower` 在调用 `NoTypeParamResolver` 前先拒绝非 ground runtime type，因此不会再把含未替换 type param 的具体 interface 实例交给 `NoTypeParamResolver`。该修复基于 runtime metadata 的通用 ground-type 边界，不依赖 `scoop.delegates` 名称或委托专用形状。
  - 复核新增 fixture `generic_class_parameterized_interface_itable_stable_id.scoop`：`Box<T> : Tagged<T>` 的 `Box<Int>` ground 实例同时经 `Tagged<Int>` interface dispatch 与 `is Tagged<Int>` runtime match 触发 itable metadata 生成，可锁定泛型 class 实现参数化 interface 时 stable type id 生成不再报 `missing stable type parameter key for \`V\`` 类错误。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1660)`）。

### [DONE] P5-T01：在 `scoop.delegates` 写 lazy/observable/vetoable 库实现

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5-T01
  - `sysroot/lib/scoop.delegates/src/delegates.scoop`（`ReadOnlyProperty`/`ReadWriteProperty` `:11-19`；`@Intrinsic` 顶层 `lazy`/`observable`/`vetoable` `:29-63`）
  - `sysroot/lib/scoop.sync/src/sync.scoop`（P4 后的库 `Mutex`/`Once`）
- 必须实现的内容：
  1. `Lazy<V>` class 持 `var inited: Bool` + `var value: V`（或等价 nullable 存储），实现 `ReadOnlyProperty`，`getValue` 内首次跑 initializer 并 memoize。
  2. `ObservableProperty<V>` / `VetoableProperty<V>` class 持 backing value + 回调，实现 `ReadWriteProperty`；observable 回调在写之后、vetoable 否决则不写。
  3. 线程安全模式内部组合库 `Mutex`（或 lazy 用 `Once`）；`lazy` 的 `LazyThreadSafetyMode`（None/Synchronized/Publication）各模式行为对齐现状。
  4. `lazy`/`observable`/`vetoable` 三个顶层函数从 `@Intrinsic` 降为返回上述包装类的普通 `fun`。
- 必须遵从的约束：可见语义必须复刻现状；不依赖任何新编译器原语或 `@InteriorMutable`。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：三者有可用的纯库实现，顶层函数无 `@Intrinsic`。
- 依赖：P4 完成；P5-T00R
- 阻塞记录：
  - 2026-05-31：尝试按计划实现 `Lazy<V> : ReadOnlyProperty<Any, V>`、`ObservableProperty<V> : ReadWriteProperty<Any, V>` 时，`tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_single_thread_ok.scoop` 在编译期触发 `scoop::itable::stable_type_id`：`missing stable type parameter key for \`V\``。该问题来自泛型 class 实现参数化 interface 的 runtime itable metadata 计算，不是委托库局部可绕过的问题；已新增 P5-T00/P5-T00R 作为前置修复。
- 完成记录：
  - 2026-05-31：在 `sysroot/lib/scoop.delegates/src/delegates.scoop` 中把 `lazy` / `observable` / `vetoable` 从声明式 `@Intrinsic` 改为普通库实现：新增 `Lazy<V>`（`V?` memoized storage + `Mutex?`，`None` 无锁、`Publication` 初始化期间不持锁、`Synchronized` 锁内单次初始化）、`ObservableProperty<V>`（锁内写入后解锁回调）和 `VetoableProperty<V>`（锁内读取 old、解锁回调、通过后再锁内提交），均实现既有 `ReadOnlyProperty` / `ReadWriteProperty` surface；三个顶层函数现在返回对应 wrapper class。`scoop.delegates` 显式依赖 P4 后的库 `scoop.sync`，以便线程安全模式组合普通 `Mutex`。
  - 同步更新默认 sysroot 依赖带来的验证期望：`p8_runtime_migration` 不再把 `scoop_sync_*` 视作普通构建禁止符号；release-hook 负例 fixture 改为只禁止本地未注解 class 的 release trampoline；HIR/effect-lowered goldens 更新默认 sysroot 新增 delegates/sync metadata 后的统计与布局快照。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1660)`）。

### [DONE] P5-T01R：Review P5-T01 委托库实现

- 必须实现的内容：复核三者实现 `ReadOnlyProperty`/`ReadWriteProperty` 正确、lazy 三模式与回调/否决语义对齐、线程安全靠库 `Mutex`/`Once` 组合。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P5-T01
- 完成记录：
  - 2026-05-31：复核 P5-T01 委托库实现：`Lazy<V>` / `ObservableProperty<V>` / `VetoableProperty<V>` 均为普通库 class，分别实现 `ReadOnlyProperty<Any, V>` / `ReadWriteProperty<Any, V>`；`lazy` / `observable` / `vetoable` 顶层函数已无 `@Intrinsic`，线程安全路径组合 P4 后的库 `Mutex`。复核 lazy 三模式：`None` 无锁单线程 memoize，`Synchronized` 锁内单次初始化，`Publication` 初始化期间释放锁、二次加锁发布；observable 写入后解锁回调，vetoable 先读取 old、解锁回调、通过后再提交，语义与现有 by-name 回归一致。
  - Review 中修正 P5-T01 暴露的真实缺口：默认 `lazy(initializer)` 不再转调同名泛型重载，改为直接构造 `Lazy`，避免 generic MIR materialization 的重载转调 contract 漂移；resolver 现在允许 interface receiver 沿 super-interface 解析继承的抽象方法；MIR lowering/materialization 修复 owner-specialized generic member store 的 receiver metadata；构造泛型 class 时会把其 itable method instances 纳入初始 materialization 种子，确保 `Lazy<Int>` 这类实现参数化 interface 且方法签名依赖 owner type param 的 wrapper 可发布对应 callable/source signature。新增 `delegate_library_wrappers_construct_basic.scoop`，直接构造三类 wrapper 与三个顶层函数，锁定普通库构造路径。
  - 验证：`cargo fmt`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/delegate_library_wrappers_construct_basic.scoop`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/generic_class_parameterized_interface_itable_stable_id.scoop`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/delegated_property_lazy_init_once_basic.scoop`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/delegated_property_observable_vetoable_basic.scoop`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1661)`）。

### [TODO] P5-T02：删除三者 by-name 合成与分叉

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5-T02
  - `hir/lower/sugar.rs`（lazy get `:201-622`、observable get/set `:624-854`、vetoable `:856-1047`）
  - `hir/lower/util/decls.rs`（lazy 字段注入 `:933-987`、observable/vetoable `:1000-1046`）
  - `hir/lower/main/impl_lowering.rs:33-36`（`SYNC_MUTEX_*` 常量）及使用点 `decls.rs:964-986/1021-1029`
- 必须实现的内容：
  1. 删除 `sugar.rs` 三者的 get/set 合成与 `decls.rs` 的 backing 字段注入。
  2. 删除 `impl_lowering.rs:33-36` 的 `SYNC_MUTEX_*` 常量及其全部使用点。
  3. 删除 `ParsedStdDelegateExpr::{Lazy,Observable,Vetoable}` 分叉，使 `DelegatedPropertyInfo` 只剩泛型分支；`by lazy{...}`/`observable(...)`/`vetoable(...)` 经普通表达式求值 + 泛型委托 lowering 接入。
- 必须遵从的约束：泛型委托路径不得改动；删除后这三者完全走泛型路径。
- 验证：`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：编译器属性委托 lowering 只剩唯一的泛型路径。
- 依赖：P5-T01
- 完成记录：
  - （待填）

### [TODO] P5-T02R：Review P5-T02 特判删除

- 必须实现的内容：复核三者合成与 `SYNC_MUTEX_*` 常量已全删、`DelegatedPropertyInfo` 只剩泛型分支、无残留 by-name 分叉。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P5-T02
- 完成记录：
  - （待填）

### [TODO] P5-T03：委托回归 + 守卫扩展

- 参考：[`PLAN.md`](./PLAN.md) §5 / P5-T03、§6（委托库化语义对齐）
- 必须实现的内容：
  1. 把现有 lazy/observable/vetoable run-pass / hir fixtures 切到库实现，验证语义不变：lazy 三模式、observable 回调在写后、vetoable 否决不写、并发可见性。
  2. 把「零编译器硬编码」grep 守卫扩展到 `lazy`/`observable`/`vetoable` 及 `scoop.sync.Mutex` 注入点；P4-T04 若保留过消费侧允许点，此时一并删除并收紧守卫。
  3. 删除/迁移已被取代的旧委托合成 fixtures。
- 必须遵从的约束：以迁移前语义为基线逐项对齐；分配开销变化（委托对象字段 vs 宿主内联字段）在基线说明中注明。
- 验证：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：委托回归与守卫绿；语义逐项一致。
- 依赖：P5-T02
- 完成记录：
  - （待填）

### [TODO] P5-T03R：Review P5-T03 回归与守卫

- 必须实现的内容：复核语义逐项对齐（尤其 lazy 三模式与 vetoable 否决）、守卫覆盖三者且无遗漏允许点、旧 fixtures 已清理。
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P5 整体收口；属性委托对编译器完全透明。
- 依赖：P5-T03
- 完成记录：
  - （待填）
