执行计划（不包含私有推理链）：

1. 先读取 `TODO.md` 作为索引，并按索引顺序打开对应的 `TODO-Px.md` 详细任务文件，确定第一个标题未带 `[DONE]` 的详细任务。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；只把阻塞当前任务的问题纳入范围。
3. 阅读当前任务的详细要求、依赖、验证标准和完成记录，必要时同步确认相关源码、测试、规范位置。
4. 按任务要求做最小正确实现；若遇到无法绕过的缺失特性或规范不匹配，则在对应 `TODO-Px.md` 中插入最小前置任务，同步 `TODO.md`，提交并停止。
5. 运行相关测试；如修改影响范围较大，再运行更广泛的验证命令，并修复当前任务引入的问题。
6. 将完成任务的详细标题加上 `[DONE]`，更新完成记录，并同步 `TODO.md` 中同一任务的 `[DONE]` 状态。
7. 检查工作区差异，提交本次任务相关的全部变更；提交后停止，不继续下一个任务。

当前状态：已定位第一个未完成详细任务为 `TODO-P7.md` 的 `P7-T02Z`。最近提交 `P7-T02Zb` 的记录提到仍有默认 run-pass 阻塞留给 `P7-T02Z` / `P7-T03`，本轮只处理 `P7-T02Z`。

当前执行步骤：

1. 已检查工作区状态，只有本轮计划文件与后续代码修改；未跟踪的 `crates/scoop/target/` 构建目录不纳入提交。
2. 已复现并修复 `async_await_minimal_int_basic.scoop` 的 `pass MIR local type`：扩展 compiler-temporary slot inference，使 direct call 已 concrete 的 RHS ABI 类型可用于声明类型仍是泛型参数的临时 local。
3. 已通过：`cargo run -p scoop -- run --no-incremental tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`。
4. 已修复 `array_lit_infer_string_char_float_basic.scoop`：`toInt` intrinsic 在 source type 仍是泛型参数但 local slot 已有 concrete ABI 时，使用 slot ABI 继续走 Float/String intrinsic lowering。单 fixture 已通过。
5. 已修复 `effect_escape_continuation_indirect_perform_closure_locals.scoop`：refactor direct entry 对 flattened lambda env tuple 现在会用全部 components 组装 env 参数；同时聚合 local 写入后的 explicit root 同步改为优先从当前 SSA value 抽取 GC leaf，避免回读未同步 alloca leaf。单 fixture 已通过。
6. 已修复 `effect_indirect_perform_nonresuming_function_value_wrapper_member_direct.scoop`：struct 字段为 effect-typed function 时，若字段值来自 plain closure，会在 struct literal 构造前为 closure object 安装 effect-step adapter。单 fixture 已通过。
7. 已修复 `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`：composed call-boundary resume 在调用 callee surface resume 前会按 source consumption 重放 caller boundary prefix。单 fixture 已通过。
8. 已修正 composed call-boundary prefix replay 条件：只在 call boundary 的 owner state 是 Resume state 时重放 prefix，避免普通 indirect perform resume 重复执行初始 handle body prefix。`effect_escape_continuation_indirect_perform_basic.scoop` 与 multi block fixture 均通过。
9. 已修正 composed replay 的二次守护：只有 owner 是 Resume state 且 source slice 从语句 0 开始时才重放 prefix；避免多 call site callee branch 中重复执行已完成的 caller prefix。相关三个 fixture 均通过。
10. 默认 run-pass 继续阻塞在 `effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop`：多个 owner continuation object 共享 owner-trampoline surface-resume schema 时，当前 handoff/ABI query 只能表达单 owner，且 wrapper projection 缺 owner 维度。
11. 已新增 prerequisite `P7-T02Zc` 到 `TODO-P7.md` 并同步 `TODO.md`，`P7-T02Z` 依赖改为 `P7-T02Zc`；本轮不标记 `P7-T02Z` 完成。
12. 已运行 `cargo fmt --all`、多项定向 run-pass fixture、`cargo test -p scoopc --lib effect_lowered`、`cargo test -p scoopc --lib llvm::codegen::effect_refactor`、`cargo clippy --all-targets -- -D warnings`；除已记录的新增前置任务阻塞外，本轮修复验证通过。
13. 下一步检查 git diff，提交本轮代码修复与任务拆分记录后停止。
