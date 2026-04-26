## 当前目标

按 `TODO.md` 的顺序，本轮要完成首个未完成任务 `T5000e3a 在 corelib / scoop.core 中新增 panic intrinsic，并收口当前直接 abort 路径`，并按 `PROMPT.md` 要求完成收尾：验证、回写 `TODO.md` / `PLAN.md`、提交一次 git commit，然后停止。

## 最新检查结果

1. 最新提交 `f457579c65c335585b0fa833ec43a686a51c2b16 (Update plan)` 只新增了 `MANAGED_ABI.md`，提交说明中没有新的已知阻塞问题。
2. 之前卡住本轮的编译器既有问题已经修复：
   - `crates/scoopc/src/mir/materialize.rs` 的 request-root HIR direct-call 实例收集会把非 concrete 的实例请求提前塞进 materializer；
   - 这会让 `materialize_for_dump_handles_type_body_generic_member_fun_roots`、`materialize_for_dump_distinguishes_companion_member_fun_effect_instances`、`typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls` 等测试多出伪实例；
   - 现已修正为仅把 concrete type/effect args 的实例加入初始请求集合。
3. `panic` 相关主体改动已经存在于当前工作树：
   - `sysroot/core.scoop` 新增 `panic(message: String): Nothing`；
   - LLVM codegen / runtime ABI / C runtime 已新增 `scoop_panic` 入口；
   - `sysroot/task.scoop` 已把语义上属于 fatal trap 的 `exit(3)` 改成 `panic(...)`；
   - 相关 fixture / 文档已同步更新；
   - 新增了 `tests/fixtures/run-pass/core_panic_intrinsic_basic.scoop`。
4. 已完成的验证：
   - `cargo test -p scoopc mir::materialize --no-fail-fast` 通过；
   - `cargo test -p scoopc llvm:: --no-fail-fast` 通过；
   - `cargo run -p scoop -- run tests/fixtures/run-pass/core_panic_intrinsic_basic.scoop` 以退出码 `3` 结束；
   - `cargo run -p scoop -- run tests/fixtures/run-pass/std_process_args_exit_basic.scoop` 正常输出 `0`；
   - `cargo run -p scoop -- build --emit-llvm tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop -o /tmp/task_atomic_claim_no_mutex_llvm.ll` 成功，IR 中包含 `@scoop_panic`；
   - `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 均通过。
5. 新暴露的既有阻塞问题：
   - `cargo run -p scoop -- test` 不再在 `object_member_call_basic.scoop` 失败，但会在 `tests/fixtures/run-pass/std_channels_basic.scoop` 触发编译器 panic；
   - 回溯定位到 `crates/scoopc/src/hir/lower/expr.rs` 的 expected-type hint 路径把 imported/sysroot `FunSig` 里的 `TypeRef` 仍当成“当前源文件”切片；
   - 当 caller 文件包含 UTF-8 中文注释时，foreign span 数字可能落进当前文件的非字符边界，最终在 `crates/scoopc/src/source.rs:63` 触发 `byte index is not a char boundary` panic。

## 当前判断

`T5000e3a` 主体功能已经基本就位，但在正式收尾前又暴露出一条必须先修的编译器既有 bug。因此当前阶段的优先级变为：

1. 先修 imported/sysroot `FunSig` expected-type hint 误用 caller source 的 panic；
2. 重新跑 `std_channels_basic.scoop` 与 `cargo run -p scoop -- test`，确认完整 fixture suite 回到全绿；
3. 再回到 `T5000e3a` 收尾，确认：
   - `panic` 确实是 `Nothing`-typed core intrinsic；
   - 语义上属于 panic/trap 的 sysroot/task 路径已统一走 `scoop.core.panic`；
   - fixture 中不再出现直接 `exit(...)` 调用；
   - `TODO.md` / `PLAN.md` 被回写为完成态；
   - 形成单次 commit 并停止。

## 本轮执行计划

1. 修复 `hir/lower/expr.rs` 中 imported/sysroot `FunSig` expected-type hint 的 foreign source 上下文问题：
   - 不再把 foreign `TypeRef` 直接用当前 caller 的 `SourceFile` 回切；
   - 需要 expected-type/array-literal hint 时，切回声明源文件上下文再 lower。
2. 补一条回归测试，锁住 `std_channels_basic.scoop` 这类 UTF-8 caller + imported signature hint 的 panic。
3. 重新运行验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo run -p scoop -- run tests/fixtures/run-pass/std_channels_basic.scoop`
   - `cargo run -p scoop -- test`
4. 若上述验证通过，再：
   - 将 `T5000e3a` 标记为 `[DONE]` 并补写完成记录；
   - 更新 `PLAN.md` 记录当前完成状态与后续 `T5000e3aR` 切入点；
   - 复查 `memory/claude_plan.md`，补记最终结果；
   - 提交一次 `git commit`，commit subject 使用 `[T5000e3a] ...` 形式。

## 执行约束

- 不接受 workaround，也不通过缩小验证范围来规避现有问题。
- 不回退用户已有改动；只在完成 `T5000e3a` 所需范围内补充修改。
- 本轮只完成一个任务；提交后立即停止，不继续进入 `T5000e3aR`。

## 最终结果

1. `std_channels_basic.scoop` 暴露的 imported/sysroot `FunSig` expected-type hint bug 已修：
   - `crates/scoopc/src/hir/lower/expr.rs` 现会在读取 foreign `TypeRef` hint 时切回声明源文件上下文，而不是继续用 caller `SourceFile` 切片；
   - 这修复了 UTF-8 注释场景下的 non-char-boundary panic；
   - 新增 `crates/scoop/src/commands/build.rs` 中的 `build_frontend_handles_imported_fun_signature_hints_with_utf8_comments` 回归测试锁定该问题。
2. `T5000e3a` 的 panic intrinsic 目标已满足：
   - `scoop.core.panic(message: String): Nothing` 已落位；
   - `sysroot/task.scoop` 的 fatal trap 路径已改走 `panic(...)`；
   - fixture 中 direct `exit(...)` 用法已清理，并新增 `core_panic_intrinsic_basic.scoop`；
   - task atomic trap IR 已确认使用 `@scoop_panic`。
3. 本轮收尾验证全部通过：
   - `cargo test -p scoopc mir::materialize --no-fail-fast`
   - `cargo test -p scoopc llvm:: --no-fail-fast`
   - `cargo run -p scoop -- run tests/fixtures/run-pass/core_panic_intrinsic_basic.scoop`
   - `cargo run -p scoop -- run tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
   - `cargo run -p scoop -- run tests/fixtures/run-pass/std_channels_basic.scoop`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo run -p scoop -- test`（`fixtures: ok (1214)`）
4. 文档回写已完成：
   - `TODO.md` 已将 `T5000e3a` 标记为 `[DONE]` 并补完成记录；
   - `PLAN.md` 已记录 `T5000e3a` 完成与两条既有编译器阻塞修复；
   - 下一步只剩按 `[T5000e3a] ...` 形式提交 git commit，然后停止。

## 2026-04-26 T5000e3aR

### 当前目标

按 `TODO.md` 的顺序，本轮要完成首个未完成任务 `T5000e3aR Review：确认 panic/trap 语义已统一收口到 Nothing-typed intrinsic`，并在完成后回写 `TODO.md` / `PLAN.md`、提交一次 git commit，然后停止。

### 初始检查

1. 最新提交 `76861f6dc7ffe124b8c4860fff329c463df90954 ([T5000e3a] Add core panic intrinsic and seal trap paths)` 的提交说明没有显式新增需要先修复的遗留问题。
2. 当前 review 需要核对的核心边界：
   - `panic` 是否稳定留在 `scoop.core`，而不是再引入一层 process-style 过渡 surface；
   - 返回类型是否真的是 `Nothing`，并被 lowering/codegen 视作 bottom；
   - `sysroot/task` 与 fixtures 是否已经清干净直接 `exit(...)` 的 fatal-trap 用法。

### 本轮记录

1. 首轮静态复核结果：
   - `sysroot/core.scoop` 已声明 `fun panic(message: String): Nothing`；
   - `crates/scoopc/src/llvm/codegen/call/dispatch.rs` 已把 `scoop.core.panic` 分派到 `codegen_sysroot_panic(...)`；
   - `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` 中 `codegen_sysroot_panic(...)` 最终返回 `CgValue::never()`；
   - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c` 已通过 `scoop_panic` 收口 runtime 边界；
   - `sysroot/task.scoop` 中语义上属于 fatal trap 的路径已改为 `panic(...)`；
   - `tests/fixtures/**` 中未再发现直接 `exit(...)` 调用；`tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 已显式断言 `@scoop_panic`。
2. 本轮验证结果：
   - `cargo test -p scoopc llvm:: --no-fail-fast` 通过；
   - `cargo run -p scoop -- test` 通过，输出 `fixtures: ok (1214)`；
   - `cargo test --all` 通过；
   - `cargo clippy --all-targets -- -D warnings` 通过。
3. Review 结论：
   - panic surface 仍稳定位于 `scoop.core.panic(message: String): Nothing`；
   - codegen/runtime 路径统一走 `scoop_panic`，并在 lowering 侧保持 bottom 语义，没有为兼容旧路径退回 `Unit`；
   - `sysroot/task` 与 fixtures 中直接 `exit(...)` 的 fatal-trap 用法已清理，剩余 `scoop.process.exit` 仅作为显式 process-control surface；
   - 未发现需要插入到 `T5000e3b` 之前的新前置缺陷任务。
