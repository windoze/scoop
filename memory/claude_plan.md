# 本轮执行计划

## 目标
- 按照 `TODO.md` 的顺序，只完成第一个尚未完成的任务，然后停止。
- 在进入计划任务前，先检查最新提交是否提到已知遗留问题；若提到，则先修复该问题。
- 在执行过程中，任何发现的既有缺陷、回归、规范不匹配、未完成实现边界或依赖缺口，都必须立即纳入当前范围：优先修复，或者若无法直接完成，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md` 后停止。

## 执行步骤
1. 查看最新一次提交，确认是否显式提到需要先处理的遗留问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并理解当前计划顺序。
3. 判断该任务是否过大或被前置缺陷阻塞。
   - 如果任务过大：在 `PLAN.md` 中拆分，并同步改写 `TODO.md`，把第一个子任务作为本轮目标。
   - 如果存在既有缺陷或缺失能力阻塞：先修复；若当前无法直接修复，则把修复任务前插到 `TODO.md`，更新 `PLAN.md`，提交并停止。
4. 对本轮目标任务做最小必要范围内的代码阅读，确认相关模块、测试和规范约束。
5. 实现任务，保持实现与规范一致，不使用规避性方案。
6. 运行必要验证：
   - 相关定向测试；
   - 必要时运行更广泛的测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 若任务影响编译/运行路径，补充对应验证命令。
7. 更新文档与计划：
   - 在 `TODO.md` 中标记本轮任务完成；
   - 在 `PLAN.md` 中反映当前状态、后续顺序与任何新依赖；
   - 持续更新本文件，记录关键决策、阻塞与完成节点。
8. 检查工作区差异，确认只包含本轮相关改动，不回退用户已有改动。
9. 提交改动，提交信息使用任务标签或能清晰描述本轮工作的格式。
10. 停止，不继续处理下一个任务。

## 当前已知约束
- 所有输出与思考记录使用中文。
- 不能以变通、收窄范围、改夹具形状、特殊分支等方式绕过问题。
- 若 `PROMPT.md` 在过程中出现改动，需要一并纳入提交，不能忽略或回退。

## 更新记录
- 已创建本计划文件，下一步开始检查最新提交和任务列表。
- 已完成最新提交、`TODO.md`、`PLAN.md` 与工作区状态检查：
  - 最新提交标题为 `[T4016T1d3] Support qualified enum variant ctor and when pattern`，标题本身未声明新的“必须先修复”的遗留问题。
  - 当前第一条未完成任务已确认是 `T4016T1d4`：让 single-file / minimal LLVM IR 路径纳入可编译 sysroot 源，与 `scoop build` 保持一致。
- 已完成 `T4016T1d4` 的最小复现与根因定位：
  - `cargo run -p scoopc --features llvm -- --emit-llvm tests/fixtures/run-pass/async_await_minimal_int_basic.scoop ...` 失败，报 `unsupported_main_body: state machine perform effect instance key`；
  - 同一输入经 `cargo run -p scoop -- build --emit-llvm ...` 成功；
  - `cargo run -p scoopc --features llvm -- --emit-llvm tests/fixtures/run-pass/stdlib_string_basic.scoop ...` 失败，报 `unresolved_member: scoop.core.String.substring`；
  - 同一输入经 `cargo run -p scoop -- build --emit-llvm ...` 成功；
  - 由此确认 single-file/minimal LLVM 路径与 build 路径至少存在两处真实偏差：
    1. 未把 `session.sysroot().compilable_source_paths`（当前如 `sysroot/string.scoop` / `sysroot/print.scoop`）并入索引、typecheck、lowering 与 source map；
    2. 仍直接走 `hir::lower_for_dump(session, source)`，绕过了 build 路径使用的完整 typecheck side table / monomorph key 收集 / `lower_for_compilation_unit_multi_files_with_type_env(...)`，导致 handled `Async.await(...)` 这类 effect/state-machine 形状在最小路径上缺失 lowering 所需信息。
- 当前执行方案已细化为：
  1. 在 `crates/scoopc/src/llvm/mod.rs` 内新增“single-file codegen frontend”辅助入口，复用 build 路径所需的 parse / resolve / typecheck / monomorph / multi-file lowering 关键步骤，但作用域仅限“当前源文件 + 可编译 sysroot 源 + 签名型 sysroot”。
  2. 让 `emit_minimal_main_ir` / `emit_minimal_main_obj_to_file` 共用该辅助入口，而不是继续直接调用 `hir::lower_for_dump`。
  3. 扩展 single-file source map，使 codegen 看到的 source set 与 lowering compilation unit 一致。
  4. 添加最小回归：
     - `emit_minimal_main_ir(...)` 对 handled `Async.await(...)` 用例成功生成 IR；
     - `emit_minimal_main_ir(...)` 对 `String.substring(...)` 这类依赖 compilable sysroot 的用例成功生成 IR。
  5. 跑格式化、定向测试、全量测试与 `clippy`，随后更新 `TODO.md` / `PLAN.md` 并提交。
- 执行中新增并已解决的前置问题：
  - 仅纳入 `session.sysroot().compilable_source_paths` 仍不足以与 build 路径一致：`sysroot/string.scoop` 会依赖 `stdlib/mutable_array.scoop` 中的 `__scoop_array_builder_*` 声明；因此 single-file frontend 最终扩展为统一加载 `stdlib/*.scoop` + compilable sysroot sources。
  - 若直接让 LLVM 单测转走 build 同款 frontend，会暴露旧 minimal path 一直绕开的非法测试输入：`@CLayout` 测试使用了旧的 `param = value` 写法，`@Extern` 测试在非 `unsafe` context 直接调用 extern。已把这些测试改成与 build 路径一致的合法输入，而不是放宽单文件前端规则。
- 本轮实现已完成：
  - 已新增 `crates/scoopc/src/llvm/frontend.rs`，并让 `emit_minimal_main_ir(...)` / `build_minimal_main_module(...)` 复用完整 single-file frontend。
  - 已确认 `cargo run -p scoopc --features llvm -- --emit-llvm tests/fixtures/run-pass/async_await_minimal_int_basic.scoop ...` 与 `... stdlib_string_basic.scoop ...` 均恢复成功。
  - 已更新 `TODO.md` / `PLAN.md`：`T4016T1d4` 标记为完成，下一条待办已前移为 `T4016T1d5`。
  - 已完成全量验证：
    - `cargo test -p scoopc --features llvm`
    - `cargo run -p scoop -- test`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 剩余收尾：
  - 检查 diff，提交本轮改动，然后停止。
