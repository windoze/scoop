# Scoop：Managed ABI / native callable ABI 收口计划

> 生成时间：2026-05-14  
> 设计基线：[`MANAGED_ABI.md`](./MANAGED_ABI.md)  
> 格式参考：`docs/archive/plans/PLAN-pipeline-gaps.md`、`docs/archive/plans/TODO-pipeline-gaps.md`  
> 当前状态：`ExternAbi::Scoop` 已完成 declaration / call lowering 及 ABI-specific binary-boundary contract；native `@Extern` 与 `FunPtr` 已在参数/返回 lowering 上部分对齐，但 ABI identity、native boundary scaffold、surface gate 仍分裂；`string cone` 仅完成部分 sysroot 化。  
> 行号说明：下文以当前文件路径和函数名为准；后续若行号漂移，优先按文件路径、符号名和 fixture 名定位。

## 0. 工作原则

- [`MANAGED_ABI.md`](./MANAGED_ABI.md) 是本轮设计基线，但不能假设当前代码已经完全符合该文档。凡是后续实现要改变本文记录的 contract，必须先回写 `MANAGED_ABI.md`，再继续改代码。
- 当前活跃计划文档只有根目录的 `PLAN.md` / `TODO.md`；旧 round 文档只作历史参考，不再回写。
- 本轮目标不是“再补一组 helper 特判”，而是把 callable 的 ABI 身份、native boundary scaffolding、以及 managed external surface 一次性收口成可追踪 contract。
- `FunctionType` 继续只表达源码级签名；后续实现不得再默认把它等同于“已知 ABI 的 callable”。
- 当前 mainline 已经接受部分 native aggregate callable surface，例如 `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`。本计划默认沿着“正式化并统一当前行为”推进；若要回退为更窄 allowlist，必须先更新 `MANAGED_ABI.md`、同步重写 fixture，再改实现。
- 当前 contract 下，`FunPtr<F>` 中的 `F` 必须保持无 effect；`@Extern` / `FunPtr` 都不能成为 effect/control 穿越 FFI 的通道。后续重构必须保留这条收窄后的用户可见能力。
- `ExternAbi::Scoop` v1 仍然遵守 `MANAGED_ABI.md` 的收窄边界：
  - 只支持顶层函数；
  - 只支持 `Pure`；
  - 不支持 effect row 参数；
  - 不支持 outward suspend / continuation crossing；
  - 不支持 generics / closure/function-value crossing。
- `@Extern` 的 `@Unsafe` / `@NoGC` 语义由 ABI 决定：默认/`abi = "c"` 隐含两者，`abi = "scoop"` 不隐含两者；无论哪种 ABI，`@Extern` 都不允许再显式叠加 `@Unsafe` / `@NoGC`。
- native surface 与 managed external surface 的验证都必须同时覆盖：
  - 直接调用；
  - 间接调用 / token round-trip；
  - IR 级合同；
  - 语义级回归；
  - 至少 `linux/amd64` 与 `macos/aarch64` 的 parity。
- 本轮要复用现有已迁移成果，不得重复做已经完成的工作：
  - `sysroot/string.scoop` 中的 `substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 已是普通 Scoop helper；
  - `unsafe_funptr_aggregate_return_tuple` 已把 `FunPtr` aggregate return 从 ordinary hidden sret 改到目标平台 native return ABI。

## 1. 当前判断

- `ExternAbi::Scoop` 已完成 front-end / HIR / declaration / call lowering 接线。
  - `crates/scoopc/src/hir/mod.rs` 中 `ExternAbi` 已区分 `C` / `Scoop`；
  - `crates/scoopc/src/typecheck/annotations.rs` 已接受 `abi = "scoop"` 并施加 v1 front-end gate，同时固定 `@Extern` 的 ABI-specific `@Unsafe` / `@NoGC` 合同；
  - `crates/scoopc/src/hir/lower/util.rs::extern_fun_of_decl()` 已把 ABI 写入 `ExternFun.abi`；
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` 与 `crates/scoopc/src/llvm/tests.rs` 已锁定 managed external call 不再回退到 native leaf scaffold。
- native `@Extern` 与 `FunPtr` 的参数/返回 ABI 已经部分对齐，但 contract 仍是“半统一”。
  - 已对齐的部分：
    - direct `@Extern` 与 `FunPtr` 都使用 native param lowering；
    - 两者都不再为 native aggregate return 套 ordinary hidden sret；
    - `unsafe_funptr_aggregate_return_tuple` 说明 `FunPtr` aggregate return 至少有一条真实平台 ABI 回归。
  - 仍分裂的部分：
    - direct `@Extern` 会插 `enter_native/leave_native`，`FunPtr` 不会；
    - direct `@Extern` 声明会打 `gc-leaf-function`，`FunPtr` 没有等价 classifier；
    - direct `@Extern` 可读取 side table 中的 calling convention，`FunPtr` 间接调用直接写死 callconv `0`；
    - `FunPtr` 的 ABI identity 会在 lowering 前丢失成“`F` + word-sized address”，没有 family / callconv / native boundary 元数据。
- native surface 仍然是 `GC-free` gate，不是 `ABI-safe` gate。
  - `crates/scoopc/src/typecheck/annotations.rs::check_extern_fun_signature_is_gc_free()` 仍是唯一 `@Extern` signature 门禁；
  - `crates/scoopc/src/typecheck/lower.rs` 对 `FunPtr<F>` 只检查“`F` 是函数类型”；
  - `crates/scoopc/src/typecheck/expr/call.rs` 对 `FunPtr` 调用只要求 unsafe context；
  - 当前没有统一的 “这类类型允许穿过 native ABI / 这类不允许” classifier。
- `FunPtr` 应只承担 native function pointer surface。
  - `FunPtr<F>` 中的 `F` 必须在前端保持无 effect；
  - `FunPtr` 调用必须始终走 ordinary native function-pointer ABI，而不是 effect/state-machine 路径；
  - 现有任务仍需把这一 contract 收口进 typed call contract、LLVM lowering 和 regression matrix。
- `string cone` 不是从零开始。
  - 已迁移到普通 sysroot/helper 的部分：
    - `sysroot/string.scoop`：`substring`、`indexOf`、`contains`、`startsWith`、`endsWith`、`split`、`trimStart`、`trimEnd`、`trim`；
    - `crates/scoopc/src/llvm/tests.rs::single_file_minimal_ir_includes_compilable_sysroot_string_helpers()` 已锁定这批 helper 会作为普通 managed 函数编进模块；
    - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 已删掉 `substring` 与 `starts_with/ends_with/index_of/contains/split/trim*` 的 runtime declaration。
  - 仍依赖 special-case / runtime helper 的部分：
    - `sysroot/core.scoop` 中 body-less builtin surface：`Int/Bool/Char/Float*.toString`、`Char.toInt/hash`、`Float*.toInt/hash/abs/isNaN/isInfinite`；
    - `resolve/scopes.rs`、`typecheck/expr/call.rs`、`hir/lower/expr.rs` 中对 `String.length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/byteLength/getByte/unsafeSliceBytes/trimIndent` 的 allowlist 或 synthetic contract；
    - `llvm/codegen/call/lowering.rs`、`llvm/codegen/intrinsics/builtin.rs`、`llvm/codegen/effect_lowered/{body.rs,value.rs}` 中按 FQN/member-name 的 lowering 特判；
    - `runtime/c/scoop_runtime.c` 中仍保留 `scoop_{bool,char,int,float}_to_string`、`scoop_string_concat/hash/is_empty/replace/repeat/compare_to/unsafe_slice_bytes/trim_indent` 等 helper。
- 因为上面的现状，当前实现顺序应调整为：
  1. 先冻结 current baseline，避免重构时误把已通过回归的 native aggregate / effectful bridge 表面回退掉；
  2. 再把 callable ABI identity 变成一等 contract；
  3. 再统一 native direct/indirect classifier 与 gate；
  4. 最后引入 `ExternAbi::Scoop`，用它清理仍未迁出的 string/runtime helper。

## 2. Gap 覆盖矩阵

| Gap | 当前状态 | 本轮动作 | 归属阶段 |
|---|---|---|---|
| `ExternAbi::Scoop` declaration / call lowering 与 ABI-specific contract 缺失 | Done | 已完成 managed extern declaration/call lowering，并收口 `unsafe` / `@NoGC` 语义 | P3 |
| `ExternFun.abi` 已有字段但没有 consumer | Partial | 让 `ExternFun.abi` 成为 declaration / call lowering 的 source of truth | P1 |
| `FunctionType` 与 callable ABI identity 混在一起 | Open | 引入显式 ABI family / callable identity，并贯穿 typed call contract 与 LLVM lowering | P1 |
| native `@Extern` 与 `FunPtr` 只在参数/返回 ABI 上部分对齐 | Partial | 建立单一 native ABI classifier，统一 declaration / direct call / indirect call / MIR path | P2 |
| native surface 仍是 `GC-free`，不是 `ABI-safe`；`FunPtr` 也未复用同一 gate | Open | 明确 current v1 native surface contract，并让 `@Extern` / `FunPtr` 共用 gate 与诊断 | P2 |
| `FunPtr` contract 需要收窄为 pure-only native surface | Open | 前端拒绝非纯 `FunPtr<F>`，并让 indirect call 始终走 ordinary native ABI | P1-P2 |
| `string cone` 已部分 sysroot 化，但 scalar/string helper 仍是 builtin/runtime 特判 | Partial | 用 `ExternAbi::Scoop` 迁走 remaining string helpers，删除 resolver/typecheck/codegen/runtime 名字驱动路径 | P4 |
| native / managed external 的 IR 与跨平台回归矩阵不完整 | Open | 补 direct/indirect parity、IR contract 与跨平台 matrix，回写文档 | P5 |

## 3. 代码入口总表

| 主题 | 入口文件 / 位置 | 当前问题 | 目标状态 |
|---|---|---|---|
| `@Extern` 注解与 HIR 元数据 | `crates/scoopc/src/typecheck/annotations.rs`、`crates/scoopc/src/hir/lower/util.rs`、`crates/scoopc/src/hir/mod.rs` | 只支持 `name` / `lib`，`ExternAbi` 只有 `C`，`ExternFun.abi` 未被消费 | `abi` 参数进入前端/LoweredHir，并成为 lowering 分流依据 |
| callable ABI identity / `FunPtr` contract | `sysroot/unsafe.scoop`、`crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/pipeline/hir_stage.rs` | `FunPtr` 只保留 `FunctionType` 与 word-sized address，typed call contract 不携带 ABI family，也缺少对非纯 `F` 的统一收口 | 直接调用、间接调用都能查询同一 ABI identity，且 `FunPtr<F>` 保持 pure-only native contract |
| native declaration / call lowering | `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/call/lowering.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` | direct `@Extern` 与 `FunPtr` 在 `enter_native/leave_native`、`gc-leaf`、callconv 上分裂；`FunPtr` 路径还残留过时的 effect boundary 分支 | 单一 native classifier 统一 declaration / direct call / indirect call / MIR path |
| native regression / matrix | `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`、`tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`、`tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`、`tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`、`tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`、`tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`、`crates/scoopc/src/llvm/tests.rs` | direct extern 合同已有 fixture，但 direct/indirect parity、callconv identity，以及 non-pure `FunPtr` rejection 还未作为一个矩阵锁定 | native direct/indirect surface 与 non-pure `FunPtr` rejection 都有稳定回归 |
| 已迁移的 sysroot string helpers | `sysroot/string.scoop`、`crates/scoopc/src/resolve/scopes.rs`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/llvm/tests.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs` | 已经 ordinary managed 化，但依赖底层 byte primitives；需要保护既有成果 | 继续保留为 ordinary managed helper，不回退到 runtime helper |
| 仍在 compiler/runtime 中 special-case 的 string helpers | `sysroot/core.scoop`、`crates/scoopc/src/resolve/scopes.rs`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/hir/lower/expr.rs`、`crates/scoopc/src/llvm/codegen/call/lowering.rs`、`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/{body.rs,value.rs}`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`runtime/c/scoop_runtime.c` | 仍按 FQN/member-name 和 runtime symbol 拦截 | 迁到 `ExternAbi::Scoop` 或普通 sysroot helper；runtime 只保留 substrate |

## 4. 顺序总览

1. P0：冻结当前 ABI baseline 与回归矩阵，明确哪些现有行为必须保持。
2. P1：把 callable ABI identity 从 `FunctionType` 中拆出来，并让 `ExternFun.abi` 真正进入 lowering source of truth。
3. P2：统一 native direct/indirect classifier、boundary scaffolding 与 surface gate。
4. P3：实现 `ExternAbi::Scoop` 的前端表示、HIR side table 与 declaration/call lowering。
5. P4：用 remaining string helpers 做 tracer bullet，清理 resolver/typecheck/codegen/runtime 中的名字特判。
6. P5：做全量稳定化、跨平台 matrix、文档与注释收尾。

依赖说明：

- P0 必须早于 P1-P4，因为当前 mainline 已有一批“看起来像设计 drift、但实际上被 fixture 锁住”的行为；不先冻结 baseline，后续很容易误把正确行为回退掉。
- P1 必须早于 P2-P4，因为 native leaf / pure-only `FunPtr` / managed external 只有在 ABI family 明确后才能统一 lower。
- P2 必须早于 P3，因为 `ExternAbi::Scoop` 不应建立在仍分裂的 native classifier 之上；否则 direct/indirect/managed external 三条线会再次交叉污染。
- P3 必须早于 P4，因为 P4 迁 string helper 的前提就是 external managed helper 已有正式出口。
- P5 之前不算完成；只做少量 string helper 或单个平台通过，不代表 callable ABI contract 已闭环。

## 5. 分阶段计划

### P0. 冻结当前 ABI baseline 与回归矩阵

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §1、§5、§10
- `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
- `tests/fixtures/run-pass/unsafe_funptr_{extern_call_basic,aggregate_return_tuple}.scoop`
- `tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`

目标：

- 把“已经在当前 mainline 上成立的 ABI 行为”先固定下来，避免后续 refactor 误回退。
- 明确哪些测试锁的是 current behavior，哪些设计主张还未实现。
- 给 P1-P4 提供一份不用再来回搜索的 baseline。

必须实现的内容：

1. 把当前 baseline 写成明确 contract：
   - direct `@Extern` native leaf；
   - native `FunPtr` indirect call；
   - non-pure `FunPtr<F>` front-end rejection。
2. 为现有行为补齐或刷新回归矩阵，至少覆盖：
   - `@Extern` direct call 会插 `enter_native/leave_native`；
   - `@Extern` direct call 不重新进入 statepoint rewrite；
   - native `FunPtr` aggregate return 继续按目标 ABI 返回，不回 ordinary hidden sret；
   - non-pure `FunPtr<F>` 必须在前端被拒绝；一旦允许调用，`FunPtr` call 必须保持 ordinary native ABI。
3. 明确记录 current design drift：
   - native surface 仍是 `GC-free`；
   - `FunPtr` 还没有统一 gate / callconv / native boundary classifier；
   - `ExternAbi::Scoop` 仍不存在。
4. 对已迁移 string helper 建立“不可回退到 runtime helper”的 regression audit：
   - `substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 应继续由 `sysroot/string.scoop` 编译进模块。

必须遵从的约束：

- P0 不提前“实现 `ExternAbi::Scoop`”，也不提前改变 native gate，只冻结 baseline 与回归。
- P0 不得把 `unsafe_funptr_aggregate_return_tuple` 这类现有 passing surface 重新解释成“应该失败所以先删掉测试”。
- P0 的审计结果若与 `MANAGED_ABI.md` 冲突，应先在 `PLAN.md` / `TODO.md` 中明确标出 drift，留待后续阶段统一处理。

阶段输出：

- 一份固定的 direct extern / native funptr baseline，以及 non-pure `FunPtr` rejection gate。
- 一份已迁移 string helper 清单与 remaining special-case 清单。
- 一组足以保护后续重构的最小 regression matrix。

验证：

1. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
6. `cargo test -p scoopc llvm_tests -- --nocapture`
   - 重点关注 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers`

完成条件：

- 后续 agent 可以直接从固定 baseline 出发，不需要重新判读哪些 current behavior 是故意的、哪些是 gap。

### P1. 建立 callable ABI identity，并让 `ExternFun.abi` 真正进入 lowering

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §4.3、§4.4、§6
- `crates/scoopc/src/hir/mod.rs`
- `crates/scoopc/src/pipeline/hir_stage.rs`
- `crates/scoopc/src/llvm/codegen/{mod.rs,call/lowering.rs,mir_body.rs}`
- `sysroot/unsafe.scoop`

目标：

- 明确切开“源码级签名”和“调用 ABI 身份”。
- 让 `ExternFun.abi`、typed call contract、native `FunPtr` 都能把 ABI family 明确传给 lowering。
- 为 P2 的 native classifier 和 P3 的 `ExternAbi::Scoop` 预留稳定输入结构。

必须实现的内容：

1. 在内部表示中新增或显式化 callable ABI identity，至少区分：
   - ordinary managed callable；
   - external C/native callable；
   - external Scoop/managed callable；
   - effect-step managed callable（用于 ordinary managed outward-effect surface，而不是 `FunPtr`）。
2. 让 `ExternFun.abi` 成为实际 consumer 使用的数据，而不再只是 HIR side table 里的占位字段。
3. 扩展 typed call contract / effect facts / MIR handoff 中与 `FunPtr` 相关的 contract，使 direct `@Extern`、native `FunPtr`、effectful `FunPtr` 不再都只剩 `FunctionType`。
4. 明确 `funPtrToUIntPtr` / `uintPtrToFunPtr` 在内部 contract 中是否保留 ABI family：
   - 如果语言表面只保留地址，内部 lowering 仍需决定在哪一层重新附着 ABI identity；
   - 不允许继续靠 callsite 末端“看到 `FunPtr<F>` 就现场猜是 native 还是 bridge”。

必须遵从的约束：

- 不得通过“再多加一个 `if is_extern` / `if nominal.fqn == FunPtr`”充当 ABI identity。
- 不得放宽当前 `extern_fun_effectful_funptr_is_error` 这类收窄后的 surface。
- `FunctionType` 仍只表示源码级签名；若需要 callconv / leaf / ABI family，必须进入新的 identity contract。

阶段输出：

- 一套可被 declaration path、direct call、indirect call共用的 ABI identity 数据结构。
- 一套能在 HIR/MIR/LLVM 之间稳定传递的 identity source-of-truth。

验证：

1. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`
2. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
4. `cargo test -p scoopc llvm_tests -- --nocapture`
   - 重点关注 `abi_baseline_native_funptr_aggregate_return_uses_native_result_abi`

完成条件：

- 后续 P2/P3 不再需要靠 `extern_funs.contains_key(fqn)` 或 `nominal.fqn == "scoop.unsafe.FunPtr"` 来推断 ABI family。

### P2. 建立单一 native ABI classifier，并统一 gate / boundary scaffolding

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §5.1、§5.4、§6.2a、§10.1、§10.4
- `crates/scoopc/src/typecheck/annotations.rs`
- `crates/scoopc/src/typecheck/lower.rs`
- `crates/scoopc/src/typecheck/expr/call.rs`
- `crates/scoopc/src/llvm/codegen/{mod.rs,call/lowering.rs,mir_body.rs}`
- `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
- `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`

目标：

- 让 direct `@Extern` 与 native `FunPtr` 使用同一个 native ABI classifier。
- 统一 declaration / direct call / indirect call / MIR path 对以下维度的决策：
  - 参数 lowering；
  - 返回 lowering；
  - aggregate return 策略；
  - calling convention；
  - `enter_native/leave_native`；
  - `gc-leaf-function`；
  - effect boundary。
- 让 `@Extern` 与 native `FunPtr` 共用同一 surface gate 与诊断。

必须实现的内容：

1. 建立单一 native ABI classifier，并让以下入口复用：
   - `llvm/codegen/mod.rs` 的 extern/native declaration；
   - `llvm/codegen/call/lowering.rs` 的 direct extern call、native `FunPtr` call、`.invoke(...)`；
   - `llvm/codegen/mir_body.rs` 的 pass MIR funptr/direct-call path。
2. 明确 current native surface 的 v1 合同。
   - 当前仓库已经接受 aggregate-return `FunPtr` surface；
   - 本阶段必须决定：
     - 正式支持并把它纳入 classifier + parity tests；或
     - 显式收紧 gate，并同步改动现有 fixture / diagnostics / `MANAGED_ABI.md`。
   - 不允许继续保持“代码默认放行、文档默认拒绝、测试又只锁一部分”的中间状态。
3. 让 `@Extern` 与 native `FunPtr` 共用同一 gate。
   - 不能存在“direct extern 被 reject，但等价 `FunPtr<F>` 仍可过”的旁路；
   - 也不能存在 direct `@Extern` 和 native `FunPtr` 对同一签名采用不同 callconv / boundary scaffold。
4. 在 classifier 中显式排除 `FunPtr` 的 effect/state-machine 路径：
   - 所有 `FunPtr` call 都必须落到 native leaf family；
   - non-pure `FunPtr<F>` 必须在前端被拒绝；
   - 不能继续让 `FunPtr` 在 classifier 不可见的情况下混入 effect-specific lowering。
5. 补 direct/indirect parity 回归，至少覆盖：
   - 标量参数 / 标量返回；
   - aggregate return；
   - token round-trip；
   - `enter_native/leave_native` / `gc-leaf` / callconv 行为；
   - `linux/amd64` 与 `macos/aarch64`。

必须遵从的约束：

- 不得重新引入 ordinary hidden sret 伪装 native aggregate return。
- 不得在 `FunPtr` callsite 继续硬编码 `callconv 0` 而跳过 classifier。
- 若决定保留 aggregate native surface，验证必须贴近真实平台 ABI，而不是再造“native helper 模拟 ordinary sret”的测试。
- 若决定收紧 gate，必须先回写 `MANAGED_ABI.md` 并同步更新现有 passing fixture，不能静默把回归改坏。

阶段输出：

- 单一 native ABI classifier。
- `@Extern` / native `FunPtr` 共用的 surface gate 与 diagnostics。
- direct/indirect parity matrix。

验证：

1. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
5. `cargo test -p scoopc llvm_tests -- --nocapture`
   - 重点关注 direct/indirect parity 与 effectful funptr boundary 相关单测
6. 跨平台补跑：
   - `linux/amd64`
   - `macos/aarch64`

完成条件：

- native callable 的 lowering 解释基于 ABI family，而不是“这次是 direct call 还是 indirect call”。

### P3. 实现 `ExternAbi::Scoop` 的前端表示与 declaration / call lowering

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §3、§4、§5.2、§6.1、§6.2
- `crates/scoopc/src/typecheck/annotations.rs`
- `crates/scoopc/src/hir/lower/util.rs`
- `crates/scoopc/src/hir/mod.rs`
- `crates/scoopc/src/llvm/codegen/{mod.rs,call/lowering.rs,mir_body.rs}`

目标：

- 为 external linkage 下的 ordinary managed helper 提供正式出口。
- 让 `ExternAbi::Scoop` 从注解、HIR、declaration path 到 call lowering 全链路可用。
- 让 imported Scoop ABI call 不再被误当作 native leaf。

必须实现的内容：

1. 扩展 `@Extern` 语法和 front-end 校验，支持显式 ABI 字段。
   - 建议沿用 `MANAGED_ABI.md` 的 `abi = "scoop"`；
   - `abi` 省略时默认仍是 `c`；
   - 若实现中选择其它等价 surface，必须保证 HIR 能稳定区分 `C` 与 `Scoop`。
2. 扩展 `ExternAbi`、`ExternFun` lowering 与相关 diagnostics：
   - `ExternAbi::C`
   - `ExternAbi::Scoop`
3. 在 declaration path 中实现 `ExternAbi::Scoop`：
   - external linkage；
   - ordinary param ABI；
   - ordinary return / hidden sret；
   - 不打 `gc-leaf-function`；
   - 不插 `enter_native/leave_native`；
   - 初版 machine callconv 仍用默认 `0`，但必须与 native callconv 路由明确分离。
4. 在 call lowering 中实现 `ExternAbi::Scoop`：
   - ordinary callsite；
   - conservative GC root spill；
   - statepoint rewrite / safepoint correctness；
   - imported aggregate return 允许 hidden sret；
   - 不走 native leaf path。
5. 锁定 v1 限制：
   - top-level only；
   - `Pure` only；
   - 无 effect row 参数；
   - 无 generics / closure/function-value crossing；
   - 无 outward suspend / continuation crossing。

必须遵从的约束：

- 不得把 `@Extern("...", abi = "scoop")` 解释成“仍然是 native leaf，只是不插 `enter_native`”。
- 不得让 `ExternAbi::Scoop` 复用 `ExternAbi::C` 的 GC-free gate。
- `ExternAbi::Scoop` 必须以 ABI family 分流，而不是再通过 helper FQN 名字决定 whether managed。
- `abi = "scoop"` 不得要求 `unsafe context`，也不得在 `@NoGC` 中被视为 leaf。
- `@Extern` 无论 ABI 为何都必须拒绝显式 `@Unsafe` / `@NoGC`；`abi = "c"` 由 ABI 隐含两者，`abi = "scoop"` 则都不隐含。

阶段输出：

- `ExternAbi::Scoop` 的 parser/typecheck/HIR/codegen support。
- imported managed external call 的 IR 与 run-pass regression。

验证：

1. 新增或更新 parser/typecheck fixture，覆盖 `@Extern(..., abi = "scoop")`
2. `cargo test -p scoopc llvm_tests -- --nocapture`
   - 重点关注 declaration / call IR 中“不出现 `enter_native/leave_native`、可出现 hidden sret、仍保留 ordinary managed call contract”
3. `cargo run -p scoop -- test --fixtures <managed-abi-fixtures>`

完成条件：

- external managed helper 已经有正式 ABI surface，不再只能用 runtime helper / compiler special-case 承接。

### P4. 用 remaining string helpers 做 tracer bullet，删除名字驱动特判

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §8、§9、§10
- `sysroot/core.scoop`
- `sysroot/string.scoop`
- `crates/scoopc/src/resolve/scopes.rs`
- `crates/scoopc/src/typecheck/expr/call.rs`
- `crates/scoopc/src/hir/lower/expr.rs`
- `crates/scoopc/src/llvm/codegen/{call/lowering.rs,intrinsics/builtin.rs,effect_lowered/body.rs,effect_lowered/value.rs,runtime_abi.rs}`
- `runtime/c/scoop_runtime.c`

目标：

- 选取仍依赖 special-case 的 string helper，验证编译器已按 ABI 而不是按 helper 名字工作。
- 在不回退 `sysroot/string.scoop` 已迁移成果的前提下，把 remaining runtime/string builtins 迁到 `ExternAbi::Scoop` 或 ordinary sysroot helper。
- 让 runtime 中的 string 职责继续收缩到 substrate / byte-level primitive。

必须实现的内容：

1. 先迁最小 tracer bullet：
   - `Int.toString`
   - `Bool.toString`
   - `Char.toString`
   - `Float32.toString`
   - `Float64.toString`
2. 再迁 current runtime-backed string helpers：
   - `String.toString`
   - `String.toInt`
   - `String.concat`
   - `String.hash`
   - `String.isEmpty`
   - `String.replace`
   - `String.charAt`
   - `String.repeat`
   - `String.compareTo`
   - `String.trimIndent`
   - 对 `String.length` / `byteLength` / `getByte` / `unsafeSliceBytes`，必须先显式分类为 substrate 或 managed helper，再决定是否迁移；不得继续留在“灰色地带”。
3. 对每个迁移项，必须同步删除或缩小以下位置的名字驱动路径：
   - `resolve/scopes.rs` 的 builtin allowlist；
   - `typecheck/expr/call.rs` 的 synthetic call contract；
   - `hir/lower/expr.rs` 的 member-access 保留逻辑；
   - `llvm/codegen/call/lowering.rs` 的 FQN/member-name short-circuit；
   - `llvm/codegen/intrinsics/builtin.rs` 与 `llvm/codegen/effect_lowered/{body.rs,value.rs}` 的 runtime intercept；
   - `llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c` 中仅为高层 helper 存在的 symbol/declare path。
4. 明确保留在 runtime substrate 的 string boundary：
   - `String` 物理布局；
   - managed allocation / object model；
   - byte-level primitives；
   - pin/handle/write barrier 等 substrate 规则。
5. 保持已迁移的 ordinary sysroot helper 不回退：
   - `substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 继续编进模块；
   - 若新增实现复用 `byteLength/getByte/unsafeSliceBytes` 等底层 primitive，应明确它们是 remaining substrate 还是下一个 migration target。

必须遵从的约束：

- P4 不是重写整个 string 子系统；只迁当前仍依赖 name special-case 的 remaining helper。
- 不得把 `sysroot/string.scoop` 已完成的普通 helper 再搬回 runtime，只为了“统一所有 string helper 都走同一路”。
- 每迁一个 helper，都必须同步清理 resolver/typecheck/codegen 中对应的名字特判，而不是保留“新 ABI 路径 + 旧 special-case”双轨。
- 若某个 helper 暂时必须保留在 runtime，需要在 `PLAN.md` / `TODO.md` 明确写出原因，不能默默留下。

阶段输出：

- 第一批 `ExternAbi::Scoop` string helper。
- 删除后的 compiler/runtime special-case 清单。
- 更小的 runtime string helper surface。

验证：

1. `cargo test -p scoopc llvm_tests -- --nocapture`
   - 重点关注 string helper IR 是否不再依赖旧 runtime intercept
2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
   - 重点关注 scalar toString、string concat/replace/repeat/compareTo、GC stress 相关样本
3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures <string-managed-abi-fixtures>`

完成条件：

- 第一批 string helper 已通过 `ExternAbi::Scoop` 或 ordinary sysroot helper 生效；
- 对应 compiler special-case 不再按 FQN/member-name 决定 lowering。

### P5. 全量稳定化、跨平台矩阵与文档收尾

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §10、§11、§12
- `PLAN.md`
- `TODO.md`

目标：

- 把 native surface 与 managed external surface 的 contract 全量锁定到测试和文档中。
- 让后续 agent 不需要重新解释“当前 repo 究竟接受哪些 ABI surface”。

必须实现的内容：

1. 全量回归至少覆盖：
   - `cargo test --all --all-targets`
   - `cargo run -p scoop -- test`
   - native direct/indirect parity suite
   - `ExternAbi::Scoop` IR / run-pass suite
   - `linux/amd64` 与 `macos/aarch64` matrix
2. 回写文档与注释：
   - 若 current native surface 合同与 `MANAGED_ABI.md` §5.1 存在持续性偏移，必须更新 `MANAGED_ABI.md`；
   - 更新 `sysroot/core.scoop`、`sysroot/unsafe.scoop` 中仍描述“body-less/codegen runtime route”的注释；
   - 更新 `runtime_abi.rs` / `runtime/c/scoop_runtime.c` 中已经迁移或保留的 helper 注释。
3. 审计 remaining special-case：
   - `ExternAbi::Scoop` 已覆盖的 helper 不得再残留 resolver/typecheck/codegen/runtime 多份同名特判；
   - 若仍保留少量 substrate helper，应在文档中列出 authoritative list。
4. 给后续 v2+ 留清晰边界：
   - dedicated managed import/export surface（例如 `import function` / `import interface`），而不是 `FunPtr` ABI 扩展；
   - generics；
   - outward effect / continuation ABI；
   - ABI version / feature bitmap。

必须遵从的约束：

- P5 不是“把剩余问题都记到以后”；剩余项必须是明确的 v2+ scope，而不是 v1 未闭环。
- 若 P2/P3/P4 实际实现改变了 `MANAGED_ABI.md` 的设计边界，文档回写是退出条件，不是可选收尾。

阶段输出：

- 完整 regression matrix。
- 与当前实现一致的 `MANAGED_ABI.md` / `PLAN.md` / `TODO.md` / sysroot/runtime 注释。
- v1 完整 contract 与 v2+ backlog 边界。

验证：

1. `cargo test --all --all-targets`
2. `cargo run -p scoop -- test`
3. `cargo test -p scoopc llvm_tests -- --nocapture`
4. CI / 手工跨平台矩阵：
   - `linux/amd64`
   - `macos/aarch64`

完成条件：

- native callable ABI 与 managed external ABI 都已成为明确、可回归、跨平台可审计的 contract；
- remaining backlog 只剩 v2+ 扩展，不再混入 v1 主线未完成项。

## 6. 预期收口状态

- `ExternAbi::Scoop` 已成为正式可用的 external managed ABI surface。
- `ExternFun.abi` 与 callable ABI identity 已成为 declaration / call lowering 的 source of truth。
- direct `@Extern`、native `FunPtr` 与 non-pure `FunPtr` rejection 的边界已被 classifier 和 regression 明确区分。
- native direct / indirect parity 已被 IR + run-pass + 跨平台 matrix 锁定。
- `sysroot/string.scoop` 已迁移 helper 保持 ordinary managed；remaining string helper 不再依赖 compiler/runtime 名字特判。
- runtime 收口到 substrate；编译器不再因为 helper FQN 而理解业务语义。
