# Scoop 0.1 编译器与运行时实现计划（LLVM/inkwell + 早期 C GC → Scoop GC）

> 目标：把 `SCOOP_FULL_SPEC.md` 落地为可用的 `scoopc` 编译器与最小运行时（含 GC、effect runtime、sysroot），并建立一套“可持续扩展”的 fixture/测试体系，保证规范与实现长期一致。

---

## 0. 总体原则（强约束）

1. **永远可回归**：每个阶段都要产出可执行的最小子集（能编译/能跑），并有 fixtures 覆盖新增语义。
2. **规范驱动**：以 `SCOOP_FULL_SPEC.md` 为唯一语言规范来源；代码块示例要能自动变成 fixtures（类似 doctest）。
3. **LLVM 为后端**：所有代码生成走 LLVM IR（Rust `inkwell`），最终产物为 `.o` + 链接运行时。
4. **GC 先 C 后 Scoop**：
   - 早期：GC/运行时用 C 实现，编译依赖 `clang`（可通过 Rust `cc` crate 或显式调用 clang）。
   - 后期：当语言具备 `@NoGC`、`@Unsafe`、指针/原子/线程等能力后，将 GC 逐步迁移到 Scoop 实现。
5. **多线程友好**：effect dispatch/unwinding 的运行时状态必须是 TLS；`Continuation` 允许跨线程 `resume`（语义为恢复其捕获的 handler stack）。

---

## 0.1 维护备注（TODO 顺序）

- 2026-03-23：`TODO.md` 中 effect lowering / async（T0613～T0625）与 effect codegen（T0818）任务原先位于其依赖（T080x/T090x/T091x）之前，导致“首个 `[TODO]` 不可直接实现”。已将这些任务移动到依赖之后，以保持 TODO 的依赖顺序可执行。
- 2026-03-23：`TODO.md` 中 program boundary 的 T0629b 依赖多包 build/link（T1107），原先位于其依赖之前导致“首个 `[TODO]` 不可直接实现”。已将 T0629b 移动到 T1107 之后，以保持 TODO 的依赖顺序可执行。
- 2026-03-25：`TODO.md` 中 T0816（shadow stack 插桩）原先位于其依赖（T0905/T0817）之前，导致“首个 `[TODO]` 不可直接实现”。已将 T0816 移动到 T0905 之后，以保持 TODO 的依赖顺序可执行。
- 2026-03-25：`TODO.md` 中 T0817（heap 分配）依赖 runtime 的 `scoop_alloc`（T0902），原先位于其依赖之前导致“首个 `[TODO]` 不可直接实现”。已将 T0817 移动到 T0902 之后，以保持 TODO 的依赖顺序可执行。
- 2026-03-27：`TODO.md` 中 T0915b（跨线程 resume 的端到端 run-pass fixture）依赖 T0618（跨线程 `resume` 语义/接入），但原先位于其依赖之前导致“首个 `[TODO]` 不可直接实现”。已将 T0915b 移动到 T0618 之后，以保持 TODO 的依赖顺序可执行。
- 2026-03-27：完成 T0618：新增 `__scoop_thread_spawn_join_resume_u64`（sysroot + LLVM codegen 映射 + runtime pthread helper），并新增 run-pass fixture `effect_escape_continuation_resume_cross_thread` 回归跨线程 resume。
- 2026-03-27：完成 T0915b：复用 `effect_escape_continuation_resume_cross_thread` 用例，并回填 `TODO.md` 状态与验收命令。
- 2026-03-27：完成 T0621：新增 run-pass fixture `generator_yield_iter_int_basic`，用 effect + escape continuation（`, k ->`）构造最小 yield/迭代器 demo，并用 stdout golden 回归输出顺序。
- 2026-03-27：完成 T0916：新增 run-pass fixture `effect_handler_stack_nearest_three_levels_and_arm_outside_scope` 回归三层嵌套 handler 的最近匹配与 arm self-capture 避免。
- 2026-03-27：完成 T0625：LLVM codegen 支持最小自定义 non-resuming effect（slot 1-word payload）的 `perform/handle`，并新增 run-pass fixture `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope` 回归嵌套 handler 的最近匹配与 arm re-perform 不自捕获。
- 2026-03-27：完成 T0917：runtime 增加最小 `Task<T>`/executor 原语（task 状态机 + continuation 入队/恢复 + completion 回调 + 显式 start），并新增 `scoop_runtime` 集成测试回归回调顺序与状态转换。
- 2026-03-27：完成 T0918：runtime 增加 once/guard 原语（`scoop_once_begin/scoop_once_end`），LLVM object/companion init 接入该原语，并新增跨线程访问的 run-pass fixture 与 `scoop_runtime` 多线程回归测试。
- 2026-03-27：完成 T0919：runtime 增加 `scoop_once_guard_canonicalize`（基于 `dlsym(RTLD_DEFAULT, ...)` 选取进程内 canonical guard），并新增 `scoop_runtime` 集成测试 `once_guard_cross_dylib` 覆盖“先访问后 dlopen”的动态链接场景（Linux 同步补齐 `-ldl`）。
- 2026-03-27：完成 T1001：parser 支持声明注解使用 `@Name(...)` 并写入 AST（最小支持无参/字面量参数），新增 parse fixture `annotation_use_fun_basic` 与更新相关 AST golden 回归。
- 2026-03-27：完成 T1002：typecheck 增加 `annotation class` 识别与 `@Name(...)` 引用校验（data-only 形态约束 + 非注解类用作注解报错），并新增 typecheck fixtures 覆盖。
- 2026-03-27：完成 T1003：typecheck 增加内建注解 `@Unsafe/@NoGC/@Extern/@Intrinsic` 的最小合法性检查；并对 `@Extern/@Unsafe` 调用点施加 unsafe context 门禁；新增 `tests/fixtures/unsafe_nogc/*` 回归。
- 2026-03-27：完成 T1004：parser/typecheck 支持 `@Unsafe { ... }` 块并在 typecheck 传播 unsafe context；新增 unsafe_nogc fixtures 覆盖“block 内允许调用 @Extern / block 外仍禁止”。
- 2026-03-27：完成 T1005：typecheck 增加 `@NoGC` 的最小静态门禁（禁止调用非 `@NoGC/@Extern`、禁止已知 boxing 分配点），并新增 unsafe_nogc fixtures 覆盖。
- 2026-03-27：完成 T1006：LLVM codegen 支持 `@Extern("symbol")` 的符号名映射与 C ABI 调用；HIR lowering 提取 extern side table；新增 run-pass fixture `extern_symbol_println_basic` 回归。
- 2026-03-27：完成 T1007：sysroot 新增 `@Intrinsic sizeOf` 的最小可调用声明（以 overload 形式暴露），LLVM codegen 将 `scoop.core.sizeOf` lowering 为编译期常量（按 LLVM TargetData 计算 store size），并新增 run-pass fixture `intrinsic_size_of_int_word` 回归。
- 2026-03-27：完成 T1008：sysroot 暴露 `GC.pin/GC.unpin` 并在 typecheck/codegen lowering 到 runtime `scoop_pin/scoop_unpin`；由于当前泛型 struct 布局尚未实现，`Pinned<T>` 暂降级为非泛型 `Pinned`（`value: Any`）；新增 run-pass fixture `gc_pin_unpin_basic` 与 compile-fail fixture `gc_pin_value_type_is_error` 回归。
- 2026-03-27：完成 T1009：typecheck 支持最小 unsafe 指针原语 `addrOf/load/store` 并强制 unsafe context 门禁；新增 unsafe_nogc fixtures 覆盖。
- 2026-03-27：完成 T1010：sysroot 新增 `scoop.unsafe` 模块声明（`Ptr<T>` + `ptrToUIntPtr/uintPtrToPtr`），并新增 resolve fixture 覆盖 `import scoop.unsafe.*` 与符号引用。
- 2026-03-27：完成 T1011：typecheck 为 `scoop.unsafe.Ptr<T>` 增加 pointee 必须为 GC-free 值类型的 well-formedness 校验（含新错误码），并新增 unsafe_nogc fixtures 覆盖 `Ptr<Int>`/`Ptr<String>`/`Ptr<Option<String>>`。
- 2026-03-25：完成 T0902：runtime `scoop_alloc` 改为基于 `malloc` 的最小可用实现，并新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-25：完成 T0819：`scoop build` 支持 `--emit-llvm/--emit-obj/--emit-asm`，fixtures runner 新增 build phase 与 `emit_llvm_basic` 用例（产物写入 `target/fixtures`）。
- 2026-03-25：完成 T0821：runtime 最小字符串承载（`ScoopString`）与 `scoop_print/scoop_println`（C），并新增 clang 链接 smoke test 覆盖输出行为。
- 2026-03-25：完成 T0822：LLVM codegen 支持字符串字面量 lowering，并把 sysroot `print/println(String)` 映射到 runtime `scoop_print/scoop_println`；新增 run-pass fixture 覆盖 `println("hello")` 的 stdout。
- 2026-03-25：完成 T0823：LLVM codegen 支持 f-string 插值（Text/Expr 分片拼接，最小支持 `{String}`/`{Int}`），并新增 runtime `scoop_format_{i64,u64}`；新增 run-pass fixture 覆盖 `val s = f"hi {name} {n}"; println(s)` 的 stdout。
- 2026-03-25：完成 T0824：tuple 字段访问语法统一为 `t._0` / `t._1`，并补齐 `print/println(Int)` 的最小 codegen（runtime formatting + `scoop_print/scoop_println`）；新增 run-pass fixture 覆盖 tuple 求和 stdout 与 `t.0` parse compile-fail 回归。
- 2026-03-25：完成 T0825：`when` codegen 支持 or-pattern（`A | B`）与 guard（`pat if cond`），guard 为 false 时会回落到后续分支；新增 run-pass fixture 覆盖两类语义。
- 2026-03-25：完成 T0826：LLVM codegen 支持 `Option<RefType>` niche 表示与 rich enum 的 oversized variant boxing；run-pass fixtures 新增 `RUN-STDERR-CONTAINS`/`RUN-STDOUT-CONTAINS` 子串断言用于稳定验证 lint warning；新增 run-pass fixtures 覆盖 Option niche 与 oversized boxing。
- 2026-03-25：完成 T0828：LLVM codegen 支持 `object` / `companion object` 的单线程 once 初始化（module-local guard）与静态属性访问；新增 run-pass fixture 覆盖 once 初始化与 `ClassName.member`。
- 2026-03-26：完成 T0615：LLVM codegen 补齐 try/catch/finally 的 `finally` 清理语义（正常路径与 raise/unwind 路径都执行一次），并新增 run-pass fixture 覆盖输出顺序。
- 2026-03-26：完成 T0620：新增 `spawn/join` 结构化并发最小模型（`Int` 句柄 + runtime helper），并补齐 typecheck/run-pass fixtures 与 runtime 测试覆盖。
- 2026-03-26：完成 T0622：引入 `Task<T>` 的最小类型/库模型：sysroot 增加 `Task`，`Async.await` 改为 `Task<T> -> T`；typecheck 侧把 `spawn` 返回值与 `await/join` 参数切到 `Task<T>`；HIR/codegen 侧把 `Task` 暂落到 word-sized 句柄并保持 run-pass 可回归。
- 2026-03-26：完成 T0623：支持 `async fun` 降糖到 `Task<T>`：parser 增加 `async` modifier；resolve/index 与 typecheck 侧保证调用点返回 `Task<T>`；`async fun` 函数体内的 `Async` performed effects 不向外层 required effects 传播；HIR lowering 将返回值包装为 task 句柄（`__scoop_task_spawn_int`，early stage）。
- 2026-03-26：完成 T0818：effect codegen（flag-based Raise/try-catch）：支持 `Raise<RuntimeError>` payload 写入/恢复、`EnumName.UnitVariant` 常量生成，并新增 run-pass fixture 回归。
- 2026-03-26：完成 T0630：perform slot ABI 升级为多 word payload（`len + words[8]`），新增读写 API；`Raise.raise` 统一采用 2-word `(kind, value)` 编码并在 handler 边界断言回归，确保 lowering/codegen/runtime 对齐。
- 2026-03-26：完成 T0907：runtime 引入 type descriptor v0（`size_bytes + trace bitmap/trace_fn`），并新增 guard page 集成测试确保扫描按 size 裁剪不越界。
- 2026-03-26：完成 T0908：runtime 对象头（`ScoopGcObjectHeader`）与最小 heap 布局：`scoop_alloc` 初始化 header 字段并固化字段偏移（static asserts），新增对象头集成测试，并同步更新 `Int -> Any` 装箱布局为 `{ header, payload }`。
- 2026-03-26：完成 T0909：GC v0 单线程 shadow stack roots 扫描（visitor API），新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-26：完成 T0910：GC v0 单线程 mark-sweep（手动触发 `scoop_gc_collect`），新增 heap 统计 debug API、runtime 集成测试与 run-pass fixture 覆盖。
- 2026-03-23：完成 T0624：use-site `Type<eff Row>` 的默认化/实例化接入 typecheck，并让名义类型的 `eff` row 参数参与 subeffecting；补齐从 `Type<eff E>` 实参类型推断 `E` 与 required effects 联动的 fixtures 覆盖。
- 2026-03-23：完成 T0626：parser/AST 支持闭合 effect row `E!` 语法（`!` 低于 `+`，作用于整个 row），并新增 parse fixtures 覆盖。
- 2026-03-23：完成 T0627：typecheck 侧为 entry point 补齐闭合 row `Pure!` 的门禁与诊断（显式写 open `/ Pure` 会提示改为 `Pure!`），并新增 `Pure!` + try/catch / unhandled Raise fixtures 覆盖。
- 2026-03-23：T0628（RowExpr 高级语义）跨度较大，已拆分为 T0628a/T0628b，先以 `E + ...` 的实例化/推断为最小可回归落点。
- 2026-03-23：完成 T0628a：typecheck 侧支持 `E + R` 形式的 row（函数类型 `/ Row` 与 use-site `Type<eff Row>`），调用点按 `found - base` 推断并回填实例化结果，新增 infer/effects fixtures 覆盖。
- 2026-03-23：完成 T0628b：引入 `TypeId` 级的 row 替换 plan，支持在 tuple/Option/多层 function type/nominal args 中实例化 `E + ...`，并补齐闭合 row 引用 row 变量（`E!`）的稳定诊断与 fixtures 覆盖。
- 2026-03-23：完成 T0629a：program boundary 的 entry point 引入 cone-aware 规则（仅 consumer cone 的 `main` 视为 entry point），并新增 `typecheck_cone` fixtures runner 与用例覆盖。
- 2026-03-23：完成 T0701：新增 `scoopc::hir` 骨架与最小 lowering，并落地 `scoop dump-hir` 调试输出命令。
- 2026-03-23：完成 T0612：HIR 增加 `perform/handle` 节点与 lowering；MIR 预留对应 terminator；新增 HIR fixtures 回归。
- 2026-03-23：完成 T0704：新增 `scoopc::monomorph::MonomorphKey`（symbol + type args + effect row args）与单测覆盖，用于后续实例缓存。
- 2026-03-24：完成 T0706：AST→HIR lowering 补齐 `member access`（`receiver.member`）节点与解析结果写入，并新增 HIR fixtures/golden 回归覆盖成员访问/成员调用/成员赋值。
- 2026-03-24：完成 T0707：MIR 引入 cleanup/unwind 最小模型（`UnwindAction` + `Terminator.unwind` + `ResumeUnwind`），并新增 MIR 单测覆盖。
- 2026-03-24：完成 T0708：引入最小 MIR lowering（if/when → CFG），新增 `scoop dump-mir` 与 `tests/fixtures/mir/**` golden 回归。
- 2026-03-24：完成 T0709：MIR lowering 补齐 while/break/continue（loop CFG + 跳转目标栈），并新增 `tests/fixtures/mir/while_break_continue.*` 回归。
- 2026-03-24：完成 T0711：捕获闭包（val 捕获）落地 capture set 计算与 env tuple lowering（`MakeTuple`/`TupleGet`），并新增 HIR/MIR fixtures 回归。
- 2026-03-24：完成 T0712：单态化 v0（函数泛型）：typecheck 收集 `MonomorphKey`，并提供 `scoop dump-ir` 输出单态化实例 MIR（`id::<Int>`/`id::<String>` 两实例可回归）。
- 2026-03-24：完成 T0713：MIR lowering 把 HIR `perform/handle` 落到 MIR terminator（`Perform/Handle`），并新增 `tests/fixtures/mir/handle_perform.*` golden 回归。
- 2026-03-24：完成 T0714：捕获闭包对 `var` 引入 CaptureBox 语义（HIR capture 标记 `mutable`；MIR 新增 `CaptureBoxNew/Get/Set` 并在函数内预扫描 closure captures 决定 boxing），新增 `tests/fixtures/{hir,mir}/closure_capture_var.*` 回归覆盖。
- 2026-03-24：完成 T0801：为 `scoopc` 增加 feature-gated `inkwell` 依赖（`llvm` feature 默认关闭）以保持 CI/本地构建可用，并在 `README.md` 说明启用方式与 LLVM/`llvm-config` 前置。
- 2026-03-24：完成 T0802：新增 `scoopc::llvm` 最小 codegen（生成空 `main` 返回 0），并提供 `scoopc --emit-llvm` 写出 `.ll` 用于验证 target triple/pipeline。
- 2026-03-24：完成 T0804：新增 `scoopc --emit-obj` 把最小 LLVM module 编译为 `.o` 并落盘，补齐单测覆盖产物非空，为后续链接（T0806）做准备。
- 2026-03-24：完成 T0805：driver `scoop build` 接入前端 parse/resolve/typecheck，并准备输出路径（当前不做 codegen/链接）。
- 2026-03-24：完成 T0806：driver 在启用 `--features llvm` 时生成 `.o` 并调用 clang 链接早期 C runtime，产出可执行文件（单测覆盖 link 与运行返回 0）。
- 2026-03-24：完成 T0807：driver 实现 `scoop run`（临时目录 build + exec，stdout/stderr 与退出码透传）。
- 2026-03-24：完成 T0112：run-pass fixtures runner 让 `EXPECT-EXIT`/`TIMEOUT` 真正生效，并新增超时/信号终止/退出码不匹配的稳定诊断与 fixtures 覆盖。
- 2026-03-24：完成 T0108：fixtures 支持 `// ENV: KEY=VALUE` 指令，run-pass 执行子进程时注入 env，并新增单测覆盖。
- 2026-03-24：完成 T0808：LLVM codegen v1 支持 Int/Bool 字面量、一元/二元运算（含位运算/移位与 shift count mask）、`val` 局部绑定、`return`/隐式返回，并新增 run-pass fixture 覆盖 `UInt8 >>` 的逻辑右移语义。
- 2026-03-24：完成 T0809：LLVM codegen v2 将 main 内 locals 统一映射为 `alloca` + `load/store`，并支持 `var` 的赋值更新；新增 run-pass fixture 覆盖读写回归。
- 2026-03-24：完成 T0810：LLVM codegen v3 支持顶层函数调用（按简单 C ABI 传参/返回），并且只为 `main` 可达的函数生成/声明，避免未使用的泛型/占位签名影响 codegen；新增 run-pass fixture 覆盖 `add(1,2)`。
- 2026-03-24：完成 T0811：LLVM codegen v4 支持 struct 值类型布局与字段访问：为 struct FQN 生成 named LLVM struct type（opaque + set_body），struct literal 构造用 `insertvalue` 组装 aggregate，字段读取对 `localStruct.field` 走 struct GEP + `load`；新增 run-pass fixture 覆盖 struct literal + 字段读取（当前因 typecheck 对整数字面量推导为 `Int`，fixture 字段类型使用 `Int` 并用 exit code 断言结果）。
- 2026-03-24：完成 T0812：LLVM codegen v5 支持 tuple 值类型布局与元素访问：为 tuple 生成 LLVM struct type，tuple literal 用 `insertvalue` 组装 aggregate；`t._0` / `t._1` 在局部变量路径走 struct GEP + `load`（fallback 用 `extractvalue`）；同时在 typecheck 中支持 tuple 元素访问语义以通过前端检查；新增 run-pass fixture 覆盖 tuple 求和并用 exit code 断言结果。
- 2026-03-24：完成 T0813：LLVM codegen 支持 rich enum 的最小 `{tag, payload}` 表示（payload 为 word-sized int），并在“期望类型语境”下支持 enum variant ctor（含 0-参数 variant 以 `None()` 形式构造），以及 `when` 对 enum 的 tag 判别与 `Some(v)` binder 解构；新增 run-pass fixture 覆盖 `Some(1)`/`None()` + `when` 分支并用 exit code 断言结果。
- 2026-03-24：完成 T0814：LLVM codegen 将 enum/bool 的 `when` 降到 LLVM `switch`（保持“按源码顺序”的首个匹配 arm 语义），并支持 tuple `when` 的字段比较与 binder；新增 run-pass fixture 覆盖 enum/bool/tuple 三类 `when` 并用 exit code 断言结果。
- 2026-03-25：完成 T0815：生成的 `i32 @main()` 在执行 Scoop `fun main` 前调用 `scoop_runtime_init()`，并更新 LLVM 单测断言 IR 含该调用。
- 2026-03-25：完成 T0901：补齐 C runtime 的 `scoop_runtime_init`（一次初始化标记 + 可选 debug 日志），并新增 `scoop_runtime` 集成测试覆盖可调用性与可观察状态。
- 2026-03-25：完成 T0904：引入 mark-sweep GC 的数据结构骨架（heap/object header/free list）与最小自检，并让 clang 链接覆盖 `runtime/c/*.c`。
- 2026-03-25：完成 T0905：shadow stack `ScoopGcFrame` + TLS 链头（`current_frame`）与 push/pop API，并新增 `scoop_runtime` 集成测试覆盖。
- 2026-03-25：完成 T0816：LLVM codegen 为含 GC 引用的函数插桩 shadow stack frame（push/pop + roots 写入），runtime 新增 debug 扫描计数接口，并新增 run-pass fixture 覆盖。
- 2026-03-25：完成 T0613：补齐 effect runtime 的最小 ABI（active flag + perform slot 的读写 API），并在 LLVM codegen 侧提供 sysroot `__scoop_effect_*` 映射；新增 `scoop_runtime`/`scoopc` 单测与 run-pass fixture 回归覆盖。
- 2026-03-25：完成 T0614：实现 `Raise.raise` 的最小 flag-based unwinding（写 slot+flag、call-site 传播、try/catch 边界消费 slot 并清 flag），并新增 run-pass fixture；同时修复 resolver 对 try/catch lowering 合成 Ident 的 FQN 推导（确保可解析到 `scoop.core.Raise.raise`）。

## 1. 仓库结构与工具链（阶段 0：工程化）

### 1.1 代码结构（Rust workspace 拆分）

- [x] `crates/scoopc/`：编译器前端 + 中端 + LLVM 后端（inkwell）（初始骨架已建立）
- [x] `crates/scoop/`：CLI（`scoop build/run/test`），负责调用 `scoopc`、链接、跑测试（已建立骨架）
- [x] `crates/scoop_runtime/`：早期运行时构建 glue（clang + C runtime）（已建立骨架）
- [x] `runtime/c/`：早期 C 运行时（GC + 基础内建 + 线程注册 + effect TLS）（已建立占位实现）
- [x] `sysroot/`：`.scoop` 形式的内建 API 声明（当前已包含 `core.scoop` + `delegates.scoop` + `collections.scoop` 的最小集合；已补齐最小 I/O（print/println，T0820）；后续补齐 integers/aliases、intrinsics、unsafe/ptr、gc、更多 io 等）
- [x] `tests/fixtures/`：所有编译期/运行期 fixtures（见 §10）（已建立最小 smoke）
- [x] `tools/`：辅助脚本（已加入 `tools/scoop_tools`：spec doctest fixtures 抽取/一致性检查；后续扩展 golden 工具）

> 现阶段仓库还很小，可以先在单 crate 内落地；当模块多起来再迁移到 workspace。

### 1.2 基础构建与开发体验

- [x] 引入依赖：`clap`、`thiserror`、`miette`（诊断）、`tracing`（后续再引入 `inkwell`）
- [x] 统一日志：`tracing` + `tracing-subscriber`
- [ ] 提供命令行（拆分为可迭代子任务）：
  - [x] `scoop test`（fixtures harness，当前为最小 smoke）
  - [x] `scoop dump-ast`（当前为占位信息输出）
  - [x] `scoop dump-hir`（HIR Debug 输出；用于后续 lowering/回归）
  - [x] `scoop dump-ir`（单态化实例 MIR Debug 输出；用于回归/调试）
  - [ ] `scoop build <main.scoop> -o <bin>`（T0805：前端检查/输出路径已落地；待 codegen + 链接）
  - [ ] `scoop run <main.scoop>`（待 build 可用后落地）
- [x] `build.rs`：编译 `runtime/c`（强制 clang；当前通过 `crates/scoop_runtime` 实现）
- [x] CI：最小矩阵（ubuntu）跑 `cargo test --all` + `scoop test`

**本阶段 DoD**
- 能构建出 `scoop` 可执行文件（哪怕只是空壳），`scoop test` 能跑一个最小 fixture。

---

## 2. 词法/语法/AST（阶段 1：前端可解析）

### 2.1 词法分析（Lexer）

- [x] Token 集：关键字、标识符、数字、字符串、基础运算符、注解（`@`）、泛型尖括号、常用 modifier（`public/internal/private/open/abstract/sealed/inline/override`）等（见 `scoopc::syntax::lexer`）
- [x] 补齐位运算与移位运算符 token：`&` `|` `^` `~` `<<` `>>`（spec §2.3.4 / Appendix B.8）
- [x] 注释：行注释 `//`、块注释 `/* */`（当前实现为**非嵌套**；若后续需要可扩展为嵌套）
- [x] 字符串：
  - 普通字符串（`"..."`）
  - `f` 插值字符串（`f"..."`）（lexer 识别字面量边界；parser 将 f-string token 拆为文本段 + 插值 expr 列表，AST `FStringExpr`/`FStringPart` 已实现）
  - raw 三引号字符串（`""" ... """`）与 `f""" ... """`
  - 大括号转义（`{{` / `}}`）属于字符串内容层语义，lexer 无需特殊处理
- [x] Span（源代码位置）基础设施：`Span` + `SourceFile` 行列映射

### 2.2 语法分析（Parser）

- [x] Parser v0（最小可用）：支持 `package` / `import` / 顶层 `fun` + 基础类型声明（`class/interface/struct/enum/effect`），函数/类型体仅保证 `{ ... }` 括号平衡并记录 span
- [x] fun 签名最小解析：参数列表 + 返回类型（支持 Path/泛型参数列表/tuple/nullable 的 `TypeRef` 子集）
- [x] 工程化：拆分 `scoopc::parser` 为多文件模块（cursor/decls/types/file），避免单文件过长，便于后续语句/表达式迭代
- [ ] Kotlin-like 声明（逐步补齐）：`class/interface/struct/enum/effect/val/var/...`
  - [x] 顶层 `val`/`var`：解析声明头；initializer 暂仅保留 span（不解析表达式）
  - [x] 类型体内部成员声明：`val`/`var`/`fun`/nested type（T0201：TypeBody + Member 建模，parse_type_body 实现）
  - [x] 类型体 `val`/`var` 成员声明头：解析 `val x: T`/`var x: T`，带 pass/fail fixtures 覆盖（T0202）
  - [x] 类型体 `fun` 成员声明头：解析 `fun name(params): Ret { ... }`（body 仍是 span），含 pass/fail fixtures 覆盖（T0203）
  - [x] 类型体嵌套类型声明：class/interface/struct/enum/effect 均可作为成员，支持多层嵌套与修饰符（T0204）
  - [x] 声明修饰符列表：顶层与类型成员支持 `public/internal/private/open/abstract/sealed/inline/override`；AST 保存 `modifiers` 并排序去重（顺序无关）；新增 parse fixture 覆盖（T0245）
  - [x] class/interface 继承列表与主构造头（简化版）：解析 `class Dog(name: String) : Animal(name), IFoo` 的最小语法；AST `TypeDecl` 新增 `primary_ctor`/`supertypes`；新增 pass/fail fixtures 覆盖（T0248）
  - [x] 属性声明与 accessors：`ValDecl` 新增 `accessors: Vec<Accessor>` 字段；`Accessor` 节点支持 `get()`/`set(value)` + 表达式体（`= expr`）或块体（`{ stmts }`）；类型体中 `parse_property_decl` 在 `parse_val_decl` 后探测 `get(`/`set(` 模式并解析 accessor；`get`/`set` 作为上下文关键字（soft keyword），不加入 lexer 关键字表；6 个 pass/fail fixtures + 5 个 unit tests 覆盖（T0234）
  - [x] 委托属性 `by expr`：`ValDecl` 新增 `delegate: Option<Expr>` 字段；`parse_property_decl` 在 `parse_val_decl` 后探测 `by` 上下文关键字并解析委托表达式；`by` 与 accessors 在语法层互斥；支持 `val x: T by lazy { ... }` 等 trailing lambda 形式；2 个 pass/fail fixtures + 3 个 unit tests 覆盖（T0235）
  - [x] Rich enum variant 声明：`Member::Variant(EnumVariant)` 新增 AST 节点；`EnumVariant` 含 `name: Ident` + `params: Vec<Param>`；`parse_type_body` 接收 `TypeKind` 参数，对 `Enum` 类型识别裸标识符作为 variant 开始；`parse_enum_variant` 解析 `Name` / `Name(val field: T, ...)` 形式；variant 参数要求 `val` 关键字 + 类型标注；1 个 pass + 2 个 fail fixtures + 3 个 unit tests 覆盖（T0236）
- [x] `typealias` 声明：解析顶层 `typealias Name = Type` 并纳入 AST（T0251，为 sysroot 标准别名与 Kotlin 兼容铺路）
- [x] Expr/Stmt 最小骨架（T0205）：Ident/IntLit/StringLit/BlockExpr/Missing + Stmt::Expr/Stmt::ValDecl
- [x] val/var initializer 解析为原子表达式（T0206）：`ValDecl.init` 从 `Option<Span>` 升级为 `Option<Expr>`，支持 ident/int/string 原子
- [x] 块表达式解析（T0207）：`parse_block_expr` 解析 `{ stmt* }` 为 `BlockExpr { stmts }`；`FunBody::Block` 改用 `BlockExpr`（含 stmts）替代旧 `Block`（仅 span）；块内支持表达式语句与 val/var 声明
- [x] 块内 val/var 局部绑定（T0208）：`parse_stmt` 已支持 `val x: T = expr`/`val x = expr`/`var x = expr`；新增 pass/fail fixtures 覆盖（含 `val = 1` 缺名报错）
- [x] 函数调用表达式（T0209）：`parse_expr` 引入后缀调用循环，解析 `f(a, b)` 为 `CallExpr { callee, args }`；支持嵌套调用 `f(g(x))`、尾随逗号；`parse_stmt` 和 `ValDecl.init` 改用 `parse_expr`
- [x] 成员访问表达式（T0210）：后缀循环新增 `.` 分支，解析 `a.b` 为 `FieldAccessExpr { receiver, field }`；支持链式 `a.b.c(1)` 与调用组合
- [ ] 语句/表达式（逐步补齐）：lambda
  - [x] Lambda AST 节点：`Expr::Lambda(LambdaExpr)` + `LambdaParam`（T0221）
  - [x] Lambda 表达式解析：`{ params -> body }` / `{ -> body }` 的 lookahead 歧义消解 + 参数列表 + block body 解析；6 个 pass/fail fixtures 覆盖（T0222）
  - [x] Trailing lambda：`f(a, b) { ... }` 与 `expr { ... }` 形式，尾随 lambda 作为最后一个 `CallArg::Positional(Lambda)`；bare `{ body }` 无 `->` 时解析为零参数 lambda；5 个 pass fixtures 覆盖（T0232）
- [x] `when` 表达式解析：`when (subject) { pattern -> body, ... }`（T0215：AST `WhenExpr`/`WhenArm`/`WhenPattern` + parser + pass/fail fixtures）
  - [x] Pattern v0（T0238）：`WhenArm.pattern` 迁移为 `Pattern`，删除 `WhenPattern`；支持 wildcard `_`、int/string/bool 字面量、`is`/`!is` Type、`else`、裸标识符 bind；2 个 pass fixtures + 6 个 unit tests 覆盖
  - [x] Pattern v1 — tuple pattern（T0239）：`parse_when_pattern` 新增 `(` 检测调用 `parse_tuple_pattern()`，解析 `(p1, p2, ...)` 为 `Pattern::Tuple`；支持嵌套 pattern、尾随逗号、空 tuple `()`；`no_call` 标志 + `looks_like_tuple_pattern_ahead()` lookahead 消解 arm body call 与下一 arm tuple pattern 的歧义；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v2 — enum variant pattern（T0240）：`parse_when_pattern` 在裸标识符后 peek `(` 调用 `parse_variant_pattern()`，解析 `Name(p1, p2, ...)` 为 `Pattern::Variant`；支持嵌套 variant（`Some(Some(x))`）、空参数（`Point()`）、尾随逗号、wildcard 字段；裸标识符（无括号）保持为 `Bind`（消歧留给 resolve 阶段）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v3 — struct pattern（T0241）：`parse_when_pattern` 在裸标识符后 peek `{` 调用 `parse_struct_pattern()`，解析 `Name { field, field: pattern, ... }` 为 `Pattern::Struct`；支持 shorthand（`x`）、rename（`x: pattern`）、空 struct（`Unit {}`）、尾随逗号、嵌套 pattern（`first: Some(x)`）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v4 — or-pattern（T0242）：`parse_when_pattern` 拆分为 `parse_when_pattern`（含 `|` 循环）+ `parse_when_pattern_atom`（单个 pattern）；`A | B` 解析为左结合 `Pattern::Or`；支持多级 `A | B | C`、嵌套在 tuple/variant/struct 内的 or-pattern、混合 literal/bind/variant/wildcard；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v5 — guard `if <expr>`（T0243）：`parse_when_arm` 在 pattern 与 `->` 之间检测 `if` 关键字，解析 guard 表达式并包装为 `Pattern::Guard`；`looks_like_tuple_pattern_ahead` 更新为同时接受 `->` 和 `if` 作为 tuple pattern 判定条件；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
- [x] `if` 表达式解析：`if (cond) thenExpr else elseExpr`（T0214：AST `IfExpr` + parser + pass/fail fixtures）
- [x] 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）（T0216：AST `WithExpr`/`WithField` + parser + pass/fail fixtures）
- [x] 运算符优先级（Pratt parser）：二元运算 `+ - * / %`、比较 `< > <= >= == !=`、逻辑 `&& ||`、位运算 `& | ^`、移位 `<< >>`；一元前缀 `- ! ~`（T0252）；括号分组 `(expr)`；`Percent` token 新增
- [x] Elvis `?:` 二元运算（最低优先级）与 not-null 断言 `!!` 后缀运算（T0212）
- [x] 类型判断/转换操作符：`is`/`!is`/`as`/`as?`（与比较运算符同优先级，RHS 为 TypeRef）（T0213）
- [x] 声明处泛型参数列表：`fun id<T>(...)` / `struct Box<T> { ... }` — AST `TypeParam` 节点 + `type_params` 字段 + `parse_type_param_list`（T0218）
- [x] 泛型语法补齐：type args 支持 `*`（star projection），type params 支持 `in/out` 声明处变型（T0249）
- [x] struct literal AST 节点：`Expr::StructLit(StructLitExpr)` + `StructLitField`（T0223）
- [x] struct literal 解析：`TypeName { field: expr, ... }`（T0224）— `looks_like_struct_lit()` lookahead 在 `parse_expr_primary` 中识别 `Ident(.Ident)*(<...>)? { (Ident: | })` 模式，调用 `parse_struct_lit_expr()` + `parse_path_type_inner()` 解析；6 个 pass/fail fixtures 覆盖
- [x] 关键歧义：struct literal vs lambda（对应 spec §12）（T0225）— `looks_like_struct_lit()` 增加 `has_arrow_inside_braces()` 扫描：在 `{ Ident :` 匹配后，前扫顶层 `->` 来排除 lambda with typed params；4 个 pass fixtures 覆盖
- [x] `return` 语句解析：`Stmt::Return(ReturnStmt)` + `parse_return_stmt`（T0226）— 支持 `return` 与 `return expr`；3 个 pass fixtures 覆盖
- [x] 赋值语句解析：`Stmt::Assign(AssignStmt)` + `parse_stmt` 中 `= rhs` 检测（T0227）— 支持 `x = expr` 与 `a.b.c = expr`；2 个 pass + 1 个 fail fixtures 覆盖
- [x] `while` 循环表达式解析：`Expr::While(WhileExpr)` + `parse_while_expr`（T0228）— 支持 `while (cond) body`；`break`/`continue` 作为 `Stmt::Break`/`Stmt::Continue`；lexer 新增 `While`/`Break`/`Continue` 关键字；2 个 pass + 1 个 fail fixtures 覆盖
- [x] 错误恢复：`parse_file_recovering()` 新 API，顶层/块内/类型体三级同步点恢复，收集多个诊断
- [x] safe-call `?.`：`FieldAccessExpr` 与 `CallExpr` 新增 `safe: bool` 标志；postfix 循环处理 `QuestionDot` token，支持 `x?.member` 与 `x?.foo(args)`；2 个 pass + 1 个 fail fixtures 覆盖（T0229）
- [x] 函数参数默认值：`Param` 新增 `default: Option<Expr>` 字段；`parse_param_list` 解析 `= expr`；1 个 pass + 1 个 fail fixtures 覆盖（T0230）
- [x] 命名参数调用：新增 `CallArg` 枚举（`Positional(Expr)` / `Named { name, value }`）；`CallExpr.args` 改为 `Vec<CallArg>`；`parse_call_arg` 通过 lookahead `Ident + =` 区分命名参数与位置参数；2 个 pass fixtures 覆盖（T0231）
- [x] 扩展函数 receiver：`FunDecl` 新增 `receiver: Option<TypeRef>` 字段；`parse_fun_receiver_and_name` 通过 lookahead 识别 `Type.name(...)` / `pkg.Type.name(...)` / `List<T>.name(...)` 模式并拆分 receiver 与函数名；type params 支持 spec 风格 `fun <T> Type.name(...)` 和 Kotlin 风格 `fun name<T>(...)`；resolve 侧同步处理 receiver TypeRef；3 个 pass + 1 个 fail fixtures + 5 个 unit tests 覆盖（T0233）

### 2.3 语法树表示（AST/Parse Tree）

- [ ] 建议区分：
  - `ParseTree`（保留所有 token/节点，利于错误恢复与格式化）
  - `AST`（更语义化的节点，利于后续分析）
- [x] AST（最小骨架）：File/Package/Import/Fun/TypeDecl/Block/Ident/Param/TypeRef，节点带 span 并可回切源文本
- [x] Pattern AST 节点（T0244/T0460）：新增 `Pattern`（Wildcard/Bind/Tuple/Struct/Variant）与 `ValBinding`，用于 block 内 `val` 解构绑定；`when` 分支模式仍使用 `WhenPat`（后续再统一迁移）
- [ ] Parser 收尾补齐：
  - [x] `import foo.bar.Baz as Qux`（Appendix B.7）
  - [x] use-site effect row 实参：`Type<eff Row>`（spec §3.4）
  - [x] pattern rest：`..`（spec §4.2）
  - [x] receiver function type：`T.(A, B) -> R / E`（spec §7.5）
  - [x] 泛型 `where` 子句（spec §3 / Appendix B）
- [ ] Kotlin-like 声明补齐：
  - [x] `init { ... }` blocks（Appendix B.2.2）
  - [x] secondary constructors（Appendix B.2.2）
  - [x] `object` / `companion object` 声明（Appendix B.9）

**本阶段 DoD**
- `scoopc` 能解析大部分 spec 示例，不做类型检查也能 `dump-ast`。

---

## 3. 包与名字解析（阶段 2：可绑定符号）

### 3.1 包系统（Cone 的源级部分）

- [x] `package` 声明、`import`、通配 `*`（已支持解析 + 最小名字绑定：TypeRef 按 import/star import 解析）
- [x] 可见性：`public/internal/private`
- [ ] 作用域：文件级、类/接口/结构体内部、泛型参数作用域（块级局部 `val/var` 已完成，见 T0304）

### 3.2 符号表与解析

- [x] 顶层符号索引（最小子集）：基于 `package + 顶层声明名` 构建 FQN 索引并检测重复定义；索引区分 type/fun/value 命名空间（见 `scoopc::resolve`）
- [x] 类型体成员索引：把 type body 的 fields/methods/nested types 纳入索引并检测同一类型体内重复定义（T0302）
- [x] 两阶段/多阶段解析（T0308）：
  - 先收集声明头（type/function/field signatures）
  - 再解析函数体与初始化表达式
- [x] import 解析与名字绑定（最小子集）：对 fun/val 顶层签名里的 `TypeRef::Path` 做存在性解析（含 star import）
- [x] import 表（T0303）：显式 import 按 type/value 命名空间拆分，并保留 `*` import 前缀（为 expr 解析准备）
- [x] `typealias` 名字解析：alias 作为 type-level symbol 纳入索引；冲突与可见性诊断
- [x] 作用域：块级（函数体/表达式块内局部 `val/var`，含遮蔽）（T0304）
- [x] 表达式裸标识符绑定写回：为 `ExprKind::Ident` 记录其解析到的局部/顶层引用（T0305）
- [x] 调用点候选收集：`Call(Ident)`/成员调用/构造调用写回候选集合 + 调用形状；多候选留给后续 typecheck 决议（T0319）
- [x] 成员访问解析（`.`）：把 `receiver.member` 绑定到类型体字段/方法并写回 `MemberIdent.resolved`（T0310）
- [x] 扩展成员 fallback：member 优先于 extension（同包）且 receiver 类型可匹配（T0312）
- [x] 作用域：泛型参数（声明处 type params 在签名内可解析）（T0309）
- [x] `where` 子句约束解析：约束左侧必须命中 type param scope，右侧 `TypeRef` 按包前缀/import 规则解析（T0320）
- [x] 作用域：`this`（类型体成员/扩展函数体）与主构造参数在成员里可见（T0313）
- [ ] 同名优先级：成员/顶层/扩展（逐步补齐）
- [x] import alias 绑定与冲突规则：`import foo.bar.Baz as Qux`（Appendix B.7）
- [x] class 初始化阶段作用域：property initializer / `init` / secondary constructor（T0316）
- [x] `object` / `companion object` 的名字解析与成员可见性（T0317：支持 `Obj.member` 与 `ClassName.member`）
- [ ] overload set 建模：
  - [x] 索引侧：顶层/成员/扩展函数与构造函数收集为候选集合（T0318）
  - [x] 调用点/构造点：从“唯一 callee”升级为“候选集合 + 调用形状”（T0319）
  - [x] typecheck：普通函数调用最小重载决议（T0453：过滤后唯一/歧义）
  - [x] typecheck：class 构造调用最小重载决议（T0454：primary/secondary + 默认参数）
  - [x] typecheck：扩展函数调用重载决议（T0455：member 优先 + receiver/参数 specificity）
  - [x] typecheck：重载冲突诊断（T0457：重复签名 / 仅返回类型不同 / 默认参数冲突）
  - [x] inference：重载决议与泛型/默认参数/命名参数/`eff` row 推断联动（T0512）
  - [x] inference：most-specific tie-break（T0513：参数/receiver 更具体 + 默认参数更少优先；歧义诊断列出候选签名）
- [ ] 跨包可见性：`public/internal/private` 在 source package / `.cone` 依赖边界上的规则与诊断（拆分为子任务；T0321b 依赖 T1105 `.cone` 读取，已在 TODO 中延后）
  - [x] T0321a：resolver 引入 cone 边界 + source-only 多 cone fixtures
  - [ ] T0321b：接入真实 `.cone` 依赖后的可见性过滤（等待 T1105）
- [ ] 跨包扩展导入：extension 在显式 import / star import / 成员候选之间的可见性、shadowing 与候选收集（依赖 T0321b）

### 3.3 sysroot 注入

- [x] sysroot 文件与 loader 骨架：可发现并解析 `sysroot/*.scoop`（当前实现见 `scoopc::sysroot`）
- [x] 编译流程注入：通过 `scoopc::session::Session` 默认加载 sysroot，并在 `build_top_level_index` 中纳入名字解析环境
- [x] sysroot：补齐内建标量类型的“可见声明”（spec §2.3.4 / runtime §3）
  - `Int/UInt`：word-sized（随 target 指针宽度变化，Swift 约定）
  - 固定位宽整数：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 说明：这些类型是语言 builtin（布局/语义由编译器固定），但它们的可见声明由 sysroot 提供
  - fixtures：`tests/fixtures/resolve/sysroot_scalar_types_ok.scoop`
- [x] sysroot：运行时错误枚举 `RuntimeError`（`NullAssertionFailed`/`ClassCastFailed`），用于 `Raise<RuntimeError>`（T0419）

**本阶段 DoD**
- 能在无类型检查情况下做 name resolution，并对未定义符号给出准确 span 的错误。

---

## 4. 类型系统（阶段 3：先类型检查再优化）

### 4.1 类型表示（核心）

- [x] 区分引用类型 vs 值类型（spec §2）：内部 `TypeKind::{Ref, Value}` 已落地（T0401）
- [x] 从 sysroot 收集内建类型/效果的声明头（`TypeEnv`：kind + arity），为后续 lowering/typecheck 提供环境起点（T0402）
- [x] TypeEnv：收集 enum variants（tag + payload fields），并检测重复 variant/字段（T0425）
- [x] enum variant ctor：支持 `Some(x)` 风格构造并做参数/类型检查（T0426）
- [x] `TypeRef` → `Type` lowering：支持 `Path`/`Tuple`/`Nullable` + 泛型 arity 检查（T0403）
- [x] Nullability 语法糖：`T?` → `Option<T>`（lowering 阶段 desugar）（T0411）
- [x] 顶层声明头检查：`fun/val/type` 的签名最小约束（类型注解等）（T0404）
- [x] 表达式类型检查 v0：字面量（Int/String/Bool/Unit）（T0405）
- [x] 表达式类型检查 v0：变量引用（局部/参数/顶层）（T0406）
- [x] 表达式类型检查 v0：函数调用（参数数量/类型匹配；无重载/无默认参数）（T0407）
- [x] 表达式类型检查：成员访问（struct 字段 + class 字段/属性最小子集，`p.x` / `this.x`）（T0408/T0438）
- [x] struct 声明最小语义检查：字段重复/`var`/默认值约束（T0409）
- [x] struct literal 类型检查：字段存在性/重复/类型匹配 + 必填字段覆盖（当前：必须显式提供所有字段）（T0423）
- [x] tuple/Unit（0 元 tuple）：tuple 类型与 tuple 字面量 typecheck（T0410）
- [x] 最小子类型规则：`Nothing <: T`（用于 `return`/不可达分支/后续 `Raise.raise`）（T0420）
- [x] `!!` 非空断言：`Option<T>` → `T` 的静态类型规则（T0421a）
- [x] `?.` safe-call 与 Elvis `?:`：`Option<T>` 语法糖的类型规则（`x?.m()` 返回 `R?`；`x ?: y` 返回 `T`）（T0422）
- [ ] 内建整数模型（spec §2.3.4 / runtime §3）
  - （已在 `scoopc::ty` 中建模 `Int/UInt/IntN/UIntN`；运算/布局语义后续补齐）
  - [x] 整数/布尔运算符类型规则：一元 `! - ~`；二元算术/比较/位运算/移位（shift count 固定为 `Int`）与 `&&/||`（T0447）
  - `Int/UInt` 的 bit width = target pointer size
  - 固定位宽整数类型与类型大小/对齐（为 FFI/序列化提供稳定布局）
  - 整数运算语义：wrap-around、算术/逻辑右移、shift count mask（避免 target 相关 UB）
- [x] `typealias` 语义：类型层展开（用于 `Byte/UIntPtr` 等 sysroot 标准别名；循环 alias 报错）（T0446）
- [x] `Unit`、tuple、`Option<T>`（`T?` sugar）：类型表示与格式化输出已完成（语义/typecheck 后续）（T0401）
- [x] 函数类型（含 effect row）：`(A, B) -> T / E`（spec §7.5）— AST `TypeFun`/`RowExpr` + `parse_paren_type`/`parse_row_expr` + pass/fail fixtures（T0219）
- [x] 函数类型（Type 表示 + lowering + 最小子类型规则）：参数逆变/返回协变 + effect row containment（T0435）
- [x] receiver function type：`T.(A, B) -> C / R`（Type 表示 + lowering；receiver 按第一个参数参与逆变比较）（T0435）
- [x] 类型参数（`TypeKind::Param`）与声明处变型（`in/out` + 最小位置规则 + variance 子类型，仅 ref args 生效）（T0437）
- [x] 泛型约束（上界）：`where` 子句语义检查与实例化满足性（T0458）
- [ ] 泛型约束（更复杂形式）：下界/更完整 bound 形式与约束传播（留给推断/求解阶段逐步补齐）

### 4.2 声明类型：class/interface/struct/enum/effect

- [x] class：主构造 `val/var` 参数作为字段/属性 + 成员方法体最小 typecheck（T0438）
- [x] class：继承/override 的最小静态规则（final/open/abstract/sealed + override 检查）（T0439）
- [ ] class：虚表/方法分发与 codegen（先单继承）
- [x] interface：多实现、默认方法（可先限制默认方法 codegen）（T0440）
- [ ] struct：布局（字段顺序/对齐），不可变，值语义
- [x] enum（rich enum）：tag + union 布局 + niche/boxing/lint 元数据（T0449；codegen 另见 §8.2）
- [x] effect：像 interface 一样声明操作签名（T0601）

### 4.3 Boxing 与 Any

- [x] 值类型装箱到 interface/`Any`（spec §2.5）
- [ ] 先实现“语义正确”，性能优化（如 O(n) 显式转换）后置

### 4.4 模式匹配与 smart cast（spec §4）

- [ ] `when` 表达式（穷尽性检查可分阶段做）
  - [x] 分支结果类型（最小 LUB）：一致 → 该类型；不一致 → `Any`（T0414）
  - [x] 分支 pattern 最小类型检查：tuple/variant 限定 + binder 注入分支作用域（T0427）
  - [x] 穷尽性检查 v0：enum/Bool/Option + `else`/`_` 规则（T0428）
  - [x] 穷尽性检查 v1：嵌套组合覆盖（tuple/enum payload 递归）（T0459）
  - [x] guard 分支视为不可覆盖（需 `else`/`_`）（T0429）
- [x] `is` / `!is` + smart cast（T0413：最小子集，仅 `if (x is T)`/`if (x !is T)`；仅参数 + `val`）
- [x] `as` / `as?`：基础类型规则已实现（T0412）；按 spec 的运行时失败路径（`Raise.raise(RuntimeError.ClassCastFailed)`）待 effect 系统（required effect row/try-catch）补齐后接入

### 4.5 值类型更新（`with` 表达式）（spec §2.6）

- [x] 语义：并行更新（静态约束：禁止重复/包含路径）（T0415）
- [x] path 解析：`a.b.c: value`（字段路径必须存在且类型匹配）（T0415）
- 说明：`TODO.md` 中的 T0424 与以上两项重复，已由 T0415 覆盖（本节保持为实现状态来源）。
- [ ] lowering：生成“拷贝 + 覆盖字段”的构造逻辑（对嵌套 path 生成中间拷贝）

### 4.6 变量绑定与解构（spec §9 + Kotlin-like）

- [x] `val`/`var`：
  - 不可变/可变规则（`val` 不可再次赋值；`var` 可）（T0416）
  - `var` 的赋值类型检查：lhs 可写性（局部 `var` / class `var` 属性）+ rhs 可赋值（T0416/T0443）
- [ ] 解构绑定（destructuring）：
  - [x] tuple/struct 的 `val (a, b) = expr` / `val Point { x, y } = expr`（T0430）
  - [x] enum 的 `val Some(x) = expr` / `val Result.Ok(v) = expr`（T0460）
  - [ ] `when` 分支中的解构 pattern
- [ ] 控制流基础：`if/while/for/return/break/continue`（非局部 return 仅允许 inline lambda 实参）
  - [x] `return`：函数内 `return expr?` 返回类型检查与诊断（T0417）
  - [x] `while`：条件必须为 Bool；`break/continue` 仅允许在循环体内（T0442）

### 4.7 属性系统（spec §10）

- [x] 类属性（T0431：typecheck 侧最小规则）：
  - [x] 默认 getter/setter 视为存在（因此可能生成 backing field）
  - [x] `field` 仅在 accessor 内可见；computed 属性引用 `field` 报错
  - [x] backing field 判定 v0：initializer 或默认 accessor
- [x] 值类型属性：
  - [x] computed property 仅允许 getter-only（禁止 setter）
  - [x] computed property 不允许 initializer（避免 backing field）
  - [x] struct/enum 内属性不允许 `var`
- [x] 扩展属性（T0433：解析 + typecheck 侧门禁）：
  - [x] 顶层语法：`val/var ReceiverType.name: Type get()/set()`
  - [x] computed 约束：禁止 initializer / 禁止 `field` / getter 必需 / `var` 需 setter
  - [ ] lowering：编译为静态 getter/setter（receiver 作为第一个参数）
- [ ] 委托属性（delegated properties）：
  - [x] T0434a：`by` 语法 + 最小静态规则（仅 class；检查 `getValue/setValue` 名称存在性）
  - [x] T0434b：对接 `PropertyMeta` 并升级为签名检查（与 §13 comptime/反射联动）
  - [ ] lowering：生成 `$delegate` 字段 + getter/setter 转发到 `getValue/setValue`（T1210）

### 4.8 函数声明细节（spec §7）

- [x] `inline`：non-local return 门禁（lambda 中 `return` 仅允许出现在 inline 调用的 lambda 实参内；T0444）
- [ ] `inline`：实际 inlining/闭包消除等优化（IR/后端阶段）
- [ ] 扩展函数：
  - [x] 解析与分发规则（静态分发、member 优先；typecheck 降糖为 receiver 第一个参数）
  - [ ] codegen：receiver 作为第一个参数的普通函数
- [x] enum 完整语义：niche optimization、oversized variant boxing、variant size disparity lint（spec §2.3.2）（T0449：前端固定元数据；后端待落地）
- [x] pattern rest `..` 的类型检查与绑定规则（spec §4.2）
- [x] class 初始化模型：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小 typecheck + delegation 门禁）
- [x] `object` / `companion object`：单例类型、成员访问、伴生对象解析（Appendix B.9）
- [x] 委托属性标准库面：`ReadOnlyProperty` / `ReadWriteProperty` 与 `scoop.delegates`（`lazy`/`observable`/`vetoable`/map-backed）（spec §10.4）
- [ ] 通用重载解析（函数 / 构造函数 / 扩展）：
  - 候选筛选：arity、receiver、可见性、命名参数、默认参数
  - 决议规则：最具体候选（most specific candidate）与稳定歧义诊断
  - [x] enum variant / pattern 在同名跨 enum 时按期望类型或 subject type 消歧

**本阶段 DoD**
- `scoopc` 能对一批无泛型/少量泛型的示例做类型检查（含 struct/enum/Option/when/is/as）。

---

## 5. 类型推断（阶段 4：约束求解）

对齐 spec §14：constraint generation + solving（非 HM W）。

- [ ] 约束表示：`τ1 <: τ2`、相等、行约束（effects）
  - [x] 相等约束 + 推断变量 + 最小 unify 骨架（T0501）
  - [x] 子类型约束 `τ1 <: τ2` 的求解（T0506）
  - [x] effects 行推断入口（T0508：public 强制 Pure、private/internal 可推断；依赖 required effects：T0604 ✅）
  - [x] effects 行参数 `eff` 推断（T0509）
- [x] 局部变量推断：`val x = expr`（T0502）
- [x] LUB（if/when 分支：相同类型 / Any fallback）（T0503）
- [x] 返回类型推断：缺省 return type 从函数体推断（T0507，spec §14.6）
- [x] lambda 推断 v0：参数类型下推（T0504，spec §14.7.2）
- [x] 泛型实参推断 v0：从调用参数推断单一类型参数（T0505）
- [ ] lambda 推断：更完整的返回类型合并与 effect row 推断（后续任务）
- [x] 错误报告：把“推断失败”映射到具体源 span 与最小可读解释（T0510）
- [ ] overload resolution 与推断联动：
  - 泛型实参、lambda expected type、默认参数、命名参数、trailing lambda 共同参与候选决议
  - effect rows / `eff` 参数也必须能参与重载筛选与歧义诊断
- [x] 真正的分支合并类型：LUB / 受限 union 的构造、比较与化简（替代简单 `Any` fallback）（T0514）
- [x] effect row 高级推断 v1：高阶返回透传 + row 归一化（T0515）

**本阶段 DoD**
- 能跑 `tests/fixtures/infer/**`：涵盖 if/when/lambda/泛型调用推断的 compile-pass/compile-fail。

---

## 6. 效果系统（阶段 5：先 `Raise`，再完整三种 arm）

### 6.1 静态层：effect row + 多态 + 推断

- [ ] 语法：
  - [x] 函数声明/函数类型的 `/ RowExpr`（T0603）
  - [x] `handle { ... } with { ... }`（T0605：仅 non-resuming arm `->`；arm 级错误恢复；`finally` 仅语法建模）
  - [ ] `eff` 作为上下文关键字：`<eff E = Pure>`、`eff E1+E2`（parser 已支持声明处 `<eff E = Pure>`；use-site `Type<eff Row>` 待补）
  - [x] `+` 并集、`Pure` 空行
  - [ ] 闭合行语法：`/ R!`（`!` 后缀作用于整个 row，不与 `+` 右操作数绑定；spec §5.8.4）
- [ ] 规则：
  - effect operation 调用（T0602）：已支持 `Raise.raise(e)` 的限定名解析与最小类型检查
  - required effects（T0604/T0606：已实现未声明的 effect 报错；支持 non-resuming `handle` 捕获；spec §14.7.1）
  - [x] RowExpr 静态语义：默认 `Pure` + `+` 并集 + containment `R1 ⊆ R2`（T0608）
  - [x] public 默认 `/ Pure` 的强制约束（T0508）
  - [x] private/internal 可推断 effect row（T0508）
  - [x] overriding：`R_over ⊆ R_base`（T0609）
  - [x] entry point 必须 `Pure`（T0610；等价于 `Pure!`，闭合语义）
  - [x] Continuation 类型建模与 `k.resume(value)` required effects 传播（T0611；spec §5.5）
  - 闭合行额外约束：所有来源的 effect（含 callback 透传）都不能逃逸出函数边界（spec §5.8.4）
  - 高级 row 语义：高级归一化、泛型 row 变量、必要的高阶 row 运算
- [ ] 语法糖：
  - [x] `try/catch/finally` → `handle { } with { Raise.raise -> } finally { }`（T0607）
  - [x] `!!` 失败 → `Raise.raise(RuntimeError.NullAssertionFailed)`（T0421b：静态 required effects；依赖 try/catch lowering：T0607）
  - [x] `as` 失败 → `Raise.raise(RuntimeError.ClassCastFailed)`（T0445；依赖 T0607）
  - [x] 多个 `catch` arm 与匹配顺序（不只单个 `catch`）

### 6.2 动态层：handler stack dispatch（Appendix A）

- [x] 运行时必须维护 **handler stack**（按“最近匹配 handler”分发）（T0913）
- [x] arm body 在 dispatch scope 之外执行（避免 self-capture）（T0913）

### 6.3 Codegen/Lowering：分三步落地

1) **非恢复 `->`（flag-based unwinding）**
   - [x] TLS：`__scoop_effect_active` + perform slot（T0906）
   - [x] `perform` 写 slot + set flag + return（T0614：先只覆盖 `Raise.raise`）
   - [x] 调用链传播：检查 flag，沿栈向外返回（T0614：先只覆盖顶层函数调用）
   - [x] handler 边界消费 slot 并清 flag，然后执行 arm（T0614）；`finally` 正确执行；必要时 re-raise（T0615）

2) **立即恢复 `-> resume`（栈 state machine）**
   - [x] 把 handle body 分段（v0：仅单个 perform 点）
   - [x] lifted locals（v0：只覆盖必要局部/跨段写回）
   - [x] while-loop 调度 state
   - [x] `resume(value)` 必须恰好一次（v0：运行期 one-shot 断言，违规 `exit(3)`）

3) **逃逸 continuation `, k ->`（堆 state machine + continuation 对象）**
   - [x] continuation 捕获 handler stack（fiber-local 语义）
   - [ ] 支持跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（见 spec §5.5）
   - [x] one-shot：原子状态位保证并发下只能成功一次

- [x] use-site effect row 实参：`Type<eff Row>` 的类型检查（默认值 + 显式实参，纳入 nominal type identity；T0511）
- [ ] use-site effect row 实参：由上下文/lambda body 反推的 row 实参推断（高阶约束与求解留待 T0515+）
- [ ] `Task<T>` 与 `async fun` 语义：
  - [x] `async fun foo(): T` desugar 为 `fun foo(): Task<T>`（T0623）
  - [x] 调用者签名不携带 `/ Async`（T0623）
  - [ ] `Task<T>` 懒执行，直到 `await` 或显式启动
- [ ] Appendix A 一致性：嵌套 handler 必须支持“最近匹配 handler”分发，不能停留在单层 handler 模型
- [ ] program boundary 不只 `main`：库导出入口、多 entry point 与 host/embedded 边界规则（TODO T0629）
  - [x] cone-aware entry point：仅 consumer cone 的 `main` 视为 entry point（TODO T0629a）
  - [ ] 库导出入口 + host/embedded entry points（TODO T0629b，依赖 T1107）
- [x] perform slot ABI：从单 slot 扩展到可承载复杂 payload / 多 effect op 的稳定表示（T0630）

**本阶段 DoD**
- compile-pass + run-pass 覆盖 `Raise`、`try/catch/finally`、自定义 effect + handle，以及一个最小 async/await demo（T0619：`tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`）。

---

## 7. 中间表示与单态化（阶段 6：为 LLVM 做准备）

### 7.1 HIR/MIR 设计

- 注：`perform` / `handle` 的 IR 节点（TODO T0612）依赖 HIR/MIR 骨架与 AST→HIR lowering（TODO T0701～T0703），因此在 TODO 中需要排在 T0703 之后，避免出现“首个 TODO 依赖未满足”的顺序问题。
- [ ] HIR：保留大部分结构但已解析/已类型化
  - [x] HIR 骨架 + `dump-hir`（TODO T0701）
  - [x] AST→HIR lowering（声明头 + 简单函数体）：`TypeRef`→`TypeId` + ident→`SymbolId`（TODO T0702）
  - [x] HIR：控制流与语句节点建模（if/when/while/assign/return）（TODO T0705）
- [ ] MIR：显式控制流（基本块）、显式临时变量、显式 drop/cleanup（用于 `finally`/effect unwinding）
  - [x] MIR 骨架：基本块/terminator/locals + CFG 校验（TODO T0703）
  - [x] MIR：cleanup/finally 的最小模型（UnwindAction + ResumeUnwind）（TODO T0707）

### 7.2 泛型单态化（monomorphization）

- [ ] 为每个具体实例生成专用 IR（含 `eff` 参数实例化）
- [ ] 缓存键：符号 + type args + effect row args
- [ ] 支持“预编译常见实例”（对齐 Cone 的 pre-specialize）

### 7.3 闭包与函数值

- [x] lambda → `{ env_struct, fn_ptr }` 形式
- [ ] 捕获变量布局与 GC trace 信息生成
- [ ] effectful function type 的调用约定统一化
- [ ] 可变捕获：捕获 `var` 时的 box/lift 策略、别名与写回语义

**本阶段 DoD**
- 纯子集（无 class 虚分发也可）能 lowering 到 MIR，并能生成可链接 `.o`（下一阶段）。

---

## 8. LLVM 后端（阶段 7：inkwell codegen）

### 8.1 LLVM Module/Pass 管线

- [x] 最小 module + `main`（`ret 0`）IR 输出（T0802）
- [x] 目标三元组与数据布局（target machine）（T0803）
- [ ] 基本优化 pass（O0/O1/O2 可选）
- [ ] 调试信息（DWARF）可后置

### 8.2 数据布局与 ABI

- [ ] 值类型（struct/tuple/enum）按 LLVM struct layout 映射
  - [x] struct：布局 + 字段访问（T0811）
  - [x] tuple：布局 + `._0` / `._1` 元素访问（T0812）
  - [ ] enum：tagged union 布局（T0813）
- [ ] 引用类型：对象头（type descriptor 指针 + flags + size 等）
- [ ] interface/虚表：最小可行实现（先只支持接口方法调用与装箱）

### 8.3 与 GC 的接口（推荐：shadow stack 精确根集）

为了避免早期实现 LLVM `gc.statepoint` 的复杂度，建议先实现 **shadow stack**：

- [x] TLS：当前线程 `current_frame`（`scoop_gc_current_frame` / `scoop_gc_frame_push/pop`）（T0905）
- [x] 每个函数 prologue/epilogue 建立 `GcFrame`（包含 prev 指针 + roots 数组）（T0816）
- [x] 在需要的地方把 GC 引用写入 roots slot（局部变量活跃区）（T0816）
- [x] runtime：遍历当前线程 frame 链枚举 roots（visitor 形式，T0909）
- [x] 分配触发 GC 时，runtime 扫描所有线程的 frame 链得到根集（T0911）

> 优点：实现难度低、语义清晰、可逐步演进到移动 GC；缺点：需要编译器插桩，性能一般，但足够 bootstrap。

- [ ] `when` lowering：补齐 or-pattern / guard（spec §4.2）
- [x] tuple 字段访问统一为 `._0` / `._1`，并同步修正文档、fixtures、lowering、codegen（spec §2.3.3）
- [ ] enum layout/codegen：补齐 niche optimization、oversized variant boxing、variant size disparity lint（spec §2.3.2）
- [ ] `object` / `companion object` codegen：单例存储、一次初始化、静态成员访问（Appendix B.9）
- [x] `trimIndent()`：运行期 fallback 与字符串 API 对接（spec §8.4）

**本阶段 DoD**
- 生成的二进制可运行（至少支持整数运算、函数调用、打印、Option/enum 基本构造）。

---

## 9. 早期运行时（C + clang）（阶段 8：可执行与可观测）

### 9.1 最小运行时组件

- [ ] 启动入口：`main`/平台 glue，初始化 TLS（GC + effect）
- [x] 分配器：`scoop_alloc(size)` v0（`malloc`）+ codegen 侧装箱调用（T0902/T0817）
- [ ] 分配器：`scoop_alloc(size, type_desc)`（带类型描述，供 GC 扫描对象字段）
- [ ] GC（先易后难）：
  - [x] v0：mark-sweep 数据结构骨架（T0904）
  - [x] v0：非移动 mark-sweep（手动触发 `scoop_gc_collect`，T0910）
  - [x] v0：pin/unpin API（pinned objects 作为额外 roots，T0912）
  - [ ] v1：可选移动/压缩（pin/unpin 在移动 GC 上才真正有意义，但 API/错误检查语义已固定）
- [ ] 类型描述（type descriptor）：
  - pointer bitmap 或 trace 回调
  - 用于扫描对象内的引用字段（struct/enum/closure env）
- [x] 线程注册：新线程必须注册到 runtime 以便 GC stop-the-world 扫描其 shadow stack（T0911）
  - [x] v0：`scoop_thread_register/unregister` 占位 + TLS 骨架（T0903）
  - [x] v0：协作式 stop-the-world + 扫描所有线程 roots（T0911）
- [ ] `object` / `companion object`：跨 DLL / 动态链接的一次初始化与全局可见性策略

### 9.2 effect runtime（C 或编译器插桩）

- [x] TLS：handler stack 指针、perform slot、flag（T0906/T0913）
- [x] 最小原语：push/pop handler frame、读写 perform slot（T0613/T0913）
- [x] continuation one-shot + resume API（T0914）
- [x] continuation 跨线程 `resume`：安装 captured handler stack，并在返回后恢复原 TLS（T0915a；端到端 fixture 见 T0915b）

### 9.3 与 clang 的构建集成

- [ ] `runtime/c` 用 clang 编译成静态库/对象
- [ ] `scoopc` 链接时自动把 runtime 拉进来
- [ ] fixtures 中提供 `--emit-llvm`/`--emit-obj`/`--emit-asm` 选项方便排查
- [ ] effect runtime 必须支持多层 handler stack（最近匹配分发 + arm body 在 dispatch scope 外；Appendix A）
- [x] `Task<T>` / executor 最小 runtime 原语：任务状态、入队/恢复、可选 start（spec §5.7）
- [ ] `object` / `companion object` 的 once/init 支持（Appendix B.9）

**本阶段 DoD**
- 有一个“运行期回归套件”（见 §10）能持续压测 GC 与 effect。

---

## 10. Fixtures 与测试体系（贯穿所有阶段，必须先行）

这里的目标是：**任何规范点都有对应的 fixture**，并且 fixtures 能区分：
- 解析是否正确
- 语义/类型/效果是否正确
- 代码生成/运行期行为是否正确

### 10.1 Fixture 目录规划（建议）

```
tests/
  fixtures/
    parse/               # 仅解析：AST snapshot / 语法错误恢复
    resolve/             # 名字解析：import/visibility
    resolve_multi/        # 名字解析：多文件编译单元（目录为 case）
    typecheck/           # 类型检查：compile-pass / compile-fail
    typecheck_multi/      # 类型检查：多文件编译单元（目录为 case）
    infer/               # 推断专项
    effects/             # effect rows / handle / required effects / entrypoint Pure
    codegen/             # 运行输出对比
    runtime_gc/          # GC/alloc/pin/unpin/压力测试
    unsafe_nogc/         # @Unsafe/@NoGC 规则
    language/            # 字符串/with/属性/委托/操作符等语法语义专项（按章节分组也可）
    comptime/            # const fun / comptime / 反射 intrinsics
    cone/                # .cone 打包/消费/单态化缓存
```

当前 runner 约定：fixture 的一级目录名就是 phase（例如 `parse/`、`resolve/`、`typecheck/`）。未实现的 phase 也必须给出清晰诊断，便于先写 fixture 再补实现。

- [x] phase 路由：按 `tests/fixtures/<phase>/**` 目录名决定执行阶段（未实现 phase 返回“未实现”诊断）

默认每个 fixture 采用“单文件 + 注释指令”的形式（类似 LLVM lit 或 Rust compiletest）。
对于需要跨文件验证的规则（例如 `private` 可见性、跨文件引用、sealed 继承等），额外提供 `<phase>_multi/<case>/`：
- `<case>/` 目录内包含 2+ 个 `.scoop` 文件
- runner 先把同一 case 的所有文件作为一个编译单元构建索引，再逐文件执行 `<phase>` 并按各自文件头注释断言 pass/fail

- [x] `// EXPECT: pass|fail`
- [x] `// EXPECT-ERROR: <substring>`（当前为子串匹配；后续可升级为 regex）
- [x] `// EXPECT-AST: <file>`（parse fixtures：AST snapshot / golden）
- [x] `// RUN-STDOUT: <file>`
- [x] `// RUN-STDERR: <file>`
- [x] `// EXPECT-EXIT: <code>`
- [x] `// TIMEOUT: <ms>`
- [x] `// ARGS: ...`

### 10.2 诊断（compile-fail）的 golden 规范

- [x] 诊断必须包含：错误码（稳定 ID）、主消息、关联 span（行列）、可选 note/help（当前 lexer/parser 已提供 code + label span）
- [x] fixtures 断言策略：支持匹配“错误码 + 错误位置（行列）+ 关键片段”（先用文件头注释指令实现；未来可再升级为独立 `.golden`）

推荐模板（compile-fail fixture 文件头）：

```
// EXPECT: fail
// EXPECT-ERROR: <关键片段>
// EXPECT-ERROR-CODE: <稳定错误码>
// EXPECT-ERROR-AT: <line>:<col>
```

### 10.3 spec doctest（强烈建议）

- [x] 工具：从 `SCOOP_FULL_SPEC.md` 抽取包含 `// FIXTURE:` 的 fenced code block，生成 `tests/fixtures/spec_doctest/*`
- [x] 约定：代码块通过注释标记其期望（`// EXPECT:` / `// EXPECT-ERROR:`），`// FIXTURE:` 指定输出路径
- [x] 在 CI 中强制：`cargo run -p scoop_tools -- spec-fixtures check` + `cargo run -p scoop -- test`
- [x] 本地修复：`cargo run -p scoop_tools -- spec-fixtures check --fix`（只写回受影响文件）

### 10.4 运行期 fixtures（run-pass）

- [x] T0106a：fixtures runner 识别 `codegen/`（或 `run-pass/`）phase，并实现 stdout golden 比对（对比逻辑可单测独立验证）
- [x] T0106b：接入 `scoop run`（T0807）真正“编译 + 运行” fixture，并断言 stdout（默认仅在启用 `scoop --features llvm` 时执行）
- [ ] 支持超时、退出码断言（fixtures 指令：`TIMEOUT`/`EXPECT-EXIT`）
- [x] T0111a：支持 stderr golden 断言（对比逻辑 + 稳定诊断，可单测）
- [x] T0111b：新增 run-pass fixtures 覆盖 stderr（需要 T0106b2 真正执行）
- [ ] 对 GC 压测类测试，支持 `SCOOP_GC_STRESS=1` 之类的环境变量切换（让 CI 可控）

### 10.5 Fuzz/性质测试（可选但很有价值）

- [x] lexer/parser fuzz（避免崩溃，保证错误恢复）— 实现为 `crates/scoopc/tests/fuzz.rs`：adversarial + deterministic random + structured fragment 三类测试（5000+ iterations）
- [ ] IR lowering fuzz（随机小 AST → 不崩溃）
- [ ] GC 压测（随机分配/释放/跨线程）

### 10.6 覆盖矩阵（建议维护）

- [x] `cargo run -p scoop_tools -- fixtures-matrix check`：按 phase 目录扫描 fixtures，报告缺少 pass 或 fail 的缺口（见 `tools/scoop_tools/src/fixtures_matrix/`）
- [ ] 后续可细化为按 spec 章节粒度检查（当前为 phase 粒度）

为每个 spec 章节至少准备：
- 1 个 compile-pass
- 1 个 compile-fail（覆盖常见误用）
- 若涉及运行期语义（GC/effect/async），再加 1 个 run-pass

---

## 11. `@NoGC` / `@Unsafe` / `@Extern`（阶段 9：实现“系统编程通道”）

- [ ] 通用注解系统（spec §15）：
  - [x] 解析注解声明（`annotation class`）
  - [x] 解析注解使用（`@Name(...)`）
  - 注解 target（函数/类型/字段/参数/表达式块等）与合法性检查
  - 注解仅编译期存在（不进运行时布局）
  - 内建注解：`@Intrinsic/@Extern/@Inline/@Deprecated`（具体名字按 sysroot 定义）
- [x] T1003：内建注解最小门禁（`@Unsafe/@NoGC/@Extern/@Intrinsic`）
- [x] T1004：`@Unsafe { ... }` 块语法与 unsafe context 传播
- [x] T1005：`@NoGC` 最小静态门禁（保守拒绝可能分配/装箱的路径）
- [x] T1009：最小 unsafe 指针原语（`addrOf/load/store`）的语法落点与门禁（unsafe_nogc fixtures 回归）
- [x] `@Unsafe`（最小落地）：
  - 函数级与块级 `@Unsafe { ... }`
  - 非 unsafe context 禁止：调用 `@Unsafe` 函数/调用 `@Extern`/使用最小 ptr 原语（`addrOf/load/store`）
- [ ] `Ptr<T>` / `UIntPtr` 与指针整数转换（spec §15.9.4 / runtime §4~§5）
  - `UIntPtr` 仅为 `UInt` 的别名（类型本身不 unsafe）
  - 指针 ↔ 整数转换必须在 unsafe context，且通过 sysroot intrinsics（不通过 `as/as?`）
  - `Ptr<T>` 的 `T` 必须是 GC-free value type（不允许直接/间接包含 GC ref）
- [ ] `@NoGC`：
  - 禁止 GC 堆分配；只能调用 `@NoGC` 与 `@Extern`
  - 编译器证明不了“无分配”就必须报错（保守）
- [ ] `@Extern`：
  - 默认视为 `@NoGC`
  - 是否默认 `@Unsafe`：建议 **调用点要求 unsafe context**（更符合“外部世界不可信”）
- [ ] 注解系统补齐：
  - 内建注解：`@TailRec/@AllowIntrinsic/@Suppress/@CLayout/@Target/@Retention`
  - `AnnotationTarget` enum 与 target 合法性检查
  - meta-annotations 与 `.cone` 导出策略
- [ ] 注解参数补齐：常量表达式、数组/enum/class-literal 等非纯字面量参数的解析与合法性检查
- [ ] 注解 use-site targets：`field:/property:/param:/get:/set:/file:`（spec §15.3）
- [ ] namespaced annotations：`@Namespace.Annotation(...)`（spec §15.4）
- [ ] 后期 runtime / std 阶段的 intrinsic 预算规则：
  - 默认不再新增 intrinsic，优先用纯 Scoop 库补 runtime/stdlib 缺口
  - 若审计证明缺少底层 primitive，则单独立项增加最小 intrinsic，并与上层库任务拆开推进
  - 集合特别约束（Array-first，最小 intrinsics）：
    - `Array<T>` / `MutableArray<T>` 允许作为 **唯一** 集合底座引入少量底层 primitive（必要时通过 `@Intrinsic` 落地）
    - `push/pop/insert/remove/splice` 等能力必须作为库 API 支持；其实现 **默认必须** 由纯 Scoop 完成（基于 `get/set` + 容量策略等），只有当审计证明存在底层 blocker 时才允许回流增加最小 intrinsic
    - `Set/Map/List/MutableList/MutableSet/MutableMap` 等上层集合 **不得** 引入新的 intrinsic；性能问题优先通过纯 Scoop 算法/专门化与优化解决

fixtures：
- `tests/fixtures/unsafe_nogc/*` 覆盖所有违规路径（必须 compile-fail）

---

## 12. Cone（包/稳定 IR/分发）（阶段 10：工程化分发）

### 12.1 Scoop IR（scoopir）

- [ ] 定义一个稳定的 IR schema（建议独立文档 + 版本号）
- [ ] `api.scoopir`：仅含 public API（用于类型检查与 IDE）
- [ ] `generics.scoopir`：含泛型/const fun 的可执行 IR（供下游单态化）

### 12.2 `.cone` 归档格式

- [ ] archive（可先用 zip/tar，后续换自定义格式）
- [ ] 读写 `Cone.toml`、依赖解析、目标平台信息
- [ ] 预编译实例（pre-specialize）：cache key 与选择规则
- [ ] pre-specialize：补齐类型实例（不只函数实例）的打包与消费规则

fixtures：
- `tests/fixtures/cone/*`：
  - 打包后消费编译的 API 兼容性
  - IR 版本兼容（旧版本可读）

---

## 13. 编译期执行与反射（阶段 11：comptime）

- [x] Parser 语法：支持 `const` 修饰符、`comptime { ... }` / `comptime if` / `comptime for`、以及 splice `value.[field]`（见 TODO T0246）
- [ ] `const fun` 解释器（先支持 value types/纯计算；`String` 作为特例允许——具有值语义）
- [ ] `const fun` 静态检查：禁止闭包/lambda（捕获环境导致 const 语义难以验证）
- [ ] `comptime { ... }` 执行上下文（限制 effect：必须 `Pure`）
- [ ] 反射 intrinsics：`fieldsOf/nameOf/sizeOf` 等（先从 sysroot 声明开始）
- [ ] 反射 intrinsics 补齐：`variantsOf/alignOf/superTypesOf/annotationsOf/paramsOf`（spec §6.4 / §15.6）
- [ ] 编译期元数据补齐：`VariantMeta/ParamMeta/FunctionMeta/AnnotationMeta/AnnotationArgMeta`（spec §6.4 / §15.6）
- [ ] 编译期注解访问：复杂参数表达式 / 数组 / enum / class-literal 的归一化与读取（不只字面量）
- [ ] `trimIndent()`：编译期求值（spec §8.4；运行期 fallback 已由 T0827 完成）
- [ ] sysroot/stdlib：补齐 scope functions（§11）；delegated property API surface 已在 sysroot 落地（spec §10.4）

fixtures：
- `tests/fixtures/comptime/*`：覆盖常量折叠、生成代码（若支持）、错误诊断

---

## 14. Kotlin 语义兼容项（阶段 11+：按需逐步补齐）

spec §16 指出以下功能“遵循 Kotlin 语义”，实现上建议按需求拆分落地，每一项都要配 fixtures：

- [ ] 操作符重载（operator overloading）
  - 解析 `a + b` → 解析/绑定到 `plus`/`minus` 等约定方法（按 Kotlin 规则）
  - 补齐位运算与移位：`and/or/xor/inv/shl/shr`（Appendix B.8）
  - 运行期与值类型/引用类型的 codegen 覆盖
- [ ] `object` 与 companion object（如需要）
- [x] `typealias`（纯类型层语法糖；当前仅非泛型别名 + 展开 + 循环检测，T0446）
- [ ] Ranges/progressions 与 `for` 迭代协议
- [ ] 基础集合与常用操作（`map/filter/fold` 等更多是库工作，但需要类型推断与泛型单态化支撑）：
  - `Array<T>`：不可变（只读集合）；支持 `get`/`length`/迭代与常用数组操作
  - `MutableArray<T>`：可变；支持 `get/set/push/pop/insert/remove/splice`，并采用容量策略保证 `push/pop` 摊还 O(1)
  - 两者都应支持迭代（`for` 协议/迭代器）与从 iterable 构造（优先 `Array.from(iterable)` / `MutableArray.from(iterable)`）：
    - 允许实现上使用内部 builder（例如内部 `MutableArray`）以获得摊还 O(1) 的增量构造
    - 若需要“零拷贝把 builder 变成不可变 Array”，必须定义**显式且安全**的语义（例如 `freeze`：冻结后任何别名都不可再变更）
    - 在缺少上述语义前，**不要**对外暴露 `MutableArray -> Array` 的零拷贝转换 API（避免把“只读视图”误当成不可变值）
  - 数组字面量 `[...]`：按 expected type 推断为 `Array<T>` 或 `MutableArray<T>`，并支持 `val xs: Array<Int> = [1, 2, 3]` 这类类型注解
  - `List<T>`：定义为 `Array<T>` 的别名（`typealias List<T> = Array<T>`）
  - `Hashable`：加入 sysroot 并为 primitive types 提供实现；`Set/Map`（含 mutable）全部用纯 Scoop 基于 `Array`/`MutableArray` 实现，不引入 intrinsics
  - `MutableList<T>`：用 `MutableArray` 做 backing pool，以纯 Scoop 实现并追求高效（`push/pop/insert/remove` 摊还 O(1)）
- [ ] import alias：`import foo.bar.Baz as Qux`（Appendix B.7）
- [ ] `object` / `companion object`：从 parse/resolve 扩展到 typecheck/codegen/初始化语义（Appendix B.9）
- [x] 类初始化语义：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小落地）
- [ ] 标准 delegated properties：`lazy`/`observable`/`vetoable`/map-backed（spec §10.4；运行期语义待补齐）
- [ ] Kotlin runtime gap closure（when applicable）：
  - 先审计 Scoop core runtime / stdlib 与 Kotlin runtime 语义缺口
  - 优先用纯 Scoop 补齐；只在审计证明无法表达时回流到 §11 的最小 intrinsic 通道
- [ ] 全量 `std` 库工程：
  - 目标能力与 Rust `std` 同量级、可比较，但不要求 API 一致
  - 建议分层：`core` / `alloc` / `std` / 平台适配层
  - 覆盖 collections、text/regex、iterators、io/fs/path/process/env、time、sync/thread/channels、net、async adapters、test/support utilities 等
  - collections 设计约束：以 `Array`/`MutableArray` 为底座；`List<T>` 为 `Array<T>` 别名；`Set/Map`（含 mutable）为纯 Scoop（不新增 intrinsics）
- [ ] Kotlin 风格重载决议兼容：
  - most specific candidate 规则
  - 默认参数 / 命名参数 / trailing lambda 与重载集合的交互
  - 扩展函数、成员函数、构造函数之间的优先级与歧义处理
- [ ] 默认参数：中间参数省略与命名参数联动
- [ ] 多 trailing lambda：语法、expected type 与重载决议联动
- [ ] varargs spread：集合/序列到 vararg 的桥接规则
- [ ] delegated properties：`lazy`/`observable`/`vetoable` 的线程安全语义与平台 policy
- [ ] 类初始化兼容：复杂继承链与 effect 细节

fixtures：
- `tests/fixtures/language/*` 下为每个特性提供 compile-pass/compile-fail + 必要的 run-pass

---

## 15. GC 迁移到 Scoop（阶段 12：自举路线）

### 15.1 迁移前置条件

- [ ] `@NoGC` 可写且可验证（GC 核心不应触发 GC 分配）
- [ ] `@Unsafe` + 指针/原子/线程 API 完备
- [ ] FFI 能调用 OS/clang runtime 的最低集合（mmap/VirtualAlloc、thread local、mutex 等）

### 15.2 迁移策略（建议渐进）

1) **在 Scoop 中实现 GC 算法库（仍由 C runtime 驱动）**
   - C runtime 负责“触发 GC/暂停世界/枚举线程/提供原子与 OS API”
   - Scoop 代码负责“标记/扫描/整理”的纯算法部分

2) **把类型描述与扫描逻辑迁移到 Scoop**
   - type descriptor 结构体改由 Scoop 定义（C 只保留 ABI glue）

3) **最终替换 C GC**
   - C runtime 仅保留极薄的启动层，甚至可以被 Scoop runtime 取代

4) **Scoop GC 进入多线程阶段**
   - 线程注册、stop-the-world / 协调协议、跨线程 root 扫描、线程本地分配策略都由 Scoop GC 接管
   - 单线程 mark-sweep 只作为 baseline；多线程正确性与可回归性必须先固定

5) **引入更高性能 GC 变体（如 Immix）**
   - 在 baseline GC 可用后，引入 Immix 或类似 line/block allocator 作为改进路径
   - 保持与 baseline GC 共存，避免把算法升级和 runtime 自举耦死

6) **GC 后端可替换 / 可编译期选择**
   - 编译期可选择 mark-sweep、Immix、embedded/minimal、WASM GC adapter 等不同实现
   - 通过稳定的 GC runtime ABI / trait 边界隔离上层 runtime 与具体 GC 算法

7) **runtime 去 C 化**
   - 逐步把启动、effect runtime、GC runtime、线程/调度 glue 从 C 迁移到 Scoop
   - 允许继续直接调用 libc / OS ABI，但 runtime 核心逻辑不再依赖 C
   - 对 non-resuming effect / unwind 路径，可评估引入 `libunwind` 作为底层依赖，而不是继续依赖 C runtime 自带异常/展开机制

fixtures：
- 运行期 GC fixtures 必须在“C GC”和“Scoop GC”两套实现下都能跑（同一套测试，不同 runtime 实现）。
- 迁移后，运行期 fixtures 应至少在两类 GC backend 下可回归：baseline GC 与高性能 GC（如 Immix）；若提供 WASM/embedded 适配器，还应维护 capability matrix 与分层禁用测试。

---

## 16. 风险点与建议的优先级

- **高风险/高复杂度**：effect（尤其 `, k ->` + 跨线程）、GC（移动/压缩 + pin/unpin）、类型推断（subtyping + effect rows）
- **建议优先级**：
  1) 先把 fixtures 与诊断体系立住（否则后期难以迭代）
  2) 先做“语义正确”的实现（优化后置）
  3) effect 先 `Raise`/`->`，再扩展 `-> resume`、`, k ->`
  4) GC 先非移动，再移动（pin/unpin 在移动 GC 上才真正有意义）
