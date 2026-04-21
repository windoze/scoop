# 执行计划与决策摘要

说明：我不会记录不可公开的原始推理细节，但会持续记录足够完整的执行计划、关键判断、阻塞原因与进度，便于审查当前工作。

## 初始计划

1. 检查最新一次 Git 提交的信息，确认是否提到了任何已知问题、后续修复项或未完成事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有既定拆解、依赖或顺序要求。
4. 检查工作区状态，识别是否存在未提交变更；避免覆盖用户已有修改。
5. 如果最新提交暴露了必须先修复的既有问题，优先修复这些问题，并补充测试。
6. 评估第一个未完成任务是否可以在本轮完整实现：
   - 若可以，直接实现、测试、更新文档、提交并停止。
   - 若不可以，按依赖与复杂度拆成更小子任务，更新 `PLAN.md` 与 `TODO.md`，执行拆分后的第一个子任务，或在被前置缺陷阻塞时仅提交任务重排与计划更新。
7. 实施过程中若发现规范不匹配、语言特性缺失、运行时缺陷或测试依赖错误：
   - 不采用绕过方案；
   - 在 `TODO.md` 中新增或重排前置修复任务；
   - 在 `PLAN.md` 与本文件中记录原因；
   - 完成必要修改与提交后停止。
8. 完成当前任务后：
   - 运行相关测试，并尽量补充到足以证明行为正确；
   - 更新 `TODO.md`、`PLAN.md`、本文件；
   - 使用清晰提交信息提交；
   - 停止，不继续下一个任务。

## 当前状态

- 已检查最新提交、`TODO.md`、`PLAN.md` 与工作区状态。
- 当前工作区仅有本文件修改。
- 最新提交 `081a7886f504453974b6da46ce332422e23a4585` 标题为 `[T4016b4a0] Preserve ordinary GC locals and split global-root blocker`，未附加额外未落地修复说明，但明确指出存在一个更前置的 global-root blocker。
- `TODO.md` 中最靠前且当前应执行的可落地未完成叶子任务为：
  - `T4016b4a0`：把 object property / top-level immutable backing globals 纳入永久 GC roots，恢复显式 GC 后的模块级引用稳定性。
- 暂不直接处理 `T4016b4b0`，因为其依赖已在 `TODO.md` 中明确写为 `T4016b4a0`。

## 当前执行计划

1. 阅读与 GC roots / LLVM 全局发射 / runtime root 注册相关的代码，找出：
   - object property globals（`__scoop_object_prop__*`）是如何生成和保存的；
   - top-level immutable backing globals 如何生成；
   - 现有永久 roots / global roots / relocation update 机制如何登记全局槽。
2. 结合现有测试与 fixture，找到最小可复现路径；必要时先跑定向失败用例确认现状。
3. 修改编译器和/或 runtime，使上述模块级 GC 指针全局槽进入永久 roots/update 合同。
4. 增加或更新回归测试，覆盖：
   - 仅通过模块级全局槽保活的对象跨显式 GC 仍可正确访问；
   - object property global 与 top-level immutable backing global 在 GC 后不会悬挂或错指。
5. 运行与任务直接相关的测试，再补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`（如时间允许再补更大范围 fixture）。
6. 更新 `TODO.md`、`PLAN.md`、本文件，标记 `T4016b4a0` 完成，提交后停止。

## 实施进展

- 已实现 runtime 级模块全局 roots 注册 API：`scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc)`。
- 已在 baseline / immix / minimal / hosted 四个 backend 中接入同一套 global roots 记录与访问逻辑。
  - mark 路径会扫描这些模块级全局槽；
  - moving / compaction / minor evacuation 的 update 路径会更新这些槽里的引用；
  - baseline / immix 的 verify-roots 路径会把这些槽作为永久 roots 校验。
- 已在 LLVM codegen 中补上：
  - backing global 的 GC-pointer 布局描述生成；
  - 模块级 roots 注册函数生成；
  - 在入口 `main` 中于 `scoop_runtime_init()` 后调用该注册函数。
- 当前登记范围不只覆盖任务最低要求的两类：
  - object property globals（`__scoop_object_prop__*`）
  - top-level immutable backing globals（`__scoop_top_level_val__*`）
  - 还一并覆盖了 object singleton instance globals（`__scoop_object_instance__*`），避免留下同类悬挂问题。
- 已新增回归：
  - Rust runtime 单测 `crates/scoop_runtime/tests/gc_global_roots.rs`
  - Scoop fixture `tests/fixtures/runtime_gc/gc_module_global_roots_move_basic.scoop`
- 已完成的定向验证：
  - `cargo test -p scoop_runtime --test gc_global_roots` 通过；
  - `cargo run -p scoop -- build tests/fixtures/runtime_gc/gc_module_global_roots_move_basic.scoop -o /tmp/gc_module_global_roots_move_basic.out` 成功；
  - `SCOOP_GC_MOVE=1 SCOOP_GC_VERIFY_ROOTS=1 /tmp/gc_module_global_roots_move_basic.out` 输出符合预期。

## 下一步

1. 回退编译器侧“统一模块 helper + main 调用”的注册路径，避免继续污染 `MainCodegen` / builder 状态。
2. 改为在各自初始化函数内就地注册：
   - `codegen_object_init_fun_body` 中注册 `__scoop_object_instance__*` 与各个 `__scoop_object_prop__*`；
   - `codegen_top_level_immutable_value_init_fun_body` 中注册 `__scoop_top_level_val__*`。
3. 删除不再需要的 helper / 命名，修掉 `llvm/mod.rs` 中的 `unused_mut`。
4. 跑 `cargo fmt`、`cargo test -p scoopc --lib`，确认 3 个 LLVM 回归消失。
5. 再跑 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`，若通过则更新 `TODO.md` / `PLAN.md` 并提交。

## 最新判断

- 已重新检查当前编译器实现，确认 `crates/scoopc/src/llvm/mod.rs` 仍在 `main` 中调用
  `ensure_module_global_gc_roots_registration_function_defined()`。
- 该 helper 会用当前 `MainCodegen`/builder 上下文额外生成一个模块级函数；结合已有失败栈，最可能的问题仍是跨函数/跨块复用了本应只属于当前函数的 GC local keepalive SSA。
- 因此后续修复方向保持不变：不再从 `main` 统一生成/调用注册 helper，而是借助 object / top-level immutable 的 once-init 函数，在初始化完成后立刻就地调用 runtime 注册接口。

## 进一步定位

- 已完成“就地注册 root”改线，但 `cargo test -p scoopc --lib` 仍保留 3 个既有回归。
- 进一步阅读 `with_conservative_gc_local_root_spills` 与 effect state-machine 代码后，确认根因并不只是模块 helper：
  - `CgLocal.ptr` 可能直接保存某个 block 内新建的 frame-slot GEP（如 `pre_slot_*` / `resume_slot_*`），随后在其它 block 里被 `with_conservative_gc_local_root_spills` 或普通 load 直接复用，触发 SSA dominance 错误；
  - 部分嵌套函数生成路径（至少 dispatch loop / task body wrapper）没有完整隔离并恢复 `env`，会把一个函数体里的 frame-slot 指针泄漏到另一个函数，触发 “Referring to an instruction in another function”。
- 下一步将同时修这两个点：
  1. 为 local slot 指针增加“在当前 block 重新物化”的读取路径，至少覆盖 frame-slot GEP；
  2. 为 dispatch loop 与 task body wrapper 的函数体生成补齐 caller context（尤其 `env`）的保存/恢复。

## 最终结果

- `T4016b4a0` 已完成。
- runtime 层面：
  - 新增 `scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc)`；
  - baseline / minimal / hosted / immix 四个 backend 都已把注册的模块级 backing globals 纳入 roots 扫描与 moving update；
  - 该合同现在不仅覆盖任务最低要求的 `__scoop_object_prop__*` / `__scoop_top_level_val__*`，也覆盖 `__scoop_object_instance__*`，避免同类 singleton instance 悬挂。
- compiler / codegen 层面：
  - 删除了 `main` 中统一生成并调用模块 helper 的路径；
  - 改为在 object init / top-level immutable init 的 once-only 函数体里就地注册对应 backing globals；
  - 为 backing global 生成可描述 nested GC refs 的 type descriptor，使 aggregate 值类型中的内嵌引用也能被 trace / relocate；
  - 在修这条路径时，同时修复了先前暴露出来的两个 LLVM 回归：
    - frame-slot GEP 现在会在当前 block 重新物化，避免 `pre_slot_*` / `resume_slot_*` 跨块复用触发 SSA dominance 错误；
    - dispatch loop / task body wrapper 的嵌套函数生成现在会完整保存并恢复 caller `env`，不再把一个函数里的 frame-slot 指针泄漏到另一个函数。
- 新增/保留的回归覆盖：
  - `crates/scoop_runtime/tests/gc_global_roots.rs`
  - `tests/fixtures/runtime_gc/gc_module_global_roots_move_basic.scoop`
  - `tests/fixtures/runtime_gc/gc_module_global_roots_move_basic.stdout.txt`

## 最终验证

- `cargo test -p scoopc --lib` 通过（310/310）。
- `cargo test --all` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- `cargo run -p scoop -- build tests/fixtures/runtime_gc/gc_module_global_roots_move_basic.scoop -o /tmp/gc_module_global_roots_move_basic.out` 成功，且
  `SCOOP_GC_MOVE=1 SCOOP_GC_VERIFY_ROOTS=1 /tmp/gc_module_global_roots_move_basic.out`
  输出符合预期。
- `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop -o /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 成功，且
  `SCOOP_GC_STRESS=1 /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out`
  正常输出 `all_done` 结束。

## 下一步

- 按 `TODO.md` 顺序，下一轮应回到 `T4016b4b0`：
  - 既然原先的 stress fixture 现已跑通，需要确认该 blocker 是否已经随 `T4016b4a0` 一并消失；
  - 若仍存在更窄的 cross-thread continuation / frame liveness / thread residual，再把它单独收口后继续推进 `T4016b4b`。
