# 执行计划

1. 读取 `TODO.md`，严格按标题是否带 `[DONE]` 判断首个未完成任务。
2. 检查最近提交是否存在与该任务直接相关且未收尾的问题；若存在且阻塞当前任务，则先在 `TODO.md` 中记录为前置依赖。
3. 阅读当前任务涉及的代码、测试与规范位置，确认最小正确修改范围。
4. 实现当前任务，避免引入规避式修补；如果出现真实阻塞，则最小化新增前置任务并停止在该点。
5. 运行任务要求的测试，以及必要的质量检查（至少包含相关测试；若改动范围合适则补充 `cargo clippy --all-targets -- -D warnings`）。
6. 更新 `memory/claude_plan.md` 记录进展；在任务完成时更新 `TODO.md` 的标题为 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 使用符合仓库风格的提交信息创建一次 git commit，然后停止，不进入下一个任务。

## 进展记录

- 已创建初始计划，下一步读取 `TODO.md` 并定位首个未完成任务。
- 已确认首个未完成任务为 `P4-T01e`：用 `Array` / `MutableArray` 填充 intrinsic 表（IR-direct），删除已被替代的 array runtime helper。
- 最近提交为 `[P4-T01d] Add named method intrinsic table`，内容与当前任务直接相关，但提交信息本身没有额外标出未完成缺陷或新前置项；当前按既定顺序继续执行 `P4-T01e`。
- 下一步：阅读 `sysroot/core.scoop`、array runtime/runtime_abi/runtime_symbols、现有 array lowering callsite 与 array 相关测试，确认当前 helper 边界和最小改动面。
- 已完成第一轮迁移骨架：
  - `sysroot/core.scoop` 的 `Array<T>` / `MutableArray<T>` 已改成 `@Intrinsic class` + method-level `@Intrinsic("array_*")` 声明；
  - resolver 的 direct member lookup 也已对 `List/MutableList/Set/MapView/...` 做 carrier 归一化，避免从 extension surface 切到 method surface 后把别名集合打断；
  - named intrinsic lowering 已接入 `array_size/array_get/array_set/array_data_ptr` 的 IR-direct 路径；
  - 旧 `scoop_array_len/get/set_*` runtime helper 已从 runtime/runtime_abi/runtime_symbols 主线删除。
- 当前剩余工作：修正/替换依赖旧 helper 的编译器测试与 runtime 测试，补 P4-T01e 要求的新 IR/run-pass fixture，并跑编译与质量验证。
- 已补完并通过的新增/更新验证：
  - `cargo test -p scoopc named_intrinsic -- --nocapture`
  - `cargo test -p scoopc array_ -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
  - 新 build fixtures：`array_intrinsic_ir_direct_int_loop_basic`、`array_intrinsic_redundant_get_cse_basic`、`array_intrinsic_write_barrier_ref_set`、`array_intrinsic_composite_copy_set`
  - 新 runtime_gc fixtures：`array_intrinsic_ref_set_gc_move_basic`、`array_intrinsic_composite_copy_gc_move_basic`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下复跑新的 array runtime_gc fixture 与既有 `gc_trace_array_string_elements_basic`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：array 相关回归已恢复；全量 run-pass 仍只剩两个既有失败 `extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 结论：`P4-T01e` 可以闭合；无需新增前置任务，也无需改 `PLAN.md` / `MANAGED_ABI.md`。
