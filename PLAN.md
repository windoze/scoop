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
5. P4 前置 I（P4-T01a/b/c）：先解锁“内建类型作为一等 struct/class implementer”的机制。
   - 让 struct/class（含 generic class）的 instance method 能用常规 `receiver.method()` 调用；
   - 让 interface dispatch 在 vtable / itable 收集与 marshal 上完全由 itable metadata 驱动，不再为内建类型保留 by-name thunk / FQN intercept；
   - 让 `@Intrinsic struct/class` 含 method body 这种 sysroot 写法走完整编译管线，layout 仍由编译器内置。
   - 这一组前置任务的硬约束是“零编译器后门（non-generic 维度）”：**未来再加非 generic 的内建 method（含 interface override 与独立 method）时，编译器不应再有任何改动**；只动 sysroot 声明源即可。
6. P4 前置 II（P4-T01d/e）：把 generic class 必须借助的"按 T 分流"后门收缩为**唯一可枚举的 intrinsic 表**，并把现有 `Array` / `MutableArray` 的 get/set/size 等基础操作从 runtime helper 改为 IR-direct emission，恢复 LLVM 优化路径（LICM / CSE / BCE / 自动向量化），同时删除对应的 runtime helper。
   - 引入 method-level `@Intrinsic("name")` declaration surface，配套编译器内置的 intrinsic 表；表的每条 entry 默认是 IR-emission 形式，**只有真正涉及 GC heap allocation / write barrier / move-GC composite copy 等 GC-adjacent 协作的少数 entry**才下放为 runtime call。
   - 用 `Array` / `MutableArray` 作为 first user，把 `array_size` / `array_get` / `array_set` / `array_data_ptr` 等 entry 实现为纯 IR；保留 `array_alloc` / `array_builder_grow` / `write_barrier` / `composite_copy` 等少数 runtime 协作 entry。
   - 删除 `runtime/c/scoop_array.c` 中已被 IR-direct 替代的 helper（`scoop_array_get_u64/ref/composite` / `scoop_array_set_*` / `scoop_array_len`），收缩 runtime substrate。
   - 这一阶段为后续 v2+ 把 runtime 进一步收缩到"GC + GC-adjacent only"打下基础（其它非 GC 内容如 `exit(3)` / `print/println` / 标量 helper 等可在后续阶段以 IR / Scoop ABI FFI 形式迁出，不在 v1 主线 scope 内）。
7. P4：用 remaining string helpers 做 tracer bullet，清理 resolver/typecheck/codegen/runtime 中的名字特判。
   - P4 主体（P4-T01）现在依赖 P4 前置 I：scalar `toString` 不再以扩展函数 + FQN intercept 形式落地，而是直接搬到 `@Intrinsic struct/class` 的 method body 内。
   - P4-T01 不依赖 P4 前置 II（scalar toString 不需要 generic-by-T 分流），但顺序上仍排在 P4 前置 II 之后，避免 intrinsic 机制半完成时穿插对内建类型的 sysroot 改写。
8. P5：做全量稳定化、跨平台 matrix、文档与注释收尾。

依赖说明：

- P0 必须早于 P1-P4，因为当前 mainline 已有一批“看起来像设计 drift、但实际上被 fixture 锁住”的行为；不先冻结 baseline，后续很容易误把正确行为回退掉。
- P1 必须早于 P2-P4，因为 native leaf / pure-only `FunPtr` / managed external 只有在 ABI family 明确后才能统一 lower。
- P2 必须早于 P3，因为 `ExternAbi::Scoop` 不应建立在仍分裂的 native classifier 之上；否则 direct/indirect/managed external 三条线会再次交叉污染。
- P3 必须早于 P4 前置 I，因为内建类型 interface method body 内会调用 `@Extern(abi = "scoop")` wrapper（例如 `scoopAbiIntToString`），需要 `ExternAbi::Scoop` 已经接通。
- P4 前置 I 必须早于 P4 前置 II，因为后者的 method-level `@Intrinsic("name")` 表机制依赖前者已经把 `@Intrinsic struct/class` 与 instance method 的全套 surface 接好。
- P4 前置 II 必须早于 P4 主体（顺序约束），避免 intrinsic 表机制半完成时穿插 scalar toString 的 sysroot 改写。
- P4 前置 I/II 都必须早于 P4 主体，因为 P4 主体的目标是“删除 by-name 特判”；如果机制侧仍然要靠 by-name 路径承接 sysroot 改动，P4 主体的删除目标就不成立。
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

### P4 前置 I. 内建类型 interface 实现机制（@Intrinsic struct/class with body methods）

参考：
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) §9
- `sysroot/core.scoop`（`struct Int : Hashable, ToString` 等已有 declaration-only 形式）
- `crates/scoopc/src/typecheck/expr/call.rs::infer_member_call_expr_type`
- `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access`
- `crates/scoopc/src/itable.rs`、`crates/scoopc/src/vtable.rs`
- `crates/scoopc/src/llvm/codegen/call/lowering.rs::{try_codegen_class_vtable_call,try_codegen_interface_itable_call_impl}`
- `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs::{try_codegen_tostring_iface_builtin,codegen_sysroot_to_string_ext}`
- `crates/scoopc/src/llvm/codegen/effect_lowered/{body.rs,value.rs}`（仍按 FQN 拦截 `ToString.toString` 与 scalar `toString` ext）
- `crates/scoopc/src/llvm/codegen/mir_body.rs::{codegen_mir_transport_to_string,mir_value_box_itable_entries}`

起因：

P4 主体（remaining string helpers tracer bullet）的目标是“按 ABI 而不是按名字 dispatch”。当前实现里，scalar `toString` / `Hashable.hash` 等内建 interface 实现仍以**扩展函数 + FQN intercept**的形式落地：

- sysroot 用 `fun Int.toString(): String` 这种 body-less 扩展函数声明；
- codegen 在 `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext` / `codegen_mir_transport_to_string` 等位置按 receiver `CgTy` 拦截到 runtime helper；
- `mir_value_box` 已经为 user struct 提供 itable 路径，但 primitive 不进这条路径（gate 在 `ValueTypeKind::Nominal` + 有 struct_layout，标量不满足）。

如果直接把 scalar `toString` 写成 sysroot bodied 扩展函数，本轮只能完成单态化路径上的删除；erased interface receiver / box 路径仍要为内建类型保留 by-name 分支。等到再加 `Hashable.hash` / `Eq` / `Ord` / `Iterable` 等内建 interface 时，又要为每一个 interface 重新加一组 codegen 后门 —— 这违反 P4 的根本目标。

正确的解构是：让内建类型成为**一等的 struct/class implementer**，唯一的特殊性是 layout 由编译器内置，其它部分（method body / interface impl / vtable / itable）和用户自定义类型完全同构。这样：

- method body 全部用 Scoop 写在 sysroot 里（不依赖编译器合成实现代码）；
- 编译器只负责"内存布局"侧 metadata：type_desc global、itable global、box header。这部分对所有 `@Intrinsic struct/class` 共用同一套机制，与具体 interface 无关；
- erased interface receiver / box 路径与单态化路径走**同一份 itable metadata**，不再有"按名字分发到 runtime helper"的捷径。

**硬约束（“零编译器后门”）**：本前置阶段必须保证，未来再加任何新的内建 interface（如 `Hashable` / `Eq` / `Ord` / `Iterable`），都不会再要求改编译器 —— 只需在 sysroot 内的 `@Intrinsic struct/class` body 里加 `override fun ...`，再加 `@Extern(abi = "scoop")` wrapper（如果 method body 需要 runtime 实现）。

目标：

- 解锁 struct/class（含 generic class）instance method 的常规 `receiver.method()` 调用，让 user 与 sysroot 的两类 declarer 共享同一前端路径。
- 把 vtable / itable 收集与 interface call ABI 收口为完全 metadata-driven（按 itable slot signature marshal），不再为内建类型保留 by-name thunk / FQN intercept。
- 让 `@Intrinsic struct/class` 含 method body 这种 sysroot 写法走完整编译管线（typecheck / HIR / MIR / codegen），且 layout 仍由编译器内置识别。**body 内允许声明任意 instance method（含 `override` 与不含 `override`）**：未来想给内建类型加非 interface method（例如 `Int.toBinaryString(): String`）时，直接写在 sysroot 的 `@Intrinsic struct/class` body 内，既不再需要扩展函数，也不需要改编译器。
- 把这些机制锁进 fixture，使得 P4 主体只剩"sysroot 改写 + 删旧 by-name 特判"，编译器侧不再为单个 interface 加任何配套改动。

必须实现的内容：

1. 解锁 struct/class instance method 的常规调用：
   - 当前 `sysroot/core.scoop` 注释明确说"typecheck 尚未支持普通成员函数调用（class/interface methods），`receiver.hash()` 仍主要走扩展函数调用的最小子集"；
   - 必须打通 user 自定义 `struct Foo { fun bar(): Int { ... } }` + `f.bar()` 直接 typecheck + lower + codegen；
   - 必须 cover generic class（Array<T> / MutableArray<T> 等 `@Intrinsic class` 的 generic 形态）。
2. 让 vtable / itable 收集统一从 struct/class body 内的 method 抽：
   - 现有 `itable.rs::collect_classes_in_type_decl` 与 `mir_value_box_itable_entries` 已经按 `<TypeFqn>.<methodName>` 查 fun_index；要保证 struct body method lower 后 FQN 一致，自然走同一路径；
   - 不再依赖"扩展函数 + 名字匹配"承接 instance method 路径。
3. interface call 的 ABI 收口为 metadata-driven marshal：
   - itable slot 函数签名与 method 实际签名一致；caller 按 slot 元数据 marshal 参数（class 类型按 ptr，value 类型按 by-value）；
   - 不为内建 value type 合成 box thunk；不在 codegen 里维持任何按 receiver `CgTy` 派生 runtime helper 名字的分支（这条要等 P4 主体清理完，前置阶段先把"接得通"做到，避免出现"两条 dispatch 路径并行"的中间状态）。
4. `@Intrinsic struct/class` 含 method body 完整落地：
   - parser / typecheck 接受这种 surface；
   - body 内不允许声明 fields（layout 由编译器内置，已在 sysroot 中如此）；
   - body 内允许声明 method（含 `override`）；method 部分照常 typecheck / lower / codegen；
   - `@AllowIntrinsic` gate 覆盖到 struct/class（user 文件需显式 `@file:AllowIntrinsic`，sysroot 文件天然过 gate）；
   - 编译器对 `@Intrinsic` 的特殊处理收敛到"layout 不来自源码"这一件事。
5. 不动 `Int` / `String` / `Char` / `Float32` / `Float64` 现有 sysroot 声明；不动 scalar `toString` / `Hashable.hash` 现有 codegen 路径。这一切真正搬迁动作留给 P4 主体（P4-T01）。

必须遵从的约束：

- 不得为本阶段引入"内建类型专用 interface dispatch 路径"。所有路径必须同时被用户自定义 struct/class 走过。
- 不得让 `@Intrinsic struct/class` 退化成"只支持 declaration-only"的轻量版本；method body 路径必须真打通。
- 不得把 P4 主体的"删除 by-name 分支"动作偷偷掺进本阶段；本阶段只新增机制，不删除既有 path（避免与 P4-T01 改动冲突）。
- 验收必须以 generic class（Array / MutableArray）作为机制覆盖性证据，但**不要求** Array / MutableArray 实现任何 sysroot interface（Iterable 等留以后做）。验证使用专用 dummy interface 作为载体即可。

阶段输出：

- `@Intrinsic struct/class` 含 method body 的 surface support。
- struct/class instance method 的常规调用支持（含 generic class）。
- itable / vtable 收集与 interface call ABI 收口为 metadata-driven。
- 一组锁定上述机制的 fixture，覆盖 user struct/class、user generic class、`@Intrinsic struct/class`（含 generic）。

验证：

1. 新增 fixture：用户自定义 `struct Foo { fun bar(): Int { ... } }` + `f.bar()` 调用通过编译并 run-pass。
2. 新增 fixture：用户自定义 `class Bar { fun baz(): Int { ... } }` + `b.baz()` 调用通过编译并 run-pass。
3. 新增 fixture：用户自定义 generic `class Box<T> { fun get(): T { ... } }` + `box.get()` 通过编译并 run-pass。
4. 新增 fixture：用户自定义 `interface I { fun m(): Int }` + struct/class（含 generic class）实现 `I` + `(it as I).m()` 走 itable dispatch 通过 run-pass。
5. 新增 fixture：测试用 `@Intrinsic struct Dummy { override fun ...(): ... { ... } }`，layout 内置识别正确，method 调用走标准路径。
6. 新增 fixture：把 `Array<T>` / `MutableArray<T>` 改为 `@Intrinsic class Array<T> : DummyIter { override fun ...(): ... { ... } }` 形式（Iterable 真接入留以后；本轮用 dummy interface 验证机制可落到 generic `@Intrinsic class` 上），通过 typecheck + lower + codegen。
7. 现有所有 pass fixture 不退化；尤其是 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers` 与 P3 完成的 managed extern 回归。
8. 编译器源代码 grep 验证：没有为本阶段新增任何 `if iface_fqn == "scoop.core.X"` / `if member_name == "toString"` 之类的 by-name 分支。

完成条件：

- user struct/class（含 generic class）能用常规 instance method 调用与 interface dispatch；
- `@Intrinsic struct/class` 含 method body 这种 sysroot 写法已可走完整编译管线；
- vtable / itable 与 interface call ABI 已收口为 metadata-driven，不再依赖内建类型 by-name 分支承接；
- P4 主体可以直接以"sysroot 改写 + 删旧 by-name 特判"形态推进，编译器侧不再需要为单个 interface 加任何配套改动。

### P4 前置 II. intrinsic 表 IR-emission 与 runtime 收缩（Array/MutableArray 起步）

参考：
- [`PLAN.md`](./PLAN.md) §5 / "P4 前置 I"（method-level intrinsic 是 type-level `@Intrinsic` 的延伸）
- `runtime/c/scoop_array.c`（`ScoopArray` layout 完全公开；`scoop_array_get_u64/ref/composite` / `scoop_array_set_*` / `scoop_array_len` 当前是 codegen 调的 helper，但 layout 已可让 codegen 直接 GEP）
- `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`、`runtime_abi.rs`（当前 array helper declaration）
- `crates/scoopc/src/llvm/codegen/intrinsics/containers.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`（当前调 array runtime helper 的 callsites）
- `crates/scoopc/src/opt/bce.rs`（已有 BCE pass，但当前因 array_get 走 helper 导致 LLVM 这层 bounds check 被 helper 内部重新引入）

起因：

P4 前置 I 把 non-generic 内建 method 的"零编译器后门"约束做完了 —— 但 generic 维度还有一处**真正不可避免的后门**：generic class 的 instance method body 没法用 Scoop 写"按 T 选哪条 lowering 路径"，因为 Scoop 没有 typecase / reflect。

调查 `Array<T>` / `MutableArray<T>` 的现状发现**两个独立问题**：

1. **必要后门尚未声明化**：当前 codegen 在 `intrinsics/containers.rs` 与 `effect_lowered/value.rs` 里按 receiver type 名字（`Array` / `MutableArray`） + element kind 分流到 `scoop_array_get_u64/ref/composite` 等 runtime helper。这是 by-name 特判的另一种形态，没有沉淀为可枚举的声明面。
2. **不必要地走 runtime**：`ScoopArray` 的 layout 完全公开（`{header, len, elem_size_bytes, data_offset_bytes, elem_desc, elem_kind, data[]}`），`array_get/set/size` 这类 access 完全可以由 codegen 直接 emit IR（GEP + load/store）。当前走 runtime helper 让 LLVM 看不穿这些操作，**LICM / CSE / BCE / 自动向量化全部失效**。这是性能 bug，不是必要的 substrate 边界。

正确解构：

- 引入 method-level `@Intrinsic("name")` declaration surface（与 type-level `@Intrinsic struct/class` 同构），配套编译器内置一张可枚举的**intrinsic 表**。
- 表的每条 entry 默认是 **IR-emission** 形式（编译器直接生成 `GEP / load / store / branch / phi` 等基础 IR）。
- **仅当**真正涉及 GC heap allocation / write barrier / move-GC composite copy 等无法用纯 IR 表达的 GC-adjacent 协作时，entry 才下放为 runtime call。这类下放 entry 的清单是可枚举的，作为 v1 substrate 边界。
- 用 `Array` / `MutableArray` 作为 first user 完成填充：`array_size` / `array_get` / `array_set` / `array_data_ptr` 等基础访问全部 IR-direct；`array_alloc` / `array_builder_grow` / `write_barrier` / `composite_copy` 保留 runtime 协作。
- 删除已被 IR-direct 替代的 runtime helper，收缩 substrate。

**长期方向**（仅作为本阶段决策的背景，不在本阶段 scope）：

runtime 的最终目标是只包含 GC 实现与 GC-adjacent 功能（GC handle / pin、thread registration / cleanup 等），以便后续向 WASM / JVM 等环境移植，或切换 GC 实现（如对接 BoehmGC）。其它能用 IR 或 Scoop ABI FFI 表达的内容（典型如 `exit(3)`、`print` / `println`、scalar helper 等）应陆续迁出 runtime。本阶段只做 Array / MutableArray 这一对，其它内容不在 v1 主线，留作 v2+ backlog。

目标：

- 引入 method-level `@Intrinsic("name")` surface 与编译器内置 intrinsic 表机制，使 generic-by-T 分流后门有唯一、可枚举、文档化的承载。
- 把 `Array<T>` / `MutableArray<T>` 的 `size` / `get` / `set` 等基础访问从 runtime helper 改为 IR-direct emission，恢复 LLVM 优化路径。
- 删除已被替代的 array runtime helper，明确 v1 substrate 边界。
- 让"零编译器后门"约束的精确措辞落地：**新内建 non-generic method 不要求改编译器；新内建 generic-by-T method 要求往 intrinsic 表加可枚举的新条目，且默认 IR-emission，仅在必要时下放 runtime**。

必须实现的内容：

1. parser / typecheck 接受 method-level `@Intrinsic("name")` declaration：
   - method 必须 body-less；
   - `"name"` 必须命中编译器内置 intrinsic 表，否则前端报错；
   - 受 `@AllowIntrinsic` gate 约束。
2. 编译器内置 intrinsic 表数据结构：
   - 每条 entry 标注 lowering 模式（`IrEmission` / `RuntimeCall`），且默认应为 `IrEmission`；
   - `IrEmission` entry 的 lowering rule 直接产生 LLVM IR，不引入新 runtime symbol；
   - `RuntimeCall` entry 必须明确说明"为什么不能用 IR 表达"（GC / write barrier / descriptor-driven copy 等），并在表里记录该理由作为审计依据。
3. 用 `Array` / `MutableArray` 填充表（first user，证明机制可用）：
   - `array_size`：emit `len` field load；
   - `array_get`：emit bounds check（与 BCE 协同，可被 unchecked 标记跳过）+ GEP + load，类型按 T 的 cg kind 选择 LLVM type，不再 widen 到 i64；
   - `array_set`：emit bounds check + GEP + store + write barrier intrinsic（仅当 T 是 ref kind）；
   - `array_data_ptr`：emit GEP 到 `data` 字段（用于 builder 内部、iterator 等）；
   - `array_alloc` / `array_builder_grow`：保留为 `RuntimeCall`（涉及 GC 分配）；
   - `write_barrier` / `composite_copy`：保留为 `RuntimeCall`（GC-adjacent 协作）。
4. 把 sysroot `Array<T>` / `MutableArray<T>` 改写为 `@Intrinsic class` 含 `@Intrinsic("...")` method declaration 形式：
   - 现有扩展函数 surface（`fun <T> Array<T>.size(): Int` 等）仍可保留作为对外 API 桥；
   - 也可直接迁移成 method 形式 + 删除扩展函数（取实施时较干净的形态，但不引入额外 by-name 路径）。
5. 删除已被 IR-direct 替代的 runtime helper：
   - `runtime/c/scoop_array.c` 中 `scoop_array_get_u64` / `scoop_array_get_ref` / `scoop_array_get_composite` / `scoop_array_set_u64` / `scoop_array_set_ref` / `scoop_array_set_composite` / `scoop_array_len`；
   - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` / `runtime_abi.rs` 中对应 declaration；
   - `crates/scoopc/src/llvm/codegen/intrinsics/containers.rs` / `effect_lowered/value.rs` 中对应 callsite；
   - 保留：`scoop_array_builder_*` / `scoop_array_alloc` 等涉及 GC 分配的 helper（作为 RuntimeCall intrinsic 入口）。

必须遵从的约束：

- intrinsic 表的 entry **必须默认 IrEmission**；任何想加 `RuntimeCall` entry 的 PR 必须在 PLAN/TODO 中给出"为什么不能 IR 表达"的明确理由，否则不予接受。
- 不得为 method-level `@Intrinsic` 加入"用户文件可声明任意 name"的 surface；name 必须在编译器表里枚举。
- 不得为本阶段引入新的 by-name 分支：`@Intrinsic("array_get")` 等的 lowering 必须按声明上的 string label 查表，而不是按 callee FQN 决定。
- 不得在本阶段顺手处理其它 runtime helper（如 `print/println` / `exit` / scalar helper 等），它们留 v2+ 阶段。
- 不得把已经 IR-direct 化的 array op 再倒回 runtime helper（即使遇到优化或 GC 边界问题）；遇到此类问题应从 RuntimeCall entry 中扩出新 helper，而不是把 IrEmission entry 改回 RuntimeCall。

阶段输出：

- method-level `@Intrinsic("name")` 与可枚举的 intrinsic 表机制。
- `Array` / `MutableArray` 的基础访问 IR-direct 化（含 BCE / LICM / 自动向量化等优化路径恢复）。
- 删除一组 array runtime helper，substrate 收缩动作有据可查。
- "runtime 长期收口到 GC + GC-adjacent" 这一长期方向的首批落地。

验证：

1. 新增 fixture：`@file:AllowIntrinsic` + 测试用 `@Intrinsic class Vec<T> { @Intrinsic("dummy_intrinsic") fun foo(): Int }`，dummy intrinsic 在表中标记为 IrEmission（emit 一个常量），通过 typecheck / lower / codegen 与 run-pass。
2. 新增 fail fixture：声明 `@Intrinsic("name_not_in_table") fun foo()`，前端必须拒绝。
3. 新增 fail fixture：声明 `@Intrinsic("array_get") fun foo() { /* body */ }`（body 不为空），前端必须拒绝。
4. 用户场景 fixture：`for i in 0..n { sum += arr.get(i) }` 在 `--opt-level=2` 下编译 IR，验证 LLVM 已能 LICM `arr.size()` 调用、能将 bounds check 与 BCE 协同消除（fixture 锁 IR 形态或最终 run-pass 性能行为）。
5. 现有所有 array 相关 pass fixture 不退化。
6. `runtime/c/scoop_array.c` / `runtime_symbols.rs` / `runtime_abi.rs` 中已删除的 helper 在仓库 grep 中确认无残留 declaration / call site。
7. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定测试不退化（涉及 array 的需更新为 IR-direct 形态）。
8. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：所有现有 pass fixture 不退化。
9. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下 array fixture 不退化（write barrier 与 move-GC composite copy 的 RuntimeCall 通路必须仍正确）。

完成条件：

- generic-by-T 分流后门有唯一可枚举载体（intrinsic 表），不再散落成 by-name 特判；
- `Array` / `MutableArray` 基础访问已 IR-direct，LLVM 优化路径完整恢复；
- 一组 array runtime helper 已删除，substrate 收缩动作完成；
- "零编译器后门"约束的精确措辞已锁进 PLAN/TODO，未来加新 generic 内建 method 走 intrinsic 表新增条目而非新增 ad-hoc by-name 分支。

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
- 内建类型已是一等的 struct/class implementer：interface 实现以 sysroot `@Intrinsic struct/class` body 内的 method 形式存在，vtable / itable 与 interface call ABI 完全 metadata-driven，未来加新内建 non-generic method 不再要求改编译器。
- generic-by-T 分流后门收敛为唯一可枚举的 intrinsic 表（method-level `@Intrinsic("name")`），表 entry 默认 IR-emission，仅在 GC / write barrier / descriptor-driven copy 等必要协作时下放为 RuntimeCall；新增 generic 内建 method 走表新增条目而非散落 by-name 分支。
- `Array` / `MutableArray` 基础访问已 IR-direct，LLVM 优化路径（LICM / CSE / BCE / 自动向量化）完整可用；对应 array runtime helper 已从 substrate 删除。
- runtime 长期方向：只承载 GC 实现与 GC-adjacent 功能（GC handle / pin、thread registration / cleanup 等），便于后续移植到 WASM / JVM 等环境或切换 GC 实现（如对接 BoehmGC）。本轮 v1 在 Array / MutableArray 上完成首批迁出，其它非 GC 内容（`exit(3)` / `print/println` / scalar helper 等）作为 v2+ backlog 逐步以 IR / Scoop ABI FFI 形式迁出，不在 v1 主线 scope 内。
