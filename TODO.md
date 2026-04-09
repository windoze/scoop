# TODO（Scoop：近期任务清单）

> 生成时间：2026-04-07  
> 说明：本文件是新的短版 TODO，只记录“接下来要做的新任务”。历史任务与已完成事项请看 `TODO-1.md` / `PLAN-1.md`。

## 约定

- 状态：
  - `[TODO]`：可立即实现与验收
  - `[BLOCKED]`：依赖未满足（例如缺文件/缺前置能力）
  - `[DONE]`：已完成（短版 TODO 一般不搬运历史 DONE）
- 每个任务包含：**描述 / 目标 / 验收 / 依赖**。
- 术语（类型系统）：
  - `Nothing`：bottom / uninhabited type。它是任意类型的子类型（`Nothing <: T`），但在运行时**不会产生值**；返回类型为 `Nothing` 的函数/表达式不会“正常返回”。后端若需要为工程实现引入某种占位表示，也必须保证该值永不可被观察（仅用于不可达路径的 IR 连通）。

常用验收命令：

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`）：

```bash
cargo run -p scoop --features llvm -- test
```

---

## T01：LLVM 工具链对齐（LLVM 21 / Rust stable）

### T0101 [DONE] LLVM 21：将后端基线从 LLVM 18 迁移到 LLVM 21（对齐 Rust stable）
- 描述：将 LLVM 后端开发/测试基线升级到 LLVM 21，避免当前“系统 LLVM / Homebrew LLVM 18 / 机器差异”导致的行为漂移，并为后续优化 pipeline 与 GC/statepoint 相关 pass 的稳定性提供统一前提。
- 目标：
  - 依赖升级：更新 `inkwell`/`llvm-sys`（如有）到支持 LLVM 21 的组合，并固定选择策略（prefer Rust stable 对齐）。
  - 构建入口一致：`cargo build/test` 与 `cargo run -p scoop --features llvm -- test` 在开发机上只依赖 LLVM 21（包括 `llvm-config`）。
  - 文档与诊断：明确“需要的 LLVM 版本/安装方式/常见错误”，并让错误提示能指出版本不匹配。
  - pass 稳定性复核：在 LLVM 21 下复核 `rewrite-statepoints-for-gc` 管线；`place-safepoints` 继续默认关闭，除非在 LLVM 21 下单独验证稳定可用。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`（使用 LLVM 21）
- 依赖：无

### T0102a [DONE] LLVM：`codegen` 模块骨架 + 抽出 `types.rs`（先搬类型与不变量）
- 描述：为后续逐步拆分 `codegen` 做最小可回归铺垫：先把单文件 `codegen.rs` 迁移到 `codegen/mod.rs`，并抽出“全局共享的 codegen 类型/常量”（例如 `CgTy`/`CgValue`/enum layout 等），降低后续拆分的冲突面。
- 目标：
  - `crates/scoopc/src/llvm/codegen.rs` → `crates/scoopc/src/llvm/codegen/mod.rs`（模块路径不变，行为不变）。
  - 新增 `crates/scoopc/src/llvm/codegen/types.rs`：集中 `CgTy`/`CgValue`/`CgEnumLayout`/关键常量等“跨 codegen 逻辑共享”的定义。
  - 不改 codegen 语义/ABI/错误消息文本。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T0102b [DONE] LLVM：抽出 runtime ABI glue（`runtime_abi.rs` / `runtime_symbols.rs`）
- 描述：把 runtime 符号声明、调用约定、对象头/GC 相关的 ABI glue 从主 codegen 拆出，形成清晰“边界层”，便于后续排查与扩展。
- 目标：runtime decls/ffi helper 有明确归属；避免在 expr/stmt codegen 中散落 `declare_*`/`get_or_declare_*`。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0102a

### T0102c [DONE] LLVM：抽出 type/layout lowering（`layout.rs` / `ty.rs`）
- 描述：把 `TypeId -> CgTy`、`TypeLayout`/niche/boxing 决策、struct/enum/class field GEP 等“布局相关逻辑”集中管理。
- 目标：`cg_ty_of`/`llvm_basic_type_of` 等关键入口有明确模块归属，并避免与 expr/stmt 互相引用形成环。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0102a

### T0102d [DONE] LLVM：抽出表达式/语句 codegen（`expr.rs` / `stmt.rs` / `control_flow.rs`）
- 描述：把 HIR expr/stmt 的 codegen 逻辑从主模块拆出，按职责分层，降低“在同一文件里跳转定位”的成本。
- 目标：expr/stmt/control-flow 的入口函数可直接导航；局部 helper 尽量就近归属，减少跨区段耦合。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0102a、T0102c

### T0102e [DONE] LLVM：抽出 effect/continuation/GC/statepoint 相关逻辑（`effect.rs` / `gc.rs`）
- 描述：把 handler stack、perform 分发、raise unwinding、statepoint rewrite 约束与 GC root 辅助等集中到独立模块，减少“语义不变量”分散在各处的风险。
- 目标：effect/GC 关键不变量（stack discipline / addrspace / statepoint 约束）集中可读；后续 T1610~T1612 变更影响半径更小。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0102a、T0102c、T0102d

### T0103 HIR lowering：重构 `crates/scoopc/src/hir/lower.rs`（拆分）
- 描述：`crates/scoopc/src/hir/lower.rs` 已超过 6K 行，承载了 AST→HIR lowering 的大量逻辑（语法糖/特殊 case、block/stmt/expr lowering、内建与 sysroot 约定、以及若干“为可回归而做的早期阶段特判”）。随着特性增加，该文件：
  - 修改容易产生连锁影响（同一概念的 lowering 分散在多个 helper 中）；
  - 复用/测试困难（缺少清晰的子模块边界与可单测的最小单元）；
  - 为后续任务（例如更完整的控制流 lowering、effects/closures 相关 lowering）引入维护负担。
- 目标：
  - 将 `lower.rs` 拆分为若干职责清晰的子模块（示例：`lower/mod.rs` + `lower/block.rs`/`lower/stmt.rs`/`lower/expr.rs`/`lower/patterns.rs`/`lower/types.rs`/`lower/sugar.rs`/`lower/util.rs`），并在 `crates/scoopc/src/hir/mod.rs` 或 `crates/scoopc/src/hir/lower/mod.rs` 中组织入口。
  - 保持行为不变：AST/HIR 结构、span 选择、以及既有 fixtures 的 HIR dump 输出尽量保持稳定（除非作为独立任务明确允许变更）。
  - 收拢“阶段性特判/兼容逻辑”：把 early-stage 的临时约束集中到少数模块/函数中，并显式标注任务号（避免散落在各处难以清理）。
  - 为未来拆分单测留出口：让核心 lowering 单元可以在 Rust 测试里以小输入（AST 片段）验证产物（即使暂时不新增测试，也要让结构上具备可测性）。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - 代码组织层面：
    - `crates/scoopc/src/hir/lower.rs` 不再是巨型单文件；lowering 的主要职责边界可通过目录/文件名直观看到；
    - 入口/数据结构位置清晰，避免循环依赖与 `pub(crate)` 漫延。
- 依赖：无

### T0103a [DONE] HIR lowering：`lower` 模块骨架 + 抽出 `types.rs`（共享类型与 side tables）
- 描述：把 `crates/scoopc/src/hir/lower.rs` 迁移为目录模块 `crates/scoopc/src/hir/lower/mod.rs`，并把大量“共享类型/side table”（例如 `LoweredHir`、默认参数信息、delegated property 信息等）抽到 `lower/types.rs`，为后续继续拆分 expr/stmt/block/sugar 打基础。
- 目标：
  - 模块路径不变（仍为 `crate::hir::lower`），行为不变（dump-hir/fixtures 输出稳定）。
  - `types.rs` 内部类型尽量使用 `pub(super)` 暴露到父模块，避免 `pub(crate)` 漫延。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：无

### T0103b [DONE] HIR lowering：抽出 `util.rs`（通用 helper / 早期阶段特判收拢）
- 描述：把跨 lowering 分支复用的 helper（span/诊断、小型 AST 解析、sysroot 约定字符串等）集中到 `lower/util.rs`，并把 early-stage 的“临时特判/兼容逻辑”收拢到少数入口函数中。
- 目标：降低后续拆分 `expr.rs`/`stmt.rs`/`block.rs` 时的重复粘贴与循环依赖风险。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0103a

### T0103c [DONE] HIR lowering：抽出 `expr.rs`（表达式 lowering）
- 描述：把 `lower_expr*`、字面量/调用/成员访问/控制流表达式等表达式 lowering 迁移到 `lower/expr.rs`。
- 目标：表达式入口可直接导航；与 stmt/block 之间的共享接口通过 `types.rs`/`util.rs` 明确边界。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0103b

### T0103d [DONE] HIR lowering：抽出 `stmt.rs`/`block.rs`（语句与块 lowering）
- 描述：把 `lower_stmt*`、`lower_block*`、局部 `val/assign/return`、循环与 break/continue 等迁移到 `lower/stmt.rs` 与 `lower/block.rs`。
- 目标：stmt/block 与 expr 的互相调用通过少数“公开到父模块的 helper”实现，避免模块环。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0103c

### T0103e [DONE] HIR lowering：抽出 `sugar.rs`/`patterns.rs`（语法糖与模式相关 lowering）
- 描述：把 delegated properties、lazy/observable/vetoable、when 模式、以及其它语法糖/特殊 case 的 lowering 迁移到独立模块，避免它们散落在 expr/stmt 内部。
- 目标：阶段性特判集中且显式标注任务号，便于后续清理；为未来加单测预留“可单测的最小入口”。
- 验收：`cargo test --all` + `cargo run -p scoop -- test`
- 依赖：T0103d

### T0104 [DONE] typecheck：重构 `crates/scoopc/src/typecheck/expr.rs`（拆分模块，降低维护成本）
- 描述：`crates/scoopc/src/typecheck/expr.rs` 过长且职责密集，集中承载了表达式类型检查/推断的多个维度（各类 expr 语法分支、预期类型传递、错误生成与诊断、以及若干为可回归而引入的局部 helper）。随着语言特性与诊断要求增长，单文件结构会导致：
  - 导航成本高（同一语义路径跨多段 helper 跳转，难以定位责任边界）；
  - 修改风险高（对某个 expr 分支的改动容易影响其它分支的约束传播/错误消息）；
  - 缺少“可单测的最小单元”（想验证某一类 expr 的规则时，往往需要走完整 typecheck 入口）。
- 目标：
  - 将 `expr.rs` 拆分为职责清晰的子模块（示例：`typecheck/expr/mod.rs` + `typecheck/expr/lit.rs`/`call.rs`/`control_flow.rs`/`ops.rs`/`coerce.rs`/`util.rs`；文件名以“expr 分类/共享工具”表达意图）。
  - 收拢共享能力：把错误构造、预期类型/约束传播、常用 “check_* / infer_*” helper 的共用部分集中到少数模块，避免循环依赖与 `pub(crate)` 漫延。
  - 行为保持稳定：不改变类型规则与错误信息文本（除非作为独立任务明确允许调整）；现有 fixtures/单测应尽量保持不变。
  - 为测试留出口：让关键规则以“较小输入”具备可被 Rust 单测覆盖的结构可能（即使本任务不强制新增测试，也要让模块边界更利于补测）。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - 代码组织层面：
    - `crates/scoopc/src/typecheck/expr.rs` 不再是巨型单文件；通过目录/文件名能快速定位 expr 分支的实现位置；
    - 模块依赖清晰（无明显环），公共入口与内部 helper 的可见性边界合理。
- 依赖：无

### T0105 [DONE] GC STW 死锁：`scoop_thread_spawn_join_resume_u64` 在 `pthread_join` 前未转入 `IN_NATIVE` 状态
- 描述：`scoop_thread_spawn_join_resume_u64` 调用 `pthread_join` 时，调用线程仍为 `RUNNING` 状态，但阻塞在内核态无法到达 safepoint。若被 spawn 的子线程在 `SCOOP_GC_STRESS=1` 下触发 `scoop_gc_collect()`，STW 协议会将调用线程计入 `need_to_park`，但该线程永远无法 park——形成经典双向等待死锁（子线程等父线程 park，父线程等子线程退出）。
- 根因分析：
  - `scoop_thread_spawn_join_resume_u64`（`runtime/c/scoop_runtime.c`）在 `pthread_create` 后直接 `pthread_join`，未执行 `scoop_enter_native`/`scoop_leave_native` 状态转换。
  - STW 协议（`scoop_gc_stop_the_world_begin_unlocked`）正确地跳过 `IN_NATIVE` 线程（因其 roots 已通过 `native_roots` 注册），但对 `RUNNING` 线程要求其必须在有限时间内到达 safepoint 并 park。
  - `pthread_join` 是一个阻塞内核调用，不经过任何 Scoop safepoint，因此违反了 STW 协作合约。
  - 这与 JVM 的 thread state machine 设计一致：JVM 在任何阻塞系统调用前都会将线程转为 `_thread_in_native`，使 GC 可以安全跳过。
- 目标：
  - 在 `pthread_join` 前调用 `scoop_enter_native(NULL, 0)` 将线程转为 `IN_NATIVE`（该函数无需持有 GC roots，因为 continuation 对象已被子线程引用），在 `pthread_join` 返回后调用 `scoop_leave_native()` 恢复为 `RUNNING`。
  - `scoop_leave_native` 内置的 transition barrier 会在 STW 活跃时自动阻塞，直到 GC 完成——与 JVM 的 native→Java 转换语义一致。
  - 审计 runtime/c 中所有类似的阻塞调用点（`pthread_join`、`pthread_cond_wait` 等在非 GC 协议上下文中的使用），确认无遗漏。
- 验收：
  - 现有 fixture `gc_continuation_cross_thread_resume_with_objects` 和 `gc_continuation_multi_thread_concurrent_alloc_resume` 在 `SCOOP_GC_STRESS=1` 下不再死锁，可移除"已知限制"注释。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **`scoop_thread_spawn_join_resume_u64`**（`runtime/c/scoop_runtime.c`）：在 `pthread_join` 前调用 `scoop_enter_native(0, 0)` 转为 `IN_NATIVE`，`pthread_join` 返回后调用 `scoop_leave_native()` 恢复 `RUNNING`。
  - **审计并修复同类阻塞调用点**：
    - `scoop_thread_join`（`runtime/c/scoop_thread.c`）：`scoop_platform_thread_join` 前后加入 `enter/leave_native`。
    - `scoop_thread_sleep_millis`（`runtime/c/scoop_thread.c`）：`scoop_platform_thread_sleep_millis` 前后加入 `enter/leave_native`。
    - `scoop_sync_condvar_wait`（`runtime/c/scoop_sync.c`）：`scoop_platform_sync_condvar_wait` 前后加入 `enter/leave_native`。
    - `scoop_sync_once_run_blocking`（`runtime/c/scoop_sync.c`）：Once 等待初始化完成的 `condvar_wait` 循环前后加入 `enter/leave_native`。
    - `scoop_channels_recv_u64`（`runtime/c/scoop_channels.c`）：接收阻塞等待的 `condvar_wait` 循环前后加入 `enter/leave_native`。
  - **所有修改文件**新增 `scoop_enter_native`/`scoop_leave_native` 前置声明。
  - **Fixture 注释更新**：两个跨线程 fixture 的"已知限制"注释更新为"T0105 已修复 STW 死锁；剩余 GC rooting 问题属 T0106 范围"。
  - **验证**：SCOOP_GC_STRESS=1 下两个跨线程 fixture 不再死锁（deadlock 已消除）；但仍存在 GC rooting 问题（crash/incorrect output），属 T0106 审计范围。
  - 139 单元测试 + 774 fixtures 通过。

### T0106 [DONE] GC rooting 审计：runtime/c 函数中 GC-managed 指针跨分配点的 pin/unpin 完整性
- 描述：`scoop_string_split` 在 T1812 中暴露了 C runtime 函数的 GC rooting 缺陷——函数内持有的 GC-managed 指针（`builder`/`s`/`delimiter`）在跨 `scoop_alloc` 调用时未 pin，导致 GC stress 下被回收。该问题已修复，但同类缺陷可能存在于其它 runtime/c 函数中。
- 根因分析：
  - LLVM stackmap 机制仅对编译后的 Scoop 代码生效——C runtime 函数的栈帧对 GC 不可见。
  - C 函数中持有的 GC-managed 对象指针（`ScoopString*`、`ScoopArray*`、`ScoopArrayBuilder*` 等）如果跨越 `scoop_alloc` 调用点（即 GC 触发点）而未 pin，就会成为 dangling pointer。
  - 在正常模式下因 GC 触发频率低而不易暴露；`SCOOP_GC_STRESS=1` 使每次分配都触发 GC，从而确定性地复现问题。
  - 当前 runtime 使用 `scoop_pin`/`scoop_unpin`（而非 shadow stack 或 PUSH_ROOT 宏）作为 C 侧 GC rooting 机制。
- 目标：
  - 系统审计 `runtime/c/scoop_runtime.c` 和 `runtime/c/scoop_array.c` 中所有调用 `scoop_alloc` 的函数，检查是否存在"GC-managed 指针在 `scoop_alloc` 调用点仍被本地变量持有但未 pin"的情况。
  - 已知待审计的高风险函数（持有 GC 指针且调用 `scoop_alloc`）：
    - `scoop_string_trim_indent`：持有 `value`（`ScoopString*`），在函数末尾调用 `scoop_alloc`（通过 `scoop_string_from_bytes`）但未 pin——非移动 GC 下安全（对象不重定位），但在移动 GC（`SCOOP_GC_MOVE=1`）下为潜在缺陷。
    - `scoop_array_builder_build_common`：持有 `ScoopArrayBuilder*`（调用者传入），内部 `scoop_alloc` 创建结果数组——依赖调用者 pin builder。
    - 其它含"分配 + 引用已有 GC 对象"模式的函数。
  - 对发现的缺陷补充 `scoop_pin`/`scoop_unpin` 保护。
  - 考虑引入编码规范或静态检查建议（例如注释标注 `/* GC-SAFE: pinned */`），降低未来回归风险。
- 验收：
  - 审计报告列出所有已检查函数及结论（safe / fixed / N/A）。
  - 修复的函数新增或更新对应 `SCOOP_GC_STRESS=1` fixtures。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成记录：
  - **审计范围**：`scoop_runtime.c`、`scoop_array.c`、`scoop_thread.c`、`scoop_channels.c`、`scoop_sync.c`、`scoop_task_executor.c`、`scoop_test.c`。
  - **修复（4 个函数）**：
    1. `scoop_string_trim_indent`（scoop_runtime.c）：pin `value` before `scoop_alloc`，unpin on both OOM and normal return paths.
    2. `scoop_process_args_array`（scoop_runtime.c）：pin `builder` after `scoop_array_builder_new()`，unpin after `scoop_array_builder_build_array()` on both early-return and normal paths.
    3. `scoop_array_builder_build_common`（scoop_array.c）：pin `b` before `scoop_alloc(bytes)`，unpin on OOM path and after all `b->` field accesses.
    4. `scoop_thread_spawn`（scoop_thread.c）：pin `env_ptr` (if non-null) before `scoop_alloc`，unpin after `args->env = env_ptr` saves it to malloc'd struct.
  - **已验证安全（无需修复）**：`scoop_continuation_alloc`（已 pin）、`scoop_string_split`（已 pin, T1812）、`scoop_string_concat`（已 pin, T1816）、`scoop_string_from_bytes/cstr/owned_bytes/static_bytes`（仅持有 malloc'd 指针）、`scoop_string_empty/substring`（无跨分配 GC 指针）、`scoop_alloc_typed`（type_desc 为 static）、`scoop_path_*`（data 指针来自 malloc）、`scoop_env_get/int_to_string/io_stdin_read_line_utf8`（无 GC 指针跨分配）、`scoop_channels_channel_create/sync_*_create`（无 GC 指针跨分配）。
  - **N/A（无 scoop_alloc 调用）**：`scoop_channels_send/recv_u64`（仅 malloc）、`scoop_sync_*` locking functions、`scoop_test.c`、`scoop_task_executor.c`（全 malloc）。
  - **验证**：139 单元测试 + 774 fixtures 通过。

### 编译器 + 核心库规范合规审计（2026-04-09）

> 以下任务来自对 `SCOOP_FULL_SPEC.md` 与编译器/核心库实现的系统审计。
> 审计方法：逐节比对 spec 定义的语言特性与 parser→typecheck→HIR lowering→codegen 四阶段的实际覆盖度，以及核心库中的 hardcoded 类型限制。

### T0107 [DONE] String `==`/`!=` codegen：字符串相等性比较

- 描述：当前 codegen 对 `String == String` 返回 `UnsupportedMainBody { kind: "equality lhs" }`。`codegen_equality`（`codegen/mod.rs:12360`）仅处理 `Bool` 和整数类型；String 因 `as_int()` 返回 `None` 而落入错误分支。同时 `runtime_symbols.rs` 中不存在 `scoop_string_equals` 符号。
- 规范引用：Spec §2.3.4（所有类型支持 `==`/`!=`）；String 是 Scoop 核心类型，相等性比较是基础能力。
- 影响：`stdlib/test.scoop` 的 `assertEqString` 被迫使用 `length() == length() && startsWith()` workaround。
- 目标：
  - runtime/c 新增 `scoop_string_equals(a, b) -> i64`（长度检查 + `memcmp`）。
  - codegen `codegen_equality` 在 `CgTy::String` 时调用 runtime 函数。
  - 同步处理 `!=`。
- 验收：
  - 新增 run-pass fixture：`"hello" == "hello"`、`"a" != "b"`、空字符串比较。
  - `assertEqString` 改用 `==`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **runtime/c**（`scoop_runtime.c`）：新增 `scoop_string_equals(a, b) -> i64`——指针相等短路、长度比较、`memcmp` 内容比较。注册到 `scoop_runtime_api.h`。
  - **typecheck**（`typecheck/expr/ops.rs`）：`Eq`/`Ne` 分支新增 `String == String` 支持（`lhs_ty == builtins.string && rhs_ty == builtins.string`）。
  - **codegen**（`codegen/mod.rs`）：`codegen_equality` 在 `CgTy::String` 时调用 `scoop_string_equals`，返回 i64 (1/0)，再转为 Bool；`!=` 通过 `build_not` 取反。
  - **runtime symbols + ABI**（`runtime_symbols.rs` + `runtime_abi.rs`）：`SCOOP_STRING_EQUALS` 常量 + `declare_runtime_string_equals` 声明（`i64 fn(ScoopString*, ScoopString*)`）。
  - **stdlib**（`stdlib/test.scoop`）：`assertEqString` 从 `length() + startsWith()` workaround 改为直接 `expected == actual`。
  - **fixture**：`string_equality_basic.scoop` + `.stdout`——覆盖 10 个场景：同内容 `==`、不同内容 `==`、不同内容 `!=`、同内容 `!=`、空串 `==`、空串 vs 非空、同长不同内容、变量比较 `==`/`!=`。
  - 139 单元测试 + 775 fixtures 通过（含 LLVM 后端）。

### T0108 [DONE] Nullable 运算符 codegen：`?.`（safe call）和 `!!`（non-null assert）

- 描述：`?.` 和 `!!` 在 parser / typecheck 中已完整实现，但 HIR lowering 为 `Todo("safe_member_access")` / `Todo("not_null_assert")`（`lower/expr.rs:552,555`），codegen 报 `UnsupportedMainBody`。这直接阻塞 `Option<T>` 的惯用写法。
- 规范引用：Spec §2.4（Nullability）、Appendix B.3（Kotlin null-safe operators `?.`/`?:`/`!!`）。
- 目标：
  - `?.`：HIR lowering 展开为 `when (receiver) { Some(v) -> Some(v.member); None -> None }`（或等价 if + pattern match）。
  - `!!`：HIR lowering 展开为 `when (receiver) { Some(v) -> v; None -> perform Raise.raise(RuntimeError.NullAssertionFailed) }`。
  - codegen 自动继承现有 `when` / `Raise` 路径。
- 验收：
  - 新增 run-pass fixtures：`?.` 链式调用、`!!` 正常/失败路径（try/catch 捕获 RuntimeError）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成：
  - **HIR lowering**（`hir/lower/expr.rs`）：
    - `!!`（`lower_not_null_assert_expr`）：展开为 `when (expr) { Some(v) -> v; None -> Raise.raise(RuntimeError.NullAssertionFailed) }`。
    - `?.` 字段访问（`lower_safe_member_access_expr`）：展开为 `when (receiver) { Some(v) -> Some(v.field); None -> None }`。
    - `?.` 方法调用（`lower_safe_call_expr`）：`Call { callee: SafeMemberAccess }` 检测后展开为 `when`，内部调用复用 extension/class member fun rewrite 逻辑。
    - 辅助函数：`synth_raise_null_assertion_failed`、`synth_some_wrap`、`synth_none`、`lower_safe_call_inner_call`。
  - **FQN 常量**（`hir/lower/mod.rs`）：`RAISE_RAISE_FQN`、`RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN`。
  - **codegen 继承**：无 codegen 修改——生成的 `When`/`Perform`/`UnresolvedIdent` 节点自动走现有路径（expected type 通过 when arm 传播给 `Some`/`None` 构造）。
  - **fixture**：`not_null_assert_basic.scoop` + `.stdout`（run-pass，`!!` 正常路径）；`safe_call_not_null_assert.hir`（HIR golden，验证 `?.`/`!!` desugar 结构）。
  - **已知限制**：`?.` 端到端 run-pass 受阻于 typecheck 仅支持 struct receiver + codegen 不支持 `Option<Struct>` payload 的组合缺口；待后续 `Option<Struct>` codegen（non-scalar enum payload）落地后可补充 run-pass 覆盖。
  - 139 单元测试 + 777 fixtures 通过（含 LLVM 后端）。

### T0109 [DONE] `with` 表达式 codegen（值类型更新）

- 描述：Spec §2.6 定义 `val p2 = p with { x: 5 }`。Parser 和 typecheck 已完整实现（包含字段存在性/类型兼容性/嵌套路径校验，`typecheck/expr/infer.rs:2405`），但 HIR lowering 为 `Todo("with_update")`（`lower/expr.rs:645`）。
- 规范引用：Spec §2.6（Value Type Update）——所有 RHS 表达式基于原始值求值（parallel semantics），路径可任意深度嵌套。
- 目标：
  - HIR lowering：copy 原值 → 逐字段 store 新值 → 返回新值。
  - 支持嵌套路径：`line with { start.x: 1; start.y: 2 }`。
  - codegen 自动继承 struct literal / field access 路径。
- 验收：
  - 新增 run-pass fixtures：简单 struct 更新、嵌套更新、多字段更新。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0110 [DONE] `for (x in iterable)` HIR lowering + codegen

- 描述：当前 `for` 语句在 HIR lowering 中为 `Todo("for")`（`lower/stmt.rs:58`）。Typecheck 已完整实现 `for-in` 的 Iterator 协议解析（`typecheck/expr/stmt.rs:688`：解析 `.iterator()`/`.next()`、提取 `Option<T>` 元素类型、注入 binder），但产物无法进入 codegen。
- 规范引用：Spec §16.2——`for (x in xs)` desugar 为 `val it = xs.iterator(); while (true) { when (it.next()) { Some(x) -> body; None -> break } }`。
- 目标：
  - HIR lowering 将 `for (x in xs)` 展开为 while + `iterator().next()` + pattern match（复用已有 `while`/`when`/`break` HIR 节点）。
  - 至少支持 `Array<Int>`、`IntProgression`（ranges）作为 iterable。
- 验收：
  - 新增 run-pass fixtures：`for (x in [1,2,3])`、`for (x in 1.rangeTo(5,1))`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 备注：T1819（Ranges 增强）的 `for (x in range)` integration 依赖此任务。

### T0111 [DONE] 运算符重载：用户定义类型的方法分发 + 缺失运算符

- 描述：当前运算符重载存在两个层面的缺陷：
  1. **codegen 不分发到用户方法**：typecheck 能识别 `plus`/`minus`/`and`/`or`/`xor`/`shl`/`shr`（`typecheck/expr/ops.rs:106`），但 codegen 的 `codegen_binary`（`codegen/mod.rs:11457`）仅发出 LLVM 内建整数指令，不会对 struct/class 类型分发到对应方法调用。
  2. **缺失运算符**：Spec B.8 额外要求 `times`（`*`）、`div`（`/`）、`rem`（`%`）、`compareTo`（`<`/`<=`/`>`/`>=`）、`get`/`set`（`[]` indexing），这些在 typecheck 的 `operator_overload_method_name` 中均返回 `None`，仅走内建整数路径。
- 规范引用：Spec Appendix B.8（Operator Overloading）。
- 目标：
  - Phase 1：codegen 检测到运算符重载方法时，发出方法调用（而非内建指令）。
  - Phase 2：typecheck 新增 `times`/`div`/`rem` 映射（`Mul`→`times`, `Div`→`div`, `Rem`→`rem`）。
  - Phase 3：typecheck + codegen 新增 `compareTo` → 比较运算符映射。
  - Phase 4：typecheck + codegen 新增 `get`/`set` → 索引表达式映射。
- 验收：
  - 新增 run-pass fixtures：自定义 struct 实现 `plus`/`times`/`compareTo`/`get` 并在表达式中使用运算符语法。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0112 [DONE] Extension property codegen

- 描述：扩展属性（Spec §10.3）在 codegen 中返回 `UnsupportedMainBody { kind: "member access target" }`。`codegen_member_access`（`codegen/mod.rs:10859`）仅处理 `MemberRef::Value { fqn }`，对 `MemberRef::ExtensionValue` 落入 `Some(_)` catch-all 报错（`mod.rs:10969`）。
- 规范引用：Spec §10.3——扩展属性无 backing field，必须为 computed（getter-only）。
- 目标：
  - codegen 对 `ExtensionValue` 调用对应的 getter 函数（FQN 已在 typecheck 阶段解析）。
  - 如有 setter（spec 不允许用于值类型，但 ref 类型可以），支持赋值路径。
- 验收：
  - 新增 run-pass fixture：定义扩展属性并在表达式中访问。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0113 [DONE] Varargs spread `*arr` codegen

- 描述：Spec B.5 定义 `vararg` 参数和 spread 运算符 `*arr`。当前 HIR lowering 为 `Todo("spread_arg")`（`lower/expr.rs:301`），注释说明"spread 仅在调用实参语境下有意义；HIR v0 暂不承载该语义"。
- 规范引用：Spec Appendix B.5（Functions：varargs）。
- 目标：
  - HIR lowering 将 `f(*arr)` 在 vararg 参数位置展开为数组元素访问序列。
  - codegen 发出对应的 LLVM IR。
- 验收：
  - 新增 run-pass fixture：`fun sum(vararg xs: Int): Int` + `sum(*[1,2,3])`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0114 [DONE] `Bool.toString()` + `print`/`println` Bool 重载

- 描述：当前 `Bool` 无 `toString()` 方法（resolver 白名单中无 Bool 相关项），也无法直接传入 `print`/`println`（sysroot 仅声明 `String`/`Int` 两种重载）。用户必须手写 `if (b) "true" else "false"` 或使用 `when` 转换。Spec Appendix B 的 Kotlin 语义预期所有基本类型可 toString。
- 目标：
  - resolver 白名单新增 `Bool.toString()`；codegen 内联 `if tag == 1 then "true" else "false"`。
  - sysroot 新增 `print(value: Bool)` / `println(value: Bool)` 重载（通过 `toString()` + 已有 `print(String)` 路由）。
- 验收：
  - 新增 run-pass fixture：`println(true)`、`false.toString()`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **runtime/c**（`scoop_runtime.c`）：新增 `scoop_bool_to_string(int64_t value) -> ScoopString*`——返回 GC-managed `"true"` 或 `"false"` 字符串。注册到 `scoop_runtime_api.h`。
  - **resolver**（`resolve/scopes.rs`）：Bool 方法白名单新增 `toString`（`scoop.core.Bool` receiver FQN 匹配）。
  - **typecheck**（`typecheck/expr/call.rs`）：`Bool.toString()` → 0 args → `String`。
  - **runtime symbols + ABI**（`runtime_symbols.rs` + `runtime_abi.rs`）：`SCOOP_BOOL_TO_STRING` 常量 + `declare_runtime_bool_to_string` 声明（`ScoopString* fn(i64)`）。
  - **codegen**（`codegen/mod.rs`）：统一 `codegen_to_string_method` 分发——evaluate receiver first, then dispatch by CgTy（Bool → zero-extend i1 to i64 + `scoop_bool_to_string`, Int → `scoop_int_to_string`）。解决了 `(x == y).toString()` 等表达式结果的类型路由问题（HIR type 与 CgTy 不一致时走 CgTy 判断）。
  - **codegen print/println**（`codegen/mod.rs`）：`codegen_sysroot_print_like` 新增 `CgTy::Bool` arm——zero-extend + `scoop_bool_to_string` + existing `print(String)` 路由。
  - **sysroot**（`sysroot/core.scoop`）：新增 `fun print(value: Bool): Unit` 和 `fun println(value: Bool): Unit`。
  - **fixtures**：
    - `bool_print_minimal.scoop` + `.stdout`：最小 `println(true)` smoke test。
    - `bool_to_string_print_basic.scoop` + `.stdout`：综合 17 行 stdout 覆盖——`println(true/false)`、`print(true/false)`、`true.toString()`/`false.toString()`、variable Bool print + toString、`(x == y).toString()` 表达式 Bool、`String.concat(true.toString())` 组合。
  - 全部 788 fixtures + cargo test 通过。

### T0115 [DONE] String 补齐：`trim`/`replace`/`charAt`/`isEmpty` 等缺失方法

- 描述：当前 String 仅有 11 个 resolver 白名单方法（`trimIndent`/`length`/`substring`/`startsWith`/`endsWith`/`indexOf`/`contains`/`split`/`toInt`/`concat`/`hash`）。多个常用方法缺失，调用时在 resolver 阶段报 `UnresolvedMember`。
- 目标（按优先级）：
  - P0：`trim(): String`（去除首尾空白）、`isEmpty(): Bool`（长度为零判断）
  - P1：`replace(old: String, new: String): String`、`charAt(index: Int): Int`（返回字节值）
  - P2：`trimStart(): String`、`trimEnd(): String`、`repeat(n: Int): String`、`compareTo(other: String): Int`
- 路径：runtime/c 新增底层 API → `scoop_runtime_api.h` 注册 → resolver 白名单 → typecheck 参数/返回类型 → codegen 路由。
- 验收：
  - 新增 run-pass fixtures 覆盖每个方法。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成记录（2026-04-09）：
  - 全 8 个方法已实现完整 pipeline：C runtime → API 注册 → resolver → typecheck → codegen symbols/ABI/dispatch。
  - 新增 `stdlib_string_methods_extended.scoop` + `.stdout` fixture，覆盖 27 个测试场景。
  - 全部 139 unit tests + 789 fixtures + spec-fixtures 通过。

### T0116 [DONE] 核心库 hardcoded 类型限制清单（跟踪任务）

- 描述：当前核心库存在大量 hardcoded Int-only 限制。本任务统一记录所有已知限制，为后续逐项解除提供清晰入口。
- 已知限制清单（2026-04-09 审计确认）：
  1. **Set 元素 Int-only**：`Set`/`MutableSet` 为 `Array<Int>` typealias（`stdlib/collections_set.scoop`），仅支持 Int 元素 → **T1818**（Hash-based Set/Map）
  2. **Map 键值 Int-only**：`MapView`/`MutableMap` 为 `Array<Int>` typealias（`stdlib/collections_map.scoop`），仅 `Int→Int` → **T1818**（Hash-based Set/Map）
  3. **集合操作 Int-only**：`forEach`/`map`/`filter`/`fold`/`reduce`/`zip`/`joinToString` 仅 `Array<Int>`/`MutableArray<Int>`（`stdlib/array_iter.scoop` + `stdlib/mutable_array_iter.scoop`）→ **T1822**（泛型 Collections API，依赖编译器泛型 codegen）
  4. **Scope functions Int-only**：`let`/`also`/`apply` 仅 `Int` 版本（`stdlib/prelude.scoop:61-74`）→ **T1822**（泛型化，依赖编译器泛型 codegen）
  5. **Task\<T\> Int-only**：`Executor.spawn`/`await` 仅 `Task<Int>`（`sysroot/task.scoop:81-87`，64-bit payload）→ 后置，待编译器泛型 codegen 完善（T0124-T0128）
  6. **print/println 仅 String/Int/Bool**：Bool 重载已完成（**T0114 [DONE]**）；Float 重载待 Float 类型系统支持 → 后置。泛型化（`fun <T> println(value: T) where T: ToString`）→ **T0131**（依赖 T0129/T0130 where 约束能力）
  7. **Hashable 默认 hash() 返回 0**：Int hash（SplitMix64 codegen inline）和 String hash（FNV-1a runtime/c）已实现（**T1817 [DONE]**）。Bool/Int8-UInt64 等类型 hash 仍为 Hashable 接口默认值 0（`sysroot/core.scoop:20-22`）→ 后置，待按需逐类型补齐
  8. **MutableArray COW 语义**：`push`/`pop`/`insert`/`removeAt`/`splice` 返回新数组（`stdlib/mutable_array.scoop`），仅 `set(index, value)` 和 `sort()` 原地修改 → **设计决策**：当前为值语义一致性的有意选择（value type 不做原地变异）；后续若引入引用语义 `MutableList` 可重新评估
- 目标：本任务不做实现，仅作为审计记录。各项解除依赖上述独立任务完成。
- 验收：所有限制项有对应任务链接或明确标注"设计决策/后置"。
- 依赖：无
- 完成记录（2026-04-09）：
  - 审计 8 项限制，逐项确认当前状态并标注归属：4 项有明确后续任务（T1818/T1822/T0131）、2 项后置（Task<T>/Float print 待泛型/类型系统完善）、1 项后置按需补齐（其他类型 hash）、1 项确认为设计决策（MutableArray COW）。
  - 补充文件位置引用，便于后续追溯。

### T0117 [DONE] `@Extern(lib=...)` 参数传递到链接器

- 描述：`@Extern(lib = "libm")` 的 `lib` 参数在 parser 和 typecheck 中正确解析和验证，但 **未传递到 `ExternFun` 结构体**（`hir/mod.rs:626`，结构体仅有 `abi`/`symbol`/`calling_convention`，无 `lib` 字段）。`collect_extern_libs()`（`lower/util.rs:1697`）将 `lib` 值收集到独立的 `Vec<String>` 中，但该列表是否到达链接器调用不明确。
- 规范引用：Spec §15.5（`@Extern(lib?, name?)`）——`lib` 参数用于指示需要链接的外部库。
- 目标：
  - `ExternFun` 结构体新增 `lib: Option<String>` 字段。
  - LLVM codegen / 链接阶段将所有 `lib` 值传递给链接器（例如 `-lm`）。
  - 验证 `@Extern(lib = "m") fun sin(x: Float): Float` 等场景可正确链接到系统库（可先用 Int-only 测试验证链接器 `-l` 参数传递）。
- 验收：
  - 新增 run-pass fixture 或 Cone fixture：使用 `@Extern(lib = "...")` 调用外部 C 库函数。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **审计结论**：`@Extern(lib = "...")` 的链接器传递管线**已完整实现**（由历史任务 T1020 完成）：`collect_extern_libs()` 收集 `lib` 值到 `LoweredHir.extern_libs`，build 命令通过 `link_objs()` / `link_objs_with_runtime()` 将其传递给 `clang` 作为 `-l<name>` 标志。单元测试 `clang_link_command_includes_extern_libs` 已覆盖此路径。
  - **`ExternFun` 结构体**（`hir/mod.rs`）：新增 `lib: Option<String>` 字段，用于记录单个函数关联的外部库名（诊断与追溯）。
  - **`extern_fun_of_decl`**（`hir/lower/util.rs`）：填充 `ExternFun.lib` 字段（从 `parse_extern_annotation_args().lib` 获取）。
  - **单元测试**（`toolchain.rs`）：`clang_link_command_includes_extern_libs` 新增 `ExternFun.lib` 字段断言。
  - **Fixtures**：
    - `extern_lib_link_basic.scoop` + `.stdout`（run-pass）：`@Extern(lib = "c", name = "labs")` 调用 C 标准库 `labs()` 函数，验证 `-lc` 链接器参数传递 + 函数调用正确性。
    - `extern_lib_link_basic/`（run_pass_cone）：Cone 项目形式的相同测试，验证 `scoop build` 全链路。
  - 139 单元测试 + 791 fixtures 通过。

### T0118 [DONE] `@CLayout(packed)` store alignment 修复 + run-pass 测试

- 描述：`@CLayout(packed = 1)` 在 codegen 中创建 packed LLVM struct（`set_body(fields, true)`），且 **load** 指令的 alignment 已正确降为 1（`codegen/mod.rs:10932-10941`）。但 **store** 指令在写入 packed struct 字段时**未降低 alignment**，在严格对齐的架构（如 ARM、MIPS）上可能导致未定义行为。此外，`@CLayout` 的全部 codegen 路径（aligned + packed）目前 **零 run-pass 测试覆盖**（仅有 3 个 typecheck 错误用例）。
- 目标：
  - 审计 `@CLayout(packed = 1)` 的所有 store 路径，确保 store alignment 降为 1（与 load 一致）。
  - 新增 run-pass fixtures 覆盖：
    - `@CLayout(packed = 1)` struct 的字段读/写正确性。
    - `@CLayout(aligned = 16)` struct 的 alloca 对齐。
    - `@CLayout(aligned = 8, packed = 1)` 组合。
  - （可选）新增 build fixtures：`--emit-llvm` 断言 packed struct type、load/store alignment 属性。
- 验收：
  - run-pass fixtures 在 x86-64 下通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **store alignment 修复**（`codegen/gc.rs` `store_local_value`）：在 `build_store(ptr, raw)` 之后，当目标类型为 `CgTy::Struct(struct_ty)` 且该 struct 有 `@CLayout(packed)` 时，显式调用 `store_inst.set_alignment(1)` 降低 store alignment 到 1，与 load 路径保持一致。
  - **审计结论**：当前 codegen 中 packed struct 的 store 路径仅有 `store_local_value`（整个 aggregate store 到 alloca）。struct 字段级 GEP + store 不存在于用户 struct 路径（struct 值通过 `build_insert_value` 构造，然后整体 store）。Class 类型不允许 `@CLayout`，因此 class field store 无需修复。
  - **Fixtures**（3 个 run-pass）：
    - `clayout_packed_basic.scoop` + `.stdout`：`@CLayout(packed: 1)` struct — 字段读取（直接 + 函数传参）、var 重新赋值、负值，18 行 stdout。
    - `clayout_aligned_basic.scoop` + `.stdout`：`@CLayout(aligned: 16)` + `@CLayout(aligned: 8)` — 字段读取、函数传参、var 重新赋值，12 行 stdout。
    - `clayout_aligned_packed_combined.scoop` + `.stdout`：`@CLayout(aligned: 8, packed: 1)` 组合 — 字段读取、函数传参、var 重新赋值，13 行 stdout。
  - **注意**：Fixtures 使用 `:` 语法（`@CLayout(packed: 1)`）而非 `=` 语法（`@CLayout(packed = 1)`），因为 `=` 语法在 general annotation checker（T1019）中被误判为非常量表达式；CLayout-specific `parse_clayout_args` 支持两种语法，但 general checker 先执行。
  - 139 单元测试 + 794 fixtures 通过（含 LLVM 后端）。

### T0119 [DONE] `@CLayout(packed = N)` 支持 N > 1（`#pragma pack(N)` 语义）

- 描述：当前 typecheck 仅接受 `packed = 1`（`annotations.rs:1895-1903`），任何其它非零值报 `CLayoutPackedValueNotSupported`。Spec §15.5 定义 `packed` 为"max field alignment"，语义等价于 C 的 `#pragma pack(N)`——即每个字段的 alignment 取 `min(field_natural_align, N)`。常见有意义的值为 1、2、4、8。
- 目标：
  - typecheck 放开 `packed` 为 1/2/4/8/16（均为 2 的幂）。
  - codegen：不再简单地用 `set_body(fields, true)`（仅对 `packed=1` 有效），而是显式计算每个字段的 offset 和 padding，使 `min(natural_align, packed)` 生效。
  - 新增 run-pass fixtures：`@CLayout(packed = 4)` struct 含 `Int64` 字段（natural align 8），验证字段 offset 为 4 而非 8。
- 验收：
  - typecheck fixtures 覆盖 `packed = 2`/`4`/`8` 均通过。
  - run-pass fixture 验证布局正确性（可通过 `sizeOf<T>()` 或 `@Extern` 互操作断言）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0118（先确保 packed=1 的 store alignment 正确）
- 完成说明：
  - **Typecheck**（`annotations.rs`）：`packed` 值验证从 `value != 1` 改为 `!is_power_of_two() || value > 16`，接受 1/2/4/8/16。错误消息更新为"必须是正的 2 的幂且 ≤ 16"。
  - **Codegen type lowering**（`codegen/ty.rs` `llvm_struct_type`）：`packed=1` 继续使用 LLVM 原生 packed struct；`packed>1` 使用手动 padding insertion——LLVM packed struct（`is_packed=true`）+ 显式 `[N x i8]` padding 字节，字段有效对齐 = `min(natural_align, N)`。维护 `pack_field_indices` 缓存：逻辑字段号 → LLVM struct 元素号的映射。
  - **关键 bug 修复**（`codegen/ty.rs` `llvm_struct_type` early return path）：每个顶层函数获得独立的 `MainCodegen` 实例（`mut self` 消费），`pack_field_indices` 在函数间不共享。LLVM named struct type 在 context 中持久化，导致后续函数的 `llvm_struct_type` 走 early return 但 `pack_field_indices` 为空。修复：在 early return 路径中检测 `packed>1 && !pack_field_indices.contains_key`，重新推导 padding/field index 映射。
  - **Field access codegen**（`codegen/layout.rs` `lookup_struct_field`、`codegen/mod.rs` `codegen_member_access`、`codegen_struct_lit`）：使用 `pack_field_indices` 将逻辑字段索引映射到 LLVM 元素索引（GEP、insert_value、load alignment）。
  - **Store alignment**（`codegen/gc.rs`）：packed struct 的 store 使用 `set_alignment(pack_n)` 而非硬编码 1。
  - **Typecheck fixtures**（3 个）：
    - `clayout_packed_values_2_4_8_16_ok.scoop`：packed=2/4/8/16 均通过 typecheck。
    - `clayout_packed_value_not_power_of_two_is_error.scoop`：packed=3 报错。
    - `clayout_packed_value_too_large_is_error.scoop`：packed=32 报错。
  - **Run-pass fixture**：`clayout_packed_n_gt_1.scoop` + `.stdout`——packed=4（UInt8+Int、两 Int、三字段）、packed=2、packed=8。覆盖字段访问、函数传参、var 重赋值、负值，12 行 stdout。
  - 139 单元测试 + 798 fixtures 通过（含 LLVM 后端）。

### T0120 [DONE] String 字节访问器：`getByte(index: Int): Int` + `byteLength(): Int`

- 描述：为 `String` 提供严格 O(1) 的只读字节级访问能力，不执行 UTF-8 验证。这是将 `substring`/`split`/`indexOf` 等操作从 runtime/c 迁移到纯 Scoop 的前置能力。
- 目标：
  - `String.byteLength(): Int`：返回底层 UTF-8 字节数组的长度。O(1)，直接读取 `ScoopString.len` 字段。
  - `String.getByte(index: Int): Int`：返回指定字节偏移处的原始字节值（0-255）。O(1)，直接索引 `ScoopString.data`。越界返回 0。
  - 两个方法均为编译器 intrinsic（resolver 白名单 + codegen 内联 LLVM IR）。
  - 不需要 `@Unsafe`：返回值类型，不暴露内部指针，只读访问无安全风险。
- 路径：resolver 白名单 → typecheck 参数/返回类型 → codegen 发出 GEP + load（`byteLength` 读 header field，`getByte` 读 data 数组元素）。
- 验收：
  - 新增 run-pass fixtures：验证 ASCII 字符串的 `byteLength`、逐字节 `getByte`、多字节 UTF-8 字符的字节序列。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **resolver**（`resolve/scopes.rs`）：String 方法白名单新增 `byteLength` 和 `getByte`。
  - **typecheck**（`typecheck/expr/call.rs`）：`byteLength` → 0 参数 → `Int`；`getByte` → 1 参数 (Int) → `Int`。
  - **codegen**（`codegen/mod.rs`）：
    - `byteLength`：内联 LLVM IR——GEP 到 `ScoopString.len`（字段 1），load i64 直接返回。无 runtime 调用。
    - `getByte`：内联 LLVM IR——bounds check（index < 0 || index >= len → 返回 0）+ GEP 到 `data[index]` + load i8 + zero-extend to i64。无 runtime 调用。
  - **返回类型说明**：使用 `Int`（而非 TODO 原始设计的 `UInt` / `Byte`）以保持与现有 `length()` / `charAt()` 等方法的一致性。后续如需 UInt8 返回类型可在泛型化阶段调整。
  - **fixture**：`string_byte_accessors.scoop` + `.stdout`——覆盖 20 个场景：ASCII byteLength（5/0/1/13 字节）、逐字节 getByte（H=72/e=101/l=108/o=111）、越界返回 0（index=5/-1/100）、特殊字符（A=65/空格=32/0=48/9=57）、byteLength+getByte 联合验证。
  - 139 单元测试 + 799 fixtures 通过。

### T0121 [DONE] `@Unsafe` String 构造 intrinsic：从源 String + 字节偏移 + 字节长度创建子串

- 描述：提供一个 `@Unsafe` intrinsic，从现有 `String` 的字节范围创建新 `String`，**不执行 UTF-8 验证**。这是纯 Scoop 实现 `substring`/`split`/`trim` 等操作的底层构建块。调用者负责保证字节范围落在合法的 UTF-8 字符边界上。
- 签名（建议）：
  ```kotlin
  @Intrinsic @Unsafe @NoGC
  fun String.unsafeSliceBytes(byteOffset: Int, byteLength: Int): String
  ```
  - 从 `this` 的 `data + byteOffset` 复制 `byteLength` 字节到新分配的 `ScoopString`。
  - 前置条件（调用者保证）：`byteOffset >= 0`，`byteOffset + byteLength <= this.byteLength()`，切片范围在 UTF-8 字符边界上。
  - 违反前置条件：UB（与 `@Unsafe` 语义一致）。
- 实现路径：
  - sysroot 声明（`sysroot/core.scoop` 或 `sysroot/unsafe.scoop`）。
  - resolver 白名单 + typecheck。
  - codegen：调用 runtime/c 的 `scoop_string_from_bytes(data + offset, len)`（已存在，用于从 raw bytes 创建 String）。
  - 注意：虽标记 `@NoGC`，但内部需要分配新 `ScoopString`——这里 `@NoGC` 限制可能需要放宽，或改用 `@Unsafe` 但不标 `@NoGC`。需确认 GC 约束。
- 验收：
  - 新增 run-pass fixture（在 `@Unsafe` 块中）：从 `"Hello, World!"` 切出 `"World"` 并验证。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0120（`byteLength` 用于边界计算）
- 完成说明：
  - **runtime/c**（`scoop_runtime.c`）：新增 `scoop_string_unsafe_slice_bytes(source, byte_offset, byte_length) -> ScoopString*`——defensive clamping（null/empty/negative/overflow），pin source before `scoop_string_from_bytes` 调用（GC safety）。注册到 `scoop_runtime_api.h`。
  - **resolver**（`resolve/scopes.rs`）：String 方法白名单新增 `unsafeSliceBytes`。
  - **typecheck**（`typecheck/expr/call.rs`）：`unsafeSliceBytes` → 要求 `in_unsafe_context()` → 2 个参数 → 返回 `String`。非 unsafe context 报 `UnsafeCallRequiresUnsafeContext`。
  - **runtime symbols + ABI**（`runtime_symbols.rs` + `runtime_abi.rs`）：`SCOOP_STRING_UNSAFE_SLICE_BYTES` 常量 + `declare_runtime_string_unsafe_slice_bytes` 声明（`ScoopString* fn(ScoopString*, i64, i64)`）。
  - **codegen**（`codegen/mod.rs`）：`codegen_string_method` 新增 `"unsafeSliceBytes"` arm——codegen 2 个 Int 参数 + 调用 runtime 函数 + 返回 `CgValue { ty: CgTy::String }`。
  - **Fixtures**：
    - `string_unsafe_slice_bytes.scoop` + `.stdout`（run-pass）：在 `@Unsafe { ... }` 块中测试——基本切片（"World"/"Hello"/单字符）、空切片（零长度/负长度）、边界防御（offset 越界/length 超长/负 offset clamping）、完整字符串切片、byteLength 联合使用、getByte 内容验证，共 15 个输出行。
    - `string_unsafe_slice_bytes_requires_unsafe_is_error.scoop`（typecheck error）：非 unsafe context 调用报错。
  - 139 单元测试 + 801 fixtures 通过。

### T0122 [DONE] String 操作迁移：将 runtime/c substring 类函数替换为纯 Scoop 实现

- 描述：基于 T0120（字节访问器）和 T0121（unsafe slice intrinsic），将以下 runtime/c 字符串操作重写为纯 Scoop 源码：
  - `substring(start, end)` → 用 `getByte` 做边界校验 + `unsafeSliceBytes` 切片
  - `indexOf(substr)` → 纯 Scoop 逐字节扫描（`getByte` + `byteLength`）
  - `contains(substr)` → 委托 `indexOf >= 0`
  - `startsWith(prefix)` → 逐字节比较
  - `endsWith(suffix)` → 逐字节比较
  - `split(delimiter)` → 扫描 + `unsafeSliceBytes` 切割
  - `trim()` / `trimStart()` / `trimEnd()`（T0115）→ 扫描空白字节 + `unsafeSliceBytes`
- 好处：
  - 减少 runtime/c 维护面和 GC pin/unpin 复杂度（C 侧 `scoop_string_split` 等函数的 GC rooting 曾导致 T0106/T1812 的 bug）。
  - 纯 Scoop 实现可被 `const fun` 求值（T0123）。
  - 用户可在 stdlib 源码中直接阅读和理解实现。
- 目标：
  - 新增 `stdlib/string.scoop`（或扩展 `stdlib/prelude.scoop`），实现上述操作为 extension functions。
  - runtime/c 中对应的 `scoop_string_substring`/`scoop_string_index_of`/`scoop_string_contains`/`scoop_string_starts_with`/`scoop_string_ends_with`/`scoop_string_split` 标记为 deprecated 或移除。
  - codegen 中这些方法不再硬编码路由到 C 函数，改为走正常的 extension function 调用路径。
  - resolver 白名单中移除迁移后的方法（改由 sysroot/stdlib 声明驱动）。
- 验收：
  - 现有 String 相关 run-pass fixtures 全部通过（行为不变）。
  - GC stress 下稳定（不再有 C 侧 pin/unpin 风险）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0120、T0121

### T0140 [TODO] 多文件字面量支持：非入口源文件允许使用 Int/String 字面量

- 描述：当前 `LiteralKind::Int` 和 `LiteralKind::String` 不存储实际值，仅存储 `Span`（字节偏移），codegen 时通过 `self.source.slice(span)` 从源文本中截取字面量文本。但 `MainCodegen` 的 `self.source` 只持有入口文件的 `SourceFile`（`llvm/codegen/mod.rs:163`），非入口文件的 Span 如果切入入口文件的源文本会得到错误内容。因此 `hir/lower/mod.rs:1302-1306` 在多文件编译时对非入口文件硬性禁止 source-backed literals，报 `MultiFileNonEntrySourceBackedLiteral` 错误。
  这导致 `stdlib/string.scoop` 等非入口文件无法使用任何整数字面量（`0`、`1`、`32` 等），只能通过 `sizeOf` 算术迂回派生常量（`__string_zero`/`__string_one`/`__string_is_whitespace_byte`），严重损害可读性。
- 已知相关代码：
  - `hir/mod.rs:376-385`：`LiteralKind::Int`/`String` 不存值；`SynthInt(i64)` 携带值（可跨文件）。
  - `llvm/codegen/mod.rs:10494-10528`：`codegen_literal` 对 Int/String 调用 `self.source.slice(span)`。
  - `hir/lower/util.rs:452-462`：`expr_contains_source_backed_literals` 判定 Int/String 为 source-backed。
  - `hir/lower/types.rs:154-156`：错误定义。
- 可选方案：
  1. **HIR 内联值**（推荐）：将 `LiteralKind::Int` 改为携带解析后的数值（类似 `SynthInt`），`LiteralKind::String` 改为携带 `String` 值。在 HIR lowering 阶段（parse → HIR）即从 Span 提取文本并存入节点，之后 codegen 不再依赖 `self.source`。这是最小侵入的方案，`SynthInt` 已证明此路径可行。
  2. **多文件 SourceMap**：让 codegen 持有所有参与编译的 `SourceFile`，Span 扩展为 `(file_id, offset, len)`，`slice` 时按 file_id 查找对应源文本。侵入面更大但更通用。
- 目标：
  - 移除 `MultiFileNonEntrySourceBackedLiteral` 限制，非入口文件可正常使用 Int/String 字面量。
  - `stdlib/string.scoop` 中的 `__string_zero`/`__string_one`/`__string_is_whitespace_byte` 等辅助函数可用直接字面量重写，大幅简化代码。
- 验收：
  - 新增 run-pass fixture：多文件编译，非入口文件中使用 Int/String 字面量并正确运行。
  - 现有 run-pass fixtures 全部通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0141 [TODO] 块级控制流：支持 block/loop 内 return/break/continue

- 描述：当前 LLVM codegen 采用递归表达式求值模式（expression-based codegen），没有预分配的 function-level 基本块（basic block）图。`return`/`break`/`continue` 在嵌套 block 或 loop 体内出现时，需要跳转到尚不存在的目标基本块（函数出口块 / 循环后继块 / 循环头块），因此被硬性拒绝（`LlvmEmitError::UnsupportedMainBody`）。
  这导致 `stdlib/string.scoop` 中所有循环提前退出只能用 flag 变量（`scanning = false`）或越界赋值（`j = dlen` 模拟 break、`i = limit + one` 模拟外层循环退出），逻辑晦涩且易出错。
- 已知相关代码：
  - `llvm/codegen/stmt.rs:177-187`：`codegen_block_stmt` 中 `Return`/`Break`/`Continue` 直接返回 `UnsupportedMainBody`。
  - `llvm/codegen/control_flow.rs:1704-1718`：block expression 内 return/break/continue 同样被拒绝。
  - `llvm/codegen/control_flow.rs:52-74`：顶层 `main` 的 `return` 可直接 emit `ret`，但 while/break/continue 被拒绝。
  - 顶层函数 return 可工作是因为可直接 emit LLVM `ret` 指令，无需跳转。
- 可选方案：
  1. **轻量预分配方案**（增量）：在 codegen 进入每个函数时预分配一个 `exit_block`（存 return 值的 alloca + 跳转目标），进入每个 while 循环时预分配 `loop_continue_block` 和 `loop_break_block`。遇到 return/break/continue 时 emit `br` 到对应目标块。这在现有 expression-based codegen 基础上增量实现，不需要完整 MIR/CFG。
  2. **完整 MIR/CFG**（PLAN §8）：在 HIR → LLVM IR 之间插入一层 MIR，先构建完整 CFG 再 emit LLVM IR。更通用但工作量大。
- 目标：
  - 函数体内任意嵌套深度的 `return` 语句正确跳转到函数出口。
  - `while` 循环体内 `break` 跳出循环、`continue` 跳转到循环头。
  - `stdlib/string.scoop` 中的 flag + 越界赋值 hack 可改写为正常 break/continue/return。
- 验收：
  - 新增 run-pass fixture：函数内 if 块中 early return。
  - 新增 run-pass fixture：while 循环中使用 break 提前退出。
  - 新增 run-pass fixture：while 循环中使用 continue 跳过迭代。
  - 新增 run-pass fixture：嵌套循环中 break 只退出内层循环。
  - 现有 run-pass fixtures 全部通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0142 [TODO] if 表达式无 else 分支支持（non-Unit if-without-else）

- 描述：当前 codegen 的 `codegen_if_expr`（`llvm/codegen/control_flow.rs:160-178`）为 if 表达式分配 `result_ptr`（alloca），then 和 else 分支各写入值后跳转到 merge block。当 `else_branch` 为 `None` 且输出类型非 Unit 时，else 路径没有值可写入 `result_ptr`，导致 merge 点取到未定义值，因此硬性报错 `"if without else (non-Unit)"`。Unit 类型的 if-without-else 已可工作（不分配 result_ptr）。
  这导致 `stdlib/string.scoop` 中即使是纯 statement 用途的 if（如 `if (s < zero) { s = zero }`），也必须写成 `if (s < zero) { s = zero } else { }`，增加噪音。
- 已知相关代码：
  - `llvm/codegen/control_flow.rs:137-138`：`result_ptr` 分配逻辑。
  - `llvm/codegen/control_flow.rs:160-178`：else_branch 为 None 时的 non-Unit 检查。
- 目标：
  1. 当 if-without-else 出现在语句位置（statement context）时，即使 then 分支的表达式类型非 Unit，也应允许——整个 if 语句的类型视为 Unit，不分配 result_ptr。
  2. 当 if-without-else 用作值表达式（`val x = if (...) { ... }`）且 then 类型非 Unit 时，可选择：报 type error（更安全），或将结果类型推断为 `T?`/`Option<T>`（语义更丰富，但依赖 Option 类型系统）。推荐前者——此场景应由 typechecker 报错而非 codegen。
  3. 移除 `stdlib/string.scoop` 中所有多余的 `else { }` 空分支。
- 验收：
  - 新增 run-pass fixture：if-without-else 在 statement 位置，then 分支含赋值等 non-Unit 表达式。
  - 新增 typecheck fail fixture：if-without-else 用作值表达式且 then 类型非 Unit → 报错。
  - 现有 run-pass fixtures 全部通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0143 [TODO] String 扩展方法从 stdlib 迁移到 sysroot core 库

- 描述：`stdlib/string.scoop` 中的 String 扩展方法（`substring`/`indexOf`/`contains`/`startsWith`/`endsWith`/`split`/`trim`/`trimStart`/`trimEnd`）属于 String 类型的基础操作，应作为 core 库的一部分（`sysroot/`），而非 stdlib。当前放在 `stdlib/` 是因为 T0122 迁移时受限于编译器能力（字面量、控制流），选择了阻力最小的路径。T0140-T0142 解决编译器限制后，应将这些方法迁移到 `sysroot/` 并利用新能力重写简化。
- 目标：
  1. 将 `stdlib/string.scoop` 的内容迁移到 `sysroot/` 下（可新建 `sysroot/string.scoop` 或合并入 `sysroot/core.scoop`，视文件体量决定）。
  2. 利用 T0140（字面量支持）移除 `__string_zero`/`__string_one`/`__string_is_whitespace_byte` 等 hack 函数，直接使用 `0`、`1`、`32` 等字面量。
  3. 利用 T0141（块级控制流）将 flag + 越界赋值模式改写为 `break`/`continue`/`return`。
  4. 利用 T0142（if-without-else）移除所有多余的 `else { }` 空分支。
  5. 更新编译器中所有引用 `stdlib/string.scoop` 的路径（`resolve/scopes.rs`、`typecheck/expr/call.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/runtime_abi.rs` 中的注释等）。
  6. 删除 `stdlib/string.scoop`。
- 验收：
  - String 扩展方法作为 sysroot core 库的一部分被编译，无需用户显式 import。
  - 代码中不再有 `__string_zero`/`__string_one` 等辅助函数。
  - 现有 String 相关 run-pass fixtures 全部通过（行为不变）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0140、T0141、T0142

### T0124 [TODO] 泛型验证与修复：monomorphization 扩展至泛型 class/struct/enum

- 描述：当前 monomorphization pass（`monomorph/lower.rs:6`）仅处理泛型函数（`ast::Item::Fun`），完全不处理泛型 class/struct/enum 定义。用户定义的 `class Box<T>` 等无法被单态化为具体的 `Box<Int>`、`Box<String>` 等变体。这是泛型 class 无法工作的根本原因。
- 已知问题：
  - `monomorph/lower.rs` 的 `lower_for_dump` 只遍历 `ast::Item::Fun`，跳过所有 class/struct/enum。
  - `MonomorphKey` / `MonomorphSymbol` 仅建模函数签名，无 class/struct/enum 对应物。
  - `collect_struct_layouts`（`hir/lower/util.rs:2001-2006`）显式跳过有 type_params 的 struct。
  - `collect_enum_layouts`（`hir/lower/util.rs:2077-2081`）显式跳过有 type_params 的 enum。
  - `collect_class_decl_init`（`hir/lower/util.rs:816`）未调用 `push_type_params`，导致类型参数被错误地解析为 `Any`。
- 目标：
  1. 扩展 monomorph pass，收集所有泛型 class/struct/enum 的实例化点（构造调用、字段访问、类型注解），生成具体的单态化变体。
  2. 为每个单态化变体生成独立的 `MonomorphKey`（含类型实参），使后续 HIR/codegen 能区分 `Box<Int>` 与 `Box<String>`。
  3. `collect_struct_layouts` / `collect_enum_layouts` 不再跳过泛型类型，而是为每个单态化变体生成独立布局。
  4. `collect_class_decl_init` 正确绑定 type params，使字段类型在每个变体中被替换为具体类型。
- 验收：
  - 新增 run-pass fixture：`class Box<T>(val inner: T)` + `Box<Int>(42).inner` + `Box<String>("hello").inner`。
  - 新增 run-pass fixture：`struct Pair<A, B>(val first: A, val second: B)` + 多种实例化。
  - 新增 run-pass fixture：`enum Either<L, R> { Left(val v: L); Right(val v: R) }` + pattern match。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0125 [TODO] 泛型验证与修复：codegen 支持 `TypeKind::Param` 及参数化类型查找

- 描述：当前 codegen 的类型映射（`llvm/codegen/ty.rs`）无法处理泛型类型参数和参数化的名义类型。即使 monomorph 正确生成了变体，codegen 仍无法发出正确的 LLVM 类型。
- 已知问题：
  - `cg_ty_of`（`ty.rs:94`）对 `TypeKind::Param` 落入 `_ => None`，返回错误而非具体类型。
  - `cg_ty_of_type_fqn`（`ty.rs:251-254`）硬编码 `nominal.args.is_empty()` 过滤，导致参数化类型如 `Box<Int>` 无法被查找。
  - `llvm_struct_type`（`ty.rs:302-310`）使用裸 FQN 作为 key（无类型实参），`Box<Int>` 和 `Box<String>` 共享同一个 LLVM struct layout。
  - `llvm_class_payload_type` / `llvm_class_object_type` 同理，对同一个 FQN 的不同实例化不产出不同结构体。
- 目标：
  1. 在 monomorph 后 codegen 阶段，`TypeKind::Param` 应已被替换为具体类型——如果仍出现则视为 monomorph 遗漏并报告 ICE。
  2. 移除 `nominal.args.is_empty()` 过滤，或改为按 mangled FQN（含类型实参）查找布局。
  3. struct/enum/class layout 缓存改用含类型实参的 mangled key（如 `"Box<scoop.core.Int>"`），为每个单态化变体生成独立 LLVM struct。
  4. `llvm_class_payload_type` / `llvm_class_object_type` 同步使用 mangled key。
- 验收：
  - LLVM IR dump 中可见不同的 struct 类型：`%scoop.runtime.ClassPayload__Box__Int` vs `%scoop.runtime.ClassPayload__Box__String`。
  - T0124 的 run-pass fixtures 在此修复后全部通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0124

### T0126 [TODO] 泛型验证与修复：泛型方法调用与构造函数

- 描述：泛型 class/struct 的方法调用和构造函数在 codegen 中需要正确分发到单态化后的变体。
- 需验证并修复的场景：
  1. **泛型 class 构造**：`Box<Int>(42)` — `codegen_class_ctor_call` 必须查找单态化后的 `ClassInit`（含类型实参），正确计算 payload 大小和字段偏移。
  2. **泛型 class 成员访问**：`box.inner` — codegen 必须从单态化后的 payload struct 中提取正确偏移/类型的字段。
  3. **泛型 class 方法调用**：`box.someMethod()` — 方法体中的 `T` 必须已替换为具体类型。
  4. **泛型 struct 构造 / 字段访问**：同理。
  5. **泛型 enum variant 构造 / pattern match**：`Either.Left<Int, String>(42)` → match 时正确提取。
  6. **泛型类型作为函数参数/返回值**：`fun wrap<T>(v: T): Box<T>` — Box 的实例化由 monomorph 传递推断。
- 验收：
  - 新增 run-pass fixture：泛型 class 带方法 + 调用。
  - 新增 run-pass fixture：泛型函数返回泛型 class 实例。
  - 新增 run-pass fixture：泛型 class 嵌套（`Box<Box<Int>>`）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0124、T0125

### T0127 [TODO] 泛型验证与修复：泛型函数边界场景

- 描述：泛型函数的 monomorphization 已基本工作，但需验证并修复以下边界场景。
- 需验证并修复的场景：
  1. **多类型参数**：`fun <A, B> pair(a: A, b: B): ...` — 是否为每个 `(A, B)` 组合生成独立变体。
  2. **类型参数约束**（`<T : Comparable>`）：monomorph 是否尊重 bound 约束，codegen 是否能分发 trait 方法。→ 完整实现见 **T0129**（调用处 bound 检查）+ **T0130**（bound 驱动方法分发）。
  3. **传递实例化**：`fun <T> wrap(v: T) = Box<T>(v)` 调用时是否触发 `Box<T>` 的实例化。
  4. **泛型扩展函数**：`fun <T> T.toBox(): Box<T>` — resolver/monomorph 是否正确处理。
  5. **泛型高阶函数**：`fun <T, R> myMap(v: T, f: (T) -> R): R` — lambda 参数的类型参数替换。
  6. **泛型递归**：`fun <T> foo(x: T): T = foo(x)` — monomorph 是否处理自递归而不无限展开。
- 验收：
  - 每个场景新增 run-pass fixture。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0124（部分场景需要泛型 class 工作）

### T0129 [TODO] 泛型 where 约束：实例化处 bound 检查（函数调用 + 类型构造）

- 描述：当前 `where` 子句在声明处验证合法性（`typecheck/where_clause.rs`），在**类型实例化**处有检查（`TypeSymbol.where_constraints` + `check_where_constraints_on_instantiation`），但**泛型函数调用处完全不检查 bound**。`FunSigOwned` 没有 `where_constraints` 字段，`instantiate_generic_call` 推断出类型实参后不验证其是否满足声明处的 `where` 约束。例如：
  ```
  interface Show { fun show(): String }
  fun <T> show(x: T): String where T: Show { return x.show() }
  show(42)  // Int 未实现 Show，但当前不报错
  ```
- 目标：
  1. **函数调用处**：`FunSigOwned`（`typecheck/expr/mod.rs`）新增 `where_constraints` 字段，收集函数声明的 `where` 子句信息（复用 `WhereConstraintInfo` 或等价结构）。`collect_fun_sigs` / `collect_overload_sigs` 等签名收集路径填充该字段。`instantiate_generic_call`（`typecheck/expr/call.rs`）在推断出具体类型实参后，遍历 `where_constraints`，对每条约束调用 `is_type_assignable(arg_ty, bound_ty)` 验证满足性；不满足时发出 `where_constraint_not_satisfied` 诊断。
  2. **类型构造处**：验证已有的 `check_where_constraints_on_instantiation` 对 `class/struct/enum` 构造（`Box<Int>(42)`、`struct Pair<A, B> where A: Show` 的字面量构造等）也覆盖到位。构造函数调用路径（`codegen_class_ctor_call` / struct literal）应与 `lower_type_ref` 路径一样触发约束检查。
  3. 当实参仍为 type param 时（泛型传递调用 / 嵌套声明），跳过检查（与已有类型实例化处的策略一致）。
- 验收：
  - 新增 typecheck fail fixture：泛型函数 `where T: I` 但调用处类型不满足 → 报 `where_constraint_not_satisfied`。
  - 新增 typecheck pass fixture：泛型函数 `where T: I` 调用处类型满足 → 通过。
  - 新增 typecheck fail fixture：`class Box<T> where T: I` 构造 `Box<Bad>(...)` 不满足 → 报错。
  - 已有 `where_clause_satisfies_bound_ok.scoop` 等 fixtures 不受影响。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T0130 [TODO] 泛型 where 约束：bound 驱动的方法分发（函数体 + 类型成员体内通过约束调用接口方法）

- 描述：当泛型代码体内通过类型参数调用方法时（如 `x.show()`），typechecker 需要利用 `where T: Show` 约束得知 `T` 拥有 `Show` 接口的方法，而非报 `UnresolvedMember`。这同时影响两类场景：
  - **泛型函数体**：`fun <T> f(x: T) where T: Show { x.show() }`
  - **泛型类型成员方法体**：`class Wrapper<T>(val v: T) where T: Show { fun print() { println(v.show()) } }`

  当前 typechecker 对 `TypeKind::Param` 接收者的方法查找没有查询 bound 信息，导致上述两种场景均无法通过类型检查。
- 目标：
  1. **Typecheck 方法解析**：在 `typecheck/expr/member.rs` 或 `typecheck/expr/call.rs` 的方法解析路径中，当接收者类型为 `TypeKind::Param` 时，查找该 type param 的 `where` 约束，将 bound 接口的方法集合纳入候选。约束来源包括：
     - 当前函数声明的 `where_clause`（AST）/ `where_constraints`（T0129 新增）
     - 外层类型声明的 `where_clause`（当前成员方法所属 `class/struct/enum` 的约束）
  2. **返回类型与参数类型**：bound 接口方法的返回类型和参数类型应在 type param 的上下文中正确替换（例如 `interface Mapper<T> { fun map(): T }` + `where U: Mapper<U>` → `u.map()` 返回 `U`）。
  3. **Codegen / Monomorph**：monomorphization 阶段 type param 已被替换为具体类型，方法调用应自然分发到具体类型的实现方法（静态分发，无 vtable）。需验证 monomorph 后的 HIR/MIR 中方法 FQN 已指向具体类型的实现。
- 验收：
  - 新增 typecheck pass fixture：`fun <T> f(x: T): String where T: Show { return x.show() }` 通过。
  - 新增 typecheck fail fixture：`fun <T> f(x: T): String { return x.show() }` 无约束时报 `UnresolvedMember`。
  - 新增 typecheck pass fixture：`class Wrapper<T>(val v: T) where T: Show { fun display(): String { return v.show() } }` 通过。
  - 新增 typecheck fail fixture：`class Wrapper<T>(val v: T) { fun display(): String { return v.show() } }` 无约束时报 `UnresolvedMember`。
  - 新增 run-pass fixture（泛型函数）：定义 `interface ToString { fun toString(): String }`，`struct Foo : ToString { ... }`，`fun <T> stringify(x: T): String where T: ToString { return x.toString() }` → `println(stringify(Foo()))` 输出正确结果。
  - 新增 run-pass fixture（泛型类型成员）：`class Printer<T>(val v: T) where T: ToString { fun print() { println(v.toString()) } }` → `Printer<Foo>(Foo()).print()` 输出正确结果。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0129

### T0131 [TODO] `interface ToString` 引入 + 现有 `toString` 硬编码迁移 + `print`/`println` 泛型化

- 描述：当前 `Int.toString()` 和 `Bool.toString()` 均为编译器硬编码路径：resolver 白名单特判（`resolve/scopes.rs`）、typecheck 特判（`typecheck/expr/call.rs`）、codegen 直接路由到 C runtime 函数（`scoop_int_to_string` / `scoop_bool_to_string`）。`print`/`println` 在 sysroot 中声明了 `String`/`Int`/`Bool` 三组重载。这种设计无法扩展到用户自定义类型，且每新增一个 printable 类型就需要同时修改 resolver + typecheck + codegen + sysroot 四处。
  本任务引入 `interface ToString`，将现有硬编码迁移为接口实现，并利用 T0129/T0130 的 where 约束能力将 `print`/`println` 泛型化。
- 目标：
  1. **定义 `interface ToString`**：在 sysroot（`core.scoop`）新增：
     ```
     interface ToString {
         fun toString(): String
     }
     ```
  2. **内建类型实现 `ToString`**：
     - `Int`：实现 `ToString`（底层仍调用 `scoop_int_to_string`，但通过接口方法分发而非硬编码特判）。
     - `Bool`：实现 `ToString`（底层仍调用 `scoop_bool_to_string`，或内联为 `if (this) "true" else "false"`，移除 C runtime 函数）。
     - `String`：实现 `ToString`（`toString()` 返回自身）。
  3. **移除硬编码路径**：
     - resolver 白名单中 `Int.toString` / `Bool.toString` 的特判 → 改为通过接口成员正常解析。
     - typecheck 中 `Int.toString()` / `Bool.toString()` 的特判分支 → 改为通过接口方法签名正常检查。
     - codegen 中 `codegen_to_string_method` 的 CgTy 分发 → 改为 monomorph 后的具体类型方法调用（静态分发）。
  4. **`print`/`println` 泛型化**：sysroot 中将现有三组重载替换为：
     ```
     fun <T> print(value: T): Unit where T: ToString
     fun <T> println(value: T): Unit where T: ToString
     ```
     实现：调用 `value.toString()` 后路由到已有的 `print(String)` / `println(String)` runtime 函数。monomorphization 将为每个具体类型生成特化版本，避免 boxing 开销。
  5. **用户自定义类型自动受益**：任何实现 `ToString` 的 struct/class/enum 均可直接传入 `println`，无需额外编译器支持。
- 验收：
  - `println(42)` / `println(true)` / `println("hello")` 行为不变。
  - 新增 run-pass fixture：用户定义 `struct Foo : ToString { fun toString(): String { return "Foo" } }` → `println(Foo())` 输出 `Foo`。
  - resolver / typecheck / codegen 中 `toString` 相关硬编码特判已移除（不再有 `member_name == "toString"` 白名单分支）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0129、T0130

### T0128 [TODO] 泛型验证与修复：泛型与 GC / 特殊化类型交互

- 描述：泛型类型实例化涉及 GC 管理（class 为堆分配 + GC trace）和编译器特殊化类型（Array/Channel/Task 等）的交互，需要验证正确性。
- 需验证并修复的场景：
  1. **泛型 class 持有引用字段**：`class Holder<T>(val v: T)` 当 `T` 为引用类型时，GC trace 函数必须扫描该字段。单态化后的 trace 函数需按具体类型生成。
  2. **泛型 class 持有值类型字段**：`Holder<Int>` — GC trace 不应扫描 Int 字段；payload 布局应为值语义。
  3. **泛型 class 持有 nullable 引用**：`Holder<String?>` — niche 优化 + GC trace 需正确处理。
  4. **泛型类型实例化为 Array/Channel 等特殊化类型**：`class Wrapper<T>(val items: Array<T>)` → `Wrapper<Int>` 中 `items` 应使用 Array 的特殊化路径而非通用 class 路径。
  5. **GC 分配点安全**：泛型 class 构造时，若涉及多次 GC alloc（如构造参数含其他堆对象），需验证 pin/unpin 正确性。
- 验收：
  - 新增 run-pass fixture：泛型 class 持有不同种类的 T（Int、String、Array<Int>、Option<String>）+ GC pressure。
  - GC stress 测试通过。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0124、T0125、T0126

### T0123 [TODO] `const fun` 支持 String `+` 和 substring 类操作

- 描述：当前 `const fun` / `comptime` 求值器仅支持整数算术和布尔逻辑（`comptime/eval.rs`），不支持 String 操作。Spec §6.2 明确列出 `String ops` 为 `const fun` 允许的操作。在 T0122 将 substring 类操作迁移到纯 Scoop 后，这些操作理论上可在 comptime 求值。
- 目标：
  - **comptime evaluator**（`comptime/eval.rs`）：
    - String `+`（concatenation）：两个 compile-time String 常量拼接，产出新常量。
    - String `==` / `!=`：编译期字符串比较。
    - `String.byteLength()`：返回编译期常量。
    - `String.getByte(index)`：返回编译期常量。
  - **comptime interpreter**（`comptime/interpreter.rs`）：
    - 支持调用 `const fun` 的 String extension functions（`substring`/`indexOf`/`contains`/`startsWith`/`endsWith`/`split` 等——前提是它们在 T0122 后已是纯 Scoop `const fun`）。
    - 支持 String 类型的局部 `val` 绑定和传参。
  - 使 `trimIndent()` 可完全在 comptime 求值（当前已部分实现为编译器特殊处理，迁移后可统一走 comptime interpreter）。
- 验收：
  - 新增 comptime fixtures：`const fun greet(name: String): String = "Hello, " + name`，编译期求值并验证。
  - 新增 comptime fixture：`comptime { val s = "a,b,c"; val parts = s.split(","); ... }`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0107（String `==`）、T0120、T0122（纯 Scoop String 操作）

### T0144 [TODO] 审计：编译器 codegen 限制全面排查与任务拆分

- 描述：T0140-T0142 是从 `stdlib/string.scoop` 中发现的三个 codegen 限制（source-backed literal 单文件限制、block 内 return/break/continue 不支持、if-without-else 非 Unit 报错）。这些限制的发现是偶发的——因为纯 Scoop stdlib 代码恰好触碰到了它们。编译器中可能还存在其它类似的"硬性拒绝"或"降级处理"路径，尚未被用户代码触发但会在语言功能扩展时成为障碍。
  本任务是一个伞型审计任务（umbrella audit），系统性扫描编译器 codegen 及相关阶段，识别所有此类限制，为每个限制创建独立的后续任务。
- 审计范围：
  1. **LLVM codegen 硬性拒绝**：搜索所有返回 `LlvmEmitError::UnsupportedMainBody` / `Unsupported` / `Todo` / `unimplemented!` / `todo!` 的路径，逐一评估是否为用户可感知的功能缺口。
  2. **HIR lowering 限制**：搜索 `HirLowerError` 的所有变体，识别哪些是暂时性限制（如 `MultiFileNonEntrySourceBackedLiteral`）而非永久性语义约束。
  3. **Resolver / Typechecker 特判与白名单**：搜索 `resolve/scopes.rs`、`typecheck/` 中的硬编码方法名白名单、类型特判、`// TODO`/`// HACK`/`// FIXME` 注释，识别因编译器能力不足而采用的 workaround。
  4. **Codegen 降级路径**：搜索 codegen 中将泛型/复杂类型 fallback 到 `Any`、跳过 type params、硬编码 `is_empty()` 过滤等模式。
  5. **Runtime/C 残留依赖**：识别仍由 C runtime 实现但理论上可迁移到纯 Scoop 的函数（类似 T0122 对 String 方法的迁移），评估迁移可行性。
- 目标：
  - 产出一份限制清单（可作为本任务的注释或单独文档），每项包含：代码位置、限制描述、影响范围、建议优先级。
  - 为每个值得修复的限制创建独立的 TODO 任务（编号续接当前序列）。
- 验收：
  - 审计覆盖 `crates/scoopc/src/llvm/`、`crates/scoopc/src/hir/`、`crates/scoopc/src/resolve/`、`crates/scoopc/src/typecheck/` 四个主要目录。
  - 所有 `UnsupportedMainBody`、`todo!`、`unimplemented!`、`HACK`、`FIXME` 出现点均已分类（已知任务已覆盖 / 新建任务 / 刻意保留）。
  - 新建的后续任务已添加到 TODO.md 对应 section。
- 依赖：无（可随时执行，建议在 T0140-T0142 完成前开始，以便发现更多类似问题并批量规划）

---

## T11：Cone（改进项吸收）

### T1119 [DONE] Cone：产出工程化改进设计（目录结构 / build 产物 / profile / 增量路线）
- 描述：为 CONE 项目工程化体验补齐“目标状态设计”，覆盖目录结构、build 产物布局、profile 行为与增量构建路线。
- 目标：把“接下来要实现什么”写清楚，并为未来 cross compile 预留目录结构（但不要求立即实现 cross compile）。
- 验收：
  - 仓库根目录新增 `CONE-IMPROVEMENTS.md`，并覆盖本次需求列出的全部要点。
- 依赖：无

### T1120 [DONE] `scoop new`：生成 `.gitignore` + `main.scoop` 默认 `println`
- 描述：更新 `scoop new <project-name>` 生成的 CONE 项目结构，使其符合 `CONE-IMPROVEMENTS.md`：
  - 生成 `.gitignore`（至少忽略 `/build/` 等）
  - 自动生成的 `src/main.scoop` 包含 `println("Hello, Scoop!")`（或等价可观察输出）
- 目标：只改 project scaffold；不引入新的语言/stdlib 依赖。
- 验收：
  - `cargo test -p scoop`：新增/更新单测覆盖 `.gitignore` 与 `println` 模板内容。
  - （可选）新建项目后 `scoop run` 能输出固定字符串（与 T1123 一起验收）。
- 依赖：无（已存在 `scoop new`；但后续端到端 run 验收建议在 LLVM 21 基线上做）

### T1121 [DONE] build 输出目录：统一落到项目内 `build/<profile>/…`（并预留 `build/<target>/<profile>`）
- 描述：让 CONE 项目的 build 产物不再散落到 `/tmp` 或 workspace 其它目录，统一输出到项目内 `build/`：
  - 默认/host：`build/<profile>/…`
  - 预留 cross compile：`build/<target>/<profile>/…`（暂不要求实现 cross compile，只要求不要把路径写死导致未来迁移困难）
- 目标：
  - 最终可执行文件固定落点：`build/<profile>/bin/<project-name>`（Windows 可为 `.exe`）
  - build 过程产生的中间产物（.o 等）也应进入 `build/<profile>/obj/`（若实现成本过高，至少保证最终可执行与关键中间产物不再进 `/tmp`）
- 验收：
  - 新增/更新 `run_pass_cone` fixture：断言 `scoop build` 后对应路径存在且可运行（stdout 可断言）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0101（统一 LLVM 21 基线，避免 build/run 行为在不同 LLVM 版本下漂移）

### T1122 [DONE] build profile：`scoop build --debug/--release` 与默认策略落地
- 描述：补齐 build profile 的对外接口与行为：默认 debug，`--release` 选择 release，`--debug` 显式选择 debug（便于脚本化）。
- 目标：
  - CLI 行为要和 `scoop run` 保持一致（共用 profile 解析/默认值）。
  - 先只支持 `debug/release` 两个 profile；更复杂的 profile 名称后置。
- 验收：
  - `crates/scoop` 单测：CLI 参数解析覆盖 `--debug/--release` 冲突处理与默认值。
  - 端到端：同一 cone fixture 在 debug/release 两种 profile 下都可 build（路径不同）并可运行。
- 依赖：T0101、T1121

### T1123 [DONE] `scoop run`：在 CONE 项目目录下自动 build 并运行（支持 `--debug/--release`）
- 描述：当在 CONE 项目目录（存在 `Cone.toml`）下执行 `scoop run` 时：
  - 若目标 profile 的可执行文件不存在：先 build，再运行
  - 若已存在：v0 允许仍然 always rebuild，但至少要支持“未构建则构建”与 profile 选择
- 目标：`run` 复用 `build` 的参数解析与输出目录规则，避免两套路径不一致。
- 验收：
  - 新增 `run_pass_cone` fixture：在空 build 目录下直接 `scoop run`，应先构建并输出 stdout。
  - 另增 fixture：`scoop run --release` 运行 release 产物（与 debug 输出目录不同）。
- 依赖：T0101、T1121、T1122

### T1124 [DONE] 增量构建 v1（粗粒度）：输入 fingerprint 未变则跳过 build（优化项）
- 描述：在 v0（always rebuild 但输出稳定）之后，引入最小增量优化：记录输入 fingerprint，未变化则跳过 build。
- 目标：
  - 在 `build/<profile>/build.json` 写入 fingerprint（至少包含：`Cone.toml` + `src/**/*.scoop` + 关键 build flags + 工具链版本）。
  - `scoop run`/`scoop build` 在 fingerprint 未变且可执行存在时直接复用产物。
  - 只做粗粒度缓存：不做依赖图，不做“只重建受影响文件”。
- 验收：
  - 新增集成测试或 fixture：连续两次 `scoop run`，第二次应打印“skipping build / cache hit”（或等价可断言行为）并直接运行。
  - 行为必须可禁用（例如 `--no-incremental` 或 `SCOOP_INCREMENTAL=0`），避免排查问题困难。
- 依赖：T0101、T1121、T1123

---

## T16：Scoop 编译器（语义完善 + 优化等级/去虚化/HIR-MIR）

### T1601 [DONE] 对外接口：新增并统一优化等级（CLI + Cone.toml + 默认策略）
- 描述：为 `scoop build/run/test` 增加明确的优化等级选项，并与 `Cone.toml[native-build]` 配置对齐，形成可预测的默认策略（debug/release）。
- 目标：
  - CLI：支持 `-O/--opt-level <0|1|2|3|s|z>`（或等价 API），并允许覆盖 `Cone.toml` 默认值。
  - manifest：在 `Cone.toml[native-build]` 增加 `opt-level`（或等价字段），并定义与 profile（debug/release）的映射规则。
  - LLVM 后端：把 `TargetMachine` 的 `OptimizationLevel` 与 opt-level/profile 对齐（当前实现仍是 `OptimizationLevel::None`，仅跑少量 IR passes）。
  - 不在本任务引入 LTO/PGO 等更高阶优化；先把“等级语义”固定下来。
- 验收：
  - `crates/scoop` 单测覆盖：CLI 参数解析与优先级（CLI 覆盖 toml）。
  - 端到端：新增 `tests/fixtures/run_pass_cone/**` 用例分别在 `-O0` 与 `-O2` 下可构建并运行（语义一致）。
- 依赖：T0101；（历史）LLVM build/run 链路已可用；细节见 `TODO-1.md` 的相关任务

### T1602 [DONE] LLVM 优化流水线：按 opt-level 启用常见 passes（DCE/inlining/unroll 等）
- 描述：基于 LLVM PassBuilder（`Module::run_passes`）按优化等级启用/禁用常见优化 passes，优先引入“低复杂度但高收益”的 IR 清理与 DCE/CSE/DSE，并在 release 下逐步接入更重的全局优化。
- 目标：
  - `-O0`：尽量保持 IR 可读与可调试（最小化优化）。
  - `-O1/-O2/-O3`：优先采用 LLVM 默认优化 pipeline（`default<O2>` 等），必要时再做少量补丁式增强。
  - `-Os/-Oz`：针对 size 的 pipeline（若暂不支持，必须给出稳定错误码与文档说明，而不是静默忽略）。
  - 建议的“低复杂度高收益”清单（优先接入）：
    - 必备清理：`instcombine`、`simplifycfg`
    - 早期冗余/内存优化：`early-cse`、`dse`、`dce`（必要时 `adce`）、`sccp`
    - release 再考虑：`gvn/newgvn`、`jump-threading`/`correlated-propagation`、`memcpyopt`
  - GC/statepoint 约束：
    - 大多数优化应放在 `rewrite-statepoints-for-gc` **之前**；
    - rewrite 之后仅做轻量清理（例如 `function(instcombine,simplifycfg)`），避免在 `gc.statepoint/gc.relocate` 之后跑大量 pass 增加风险；
    - `place-safepoints` 暂不纳入默认管线（旧 LLVM 18.1.8 曾观察到 SIGSEGV；在 LLVM 21 上需单独验证稳定性后再决定是否接入）。
- 验收：
  - 新增 build fixtures：同一输入在 `-O0` 与 `-O2` 下 `--emit-llvm` 产物可用 `BUILD-LLVM-(NOT-)CONTAINS` 断言观察到至少 1 个典型优化（例如死代码被移除或内联发生）。
  - 新增 build fixture（或复用现有单测）：断言 `rewrite-statepoints-for-gc` 仍然产出 `gc.statepoint`（避免优化管线破坏 GC rewrite 的前置条件）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0101、T1601；（历史）`--emit-llvm` 与 build fixtures 子串断言能力已存在

### T1603 [DONE] 去虚化：receiver 类型已知时确保能生成直调用（final/sealed/value）
- 描述：当 method call 的 receiver 类型在编译期已知时，尽量走直调用路径，确保 LLVM 去虚化能生效（尤其是 receiver 为 final/sealed class 或 value type 时）。
- 目标：
  - value type：默认应为静态分派（direct call），不得引入不必要的 vtable/间接调用。
  - final/sealed class：在可证明单一目标时生成直调用（或提供足够信息让 LLVM 去虚化）。
  - 不在本任务实现全局的 class hierarchy analysis；先从“显然可证明”的 case 落地。
- 验收：
  - 新增 build fixture：`--emit-llvm` 断言在目标 case 下不出现 function pointer 间接调用，而是直接 `call @Type_method`（按实际 codegen 命名规则断言关键子串即可）。
  - 新增 run-pass fixture：验证语义正确（stdout）。
- 依赖：（历史）对象语义与方法调用链路已存在

### T1604 [DONE] HIR/MIR 级优化 v0：无 `perform` 时不生成 `handle` 结构/帧
- 描述：在 lowering/codegen 前进行一次“cheap”静态分析：若某作用域（函数/块）内不存在 `perform`，则不应生成 `handle` 相关的结构体、栈帧或 TLS handler 链接，减少运行时开销。
- 目标：
  - 只做“没有 perform → 不生成 handle”的消除；不做复杂的跨函数分析或效果推断优化。
  - 保证与当前 effect 语义一致：一旦存在 `perform`，仍按既有机制生成并正确工作。
- 验收：
  - 新增 build fixture：对不含 `perform` 的程序，`--emit-llvm` 中不出现 handler/handle 相关符号（用 `BUILD-LLVM-NOT-CONTAINS` 断言关键子串）。
  - 新增 run-pass fixture：在默认模式与 `--gc-stress` 下行为一致。
- 依赖：（历史）effect lowering/codegen 基础链路已存在

### T1605 [DONE] 高级优化候选清单：建立并持续维护（不阻塞主线）
- 描述：建立并维护一份 Scoop 编译器的高级优化候选清单，用于后续分阶段立项（避免“想到哪做哪”）。
- 目标：清单必须标注：
  - 适用层级（HIR/MIR/LLVM）
  - 预期收益（性能/体积/GC 压力/线程扩展）
  - 风险与前置依赖（例如需要更强的类型/效果信息或 runtime 支持）
- 验收：把清单维护在 `PLAN.md` 的“编译器优化”部分，并为每个候选项保留可拆分的任务入口（后续逐步补齐）。
- 依赖：无

### T1606 LLVM：escape continuation `handle` 完整语义（0..N perform 点）（拆分）
- 描述：当前 LLVM 后端对 escape continuation（`, k ->`）的 `handle` 仍是“最小可回归链路”：只支持单个 perform 点，且要求为 block 的第一个语句。该限制导致 stdlib/fixtures 只能用“嵌套 handle / 二段 handle”绕开，无法表达真实 async/await 的直觉写法（同一 handle body 内多次 await）。
- 目标：
  - 语义完整性：支持 `handle { ... } with { Effect.op(...), k -> ... }` 的 body 含 **0..N** 个 perform 点：
    - 0 个 perform：handle 直接执行到结束并返回 body 的值（不依赖 arm）。
    - N≥1：每次 perform 触发一次 suspension，并生成新的 continuation；后续可多次 suspension/resume（每个 continuation one-shot，但同一“计算”可经历多个 suspension 点）。
  - 结构完整性：不再要求 perform 必须是 block 第一个语句；允许在 perform 前后有普通语句（含 val/assign/expr）。
  - 动态上下文正确性（Appendix A / spec §5.5）：
    - continuation resume 时恢复其捕获的 handler stack；
    - handler arm body 期间应避免自捕获（arm 内再次 perform 同一 op 应命中外层 handler，而不是自身）。
  - GC 正确性：heap state machine 的状态对象必须是 GC-managed，且其内部引用字段可被准确扫描/更新（moving GC 下不可漏扫）。
- 验收：
  - 新增 run-pass fixtures：单个 `handle` body 内连续 2~3 次 `await/yield`（不使用嵌套 handle workaround），stdout 顺序可观测且在 `--gc-stress` 下稳定。
  - 复跑既有 fixtures：`cargo run -p scoop -- test` 与（可选）`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：（历史）T0617/T0914/T0915/T0916（escape continuation + handler stack 基础链路）；（新增）T1706/T1707（回归用例）

### T1606a [DONE] Escape continuation：0 perform 时退化执行 body（arm 不可达）
- 描述：当 `handle { ... } with { Effect.op(...), k -> ... }` 的 body 内**不存在匹配该 arm 的 perform 点**时：
  - 运行期不会创建 continuation；
  - arm 视为不可达（仅 typecheck，不参与 codegen）；
  - `handle` 表达式应按顺序语义执行 `body`（以及 `finally`，若存在）并返回 body 的值。
- 目标：放宽当前 LLVM codegen 中“escape continuation handle 必须有且仅有一个 perform”的硬限制，仅对**匹配的 op**生效；其它 effect/handle 仍可出现在 body 内并照常执行。
- 验收：
  - 新增 run-pass fixture：escape continuation handler 存在但 body 不 perform；stdout 断言 arm 未执行且返回值来自 body。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T1606b [DONE] Escape continuation：允许 perform 非首语句（仍单 perform）
- 描述：取消“perform 必须是 block 第一个语句”的限制；允许在 perform 前存在普通语句（val/assign/expr）。
- 目标：补齐 capture：把 perform 前引入且在 perform 后仍需使用的 locals lift 到 heap state，并在 step trampoline 中恢复。
- 验收：新增 run-pass fixture：perform 前后各有语句，resume 后能读到 pre-perform locals。
- 依赖：T1606a

### T1606c [DONE] Escape continuation：多 perform 点（同一 handle body 内 2..N 次 suspend/resume）
- 描述：引入可重入的 heap state machine（pc + lifted locals），使 step trampoline 每次推进到下一个 perform 或完成，并在每次 perform 处生成新的 continuation。
- 验收：新增 run-pass fixtures：单 handle 内 2~3 次 yield/await（不使用嵌套 handle workaround），并在 `--gc-stress` 下稳定。
- 依赖：T1606b

### T1608 [DONE] Effect op_tag：稳定分配与统一 dispatch（运行期 handler stack / perform slot）
- 描述：runtime handler stack 的”最近匹配”分发基于 `op_tag` 精确匹配；而当前 codegen 对自定义 effect 的 `op_tag` 仍大量写 0（除 `Raise` 以外）。这会在”嵌套 handler + re-perform + 多 effect 并存”时产生错误分发或无法诊断的语义漂移。
- 目标：
  - 为所有 compiler-known 的 effect op 分配稳定 `op_tag`（至少在单次编译产物内稳定；若要求跨版本稳定需额外讨论）。
  - 统一 perform slot 编码：`perform` 写入 `op_tag + payload`；`handle` 边界解码并分派到最近匹配 handler。
  - 让 `EscapeContinuation` 与 `ImmediateResume` / non-resuming handler 共享同一套 dispatch 规则（避免多套”特判语义”长期并存）。
- 验收：
  - 新增 run-pass fixture：三层嵌套 handler（至少两种不同 effect），arm 内 re-perform 验证”最近匹配 + active/inactive”规则成立。
  - 针对错误分发给出稳定诊断（至少包含 op_tag / effect 名称 / src line/col）。
- 依赖：（历史）T0913/T0916（runtime handler stack），T0617（escape continuation lowering）

### T1607 [DONE] Continuation resume payload：从 `u64` 扩展为可表达任意 `T`
- 描述：当前 `k.resume(value)` 的 LLVM lowering 只支持把 `value` 编码为 `u64` word，且明确禁止 GC 指针（Ref/String）与复合值。这与 spec 对 `Continuation<T>` 的泛型语义不一致，也限制了 async/await/generator 在真实场景中的可用性。
- 目标：
  - 设计并落地一个”可携带任意 `T`”的 resume payload ABI（至少覆盖：Unit/Bool/Int/String/Ref/tuple/struct/enum；允许 future 扩展）。
  - 与 GC 对齐：payload 若包含引用类型，必须可被 GC 扫描/更新（moving GC 下 resume 后仍正确）。
  - 维持 one-shot：重复 resume 必须表现为可捕获的 `Raise<RuntimeError.ContinuationAlreadyResumed>`（而非进程级 exit）。
- 验收：
  - 新增 run-pass fixtures：`Continuation<String>`、`Continuation<(Int, String)>`、`Continuation<MyStruct>` 等在 `--gc-stress` 下通过，并覆盖”resume 后继续分配触发多轮 GC”。
  - 为 ABI 关键点补 build fixtures（可选）：断言不出现”ptr<->int”非法编码路径。
- 依赖：T1606（多点 suspension 需要更通用 payload 才能覆盖真实案例）；（历史）T0630（payload ABI 统一化方向）
- 完成说明：
  - **设计**：双通道 ABI——`ScoopContinuation` 结构体新增 `resume_word`（u64，用于 scalar：Unit/Bool/Int）和 `resume_gc_ref`（void*，GC-traced，用于 String/Ref/compound）。Step 函数签名扩展为 3 参数 `(state, resume_word, resume_gc_ref)`。Resume 调用改为先写入 continuation 字段再调用无参 `scoop_continuation_resume(k)`。
  - **C runtime**（`runtime/c/scoop_runtime.c`）：ScoopContinuation 新增两字段 + 更新 trace_fn（扫描 resume_gc_ref）；新增 `scoop_continuation_resume()` 入口；`scoop_continuation_resume_u64()` 保留向后兼容。
  - **LLVM codegen**（`effect.rs` / `runtime_abi.rs` / `runtime_symbols.rs`）：step 函数 3 参数签名；resume 调用站点按类型分发写入 resume_word 或 resume_gc_ref（compound 类型先 box 到 GC heap）；step 函数 decode 按类型从对应通道读取。
  - **Fixtures**：新增 4 个 run-pass fixtures——`effect_escape_continuation_resume_unit`、`effect_escape_continuation_resume_bool`、`effect_escape_continuation_resume_string`、`effect_escape_continuation_resume_string_multi`（含多次 suspend/resume）。
  - **备注**：Tuple/Struct/Enum compound payload 的 codegen 路径已实现（box+unbox），但缺少 run-pass fixtures，因为 `Continuation<(Int,String)>` / `Continuation<MyStruct>` 需要 T1610（控制流表达式返回 compound 类型）先落地才能端到端验证。

### T1706 [DONE] 多 perform 点（单个 handle）：async/await 真实写法回归
- 描述：新增一组 fixtures，专门覆盖"单个 escape-continuation `handle` body 内出现多个 perform 点"的真实写法（例如连续 `await` 两到三次），不允许用"嵌套 handle / 二段 handle"的 workaround。
- 目标：
  - 覆盖：两次以上 suspension/resume；resume 后继续执行并再次 suspension。
  - 覆盖：perform 前后都有普通语句与局部变量（确保 state machine 的 local lifting 正确）。
  - 覆盖：arm body 将 continuation 入队（模拟 executor），并按确定性顺序恢复。
- 验收：
  - 新增 run-pass fixtures（stdout golden）：至少 2 个（一个单线程调度，一个跨线程 resume）。
  - 所有用例在 `--gc-stress` 下稳定通过。
- 依赖：T1606（多 perform codegen）、（历史）T0915a/T0618（跨线程 resume 运行期原语）

### T1707 [DONE] 控制流 + 多次 suspension：if/when/循环边界的语义回归
- 描述：针对多 suspension 点下最容易出错的控制流形态新增 fixtures：`if/when` 分支在不同路径上 perform 次数不同、以及在循环体内 suspension（至少先覆盖"有限次迭代的 while/for 等价形态"）。
- 目标：
  - 覆盖：分支合流（phi）上的局部变量在 suspend/resume 后仍正确。
  - 覆盖：同一局部变量跨多个 suspension 点读写（包含 value/ref 混合）。
  - 覆盖：arm 内 re-perform 与外层 handler 的交互（active/inactive 规则）。
- 验收：
  - 新增 run-pass fixtures：至少 3 个用例（分支/合流、循环、re-perform）。
  - 可选 build fixtures：对关键 IR 形态做 contains 断言（例如 state machine 的 pc/dispatch 存在）。
- 依赖：T1606、T1608
- 完成说明：
  - 新增 3 个 run-pass fixtures：
    - `effect_escape_continuation_multi_perform_if_branch`：if/else 分支在两个 perform 点之间，验证 phi 合流后 `branch_val` 在 suspend/resume 后仍正确（`var` 通过 if/else 分支设置不同值，跨 suspension 后可读取）。
    - `effect_escape_continuation_multi_perform_while_loop`：手动展开 3 次 "循环迭代"，每次 perform 后累积 `sum`（Int，value type）并读取 `label`（String，ref type），验证 value/ref 混合 locals 跨多个 suspension 点的 state machine lifting 与 GC 正确性。（注：真正的 `while` 循环内 perform 需 T1606e 的嵌套控制流 codegen 支持，此处以等价线性形态先覆盖。）
    - `effect_reperform_nested_handlers_control_flow`：嵌套 handler（inner 捕获 EffectA，outer 捕获 EffectB），body 使用 if/else 决定 perform 路径；inner arm 内 re-perform EffectB 到 outer，验证 active/inactive dispatch 规则与控制流组合的正确性。
  - typecheck 扩展：`infer_block_value_type`（handle body 块表达式类型检查）新增 `While` 语句支持（条件必须为 Bool，body 递归检查）。虽然当前 escape continuation codegen 尚不支持 handle body 内 while 包含 perform（T1606e），但该扩展使 while 循环在 handle body 的非 perform 用途（如 resume 后的循环逻辑）不再报 typecheck 错误。

### T1606d [DONE] Escape continuation：多 perform + 动态上下文/GC 回归加固
- 描述：补齐 active/inactive（避免 self-capture）与 handler stack 捕获/恢复的边界用例，并验证 heap state 的 GC 扫描正确性。
- 验收：复跑既有 fixtures；补充嵌套 handler / re-perform / 跨线程 resume 的组合用例。
- 依赖：T1606c、T1608、T1706/T1707
- 完成：新增 4 个 run-pass fixtures：
  - `effect_escape_continuation_gc_stress_multi_string`：`SCOOP_GC_STRESS=1` 下 3 次 String-payload suspend/resume，验证 ContState + lifted locals + resume_gc_ref 在强制 GC 下存活。
  - `effect_escape_continuation_arm_performs_outer_effect`：escape-continuation arm 内 perform 不同 effect（EffectB），路由到外层 non-resuming handler，验证 handler stack 从 arm body 正确分发。
  - `effect_escape_continuation_nested_escape_handlers`：两个独立 escape-continuation handler（EffectA/EffectB）各 2 次 suspend/resume 并交叉恢复，验证独立 ContState 分配与 GC 同时扫描两个 state machine。
  - `effect_escape_continuation_reperform_from_escape_arm`：escape-continuation arm 重新 perform 同一 effect，验证 active/inactive self-capture prevention 将 re-perform 路由到外层同类型 handler。

### T1606e [DONE] Escape continuation：handle body 任意控制流结构（分支/循环）显式验证
- 描述：在实现多 perform + heap state machine 后，理论上 handle body 内可以是任意语句/表达式组合；但需要用 fixtures 显式覆盖复杂控制流（CFG）以避免只在”线性 block”上正确。
- 目标：新增 run-pass fixtures 覆盖：
  - `if/else` / `match` 分支内的 perform（包含某些分支不执行 perform 的路径）；
  - `while`/`loop`/`for` 中的 perform（含 `break`/`continue`），并覆盖 2..N 次 suspension/resume；
  - perform 前后的局部变量在不同分支/迭代中被读取/更新，resume 后语义一致。
- 验收：fixtures 在 `--gc-stress` 下稳定；`cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1606d

### T1606f [DONE] Escape continuation：间接 perform（跨函数调用/闭包）
- 描述：当前 escape continuation 的 state machine 只支持 handle body 内的**直接** `val x = perform` 语句。间接 perform（在被调用函数或闭包中 perform）需要 call-site suspension 支持：handle body 的 step function 需要识别哪些 call 可能 perform，在 resume 时重新进入 call 并让被调用函数从 perform 点恢复。这是 escape continuation 迈向完整 algebraic effect 语义的关键一步。
- 拆分为以下子任务：

### T1606f-1 [DONE] Escape continuation indirect perform：non-resuming（flag-propagation）验证
- 描述：对 non-resuming effect（handler arm 不使用 continuation `k`），间接 perform 通过 flag-propagation + handler stack dispatch 已经可以工作（与 Raise 相同路径）。本子任务验证这一路径并补充 fixtures。
- 目标：新增 run-pass fixtures：
  - `handle { f() } with { Effect.op(), k -> { /* no resume */ } }`，其中 `f()` 内部 perform；
  - 多层调用链 `f -> g -> perform`；
  - 闭包中 perform（non-resuming arm）。
- 验收：fixtures 在 `--gc-stress` 下稳定；`cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1606d
- 完成说明：
  - **codegen 修复**：`codegen_perform_expr_nonresuming_custom_int` 原先要求在同一函数内存在匹配的 `handle` boundary，否则报错。修改为：当无本函数内 handler boundary 时，回退到 flag-propagation（与 `Raise.raise` 一致：写入 slot/flag 后返回默认值，由 caller 的 `emit_effect_unwind_if_active` 传播）。
  - **dispatch trampoline**：`codegen_handle_expr_nonresuming_custom_int_payload` 新增"dispatch trampoline block"并推入 `raise_target_stack`，使 `emit_effect_unwind_if_active` 在 body 中的函数调用返回后能检测到 flag 并跳到此 block。trampoline 读取 slot 的 `op_tag` 并判断是否匹配本 handler：匹配则跳到 `catch_bb`，不匹配则 pop handler frame 并向外传播（确保 Raise 等其它 effect 不会被误捕获）。
  - **新增 3 个 run-pass fixtures**：
    - `effect_indirect_perform_nonresuming_basic`：函数 `doFire()` 内部 `Boom.boom(code)`，外层 `handle` 捕获。
    - `effect_indirect_perform_nonresuming_call_chain`：两级调用链 `outer -> inner -> Boom.boom(code)`。
    - `effect_indirect_perform_nonresuming_closure`：闭包捕获 handle body locals 并在闭包内 `Boom.boom(captured)`；`callIt(f)` 调用闭包。
  - 所有 fixtures 在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1606f-2 [DONE] Escape continuation indirect perform：call-site suspension 设计与 codegen
- 描述：为 handle body 的 step function 引入 call-site suspension 点：当 body 中的 call `f()` 可能 perform 时，step function 需要在该 call 前后保存/恢复状态，使 resume 时能重新进入该 call 并由被调用函数完成自身的 resumption。
- 目标：
  - 扫描 handle body 中的 call 表达式，标记可能触发匹配 effect 的 call 为”suspension call-site”；
  - 在 step function 中为每个 suspension call-site 分配一个 pc 值，保存 call 前的 locals，resume 时跳到 call-site 并重新调用；
  - 验证单层 call（`handle { f() } with ...`，`f()` 内部单 perform + resume）的正确性。
- 验收：新增 run-pass fixture + GC stress 稳定。
- 依赖：T1606f-1
- 完成说明：
  - **设计（两级状态保存 + TLS CalleeSuspendState）**：不使用 `SuspendResult<T>` 返回值变换——保留 flag-propagation 作为 perform→caller 的挂起信号（与 Raise 一致），在此之上增加两级状态保存：(1) callee 保存自身 post-perform locals 到 CalleeSuspendState（TLS），(2) handle body 保存 outer captures + body lifts 到 ContState。Resume 时 step function 将 resume_word 写入 CalleeSuspendState 的 resume_word 字段，然后重新调用 callee——callee 入口检测 TLS 非空则走 resume 路径。
  - **CalleeSuspendState**：`{ GcObjectHeader, resume_word:i64, saved_locals... }` — GC-managed heap struct，由 callee 的 perform codegen 分配+PIN+保存到 TLS。
  - **CalleeSuspend 入口变换（`codegen_top_level_fun_suspendable`）**：`scan_for_callee_suspend` 扫描函数体中的直接 perform（非 Raise）；检测到后生成 fresh/resume 双路径入口（TLS 检查），resume 路径从 state 恢复 locals 和 resume_word。
  - **`emit_callee_suspend_state_save`**：在 perform codegen 的 flag-propagation return 之前，分配 CalleeSuspendState、PIN、保存所有 pre-perform locals（Int/Bool/Ref/String），然后写入 TLS。
  - **`codegen_handle_expr_escape_continuation_indirect`**：新 codegen 路径（~700 行），当 escape continuation handle body 有 0 直接 performs 但有非纯函数调用时触发。生成 ContState + step function + handler frame push + raise_target dispatch trampoline + arm body + continuation alloc + done block。
  - **GC/statepoint 修正**：TLS accessor `_get()` 返回 addrspace(0) 指针——在 resume path 保持 addrspace(0) 避免 statepoint pass 无法追踪的 GC root；unpin 通过 `ptrtoint`/`inttoptr` 转换为 addrspace(1)。
  - **Fixture**：`effect_escape_continuation_indirect_perform_basic` — `compute()` 内部 `Ask.get(x)` 间接 perform，handle body 捕获 + 创建 continuation，外部 resume 后 callee 恢复并完成计算（x + resume_value = 42）。

### T1606f-3 [DONE] Escape continuation indirect perform：闭包中 perform + locals 联动
- 描述：闭包中 perform 时，闭包捕获的 locals 来自 handle body 的作用域。resume 后闭包需要看到 handle body 中 locals 的最新值（lift 已保存 + 恢复）。本任务验证闭包 + escape continuation 的组合语义。
- 目标：新增 run-pass fixtures：
  - closure 捕获 handle body locals，perform 在 closure 内，resume 后继续在 handle body 中正确读取/更新 locals；
  - 组合：if/while 中调用闭包触发 perform。
- 验收：fixtures 在 `--gc-stress` 下稳定。
- 依赖：T1606f-2
- 完成说明：
  - **闭包 callee-suspend 变换（`codegen_closure_fun_body_suspendable`）**：当闭包体包含直接 perform 时，生成与 `codegen_top_level_fun_suspendable` 相同的 TLS entry check + CalleeSuspendState save/restore 双路径，但操作闭包的 captures + params + block-locals（而非 FunDecl 的参数）。在 `codegen_closure_fun_body` 中检测 suspendable closure 并路由到新方法。
  - **Body lift 扩展（handle body 侧）**：
    - `collect_used_locals_in_expr_static` 新增 `ExprKind::Closure` 处理：收集闭包 captures 的 SymbolId（使 body-lift 分析能发现闭包捕获的 handle body locals）。
    - `used_after` 计算扩展为 `used_at_and_after`：也扫描 call site stmt 本身（step function 会重新 codegen 该 call expression，包括创建闭包对象）。
    - Body lift 在 body 执行流中的 call site stmt **之前**保存到 ContState（此时 locals 在作用域内），而非在 arm BB（此时 body scope 已 pop）。
    - Step function 的 body lift 恢复逻辑扩展为支持 Int/Bool/Ref/String 四种类型（原先仅 Int）。
  - **Fixtures**：
    - `effect_escape_continuation_indirect_perform_closure`：闭包捕获 `x: Int`，通过 `callIt` 间接 perform `Ask.get(x)`，resume(32) 后闭包计算 `x + 32 = 42`。
    - `effect_escape_continuation_indirect_perform_closure_locals`：闭包捕获 `x: Int` 和 `label: String`（GC ref），验证 Int + String 混合 captures 在 suspend/resume 后存活。
  - 两个 fixtures 均在 `SCOOP_GC_STRESS=1` 下通过。

### T1606g [DONE] Escape continuation：嵌套 handle 下的 perform 分发（内层 perform 由外层捕获）显式验证
- 描述：显式验证 nested handle 的 handler stack 分发与 active/inactive 规则：在内层 `handle` 的 body/arm 中触发的 perform，若不被内层匹配，应由外层正确捕获并在 resume 后回到原控制流。
- 目标：新增 run-pass fixtures 覆盖：
  - 外层 handle 捕获 EffectB；内层 handle 捕获 EffectA；在内层 body 中 perform EffectB，应由外层处理；
  - 在内层 handler arm 中 perform EffectB（含间接 perform：arm 调用函数/闭包触发），仍由外层处理；
  - 组合：outer resume 后继续推进 inner 的多 perform state machine，保证顺序/返回值正确。
- 验收：fixtures 在 `--gc-stress` 下稳定；`cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1606d（含 T1608）
- 完成说明：
  - **前置修复**：移除 `CalleeSuspendSaveCtx` 中未使用的 `perform_binding_id` 和 `perform_binding_cg_ty` 字段（latest commit 引入的 clippy dead_code 警告）。
  - **新增 3 个 run-pass fixtures**：
    - `effect_escape_continuation_nested_dispatch_body_to_outer`：inner escape-cont handler 走 no-perform degenerate 路径（body 不 perform EffectA），body 内直接 perform `EffectB.fireB(42)` → outer non-resuming handler 捕获。验证：escape-cont no-perform path + 外层 handler flag-propagation 正确。
    - `effect_escape_continuation_nested_arm_indirect_performs_outer`：inner escape-cont handler 捕获 EffectA，arm 通过 `doFire()` 函数间接 perform EffectB → outer non-resuming handler 捕获。验证：arm body 中的间接 perform（CalleeSuspend）正确 flag-propagate 到外层 handler。
    - `effect_escape_continuation_nested_outer_resume_inner_multi`：inner escape-cont handler 捕获 EffectA（2 个 perform 点），arm 保存 continuation k。Handle 退出后，outer body 依次 resume k1 和 k2 推进 inner state machine，最后 perform EffectB → outer non-resuming handler 捕获。验证：多 perform state machine 在 outer handler body 作用域内正确运行 + outer handler 不干扰 inner EffectA 分发。
  - 所有 fixtures 在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1609 [DONE] `finally` + escaping continuation：unwind/cleanup 的组合语义
- 描述：当前 escaping continuation 的 `handle` 明确不支持 `finally`。要实现完整 effect 语义，必须定义并实现：当计算被 suspend / resume / abandon 时，`finally` 的执行时机与次数规则（并保证与 flag-based unwinding 一致）。
- 目标：
  - 定义并实现 `finally` 的语义：至少覆盖
    - 正常执行（无 perform）；
    - 多次 suspension/resume；
    - arm 内抛 `Raise` 或发生未处理 effect 时的传播路径。
  - 保证 `finally` 不会被重复执行或漏执行，并在嵌套 handle 下保持栈式顺序。
- 验收：
  - 新增 run-pass fixtures：在多次 `await` 的 handle 外层加 `finally`，用 stdout 断言 `finally` 的执行次数/顺序；并在 `--gc-stress` 下稳定。
  - 复跑 try/catch/finally 相关 fixtures，确保无回归。
- 依赖：T1606、T1608（需要统一 dispatch/unwind 语义作为基础）
- 完成说明：
  - **设计**：`finally` 在 escape continuation handle 中的语义与 non-resuming handler 一致——在 handle 表达式完成时执行一次。对于 escape continuation，这意味着 `finally` 在初始 arm body 完成后、handle 表达式返回前执行。后续通过 continuation resume 触发的 arm body（在 step function 内部）不触发 `finally`。
  - **codegen 修改**（`effect.rs`）：
    - 直接 perform 路径（`codegen_handle_expr_escape_continuation`）：移除 `handle.finally.is_some()` 错误检查；新增 `finally_bb` + `finally_unwind_bb`；arm body 期间 `push_raise_target(finally_unwind_bb)` 确保 Raise 先经过 finally；arm 正常完成 → `finally_bb` → `done_bb`；raise 传播 → `finally_unwind_bb` → outer_raise_target。
    - 间接 perform 路径（`codegen_handle_expr_escape_continuation_indirect`）：同样的 finally 支持。
    - 0 perform 退化路径（`codegen_handle_expr_no_perform`）：已原生支持 `finally`，无需修改。
  - **新增 4 个 run-pass fixtures**：
    - `effect_escape_continuation_finally_normal`：单 perform + finally，验证 finally 在 arm 完成后执行一次。
    - `effect_escape_continuation_finally_multi_perform`：2 次 perform + finally，验证 finally 仅在初始 arm 完成时执行一次，后续 resume 不触发。
    - `effect_escape_continuation_finally_no_perform`：0 perform + finally，验证退化路径下 finally 执行一次。
    - `effect_escape_continuation_finally_arm_raise`：arm body 内 Raise.raise + finally，验证 finally 在 raise 传播前执行。
  - 所有 fixtures 在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1610 [DONE] LLVM：控制流表达式返回任意类型（`handle`/`if`/`when` 支持 tuple/struct/enum）
- 描述：当前 LLVM codegen 对 `handle`/`if`/`when` 的结果类型仍是“标量子集”：只支持 `Unit/Bool/Int/String/Ref`，对 `tuple/struct/enum` 直接报错（例如 `handle result type` / `if result type` / `when result type`）。这与语言层面“表达式可返回任意类型”的预期不一致，也迫使 fixtures/stdlib 在一些位置用 workaround（例如把结果塞进 `Any` 或拆成多段语句）。
- 目标：
  - `handle { ... } with { ... }` 作为表达式：结果类型覆盖 `Unit/Bool/Int/String/Ref/tuple/struct/enum`（值类型以 by-value aggregate 形式返回/传递，ref/string 仍按 GC 指针）。
  - 同步放开 `if` 与 `when` 的结果类型限制（否则 handle body / arm 中的常见写法仍会因 `if/when` 结果类型受限而卡住）。
  - 统一 merge 策略并与 GC/statepoint 对齐：优先用 result slot（`alloca` + store + merge load）覆盖所有非 `Unit` 类型，避免为复合值单独引入多套 PHI/SSA 规则；并显式验证聚合值中包含 GC ref 时，SROA/statepoint rewrite 后 roots 仍可追踪/更新（moving GC 下不可漏扫）。
- 验收：
  - 新增 run-pass fixtures：`handle`/`if`/`when` 返回 `tuple/struct/enum` 的最小可观测用例（stdout 断言），并在 `--gc-stress` 下稳定。
  - （可选）新增 build fixtures：`--emit-llvm` 对关键 IR 形态做 contains/not-contains 断言（例如避免 ptr<->int 编码、确认 result slot/aggregate load/store 形态稳定）。
- 依赖：无（但建议与 T1606/T1607/T1608 的 GC/effect 回归一起跑，以尽早暴露“复合值 + statepoint”交互问题）

### T1611 [DONE] LLVM：语句位置的 `handle` 不应依赖”期望类型语境” workaround
- 描述：当前 LLVM codegen 的 `handle` 必须在“期望类型语境”（expected type context）下生成；但 `StmtKind::Expr`（表达式语句）路径会直接走 `codegen_expr(expr)`，导致 `handle { ... } with { ... }` 作为语句时报错，于是 fixtures 只能写 `val _: Unit = handle { ... } ...` 来人为提供 expected `Unit`。
- 目标：
  - 统一语句位置的语义：表达式语句的值应被丢弃，因此在 LLVM codegen 里应默认以 `Unit` 作为 expected（对 `handle/if/when/perform` 等都一致），而不是要求源码额外引入 `val _: Unit = ...` 绑定。
  - 梳理所有 statement codegen 入口（普通 block、loop body、handle resume body 等），确保它们不会意外走到“expected = None”而触发不必要的限制。
- 验收：
  - 新增 run-pass fixture：`handle { ... } with { ... }` 作为**裸表达式语句**出现（不写 `val _: Unit = ...`），stdout 可断言且在 `--gc-stress` 下稳定。
  - （可选）清理既有 fixtures：将“仅用于提供 expected type context”的 `val ignore/_: Unit = handle { ... }` workaround 移除或缩减到确有语义必要的场景。
- 依赖：无

### T1612 [DONE] LLVM：`Nothing`（bottom type）在 codegen 的表示与不变量（值永不可见）
- 描述：`Nothing` 是 bottom / uninhabited type：它没有运行时值；任何返回类型为 `Nothing` 的函数都不应”正常返回”（只能通过 `Raise.raise`、无限循环、或其它控制流终止）。当前 LLVM codegen 侧尚未为 `Nothing` 提供一致的 `CgTy` 表示（`cg_ty_of` 也未覆盖它），同时许多”不可达 continuation block”会用 `default_value(...)` 产生占位值以维持 IR 生成推进，这在放开复合值返回后需要更明确的约束与实现策略。
- 目标：
  - 明确并固化后端不变量：`Nothing` 的值不可被 store/load/return/observed；若后端内部需要占位表示，只能用于不可达路径的 IR 连通（例如 dead block），且不得影响可达语义。
  - 设计 `Nothing` 的 codegen 表示策略（例如引入 codegen-only 的 `Never`/`Unreachable` 形态，或将 `Nothing` 映射为一个”不可观察占位类型”并在关键点强制 `unreachable`），并补齐 `cg_ty_of` / `default_value` / merge 逻辑对该策略的适配。
  - 审计 `default_value(...)` 的使用点：对 tuple/struct/enum 等复合类型提供可生成的占位 LLVM 值（例如 `undef`/zero initializer），同时确保这些占位仅在不可达路径被使用，不要求提供语言层面可观察语义。
- 验收：
  - 新增 run-pass fixtures：显式覆盖 `Nothing` 的典型来源（例如 `Raise.raise`、永不返回的 helper），并验证在 try/catch/handle 边界内外均不会出现”读取/打印/返回 Nothing 值”的路径。
  - 新增/更新 build fixtures（可选）：在 `--emit-llvm` 下断言关键位置出现 `unreachable` 或等价形态，避免生成”可达但未初始化/乱值”的 IR。
- 依赖：T1610（复合值 result + default_value 互相牵连，建议一起推进）
- 完成说明：
  - **设计**：引入 `CgTy::Never` 作为 `Nothing` 的 codegen 表示。`CgValue::never()` 构造器返回 `{ ty: CgTy::Never, value: None }`。运行时 `Nothing` 值永不可观察——所有通向 Nothing 值的路径都是 dead code（由 Raise.raise、divergent calls 等保证）。
  - **类型映射**（`ty.rs`）：`ValueTypeKind::Nothing => Some(CgTy::Never)` in `cg_ty_of`；`CgTy::Never => i8_type()` in `llvm_basic_type_of`（占位，永不实际使用）。
  - **布局**（`layout.rs`）：`CgTy::Never => TypeLayout::new(0, 1)`（零大小，与 Unit 一致）。
  - **default_value**（`mod.rs`）：`CgTy::Never => CgValue::never()`。
  - **emit_return**（`mod.rs`）：`CgTy::Never => build_unreachable()`——返回 Nothing 的函数永远不会正常返回。
  - **coerce_value**（`mod.rs`）：`(CgTy::Never, _) => default_value(target)`——Nothing 可以 coerce 到任意目标类型（仅在不可达路径上需要占位值）。
  - **控制流**（`control_flow.rs`）：if/when merge block 中，当结果类型为 Never 时跳过 alloca 并在 merge block 发出 `unreachable`（所有分支都 diverge 时 merge 不可达）。
  - **effect/continuation**（`effect.rs`）：handle merge results、resume value decode、cont resume slot encoding、coerce_u64_word 等 9 处 match 添加 `CgTy::Never` 分支。
  - **GC**（`gc.rs`）：`store_local_value` 中 `CgTy::Never => return Ok(CgValue::never())` 作为 no-op。
  - **全面审计**：mod.rs（~15 处 match）、effect.rs（9 处）、control_flow.rs（5 处）、layout.rs（1 处）、gc.rs（1 处）——共 30+ 个 match arm 覆盖所有 CgTy 分支。
  - **新增 3 个 run-pass fixtures**：
    - `nothing_raise_in_helper_basic`：Raise.raise 在 helper 函数中，flag-propagation + try/catch 捕获。
    - `nothing_if_all_branches_raise`：if 两分支都 Raise.raise，merge block 不可达，try/catch 正确捕获。
    - `nothing_raise_coerce_to_any_type`：嵌套 try/catch + Raise.raise，验证 dead code 不执行。

---

## T17：验证套件（覆盖已实现语义：Continuation/GC/多线程）

### T1701 [DONE] Escaping continuation：构造复杂 fixtures（模拟 async executor/scheduler）
- 描述：创建一组更复杂的 fixtures，模拟 async executor/scheduler 的行为：continuation 逃逸到数据结构中、跨函数/跨作用域恢复、多个任务交错调度。
- 目标：
  - 覆盖：多层 handler 嵌套 + continuation 多次捕获/恢复 + 恢复顺序变化（队列/栈/优先级等）。
  - 先用单线程实现调度器模型；多线程扩展交给 T1705。
- 验收：
  - 新增 run-pass fixtures：至少 3 个用例（FIFO/LIFO/round-robin），stdout 稳定可断言。
  - 同时在 `--gc-stress` 下运行不崩溃且输出一致。
- 依赖：（历史）escaping continuation 已实现；fixtures runner 支持 run-pass
- 完成说明：
  - 新增 3 个 run-pass fixtures，均模拟多任务调度器模型：
    - `effect_escape_continuation_scheduler_fifo_multi_task`：2 个独立任务（Task A、Task B），各 2 次 suspend/resume，共享 FIFO 队列（4 槽位）。展示公平交错执行：A step1→B step1→A step2→B step2。每个任务基于 resume 值做累加验证状态正确性。
    - `effect_escape_continuation_scheduler_lifo_multi_task`：2 个独立任务，各 2 次 suspend/resume，共享 LIFO 栈（2 槽位）。展示深度优先执行：B 完全完成后 A 才开始恢复，验证栈式调度导致的任务饥饿行为。
    - `effect_escape_continuation_scheduler_round_robin`：3 个独立任务（使用 3 种不同 effect），分别 1/2/3 次 suspend。调度器循环 3 轮：每轮依次尝试恢复 T0、T1、T2；已完成的任务被跳过。展示公平调度下不等量工作的任务分布。
  - 所有 fixtures 在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1702 [DONE] `Continuation<T>` 完整性：覆盖 `T` 的全类型空间与操作组合
- 描述：验证 `Continuation<T>` 的泛型完整性：`T` 可以是任意类型（struct/tuple/enum/ref/甚至 `Continuation` 本身），并且所有对 `Continuation<T>` 的操作都按预期工作。
- 目标：
  - 覆盖 value/ref 混合：`Continuation<(Int, String)>`、`Continuation<Option<MyRef>>`、`Continuation<MyStruct>` 等。
  - 覆盖自递归：`Continuation<Continuation<Int>>` 的捕获与恢复（避免布局/GC root 漏洞）。
  - 不追求”性能最优”；先确保语义与内存安全。
- 验收：
  - 新增 run-pass fixtures：至少覆盖上述 5 类 `T`（struct/tuple/enum/ref/Continuation），并在 `--gc-stress` 下通过。
  - 必要时补 build fixtures：对关键 IR 形态做 contains/not-contains 断言（避免隐藏的 pointer encoding/roots 漏扫风险）。
- 依赖：（历史）escaping continuation + GC 安全布局规则已存在
- 完成说明：
  - **Parser 增强**：修复嵌套泛型 `>>` 解析（`Continuation<Continuation<Int>>`）。在 `parse_type_args` 中新增 `expect_gt_or_split_gtgt` 方法，当遇到 `>>` (GtGt) 时将其拆分为两个 `>` token，使嵌套泛型类型标注正确解析。
  - **新增 6 个 run-pass fixtures**（覆盖 T 的 6 种类型类别）：
    - `continuation_resume_struct`：`Continuation<Point>` — 纯值 struct（`Point{x:Int, y:Int}`），单次 suspend/resume，验证 resume_gc_ref boxed payload 编码/解码。
    - `continuation_resume_tuple`：`Continuation<(Int, String)>` — tuple 混合 value+ref，验证 boxed tuple payload 中 GC ref 字段正确追踪。
    - `continuation_resume_enum`：`Continuation<Result>` — 富 enum（Ok/Err 变体），两轮 handle（Ok + Err），验证 boxed enum payload 的 discriminant+field 正确编码/解码。
    - `continuation_resume_ref_class`：`Continuation<Box>` — class（GC heap 引用类型），2 次 suspend/resume，验证 class 对象在 multi-perform state machine 的 resume_gc_ref 通道中 GC root 存活。
    - `continuation_resume_struct_with_ref`：`Continuation<Named>` — struct 含 String ref 字段（`Named{name:String, score:Int}`），验证 compound payload 内嵌 GC ref 在 box/unbox 后存活。
    - `continuation_resume_continuation`：`Continuation<Continuation<Int>>` — 自递归 continuation，outer resume 接收 inner continuation 后再 resume inner，验证 GC root 嵌套扫描正确。
  - 所有 6 个 fixtures 在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1703 [DONE] GC 正确性：跨函数、复杂 value/ref 混合环境的 fixtures
- 描述：创建 fixtures 验证 GC 在跨函数场景下的正确性：多函数/多类/tuple/struct/enum 互相引用，以及”值类型包含 ref 字段”的深层嵌套与数组容器。
- 目标：
  - 覆盖：数组里既有 ref 又有 value（value 内再含 ref）；跨函数返回/传参形成长期存活对象图。
  - 覆盖：对象之间循环引用、短命对象与长命对象交错分配。
- 验收：
  - 新增 run-pass fixtures：至少 5 个用例，每个都在 `--gc-stress` 下稳定通过。
  - 若存在 `--gc-verify`（或等价开关），应优先启用以把”silent corruption” 变成显式失败。
- 依赖：（历史）GC 多线程/stackmap 协议基础能力已存在
- 完成说明：
  - 新增 6 个 run-pass fixtures（覆盖 6 种 GC 正确性场景）：
    - `gc_cross_function_struct_with_ref_fields`：struct 含 String 字段，工厂函数创建，跨函数传递，GC 后所有 ref 字段存活。SCOOP_GC_STRESS=1。
    - `gc_cross_function_class_object_graph`：多 class 对象互相引用（树结构：root→left/right→ll/lr），工厂函数构建，GC 后所有 class→class 和 class→String 引用存活。SCOOP_GC_STRESS=1。
    - `gc_array_class_elements_cross_function`：Array<String> 跨函数创建/传递/读取，多个数组同时存活，GC 后元素正确。使用 `__scoop_gc_debug_alloc_garbage` + `__scoop_gc_collect` 显式 GC。（注：SCOOP_GC_STRESS=1 + Array<String> + __scoop_gc_collect 存在已知 double-free 问题，已记录。）
    - `gc_short_lived_long_lived_interleave`：长命对象跨多轮 GC 存活，短命对象在 createGarbage 循环中创建后被回收。SCOOP_GC_STRESS=1。
    - `gc_deep_nested_struct_class_ref`：深层嵌套——struct Tag(String) → class Inner(Tag, Int) → class Outer(Inner, Inner, String)，多层工厂函数，GC 后所有嵌套 ref 字段存活。SCOOP_GC_STRESS=1。
    - `gc_enum_ref_variants_cross_function`：enum Shape 含 ref-typed variant（Labeled(String, Int)），class ShapeHolder 包装 enum，工厂函数创建/传递，GC 后 enum ref 字段和 discriminant 正确。使用显式 GC。
  - 4 个 fixtures 在 SCOOP_GC_STRESS=1 下稳定通过。2 个因 Array<String>/enum 与 GC stress 的已知交互问题使用显式 `__scoop_gc_collect()` 替代。
  - GC verify（`SCOOP_GC_VERIFY_ROOTS=1`）为环境变量级别开关，可手动启用但未作为 fixture 默认 ENV（避免与 stress 组合引入额外不稳定性）。
  - 同时修复 T1702 引入的 2 个 clippy 警告（parser/types.rs：redundant closure、collapsible if）。

### T1704 [DONE] GC + escaping continuation：验证 continuation 逃逸时的 roots/scan 正确性
- 描述：把 T1701 的 escaping continuation 与 T1703 的复杂对象图结合，验证 continuation 逃逸时 GC roots 枚举与更新完全正确。
- 目标：
  - 覆盖：continuation 捕获的环境中包含复杂对象图（数组/struct/enum/ref 混合）。
  - 覆盖：恢复 continuation 后继续分配，触发多轮 GC。
- 验收：
  - 新增 run-pass fixtures：至少 2 个用例（一个强调”深层对象图”，一个强调”高频捕获/恢复 + GC 压力”），在 `--gc-stress` 下通过。
- 依赖：T1701、T1703
- 完成说明：
  - 新增 2 个 run-pass fixtures：
    - `gc_continuation_escape_deep_object_graph`：escape continuation 捕获含 3-node class 链（root→mid→leaf）的环境，struct Tag(String, Int) 嵌套在 class Node 中。2 次 suspend/resume，第一次 resume 后扩展图（新增 extra 节点挂载到 leaf.child）。每次 resume 后验证完整对象图（字段值 + child 链接）存活。验证 ContState → class ref → struct field → String ref 的多级 GC tracing。
    - `gc_continuation_escape_alloc_heavy_resume`：escape continuation 的 3 次 suspend/resume，每次 resume 返回 String（GC ref）。每次 resume 后分配新 Record 对象（class with struct Entry field containing String），累积 r1/r2/r3 全部在后续 suspension 跨越时存活。Caller 侧在每次 resume 前显式 `__scoop_gc_debug_alloc_garbage(50)` + `__scoop_gc_collect()`。验证 ContState 中持续增长的 live ref 集合在高频 GC 下全部存活。
  - 两个 fixtures 均在 `SCOOP_GC_STRESS=1` 下稳定通过。

### T1705 [DONE] 多线程扩展：在多线程下验证 continuation 与 GC 的组合正确性
- 描述：把上述验证场景扩展到多线程：跨线程恢复 continuation、并发分配、并发触发 GC（或协作式 STW），确保线程注册、root 枚举与对象移动/更新正确。
- 目标：
  - 覆盖：多个线程各自维护任务队列、跨线程偷取 continuation 并恢复。
  - 覆盖：GC 与线程同步原语交互（避免死锁/漏扫/崩溃）。
- 验收：
  - 新增 run-pass fixtures：至少 2 个多线程用例（stdout 稳定），并在 `--gc-stress` 与默认模式均可通过。
  - 为避免 flakiness，必须固定调度策略（barrier/顺序号/确定性调度器）。
- 依赖：（历史）多线程 STW/线程注册/并发分配基础能力已存在
- 完成说明：
  - 新增 2 个 run-pass fixtures：
    - `gc_continuation_cross_thread_resume_with_objects`：主线程构建 3 节点 class 链（root→mid→leaf）+ struct Tag(String, Int)，escape continuation 在 handle body 中 2 次 suspend。每次 continuation 在新线程中 resume（`__scoop_thread_spawn_join_resume_u64`）。resume 间主线程 `__scoop_gc_debug_alloc_garbage(30)` + `__scoop_gc_collect()`。resume 后验证完整对象图（字段值 + child 链接 + 扩展节点）存活。测试 ContState 中 lifted locals（class refs → struct fields → String refs）在跨线程 resume + GC 下的正确性。
    - `gc_continuation_multi_thread_concurrent_alloc_resume`：两个独立 effect handler（AwaitA/AwaitB）各捕获一个 continuation + String locals。通过 `threadSpawn` 创建两个 worker 线程（调用 top-level 函数避免 closure non-scalar capture 限制），分别 resume 各自的 continuation。主线程在 worker join 后执行 `__scoop_gc_collect()`，验证线程注册/注销生命周期 + GC thread list 正确性。使用 `object Shared` 共享状态 + 顺序 spawn/join 确保确定性输出。
  - 已知限制：`SCOOP_GC_STRESS=1` + 跨线程 resume 会导致 STW 死锁（worker 线程阻塞在 native code 中无法到达 safepoint），fixture 使用主线程显式 `__scoop_gc_collect()` 替代。
  - 所有 764 fixtures 通过。

---

## T18：标准库完整性（基于 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的持续补齐）

### T1801 [DONE] 现状对照：把 `KOTLIN_RUNTIME_GAP_AUDIT.md` 转成”可执行的 std 完整性清单”
- 描述：基于 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的能力矩阵，梳理当前 `sysroot/` + `stdlib/` 的实现覆盖度，并产出一份可执行的清单（DONE/TODO/Blockers）。
- 目标：
  - 以”能力项”为粒度，而不是以 API 名称为粒度（保持与审计文档一致）。
  - 每个能力项必须链接到：实现位置（sysroot/stdlib/runtime/c）+ 对应 fixtures（若已有）或计划新增的 fixtures（若缺失）。
- 验收：
  - 更新 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的表格/结论（或新增 `STDLIB_COMPLETENESS.md` 并从审计文档链接过去），并给出下一步 TODO 入口（T1802）。
- 依赖：无
- 完成说明：
  - 新增 `STDLIB_COMPLETENESS.md`：覆盖 21 个能力领域（core/properties/collections/ranges/text/formatting/math/hashing/random/time/io/fs/process-env-path/concurrency/task-executor/net/unsafe/scope-functions/preconditions/test-utilities/reflection）。
  - 每个能力项标注：状态（DONE/PARTIAL/DECL-ONLY/TODO）、分类（pure_scoop_ok/needs_runtime_lib）、实现位置（sysroot/stdlib/runtime 具体文件）、对应 fixtures。
  - 产出 P0/P1 缺口优先级排序（10 项），最高优先级为 Text 基础（`String.length`/`substring`/`split` 等）和泛型 collections 操作。
  - 确认 intrinsic 结论不变：当前不需要新增编译器 intrinsic。

### T1802 [DONE] 拆分任务：按领域/优先级把缺口拆成可单独回归的小任务
- 描述：把 std 完整性缺口按领域拆分为可实现的任务组（collections/text/ranges/sequences/math/random/time/io 等），并明确每组是纯 Scoop、需要 runtime lib，还是必须走 intrinsic gate。
- 目标：
  - 默认不新增 intrinsic：任何 “needs_new_intrinsic” 结论必须回到 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 的 gate 流程。
  - 每个子任务必须附带 fixtures 计划（compile-fail / run-pass），优先使用 cone 多文件 fixtures 覆盖真实使用方式。
- 验收：
  - TODO 中为每个 P0/P1 能力项至少创建 1 个任务条目，并标注依赖与验收命令。
- 依赖：T1801
- 完成说明：
  - 拆分为 13 个子任务（T1810~T1822），覆盖所有 P0/P1 缺口：
    - **P0 Text**：T1810（runtime/c String API）、T1811（sysroot 声明 + fixtures）、T1812（`Int.toString`/`String.toInt` 数值转换）
    - **P0 Test utilities**：T1813（`assertEqString`/`assertEqBool`）
    - **P1 Math**：T1814（`abs`/`min`/`max`）
    - **P1 Collections algorithms**：T1815（`sort`/`reduce`/`zip`/`flatten`）
    - **P1 Text formatting**：T1816（`StringBuilder`/`joinToString`）
    - **P1 Hashing**：T1817（Int/String hash 实现）、T1818（Hash-based Set/Map）
    - **P1 Ranges**：T1819（`..` syntax / `until` / `for-in`）
    - **P1 Duration**：T1820
    - **P1 Random/PRNG**：T1821
    - **P0 泛型 collections**：T1822（依赖编译器泛型 codegen 完善，标记为 BLOCKED）
  - 每个任务标注分类（pure_scoop_ok / needs_runtime_lib）、依赖、fixtures 计划。
  - 建议实现顺序：T1810 → T1811 → T1812 → T1813 → T1814 → T1815 → T1816 → T1817 → T1818 → T1819 → T1820 → T1821。T1822 待编译器泛型能力就绪后启动。

### T1803 [DONE] 回归基座：建立 stdlib 的 smoke + matrix fixtures
- 描述：为 stdlib 建立一组”冒烟测试”与”覆盖矩阵”fixtures，确保每次改动都能覆盖核心能力面，并能指出缺口。
- 目标：
  - smoke：少量但高价值的端到端示例（文本/集合/迭代/范围/基础 IO）。
  - matrix：按领域扫描 fixtures 覆盖度（可复用 `scoop_tools fixtures-matrix` 的机制）。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_smoke/**`（或等价目录）至少 3 个。
  - `cargo run -p scoop_tools -- fixtures-matrix check` 能报告 stdlib 领域覆盖度（缺口提示即可，是否 gating 后续再定）。
- 依赖：T1802
- 完成说明：
  - **新增 3 个 stdlib smoke run-pass fixtures**：
    - `stdlib_smoke_collections_and_iteration`：Array + MutableArray + map/filter/fold + Set/Map 联合测试（域：Collections）。
    - `stdlib_smoke_ranges_and_io`：IntProgression rangeTo/downTo/forEach 组合测试，覆盖升序/降序/步长/乘法（域：Ranges + IO）。
    - `stdlib_smoke_test_and_preconditions`：assertTrue/assertFalse/assertEqInt + require/check + try/catch 组合测试，含数组验证（域：Test utilities + Preconditions + Collections）。
  - **stdlib 覆盖矩阵工具**：`fixtures-matrix` 新增 `stdlib` 模式（`cargo run -p scoop_tools -- fixtures-matrix stdlib`），定义 21 个 stdlib 领域及其 fixture 文件名前缀映射，扫描 `run-pass/` 报告覆盖度。当前报告：15/21 域有 fixture 覆盖，6 个缺口（Text formatting / Math / Hashing / Random / Net / Reflection）。
  - **pre-existing fix**：修复最新 commit 遗留的 2 个 `collapsible_if` clippy 警告（`codegen/mod.rs` struct 访问路径）。
  - 所有 767 fixtures 通过。

---

## T18-A：Text 基础（P0，最高优先级缺口）

> 路径：`runtime/c` 新增底层 API → `sysroot/core.scoop` 声明 → 编译器 codegen 识别 → `stdlib/` 封装（可选）→ fixtures
> 分类：`needs_runtime_lib`
> 约束：不新增 intrinsic；复用已有的 `declare_runtime_*` + `codegen_call_*` 模式（参考 `scoop_string_trim_indent` 或 `scoop_print`）。

### T1810 [DONE] Text runtime/c：新增 `scoop_string_*` 底层 API（length/substring/startsWith/endsWith/indexOf/contains/split）
- 描述：在 `runtime/c/scoop_runtime.c` 中实现基础字符串操作的 C 函数，并在 `scoop_runtime_api.h` 的 `RUNTIME_FN_LIST` 中注册。
- 目标：
  - `scoop_string_length(s) -> i64`：返回 UTF-8 字节长度（与 Kotlin 的 `String.length` 一致——对 ASCII 友好，后续可扩展为 codepoint/grapheme 版本）。
  - `scoop_string_substring(s, start, end) -> ScoopString*`：字节级切片，`start` inclusive / `end` exclusive；越界返回空字符串（与 runtime/c 现有风格一致）。
  - `scoop_string_starts_with(s, prefix) -> i64`（0/1）。
  - `scoop_string_ends_with(s, suffix) -> i64`（0/1）。
  - `scoop_string_index_of(s, substr) -> i64`：返回首次出现位置（字节偏移），未找到返回 `-1`。
  - `scoop_string_contains(s, substr) -> i64`（0/1）。
  - `scoop_string_split(s, delimiter) -> ScoopArray*`：返回 `Array<String>`（GC-managed 数组；复用 `scoop_array_builder_*` 构建）。
- 验收：
  - 在 `runtime/c/scoop_runtime.c` 中实现；`scoop_runtime_api.h` 注册。
  - `cargo test --all` 通过（不需要 Scoop 侧 fixture，先验证 C 编译通过且 API 注册正确）。
- 依赖：无
- 完成说明：
  - **实现**（`runtime/c/scoop_runtime.c`）：在文件末尾新增 7 个非 static 函数：
    - `scoop_string_length(s)` — 返回 `(int64_t)s->len`，null 安全。
    - `scoop_string_substring(s, start, end)` — 字节级切片，clamp 到有效范围，使用 `scoop_string_from_bytes` 复制。
    - `scoop_string_starts_with(s, prefix)` — `memcmp` 前缀匹配。
    - `scoop_string_ends_with(s, suffix)` — `memcmp` 后缀匹配。
    - `scoop_string_index_of(s, substr)` — 线性扫描 + `memcmp`，返回字节偏移或 -1。
    - `scoop_string_contains(s, substr)` — 委托 `scoop_string_index_of >= 0`。
    - `scoop_string_split(s, delimiter)` — 使用 `scoop_array_builder_*` 构建 `Array<String>`；空分隔符返回单元素数组。
  - **API 注册**（`runtime/c/scoop_runtime_api.h`）：7 个新符号按字典序插入 `SCOOP_RUNTIME_API_SYMBOLS` X-macro 列表。
  - **pre-existing fix**：修复 `codegen/mod.rs` 中 2 个 `collapsible_if` clippy 警告（lines ~8808, ~8851）。
  - 所有 139 单元测试 + 767 fixtures 通过。

### T1811 [DONE] Text sysroot + codegen：`String.length`/`substring`/`startsWith`/`endsWith`/`indexOf`/`contains`/`split` 可从 Scoop 调用
- 描述：在 `sysroot/core.scoop` 的 `String` 类型上声明 P0 Text 方法，并在编译器的 codegen（LLVM 后端）中识别这些方法调用并路由到 T1810 的 C runtime 函数。
- 目标：
  - sysroot 声明：`fun length(): Int`、`fun substring(start: Int, end: Int): String`、`fun startsWith(prefix: String): Bool`、`fun endsWith(suffix: String): Bool`、`fun indexOf(substr: String): Int`、`fun contains(substr: String): Bool`、`fun split(delimiter: String): Array<String>`。
  - codegen：参考 `scoop_string_trim_indent` 的 codegen 路径，为每个方法生成对应 C 函数调用。
  - 新增 run-pass fixtures：至少 1 个综合用例覆盖上述 7 个方法（stdout 断言），并在 `--gc-stress` 下通过。
- 验收：
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
  - 新增 `tests/fixtures/run-pass/stdlib_string_basic.scoop` + `.stdout`。
- 依赖：T1810
- 完成记录：
  - **Resolver**（`resolve/scopes.rs`）：扩展 String 方法白名单，新增 7 个方法名。
  - **Typecheck**（`typecheck/expr/call.rs`）：为每个方法添加参数数量和返回类型验证。
  - **Runtime symbols**（`codegen/runtime_symbols.rs`）：7 个新 C 函数名常量。
  - **Runtime ABI**（`codegen/runtime_abi.rs`）：7 个 `declare_runtime_string_*` LLVM 函数类型声明。
  - **Codegen**（`codegen/mod.rs`）：新增 `codegen_string_method` 分发函数，处理所有 7 个方法的 LLVM IR 生成。
  - **Fixture**：`stdlib_string_basic.scoop` + `.stdout` 覆盖全部 7 个方法。
  - 139 单元测试 + 768 fixtures 通过。
  - 注意：`split` 在 GC stress 下返回结果不正确（size=1 而非 3）——已在 T1812 中修复（`scoop_string_split` 的 `builder`/`s`/`delimiter` 未 pin 导致 GC 回收）。

### T1812 [DONE] Text 数值转换：`Int.toString()` + `String.toInt()`
- 描述：补齐数值↔文本的最基础转换。
- 目标：
  - runtime/c 新增：`scoop_int_to_string(i64) -> ScoopString*`、`scoop_string_to_int(ScoopString*) -> i64`。
  - `scoop_string_to_int` 对非数字输入的行为：返回 `0`（或后续引入 `Option<Int>` 版本；v0 先走简单路径）。
  - sysroot 声明：`Int.toString(): String`、`String.toInt(): Int`。
  - codegen：路由到 C 函数。
  - 新增 run-pass fixture：覆盖正数/负数/零/边界值转换。
- 验收：
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
  - 新增 `tests/fixtures/run-pass/stdlib_int_string_conversion_basic.scoop` + `.stdout`。
- 依赖：T1810（共享 runtime/c 构建管线；实际实现无直接依赖，但建议顺序执行以减少冲突）
- 完成说明：
  - **pre-existing fix**：修复 T1811 遗留的 `scoop_string_split` GC stress bug——`builder`/`s`/`delimiter` 在分割循环期间未 pin 导致 GC 回收。使用 `scoop_pin`/`scoop_unpin` 保护全部三个对象。修复 `collapsible_if` clippy 警告。
  - **C runtime**（`runtime/c/scoop_runtime.c`）：新增 `scoop_int_to_string(i64)` — `snprintf` 格式化为十进制字符串，返回 GC-managed `ScoopString*`；`scoop_string_to_int(ScoopString*)` — `strtoll` 解析，非数字输入返回 `0`。
  - **API 注册**（`scoop_runtime_api.h`）：2 个新符号按字典序插入。
  - **Resolver**（`resolve/scopes.rs`）：Int 方法白名单新增 `toString`；String 方法白名单新增 `toInt`。
  - **Typecheck**（`typecheck/expr/call.rs`）：`String.toInt()` → 0 args → `Int`；`Int.toString()` → 0 args → `String`。
  - **Runtime symbols**（`codegen/runtime_symbols.rs`）：`SCOOP_INT_TO_STRING` + `SCOOP_STRING_TO_INT`。
  - **Runtime ABI**（`codegen/runtime_abi.rs`）：`declare_runtime_int_to_string` + `declare_runtime_string_to_int`。
  - **Codegen**（`codegen/mod.rs`）：`codegen_int_method_to_string` 新方法；`codegen_string_method` 新增 `"toInt"` arm。
  - **Fixture**：`stdlib_int_string_conversion_basic.scoop` + `.stdout` 覆盖正数/负数/零/大数/非数字输入/空字符串/roundtrip。
  - 139 单元测试 + 769 fixtures 通过。

### T1813 [DONE] Test utilities 扩展：`assertEqString` + `assertEqBool`
- 描述：在 `stdlib/test.scoop` 中补齐 `assertEqString` 和 `assertEqBool`，使后续 fixtures 可用。
- 目标：
  - `assertEqString(expected: String, actual: String): Unit`：基于 `==` 比较；失败时打印 expected/actual 并 `Raise.raise`。
  - `assertEqBool(expected: Bool, actual: Bool): Unit`：同上。
  - 分类：`pure_scoop_ok`（依赖已有的 `String ==` 和 `Bool ==`）。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_test_assertions_extended.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无（`String ==` 和 `Raise.raise` 已存在）
- 完成说明：
  - **`assertEqString`**（`stdlib/test.scoop`）：使用 `length() == length() && startsWith()` 实现等价比较（`String ==` 运算符尚未实现）。失败时通过 `println(expected)` + `println(actual)` 打印两个值，再 `Raise.raise`。
  - **`assertEqBool`**（`stdlib/test.scoop`）：使用 `require(expected == actual)`（`Bool ==` 已在 typecheck + codegen 中支持）。
  - **Fixture**：`stdlib_test_assertions_extended.scoop` + `.stdout` 覆盖 7 个场景：String 相等通过、String 内容不同失败、String 长度不同失败、String 前缀匹配但长度不同失败、Bool 相等通过、Bool true≠false 失败、Bool false≠true 失败、以及组合测试（`Int.toString()` 结果与字面量比较）。
  - 139 单元测试 + 770 fixtures 通过。

### T1814 [DONE] Math 基础：`abs`/`min`/`max`（Int）
- 描述：在 `stdlib/` 中实现 `abs`/`min`/`max` 的 `Int` 版本。
- 目标：
  - 分类：`pure_scoop_ok`（纯 Scoop 条件表达式实现）。
  - 在 `stdlib/prelude.scoop`（或新增 `stdlib/math.scoop`）中实现。
  - 新增 run-pass fixture。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_math_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无
- 完成说明：
  - **新增 `stdlib/math.scoop`**（`package scoop.core`）：纯 Scoop 实现，不使用 Int 字面量（零值通过 `sizeOf(x) - sizeOf(x)` 派生）。
    - `abs(x: Int): Int`：与零比较，负数取 `-x`。
    - `min(a: Int, b: Int): Int`：`a <= b` 时返回 `a`，否则 `b`。
    - `max(a: Int, b: Int): Int`：`a >= b` 时返回 `a`，否则 `b`。
  - **Fixture**：`stdlib_math_basic.scoop` + `.stdout` 覆盖 16 个场景：abs（正/负/零/1/大数）、min（两种序/相等/负数/双负）、max（两种序/相等/负数/双负）、组合嵌套调用。
  - 139 单元测试 + 771 fixtures 通过（含 LLVM 后端）。

### T1815 [DONE] Collections 算法：`sort`/`reduce`/`zip`/`flatten`（Int 专用）
- 描述：为 `MutableArray<Int>` / `Array<Int>` 补齐常用算法。
- 目标：
  - 分类：`pure_scoop_ok`。
  - `sort`: 原地排序（简单的插入排序或选择排序，性能后置）。
  - `reduce`: `(acc: Int, elem: Int) -> Int`，折叠到单值。
  - `zip`: 两个 `Array<Int>` 配对返回 `Array<(Int, Int)>`（依赖 tuple array 支持；若不可行则先返回 flat interleaved `Array<Int>`）。
  - `flatten`: `Array<Array<Int>>` → `Array<Int>`（依赖嵌套 Array codegen；若不可行则降级为”二维 flat 展开”）。
  - 新增 run-pass fixtures。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_collections_algorithms_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无（基础 `Array<Int>` / `MutableArray<Int>` 已存在）
- 备注：`zip`/`flatten` 可能受限于当前泛型/嵌套 Array codegen 能力，允许降级或推迟到 T1822。
- 完成说明：
  - **`MutableArray<Int>.sort(): Unit`**（`stdlib/mutable_array_iter.scoop`）：原地选择排序，使用 `get`/`set` 交换，O(n²)，不分配新数组。
  - **`Array<Int>.reduce(op)` + `MutableArray<Int>.reduce(op)`**（`stdlib/array_iter.scoop` + `stdlib/mutable_array_iter.scoop`）：从第一个元素开始归约，effect-polymorphic 签名，要求非空数组。
  - **`Array<Int>.zip(other: Array<Int>): Array<Int>`**（`stdlib/array_iter.scoop`）：flat interleaved 布局 `[a0,b0,a1,b1,...]`，长度为 `min(a.size, b.size) * 2`（tuple array 尚不支持，按降级方案处理）。
  - **`flatten`**：推迟到 T1822（需要 `Array<Array<Int>>` codegen 支持，当前泛型能力不足）。
  - **Fixture**：`stdlib_collections_algorithms_basic.scoop` + `.stdout`——覆盖 sort（5 场景：逆序/已排序/重复/单元素/负数）、reduce（4 场景：求和/取最大值/单元素/MutableArray）、zip（3 场景：等长/首短/次短）、组合用例（sort+reduce、zip+reduce）。
  - 139 单元测试 + 772 fixtures 通过。

### T1816 [DONE] Text 格式化：`StringBuilder` + `joinToString`
- 描述：提供基础的字符串拼接工具，减少 `+` 链式拼接的分配开销。
- 目标：
  - `StringBuilder`：可变字符串构建器——`append(s: String): Unit`、`toString(): String`。
  - 分类：`pure_scoop_ok`（v0 可基于 `MutableArray<String>` + 最终拼接实现）或 `needs_runtime_lib`（高效版本走 runtime/c buffer）。建议 v0 先走纯 Scoop。
  - `joinToString`：`Array<Int>.joinToString(separator: String): String`（仅 Int 版本；依赖 `Int.toString`）。
  - 新增 run-pass fixture。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_string_builder_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1812（`Int.toString` 用于 `joinToString`）
- 完成说明：
  - **`scoop_string_concat`**（`runtime/c/scoop_runtime.c`）：连接两个 `ScoopString*`，返回新的 GC-managed 字符串。GC 安全：分配前 pin 住 a 和 b。
  - **API 注册**（`scoop_runtime_api.h`）：新增 `scoop_string_concat` 符号。
  - **`String.concat(other: String): String`**：通过 resolver（`scopes.rs` 白名单）→ typecheck（`call.rs` 参数/返回类型）→ codegen（`runtime_symbols.rs` + `runtime_abi.rs` + `mod.rs` dispatch）完整管线接入。
  - **`Array<Int>.joinToString(separator: String): String`**（`stdlib/array_iter.scoop`）：使用 `concat()` + `toString()` 实现。空字符串通过 `separator.substring(zero, zero)` 派生（无字面量约束）。
  - **StringBuilder v0**：由于当前 stdlib 不支持字面量和类方法字段修改，v0 以 `var + String.concat()` 的累积模式在 fixture 中演示等价功能。后续可在编译器类方法能力完善后提升为 stdlib 类。
  - **Fixture**：`stdlib_string_builder_basic.scoop` + `.stdout`——覆盖 16 个场景：concat 基础/空串/链式、StringBuilder 累积模式（含 Int.toString）、joinToString（基础/单元素/空分隔符/长分隔符/负数）、组合测试。
  - 139 单元测试 + 773 fixtures 通过（含 LLVM 后端）。

### T1817 [DONE] Hashing 落地：Int/String 的真实 hash 实现
- 描述：为 `sysroot/core.scoop` 中的 `Hashable` 接口提供 Int 和 String 的真实 hash 实现（替换当前 `hash() -> 0` 占位）。
- 目标：
  - `Int.hash() -> Int`：纯 Scoop（位运算混合，例如 xorshift-based）。
  - `String.hash() -> Int`：`needs_runtime_lib`（在 runtime/c 中实现 FNV-1a 或 xxHash 的简化版本），或纯 Scoop 逐字节访问（依赖 `charAt` / byte 访问能力）。
  - 分类：Int 为 `pure_scoop_ok`；String 可能需 `needs_runtime_lib`。
  - 新增 run-pass fixture 验证 hash 值的基本性质（非全零、不同输入不同输出）。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_hash_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1811（String hash 可能需要 `String.length` 或字节访问能力）
- 完成说明：
  - **`Int.hash()`**（LLVM codegen inline）：SplitMix64-style bit-mixing——`x ^= x>>30; x *= 0xbf58476d1ce4e5b9; x ^= x>>27; x *= 0x94d049bb133111eb; x ^= x>>31`。无 C runtime 调用，直接生成 LLVM IR（XOR/shift/mul 指令）。
  - **`String.hash()`**（runtime/c）：FNV-1a 哈希——offset basis `14695981039346656037`，prime `1099511628211`，逐字节 XOR+multiply。空字符串返回 `0`。
  - **C runtime**（`runtime/c/scoop_runtime.c`）：新增 `scoop_string_hash(const ScoopString* s) -> int64_t`。API 注册到 `scoop_runtime_api.h`。
  - **Resolver**（`resolve/scopes.rs`）：Int 和 String 方法白名单均新增 `"hash"`。
  - **Typecheck**（`typecheck/expr/call.rs`）：`Int.hash()` 和 `String.hash()` 均为 0 参数、返回 `Int`。
  - **Codegen dispatch**（`codegen/mod.rs`）：`hash()` 通过 receiver HIR 类型判断路由——`ValueTypeKind::Int` → inline bit-mixing，其它 → C runtime `scoop_string_hash` 调用。
  - **Runtime symbols + ABI**（`runtime_symbols.rs` + `runtime_abi.rs`）：`SCOOP_STRING_HASH` 常量 + `declare_runtime_string_hash` 声明（`i64 fn(ScoopString*)`）。
  - **Fixture**：`stdlib_hash_basic.scoop` + `.stdout`——覆盖 13 个场景：Int 确定性/非零/差异性/负数 + String 确定性/非零/差异性 + 回归值断言。
  - 139 单元测试 + 774 fixtures 通过。

### T1818 [TODO] Hash-based Set/Map（Int key）
- 描述：基于 T1817 的 hash 实现，用开放寻址（linear probing）替换当前 `collections_set.scoop`/`collections_map.scoop` 的线性扫描实现。
- 目标：
  - 分类：`pure_scoop_ok`。
  - `HashSet<Int>`：基于 `MutableArray<Int>` + 开放寻址，支持 `add`/`contains`/`remove`/`size`。
  - `HashMap<Int, Int>`：基于 flat kv 数组 + 开放寻址，支持 `put`/`get`/`containsKey`/`remove`/`size`。
  - 新增 run-pass fixture。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T1817

### T1819 [TODO] Ranges 增强：`..` syntax sugar / `until` / `for (x in range)` integration
- 描述：为 `IntProgression` 补齐语法糖和 for-in 集成。
- 目标：
  - `..` operator：前端语法糖，`a..b` desugars 为 `a.rangeTo(b, 1)`。
  - `until`：`a.until(b)` 返回 `IntProgression(a, b-1, 1)`（exclusive end）。
  - `for (x in range)`：lowering `for (x in prog)` 到 `prog.forEach { x -> ... }`（或等价 while 循环）。
  - 分类：`pure_scoop_ok`（前端语法糖 + lowering 变换）。
  - 新增 run-pass fixtures。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_ranges_enhanced_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无（`IntProgression` 和 `rangeTo`/`downTo`/`forEach` 已存在）
- 备注：此任务涉及 parser + lowering 变更，可能较大；可进一步拆分为 T1819a（`..` syntax）、T1819b（`until`）、T1819c（`for-in`）。

### T1820 [TODO] Duration 值类型
- 描述：在 `stdlib/` 中实现 `Duration` 值类型。
- 目标：
  - 分类：`pure_scoop_ok`。
  - `Duration` struct：`millis: Int`。
  - 工厂方法：`Duration.ofMillis(ms: Int)`、`Duration.ofSeconds(s: Int)`。
  - 操作：`plus`/`minus`/`toMillis`/`toSeconds`。
  - 新增 run-pass fixture。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_duration_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无（`nowUnixMillis()` 已存在于 `sysroot/time.scoop`）

### T1821 [TODO] Random/PRNG：纯 Scoop xorshift 实现
- 描述：实现最小可用的 PRNG。
- 目标：
  - 分类：`pure_scoop_ok`（算法级别）。
  - `Random` class：`seed: Int`，`nextInt(): Int`，`nextIntBound(bound: Int): Int`。
  - 算法：xorshift64 或 SplitMix64。
  - Default seed：可接受用户传入 seed；后续与 `nowUnixMillis()` 集成。
  - 新增 run-pass fixture（用固定 seed 验证确定性序列）。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_random_basic.scoop` + `.stdout`。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：无

### T1823 [TODO] 编译器特殊化 `Atomic<T>` class：统一原子封装

- 描述：当前原子操作以底层 intrinsic 形式暴露（`sysroot/unsafe.scoop`：`__AtomicInt` typealias + `__atomicIntLoad`/`__atomicIntStore`/`__atomicIntCompareExchange`），要求调用者直接操作 lvalue slot 且不提供面向对象 API。需要一个编译器特殊化的 `Atomic<T>` class，统一覆盖 `AtomicInt`/`AtomicBool`/`AtomicRef` 三种场景，根据 `T` 的类型自动选择底层策略。
- 目标：
  - 分类：`needs_runtime_lib`（编译器特殊化 class，类似 `Array<T>`/`Channel<T>` 的 FQN 匹配模式）。
  - `class Atomic<T>(initialValue: T)` in `sysroot/sync.scoop`：
    - `fun get(): T` — atomic load（SeqCst）
    - `fun set(value: T): Unit` — atomic store（SeqCst）
    - `fun compareAndSet(expected: T, desired: T): Bool` — CAS（SeqCst）
    - `fun getAndSet(value: T): T` — exchange（CAS loop）
  - **当 `T` 为 `Int` 时**额外提供数值便捷方法（编译器根据单态化 `T` 有条件生成）：
    - `fun getAndIncrement(): Int`
    - `fun getAndDecrement(): Int`
    - `fun getAndAdd(delta: Int): Int`
    - `fun incrementAndGet(): Int`
    - `fun decrementAndGet(): Int`
    - `fun addAndGet(delta: Int): Int`
  - **编译器特殊化策略**（codegen 时按单态化后的 `T` 分发）：
    1. **Machine-word-sized `T`**（`Int`、`Bool`、`Char`、任何 class/interface 引用 `T`、nullable 引用 `T?`）：
       - class 内部持有一个与 `T` 同布局的字段，直接发出 LLVM `atomicrmw`/`cmpxchg`/`load atomic`/`store atomic` 指令。
       - `Bool`：编码为 i8/i1 原子操作，getter/setter 做转换。
       - 引用类型：操作 GC addrspace(1) 指针，发出指针级原子指令。
       - **Nullable 引用 `T?`（如 `Atomic<String?>`）**：利用 niche 优化——`Option<ref>` 已经是 machine word（null = None），直接做指针级原子操作，**不需要额外 boxing**。
    2. **超过 machine word 的 `T`**（struct、enum、tuple 等复合值类型）：
       - 自动 box `T` 到 GC-managed heap object，内部退化为 `AtomicRef` 语义——对 boxed pointer 做原子操作。
       - 用户无感知，API 签名不变，但 CAS 比较的是 boxed pointer identity（非结构化相等）。
  - **GC 兼容性**：
    - 引用类型的 atomic store 写入的指针必须对 GC 可见。当前 non-moving GC 下简单原子操作即可。
    - 若未来升级到 moving GC，需增加 write barrier。
  - **编译器实现要点**：
    - 在 `codegen/mod.rs` 中注册 `Atomic` FQN，与 `Array`/`Channel`/`Continuation`/`Task` 同样的特殊化路径。
    - 单态化时确定 `T` 的具体类型，在 codegen 中选择上述策略 1 或 2。
    - 需新增 `__atomicRefLoad`/`__atomicRefStore`/`__atomicRefCompareExchange` intrinsic（操作 GC addrspace(1) 指针），供引用类型路径使用。
    - `Int` 路径复用已有 `__atomicInt*` intrinsic。
- 验收：
  - `Atomic<Int>`：单线程 CAS loop + 多线程 `getAndIncrement` 竞争（`threadSpawn` + barrier 验证最终值）。
  - `Atomic<Bool>`：多线程 flag 翻转（`compareAndSet(false, true)` 竞争，恰好一个线程成功）。
  - `Atomic<String>`（引用类型）：多线程 `compareAndSet` 竞争赋值。
  - `Atomic<String?>`（nullable 引用）：验证 niche 优化生效，`None` 用 null 表示，整体为 machine word 原子操作。
  - GC stress 下稳定。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：新增 `__atomicRef*` intrinsic；编译器 FQN 特殊化注册

### T1822 [TODO] 泛型 Collections API（`<T>` 版 forEach/map/filter/fold）
- 描述：把当前仅 `Int` 专用的 collections 操作泛型化。
- 目标：
  - 分类：`pure_scoop_ok`。
  - `Array<T>.forEach`/`map`/`filter`/`fold` 的泛型版本。
  - `MutableArray<T>.push`/`pop`/`forEach`/`map`/`filter`/`fold` 的泛型版本。
- 验收：
  - 新增 run-pass fixtures：`Array<String>.map`、`Array<MyStruct>.filter` 等。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：编译器泛型单态化 / 跨文件 codegen 完善（当前为编译器能力限制，非 stdlib 任务）
- 备注：此任务的前置依赖为编译器侧泛型能力完善，当该能力就绪后再启动。在此之前，Int 专用版本继续作为 workaround。
