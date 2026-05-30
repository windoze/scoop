# TODO-3：P3-P4 `@InteriorMutable` + `__AtomicInt` 与 immortal 运行期

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P3-P4
> 包目标：把 interior mutability 表达成抗 aliasing 的类型特征（`@InteriorMutable` + `__AtomicInt` struct），并让运行期能透明承载 immortal ref 对象、byte 数组按内容去重。

## P3：`__AtomicInt` 升为 `@InteriorMutable struct`

### [DONE] P3-T01：新增 `@InteriorMutable` 注解

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Interior mutability — the `@InteriorMutable` marker”
- 目标：
  - 引入一个 metadata-only 注解，标记“经 unsafe 原地改、`val`/`var` 字段无法体现”的内部可变类型，供 P5 谓词读取。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/{annotations.rs,builtin_annotations.rs}`
  - 注解在 nominal 元数据上的承载点（供 MIR/codegen 阶段可查）
- 必须实现的内容：
  1. 新增 `@InteriorMutable` builtin annotation，typecheck 识别并校验合法 target（至少允许 struct/class 声明）。
  2. 把该标记挂在 nominal 元数据上，确保它**不随 typealias / 类型擦除丢失**，且 MIR/codegen 阶段可查询。
  3. 注解 metadata-only：不生成任何 codegen / runtime 表项。
- 必须遵从的约束：
  - 标记必须 key 在“可变”这个特征上，不得用名字匹配 `__AtomicInt` 代替。
  - 不得复用 `@Intrinsic` 表达可变性（多数 intrinsic 类型不可变）。
- 验证：
  1. `cargo test --all --all-targets`
  2. 新增 typecheck 单元/fixture：带 `@InteriorMutable` 的类型其标记可被查询；非法 target 报错。
- 完成条件：
  - 谓词可凭 nominal 上的标记判定，无需名字匹配。
- 依赖：P0-T02R
- 完成记录：
  - 2026-05-30：已完成。
  - 实现：新增 compiler-recognized `@InteriorMutable` builtin annotation，允许标注 `struct` / `class` 类型声明，拒绝 typealias、函数等非法 target，并保持 metadata-only（不生成 codegen/runtime 表项）。
  - 元数据：`TypeSymbol` 持有 `is_interior_mutable`，提供 `TypeEnv::nominal_is_interior_mutable` 查询；HIR `NominalDecl` 与 MIR `NominalMetadata` 继续承载该标记，供后续 MIR/codegen 阶段按 nominal 查询。
  - Cone：ScoopIR public type declaration 导出/导入 `is_interior_mutable`，cached cone 注入后标记不会丢失。
  - 测试：新增 TypeEnv 单元测试覆盖标记查询与 alias lowering 回到标记 nominal；新增 typecheck fixtures 覆盖 struct/class 合法使用与 typealias 非法 target。
  - 验证：`cargo fmt`；`cargo test -p scoopc_hir type_env_tracks_interior_mutable_nominal_marker_through_alias_lowering --all-targets`；`cargo test -p scoopc_cone artifact_frontend_import_injects_public_and_visibility_payload --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/interior_mutable_struct_and_class_ok.scoop --exit-on-failure`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/interior_mutable_typealias_is_error.scoop --exit-on-failure`；`cargo build -p scoopc`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P3-T01R：Review `@InteriorMutable` 注解

- 参考：
  - P3-T01 完成记录
- 目标：
  - 复核标记是否抗 aliasing、是否 metadata-only、是否可被后续阶段查询。
- 必须检查的文件/位置：
  - P3-T01 的注解定义与承载点
- 必须实现的内容：
  1. 确认 alias 到带标记类型时标记不丢。
  2. 确认无 codegen/runtime 副作用。
  3. 确认 MIR/codegen 可查询该标记。
- 必须遵从的约束：
  - 若标记可被 aliasing 抹掉或有副作用，必须修正后才进入 P3-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 标记机制可靠。
- 依赖：P3-T01
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：`@InteriorMutable` 仍是 compiler-recognized metadata-only 注解；合法 target 限定为 struct/class，非法 typealias target 继续报错，无 codegen/runtime 表项或运行期副作用。
  - 抗 aliasing：TypeEnv 查询保持 key 在解析后的 nominal 上；alias lowering 回到被标记 nominal，且 alias 自身不被误记录为 interior-mutable nominal。
  - MIR/codegen 查询：复核并补齐 `LoweredHir::interior_mutable_nominals` 与 `LlvmStageBaseContext::nominal_is_interior_mutable`，从 TypeEnv（含 cached cone import）或 AST fallback 收集标记 nominal，确保后续 LLVM codegen 可按 nominal FQN 查询。
  - 测试：新增/扩展 MIR stage 与 LLVM codegen stage 单元测试，覆盖 MIR `NominalMetadata.is_interior_mutable` 与 LLVM base context 查询面，确认 plain nominal 与 typealias 不被误标。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P3-T02：`__AtomicInt` 升为 `@InteriorMutable struct`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “`scoop.unsafe.__AtomicInt`: typealias → marked struct”
- 目标：
  - 把 `__AtomicInt` 从 `typealias = Int` 升为与 Int 同布局、类型相异、带 `@InteriorMutable` 的 struct，原子访问纪律落进类型。
- 必须修改的文件/位置：
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop:163`
  - `sysroot/lib/scoop.core/src/core.scoop`（`AtomicInt`/`AtomicBool` 构造）
  - 擦除点：`crates/scoopc_hir/src/typecheck/lower.rs:2662,3522`、`scoopc_codegen_llvm/.../mir_body/types.rs:436`、`scoopc_hir/src/hir/lower/util/generic_layouts.rs:89`、`.../hir/lower/main/impl_lowering.rs:1724`
- 必须实现的内容：
  1. `unsafe.scoop`：
     ```scoop
     @InteriorMutable
     public struct __AtomicInt { val raw: Int }
     ```
     依赖普通单字段 struct 的派生 word 布局与派生构造器 `__AtomicInt(initial)`，不写专用 codegen。
  2. 5 处擦除点从“类型 = `Int`”改为“类型 = `__AtomicInt` nominal、布局/ABI = `Int` word”；`__atomicIntLoad/Store/CompareExchange` 签名不变（按 lvalue 取存储当 i64 操作）。
  3. `core.scoop` atomics 构造改显式：`var raw: __AtomicInt = __AtomicInt(initial)`、`= __AtomicInt(__atomicBoolToInt(initial))`；无隐式 Int↔__AtomicInt coerce。
- 必须遵从的约束：
  - `__AtomicInt` 必须与 `Int` 类型相异、布局相同；aliases 解析回它时标记不丢。
  - struct 只能 `val` 字段（语言刻意不放开 `var` struct 字段）；`@InteriorMutable` 因此 load-bearing。
  - 不引入隐式 coerce；构造/load/store 三个面都显式。
- 验证：
  1. `cargo test --all --all-targets`
  2. 现有 atomics fixtures / 单元测试（`AtomicInt`/`AtomicBool`/`Atomic`）回归通过。
  3. 类型层不再把 `__AtomicInt` 等同 `Int`（加断言/测试）。
- 完成条件：
  - `__AtomicInt` 是带标记的相异 struct，P5 谓词可安全否决它，且不会被误当 Int 常量化。
- 依赖：P3-T01R
- 完成记录：
  - 2026-05-30：已完成。
  - Sysroot：`scoop.unsafe.__AtomicInt` 已从 `typealias Int` 改为 `@InteriorMutable public struct __AtomicInt { val raw: Int }`；`AtomicInt` / `AtomicBool` 与所有原子 fixtures 改为显式 `__AtomicInt(...)` 构造，不再依赖隐式 `Int` 初始化。
  - 类型层：移除 typecheck、typed HIR 与 layout collection 中把 `__AtomicInt` 直接擦成 `Int` 的路径；新增 TypeEnv 单元测试与 typecheck 负例，确认 `__AtomicInt` 是 marked nominal struct、alias lowering 不丢标记且不等同于 `Int`。
  - Codegen：保留 LLVM/MIR 后端将 `__AtomicInt` nominal 存储按 word `Int` ABI 处理；补充通用单字段 scalar-layout struct literal lowering，让 `__AtomicInt(...)` 构造产生 word 值而不引入专用构造器 codegen。
  - Fixtures/golden：更新 raw atomic、top-level atomic storage、field-lvalue、runtime GC cross-thread roots、B-26 atomic gate fixtures，以及 HIR / effect-lowered golden 中因 sysroot 新增 nominal/field/vtable 引起的稳定计数变化。
  - 验证：`cargo fmt`；`cargo build -p scoopc -p scoop`；`cargo test -p scoopc_hir sysroot_atomic_int_lowers_to_marked_nominal_not_int --all-targets`；相关 atomic/build/effect_lowered targeted fixtures；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc --lib`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P3-T02R：Review `__AtomicInt` struct 化

- 参考：
  - P3-T02 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Interior mutability”
- 目标：
  - 复核类型相异性、布局一致性、擦除点完整性与显式构造。
- 必须检查的文件/位置：
  - P3-T02 对 `unsafe.scoop`、`core.scoop`、5 处擦除点的改动
- 必须实现的内容：
  1. 反向 grep 确认无遗漏的 `__AtomicInt`→Int 等同点。
  2. 确认布局/ABI 仍是 Int word，原子 intrinsic 仍正确操作其存储。
  3. 确认无隐式 coerce，构造点已显式。
- 必须遵从的约束：
  - 若仍有擦除点把它当 Int、或布局/ABI 偏移，必须修正后才进入 P4-T01。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - `__AtomicInt` 类型化收口。
- 依赖：P3-T02
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：`__AtomicInt` 已类型化收口。反向 grep 确认原 P0/P3 记录的 5 个类型层/HIR/MIR erasure 点不再把 `scoop.unsafe.__AtomicInt` 等同为 `Int`；sysroot 声明保持 `@InteriorMutable public struct __AtomicInt { val raw: Int }`，TypeEnv 单元测试确认 alias lowering 回到 marked nominal 且 `atomic_ty != builtins.int`。
  - 布局/ABI：剩余 Rust 侧 `__AtomicInt` 命中集中在 LLVM layout/ABI 映射与测试；`ty.rs` / `effect_lowered/layout/abi.rs` 只保留 word-sized signed integer 的 codegen representation，原子 intrinsic lowering 继续要求 addressable lvalue slot 并校验 `Int` word 宽度，不改变 source-level 类型身份。
  - 显式构造：`AtomicInt` / `AtomicBool` 与 atomic fixtures 均使用 `__AtomicInt(...)` 构造；`atomic_int_not_int_initializer_is_error.scoop` 覆盖隐式 `Int -> __AtomicInt` 初始化拒绝路径。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_hir sysroot_atomic_int_lowers_to_marked_nominal_not_int --all-targets`；targeted fixtures：`tests/fixtures/typecheck/atomic_int_not_int_initializer_is_error.scoop`、`tests/fixtures/run-pass/unsafe_atomic_int_basic.scoop`、`tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.scoop`、`tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`、`tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

## P4：Immortal 运行期支持与 content-hash 键

### [DONE] P4-T01：运行期 `SCOOP_GC_FLAG_IMMORTAL` 与 marker 短路

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Runtime change — recognize immortal headers”
- 目标：
  - 让运行期把带 immortal header 的对象视为透明（永不写、永不 trace）。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc.h:210-244`
  - `runtime/c/scoop_gc_backend_immix.c:2719-2737`（serial `scoop_gc_mark_object_if_needed`）、`:2950-2975`（parallel marker），参考 `:2739-2760`、`:5043-5060`、`:5169-5186`
  - `runtime/c/scoop_gc.c:1313-1323`、`runtime/c/scoop_gc_backend_minimal.c:503-513`、`runtime/c/scoop_gc_backend_hosted.c:513-523`（其它 backend marker helper）
- 必须实现的内容：
  1. `scoop_gc.h` 新增 `#define SCOOP_GC_FLAG_IMMORTAL 0x80000000u` 与 `#define SCOOP_GC_MARK_IMMORTAL 0xFFFFFFFFu`。
  2. 所有 backend marker helper 开头加 `if ((obj->flags & SCOOP_GC_FLAG_IMMORTAL) != 0) return;`；Immix parallel marker 必须在 atomic load/CAS 前同样短路。
  3. 短路必须覆盖 pinned/handle 扫描等不经 membership 预检的入口；Immix slot visitor 不改（membership 已过滤堆外指针）；sweep 不改（immortal `next=null` 永不上链）。
- 必须遵从的约束：
  - 短路必须 flag-gated，不得 blanket 跳过普通堆对象。
- 验证：
  1. runtime C 单元测试：栈上构造带 `SCOOP_GC_FLAG_IMMORTAL` 的 header，推上 mark stack，断言 `mark`/`flags` 字节不变（ASan）；同测堆 header 断言 `mark` 被更新；覆盖 Immix serial/parallel 与 baseline/minimal/hosted marker helper 可达路径。
  2. `cargo test --all --all-targets`
- 完成条件：
  - immortal flag 短路正确且 flag-gated。
- 依赖：P2-T04R
- 完成记录：
  - 2026-05-30：已完成。
  - Runtime header：`scoop_gc.h` 新增 `SCOOP_GC_FLAG_IMMORTAL` 与 `SCOOP_GC_MARK_IMMORTAL`，作为后续 codegen 发射 immortal ref 对象 header 的运行期 contract。
  - Marker 短路：baseline、minimal、hosted marker helper 在写 `mark` / push mark stack 前先按 flag 返回；Immix serial marker 与 parallel marker 同样 flag-gated，其中 parallel 路径在 `obj->mark` atomic load/CAS 前返回；Immix minor marker 也加同类短路，避免 minor 直接入口写 immortal header。
  - 测试：新增 `scoop_test_gc_immortal_marker_smoke` test-only C helper 和 `crates/scoop_runtime/tests/gc_immortal_marker.rs`，覆盖栈上 immortal header 不改 `flags`/`mark`、普通对象仍被标记；默认 Immix helper 覆盖 serial、parallel 与 minor marker，非默认 backend helper 覆盖 baseline/minimal/hosted 可达路径。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo test -p scoop_runtime --no-default-features --features gc-baseline --test gc_immortal_marker`；`cargo test -p scoop_runtime --no-default-features --features gc-minimal --test gc_immortal_marker`；`cargo test -p scoop_runtime --no-default-features --features gc-hosted --test gc_immortal_marker`；`python3 tools/run_fixtures.py`。

### [TODO] P4-T01R：Review immortal 运行期短路

- 参考：
  - P4-T01 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Runtime change”
- 目标：
  - 复核短路是否 flag-gated、是否覆盖 pinned 路径、是否对普通对象无影响。
- 必须检查的文件/位置：
  - P4-T01 对 `scoop_gc.h` 与所有 backend marker helper（含 Immix parallel marker）的改动
- 必须实现的内容：
  1. 确认普通堆对象 marker 行为不变。
  2. 确认 pinned 扫描等入口被覆盖。
  3. ASan 下确认 immortal header 不被写。
- 必须遵从的约束：
  - 若短路非 flag-gated 或漏 pinned 路径，必须修正后才进入 P4-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 运行期可安全承载 immortal 对象。
- 依赖：P4-T01
- 完成记录：
  - （待执行）

### [TODO] P4-T02：byte 数组 content-hash 键与 `unnamed_addr`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Cache and dedup keys”、Phasing 3
- 目标：
  - 把字符串 byte 数组从 span-key 改成 content-hash 键并加 `unnamed_addr`，为 dedup 与跨 TU 折叠铺路（无行为变化）。
- 必须修改的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:56-72`（`get_or_create_global_bytes`）
- 必须实现的内容：
  1. 键从 `__scoop_str_data_{span.start}_{span.end}` 改为 `base16(SHA-256(bytes)[..16])`。
  2. 对 byte 数组全局 `set_unnamed_addr(true)`。
  3. 相同字面量在多处只产生一个 `__scoop_str_data_<hash>`。
- 必须遵从的约束：
  - 不得改变字符串语义；只影响全局名与去重。
- 验证：
  1. golden-file：相同字面量多处只产生一个 byte 数组。
  2. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - byte 数组按内容去重、可被 linker 折叠。
- 依赖：P4-T01R
- 完成记录：
  - （待执行）

### [TODO] P4-T02R：Review content-hash 键

- 参考：
  - P4-T02 完成记录
- 目标：
  - 复核 content-hash 键无碰撞风险、无语义变化、golden 已更新。
- 必须检查的文件/位置：
  - P4-T02 对 `alloca.rs` 与受影响 golden 的改动
- 必须实现的内容：
  1. 确认 hash 截断长度足够避免实际碰撞，或有碰撞兜底。
  2. 确认字符串语义不变、golden 已同步。
- 必须遵从的约束：
  - 若键有碰撞风险或语义偏移，必须修正后才进入 TODO-4。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - content-hash 键稳定，immortal codegen 前置就绪。
- 依赖：P4-T02
- 完成记录：
  - （待执行）
