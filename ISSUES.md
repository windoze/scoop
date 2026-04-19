# Issues

## 范围

- 本轮只记录“语言特性 / 规范兑现 / 编译链语义”上的缺口。
- 本轮先不展开 executor framework / 调度策略 / wakeup API；除非它们直接阻塞 `Task` 形状或 effect codegen，否则统一留到下一阶段。
- 明确不单列：纯 stdlib / sysroot API 占位、并发基础库 surface、MIR `Todo(...)`、以及通用 host-only GC / target capability 路线图；只有当它们直接阻塞某条语言规则时，才并入对应条目。
- 以下结论已交叉验证：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`。

## 已确认不再计为 issue 的历史说法

- member call / interface dispatch 已经打通；`tests/fixtures/typecheck/member_call_interface_dispatch_not_supported_is_error.scoop` 现在实际是 pass fixture，只剩历史文件名。证据：`crates/scoopc/src/typecheck/expr/call.rs:4384-4828`；`tests/fixtures/typecheck/member_call_interface_dispatch_not_supported_is_error.scoop:1-18`；`tests/fixtures/run-pass/member_call_interface_dispatch_basic.scoop:1-23`。
- class 实例字段 member access codegen 已支持；`tests/fixtures/run-pass/gc_trace_class_ref_field_basic.scoop` 里的“尚未支持 class 字段 member access codegen”注释已经过时。证据：`crates/scoopc/src/llvm/codegen/mod.rs:12801-12875`；`tests/fixtures/run-pass/gc_trace_class_ref_field_basic.scoop:4-7`。
- `@CLayout(..., packed = n)` 已支持 `n <= 16` 且为 2 次幂；`sysroot/core.scoop` 里“v0 仅支持 packed = 1”的历史注释不再准确。证据：`crates/scoopc/src/typecheck/annotations.rs:1757-1767`；`crates/scoopc/src/llvm/codegen/ty.rs:396-407`；`sysroot/core.scoop:128-128`。
- multi-file lowering 对“非入口文件 source-backed literals”的旧限制已解除，相关 stdlib 注释过时，不再算语言 feature 缺口。证据：`crates/scoopc/src/hir/lower/mod.rs:1509-1510`；`stdlib/array_iter.scoop:9-9`；`stdlib/mutable_array.scoop:15-15`；`stdlib/mutable_array_iter.scoop:8-8`；`stdlib/mutable_list.scoop:10-10`；`stdlib/task.scoop:10-10`。

## 1. effect / continuation 主路径已足够支撑手动 step 的 `Task`，但仍未覆盖完整规范语义

- 现状：escape continuation 已经可以 capture、堆存、跨作用域 later-resume，并且 `resume` payload 已覆盖 `Unit`、GC 引用与带引用字段的复合值；现有 FIFO/LIFO scheduler fixtures 也已经证明 pure Scoop 代码可以把 `Continuation<T>` 存进普通 class 字段后手动推进计算。因此，从“Task 内部靠 continuation stepping”这个角度看，effect codegen 主路径已经可用。但 richer 语义仍未补齐：handler arm head 仍只接受 effect operation；effect type param 仍只支持单一 type param；receiver effect op 仍被拒绝；escape continuation binder 仍注入默认 effect row 的 `Continuation<T>`；`Continuation.resume` 仍只接受一个实参，并且命名实参只接受 `value = ...`。另外，在 async 组合子路径上，当前 LLVM 后端对 escape continuation 的组合能力仍偏弱，`Task<Int>.andThen` 这类“一个 setup 里串两个 await”仍需要拆成两段 `handle`。
- 影响：effect / continuation 已经不再是 “Task redesign 无法开始” 的 blocker；它已经足以支撑“Task 是普通对象、内部保存并推进 continuation”的方向。真正尚未完成的是更完整的 effect polymorphism、receiver-op 支持，以及更自然的多 suspend / 多 await 组合写法。
- 证据：`tests/fixtures/run-pass/effect_escape_continuation_scheduler_fifo_multi_task.scoop:18-163`；`tests/fixtures/run-pass/effect_escape_continuation_scheduler_lifo_multi_task.scoop:19-155`；`tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop:12-40`；`tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop:12-38`；`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop:19-58`；`crates/scoopc/src/typecheck/expr/infer.rs:1246-1261`；`crates/scoopc/src/typecheck/expr/infer.rs:1719-1729`；`crates/scoopc/src/typecheck/expr/call.rs:3419-3436`；`crates/scoopc/src/typecheck/expr/call.rs:3646-3735`；`stdlib/task.scoop:116-149`。

## 2. `Task` 仍未定型为通用的 pollable object；当前实现还绑在 executor-centric 的 hard-coded ABI 上

- 现状：下一阶段的 executor framework 还可以继续留白，但 `Task` 本体的方向其实已经足够明确：它应该是通用 API，语义上类似可手动 `poll` / `step` 的 lazy object；`Continuation` 则应视为较低层、one-shot 的 advanced API，由 `Task` 或库层封装使用。当前实现却仍停留在旧设计：`spawn { ... }` 仍要求 body 可赋给 `Int`；`Task<T>` 与 `scoop.task.Executor` 在后端仍直接映射成 word-sized handle；`taskCreate` 仍由编译器拆 closure object 取出 `{env_ptr, fn_ptr}` 后调用 `scoop_task_u64_create`；`Task.onComplete` 仍直接把 raw continuation pointer 传给 `scoop_task_u64_on_complete_resume_u64`；`join` lowering 仍直连 `__scoop_task_join_int`。这意味着当前 `Task` 还不是“普通对象 + 内部状态机 + 手动 poll contract”的形状。
- 影响：当前真正待收敛的不是 executor 队列/调度策略，而是 `Task` 的对象模型与 lowering 合同。只要 `Task` 仍被绑定到 handle ABI，就没法把它定成通用 API，也没法把 continuation one-shot 细节安全地藏到 advanced API 背后。
- 证据：`sysroot/core.scoop:447-472`；`crates/scoopc/src/typecheck/expr/infer.rs:478-603`；`crates/scoopc/src/hir/lower/expr.rs:560-680`；`crates/scoopc/src/llvm/codegen/ty.rs:28-44`；`crates/scoopc/src/llvm/codegen/mod.rs:7250-7359`；`crates/scoopc/src/llvm/codegen/mod.rs:7757-7870`；`crates/scoopc/src/llvm/codegen/mod.rs:8509-8644`；`crates/scoopc/src/llvm/codegen/runtime_abi.rs:735-882`；`runtime/c/scoop_task_executor.c:1-481`；`runtime/c/scoop_runtime.c:1308-1351`。

## 3. lambda 推断与 receiver lambda 仍不完整

- 现状：没有 expected function type 时，未标注类型的 lambda 参数仍会直接报错；当前 expected-type 向下传播仍只覆盖 0/1/2 参数 lambda；receiver function type 虽可进入类型系统，但 lambda body 里仍不会自动注入 `this`。
- 影响：lambda 的基本语法已经可用，但“先写 lambda 再让上下文推断”“更多参数形态”“receiver lambda 直接使用 `this`”这些常见写法仍不完整。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:305-321`；`crates/scoopc/src/typecheck/expr/infer.rs:2049-2065`；`crates/scoopc/src/typecheck/expr/infer.rs:2368-2369`。

## 4. 调用语义仍有多处早期门禁

- 现状：函数值调用仍不支持命名实参；函数指针调用同样不支持命名实参，并明确拒绝 receiver function type 作为签名；`callee<T>` 仍不能作为一等值传递；函数签名仍只支持“至多一个 vararg，且必须为最后一个形参”；`super(...)` / `this(...)` 构造器委托调用仍只允许位置参数。
- 影响：Kotlin-like 调用规则尚未在所有 callee 形态上统一落地，调用系统仍带着较多“按 callee 形态分流”的早期特判。
- 证据：`crates/scoopc/src/typecheck/expr/call.rs:789-800`；`crates/scoopc/src/typecheck/expr/call.rs:943-978`；`crates/scoopc/src/typecheck/expr/call.rs:1560-1564`；`crates/scoopc/src/typecheck/expr/mod.rs:147-160`；`crates/scoopc/src/typecheck/expr/entry.rs:910-911`。

## 5. 泛型约束、参数化超类型与 star projection 已收口

- 现状：`where` 子句已支持带类型实参的 nominal bound；`TypeEnv` 记录 direct supertypes 时会同时保留 FQN 与原始 type args；assignable / 上转规则已改为沿“具体化后的 direct supertypes”做 DFS；spec §3.3 的 star projection 也已落到独立的 typecheck 内部表示，并在导出 / RTTI / LLVM 侧擦除为 `Any?` 读视图，而不再直接退化成裸 `Any`。
- 影响：泛型约束、参数化子类型关系与 `List<*>` / `Array<*>` 一类 star projection 主线现已贯通，不再是后续 lambda / 调用 / 跨文件实例化任务的前置 blocker；后续只需在 `T4001R` 中继续复核“没有回退到局部特判”。
- 证据：`crates/scoopc/src/typecheck/type_env.rs:207-215`；`crates/scoopc/src/typecheck/lower.rs:1058-1071`；`crates/scoopc/src/typecheck/lower.rs:2181-2218`；`crates/scoopc/src/typecheck/assignable.rs:44-122`；`crates/scoopc/src/llvm/codegen/mod.rs:15082-15164`；`tests/fixtures/typecheck/where_clause_parameterized_bound_method_ok.scoop:1-22`；`tests/fixtures/typecheck/where_clause_parameterized_bound_not_satisfied_is_error.scoop:1-22`；`tests/fixtures/typecheck/star_projection_value_type_requires_boxing_is_error.scoop:1-11`；`tests/fixtures/typecheck/star_projection_nullable_ref_read_view_ok.scoop:1-12`；`tests/fixtures/run-pass/parameterized_supertype_interface_dispatch.scoop:1-33`；`tests/fixtures/run-pass/star_projection_array_read_view.scoop:1-23`。

## 6. 顶层 pattern binding 仍不支持

- 现状：顶层 `val` 的声明头检查仍会直接拒绝 `val (a, b) = ...` 这类 pattern binding。
- 影响：局部解构绑定已经逐步推进，但顶层声明还不能复用同一套语法，特性覆盖不一致。
- 证据：`crates/scoopc/src/typecheck/headers.rs:26-32`；`crates/scoopc/src/typecheck/headers.rs:173-193`。

## 7. 值类型建模仍不完整：`struct` 字段与 `with` 更新仍有硬限制

- 现状：`struct` 字段仍不允许 `var`，也不允许默认值；`with` 更新虽然已经支持嵌套字段路径，但 base 仍只支持 `struct`，不会覆盖 tuple / enum 这类其它值类型。
- 影响：值对象的声明与更新能力仍偏弱，无法完整覆盖更接近 Kotlin / record-style 的常见写法。
- 证据：`crates/scoopc/src/typecheck/structs.rs:3-7`；`crates/scoopc/src/typecheck/structs.rs:35-50`；`crates/scoopc/src/typecheck/expr/infer.rs:2531-2551`；`crates/scoopc/src/typecheck/expr/error.rs:926-935`。

## 8. `when` 的 or-pattern 仍不能引入 binder

- 现状：or-pattern 已可用于简单判别，但一旦在 `A(x) | B(x)` 这类模式里引入 binder，就会被直接拒绝。
- 影响：模式匹配的表达能力仍被限制在“无绑定的 or-pattern”，不能把多个分支合并成共享绑定的写法。
- 证据：`crates/scoopc/src/typecheck/when_pat.rs:90-98`。

## 9. annotation 系统仍只覆盖部分规范

- 现状：annotation class 仍不支持继承 / 实现接口，也不支持类型体；实现上只接受“主构造参数承载数据”的 data-only 子集。与此同时，编译器硬编码识别的 built-in annotation 仍只有 `Unsafe / Safe / NoGC / Extern / Intrinsic / CallingConvention`，spec 里写到的 `@Deprecated`、`@Inline`、`@AllowIntrinsic`、`@Suppress` 等编译器语义并未落地。
- 影响：注解系统已经有基本声明、use-site target 与 `@Target/@Retention` 校验，但 richer annotation model 与多项 built-in annotation behavior 仍未兑现。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:77-90`；`crates/scoopc/src/typecheck/builtin_annotations.rs:12-18`；`crates/scoopc/src/typecheck/builtin_annotations.rs:53-66`；`crates/scoopc/src/resolve/mod.rs:311-340`；`SCOOP_FULL_SPEC.md:2285-2299`。

## 10. `inline` / non-local return 语义与规范仍不一致

- 现状：spec §7.2 明确写的是“`inline` 只是优化提示，没有语义效果，也不存在 non-local return”；但当前 typecheck 的 statement checker 会把 `inline` 当成控制流语义的一部分，允许 inline 函数的 lambda 实参里出现 non-local return。更进一步，这条规则本身仍是不完整的：部分调用路径（例如跨文件顶层函数、若干 member call 路径）仍默认按 `inline = false` 处理。
- 影响：当前仓库对 `inline` 的语言模型本身就是分裂的：规范说“无语义”，实现却在静态门禁上赋予了语义，而且还没有统一传播到所有调用形态。
- 证据：`SCOOP_FULL_SPEC.md:1341-1348`；`crates/scoopc/src/typecheck/expr/stmt.rs:196-224`；`crates/scoopc/src/typecheck/expr/stmt.rs:1365-1401`；`crates/scoopc/src/typecheck/expr/call.rs:1154-1155`；`tests/fixtures/typecheck/return_in_inline_lambda_ok.scoop:1-12`。

## 11. FFI / ABI 仍停留在 C ABI + GC-free 最小子集

- 现状：`@CallingConvention` 仍只接受 `"c"` / `"cdecl"`；`@Extern` 顶层变量与函数 ABI 签名仍要求 GC-free 值类型；`FunPtr<F>` 仍不支持 receiver function type；codegen 对函数指针调用也仍只按 C ABI 发 indirect call。
- 影响：系统互操作虽然已经打开，但 ABI 维度的表达能力仍很窄，无法覆盖更复杂的宿主互操作场景。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:145-156`；`crates/scoopc/src/typecheck/annotations.rs:176-191`；`crates/scoopc/src/typecheck/lower.rs:146-150`；`crates/scoopc/src/typecheck/expr/call.rs:974-978`；`crates/scoopc/src/llvm/codegen/mod.rs:9659-9663`。

## 12. const / comptime 仍然只覆盖很窄的纯计算子集

- 现状：声明头检查仍会直接拒绝 `const fun` 上的非 `Pure` effect row 与任何 `eff` 参数；常量 evaluator 仍只覆盖字面量与一元/二元运算，不支持函数外的控制流 / effects / 循环；`const fun` 解释器当前也仍只支持同文件、按“函数名 + 参数个数”的最小选择。
- 影响：编译期执行链路已经不是空壳，但仍停留在最保守的纯函数 / 常量折叠模型，离更完整的 comptime 语义还有明显距离。
- 证据：`crates/scoopc/src/typecheck/headers.rs:34-49`；`crates/scoopc/src/typecheck/headers.rs:94-116`；`crates/scoopc/src/comptime/eval.rs:295-650`；`crates/scoopc/src/comptime/interpreter.rs:412-442`；`crates/scoopc/src/comptime/interpreter.rs:1169-1242`。

## 13. Elvis `?:` 已有静态规则，但仍未进入可执行 lowering / codegen

- 现状：parser 和 typecheck 已经接受 Elvis，且有独立的 nullable / rhs type 规则；但 HIR lowering 仍把它留在 `Any` fallback，LLVM codegen 仍直接报 `elvis operator` unsupported。
- 影响：spec Appendix B.3.2 中已经公开的 `?:` 目前只存在于“语法 + 静态规则”层，无法成为稳定的可执行语言特性。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:270-270`；`crates/scoopc/src/typecheck/expr/member.rs:58-82`；`tests/fixtures/typecheck/safe_call_and_elvis_ok.scoop:1-16`；`crates/scoopc/src/hir/lower/expr.rs:3112-3113`；`crates/scoopc/src/llvm/codegen/mod.rs:13813-13816`。

## 14. 跨文件 / 跨包编译链路已收口

- 现状：compilation-unit 主线现在会按整个编译单元聚合顶层值类型；`scoop build/run` 会从全部源文件收集 monomorph keys，并在 HIR lowering 阶段为跨文件顶层泛型函数生成实例；resolver/typecheck 对 extension 的候选收集已统一覆盖“同包隐式可见 + star import + 显式 import”的跨包 / 跨 cone 发现路径。
- 影响：跨文件顶层值、跨文件泛型实例化与跨包扩展解析已不再是语言规则缺口；`T4007` 之后的工作可以直接建立在统一的 compilation-unit 语义上继续推进。单文件 `scoop dump-ir` 调试入口仍按单文件输入建模，但它不再计入这里的编译链 issue。
- 证据：`crates/scoopc/src/typecheck/expr/collect.rs`；`crates/scoop/src/commands/build.rs`；`crates/scoopc/src/hir/lower/util.rs`；`crates/scoopc/src/resolve/scopes.rs`；`tests/fixtures/run_pass_cone/cross_file_generic_top_level_val_basic/src/main.scoop`；`tests/fixtures/typecheck_cone/cross_cone_extension_imports/app/star_import_ok.scoop`；`tests/fixtures/resolve_cone/extension_imports/app/star_import_ok.scoop`。

## 15. 旧 RTTI 导出 API 对泛型 / `eff` 参数化 nominal 的缺口已收口

- 现状：旧 RTTI `dump_type_rtti` 现已支持通过当前文件 package/import 语境解析参数化 nominal query；generic struct 会按声明处文件上下文重新实例化字段类型并计算布局，parameterized interface / `eff` target 也会导出与 `type_desc` metadata 一致的 canonical name / `type_id`。
- 影响：运行期 `is/as/as?`、`dump-rtti` 的 type descriptor 输出，以及旧 RTTI API 对参数化 nominal 的可观测 identity 现在都已对齐，不再残留“旧 API 直接拒绝 args / eff”的独立语言缺口。
- 证据：`crates/scoopc/src/rtti/mod.rs`；`crates/scoopc/src/typecheck/lower.rs`；`crates/scoopc/src/rtti/type_desc.rs`；`tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`；`crates/scoopc/src/rtti/mod.rs` 单测 `rtti_parameterized_struct_query_instantiates_field_types` 与 `rtti_parameterized_nominal_query_matches_type_desc_metadata`。
