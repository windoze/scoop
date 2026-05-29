# TODO-3：P3-P4 `@InteriorMutable` + `__AtomicInt` 与 immortal 运行期

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P3-P4
> 包目标：把 interior mutability 表达成抗 aliasing 的类型特征（`@InteriorMutable` + `__AtomicInt` struct），并让运行期能透明承载 immortal ref 对象、byte 数组按内容去重。

## P3：`__AtomicInt` 升为 `@InteriorMutable struct`

### [TODO] P3-T01：新增 `@InteriorMutable` 注解

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
  - （待执行）

### [TODO] P3-T01R：Review `@InteriorMutable` 注解

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
  - （待执行）

### [TODO] P3-T02：`__AtomicInt` 升为 `@InteriorMutable struct`

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
  - （待执行）

### [TODO] P3-T02R：Review `__AtomicInt` struct 化

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
  - （待执行）

## P4：Immortal 运行期支持与 content-hash 键

### [TODO] P4-T01：运行期 `SCOOP_GC_FLAG_IMMORTAL` 与 marker 短路

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Runtime change — recognize immortal headers”
- 目标：
  - 让运行期把带 immortal header 的对象视为透明（永不写、永不 trace）。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc.h:210-244`
  - `runtime/c/scoop_gc_backend_immix.c:2719-2737`（`scoop_gc_mark_object_if_needed`），参考 `:2739-2760`、`:5177/5185`
- 必须实现的内容：
  1. `scoop_gc.h` 新增 `#define SCOOP_GC_FLAG_IMMORTAL 0x80000000u` 与 `#define SCOOP_GC_MARK_IMMORTAL 0xFFFFFFFFu`。
  2. `scoop_gc_mark_object_if_needed` 开头加 `if ((obj->flags & SCOOP_GC_FLAG_IMMORTAL) != 0) return;`，覆盖 pinned 扫描等不经 membership 预检的入口。
  3. slot visitor 不改（membership 已过滤堆外指针）；sweep 不改（immortal `next=null` 永不上链）。
- 必须遵从的约束：
  - 短路必须 flag-gated，不得 blanket 跳过普通堆对象。
- 验证：
  1. runtime C 单元测试：栈上构造带 `SCOOP_GC_FLAG_IMMORTAL` 的 header，推上 mark stack，断言 `mark`/`flags` 字节不变（ASan）；同测堆 header 断言 `mark` 被更新。
  2. `cargo test --all --all-targets`
- 完成条件：
  - immortal flag 短路正确且 flag-gated。
- 依赖：P2-T04R
- 完成记录：
  - （待执行）

### [TODO] P4-T01R：Review immortal 运行期短路

- 参考：
  - P4-T01 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Runtime change”
- 目标：
  - 复核短路是否 flag-gated、是否覆盖 pinned 路径、是否对普通对象无影响。
- 必须检查的文件/位置：
  - P4-T01 对 `scoop_gc.h` 与 marker 的改动
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
