# Claude Plan

## 执行原则
- 先以 `TODO.md` 为唯一任务真源，定位首个未完成任务。
- 仅处理该任务；若存在阻塞该任务的真实前置问题，则先把最小前置任务写回 `TODO.md`，提交后停止。
- 变更前后持续更新本文件，记录当前计划、关键发现、执行进度与验证结果。

## 初始执行计划
1. 读取 `TODO.md`，确认首个未完成任务及其约束、依赖、验证要求、完成记录格式。
2. 检查最近一次提交信息，判断是否有与当前任务直接相关且明确未完成的问题；若有，则视作当前任务内容或前置依赖处理。
3. 阅读实现与测试相关文件，建立最小必要上下文，不做开放式问题扫描。
4. 按任务要求进行实现；若发现阻塞当前任务的规范不匹配、缺失特性或回归，则停止向前推进，先把该阻塞作为最小前置任务写入 `TODO.md`。
5. 运行任务要求的验证；至少覆盖相关测试，并检查 `cargo clippy --all-targets -- -D warnings` 是否通过；如有必要再运行更小或更大的相关命令。
6. 更新文档与任务记录：
   - 在 `TODO.md` 中将当前任务标题标记为 `[DONE]` 并补全完成记录；若新增前置任务或调整顺序，也同步更新。
   - 仅当阶段计划或依赖结构变化时更新 `PLAN.md`。
   - 在本文件记录关键完成情况与最终验证结论。
7. 按仓库提交风格创建一次 git 提交，提交信息包含当前任务编号；若本次是在恢复上次未提交的同一任务，也将当前所有未提交文件一并纳入。
8. 停止，不继续处理下一个任务。

## 当前状态
- 已确认首个未完成任务是 `CG-T07S0a24a`。
- 最近一次提交 `f84fc17cf5a215130af520fc20e9be0584993653` 明确将该问题登记为 `CG-T07S0a` 的新前置 blocker，因此本次直接实现该任务，不做额外历史问题扫荡。

## 当前任务理解：CG-T07S0a24a
- 需要同时修复两个真实 blocker：
  1. refactor LLVM 对 top-level `@Global __AtomicInt` lvalue 的 lowering 漂移，必须直接面向共享静态存储发射 atomic load/store/cmpxchg。
  2. run-pass fixture 超时后只杀父进程、不回收后代 `a.out`，导致继承 pipe 的 orphan process 让 `scoop test` 卡在 reader join。
- 验证要求包含单 fixture、`runtime_gc` 组、timeout 定向测试、全量 `cargo run -p scoop -- test` 以及 `cargo clippy --all-targets -- -D warnings`。

## 下一步调查计划
1. 搜索 `__atomicIntLoad` / `__atomicIntStore` / `__AtomicInt` / top-level global lowering 相关代码，定位 top-level lvalue 在 LLVM lowering 中何处被错误退化成 ordinary value load。
2. 搜索 `crates/scoop/src/fixtures/run_pass.rs` 及其超时处理、子进程启动与回收路径，确认当前只终止父进程的实现点。
3. 在最小必要范围内阅读相关测试与 fixture，必要时先复现单 fixture / timeout 定向问题，再实施修复。

## 当前进展
- 已确认 refactor atomic lowering 的问题点在 `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs`：`atomic_int_lvalue_ptr()` 只识别成员字段和本地 slot，未把 `Assign { target: lN, value: TopLevelRef(...) }` 重新解析为静态存储地址。
- 已修改 `atomic_int_lvalue_ptr()`，为 direct `TopLevelRef` local 增加 `atomic_top_level_place_for_local()` 回溯，命中 top-level `@Global` / `@ThreadLocal` / extern-global 时直接返回静态全局指针，不再回退到局部副本 slot。
- 已修改 `crates/scoop/src/fixtures/run_pass.rs`：timeout 路径在 Unix 上把被测命令放入独立 process group，并在超时时对整个子进程树发 `SIGKILL`；同时新增 descendant cleanup 单测草稿。
- 下一步：运行 `cargo fmt`、定向测试与目标 fixture，必要时补建 LLVM build 回归 fixture 来锁定 top-level atomic IR 形状。

## 最终结果
- 已补充 `tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`，锁定 top-level `@Global` / `@ThreadLocal __AtomicInt` 的静态存储与 atomic IR 形状。
- 重新导出的 `gc_stw_cross_thread_roots_basic.ll` 仅保留直接针对 `@__scoop_top_level_var__fixtures.codegen.ready` / `proceed` 的 atomic load/store；此前多余的 ordinary `load_top_level_var` 已消除。
- run-pass timeout 现已能稳定回收 `scoop run` 的后代进程，不再因 orphan executable 继承 stdout/stderr pipe 而让 `scoop test` 卡在 reader join。
- 全量验证中额外发现一个与本任务实现无关、但会阻塞 `cargo run -p scoop -- test` 的 stale fixture：`tests/fixtures/typecheck_cone/std_task_async_await_impl_ok/` 为空目录，已直接移除以恢复 full-suite。

## 已执行验证
- `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`
- `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
- `cargo test -p scoop --bin scoop fixtures::run_pass::tests::run_fixture_command_timeout_has_stable_code -- --exact --nocapture`
- `cargo test -p scoop --bin scoop fixtures::run_pass::tests::run_fixture_command_timeout_kills_descendants -- --exact --nocapture`
- `cargo run -p scoop -- test --fixtures tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`
- `cargo run -p scoop -- test`
- `cargo clippy --all-targets -- -D warnings`

## 待收尾
- 更新 git 状态并创建 `CG-T07S0a24a` 提交，然后停止，不继续处理下一条 TODO。
