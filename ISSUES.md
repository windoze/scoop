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
- multi-file lowering 对“非入口文件 source-backed literals”的旧限制已解除，相关 stdlib 注释过时，不再算语言 feature 缺口。证据：`crates/scoopc/src/hir/lower/mod.rs:1509-1510`；`stdlib/array_iter.scoop:9-9`；`stdlib/mutable_array.scoop:15-15`；`stdlib/mutable_array_iter.scoop:8-8`；`stdlib/mutable_list.scoop:10-10`。

## 1. effect / continuation 主路径已足够支撑手动 step 的 `Task`，但 richer async 组合写法仍未完全收口

- 现状：escape continuation 已经可以 capture、堆存、跨作用域 later-resume，并且 `Continuation.resume` surface 现已和 payload transport 对齐：`Continuation<Unit>` 支持 `k.resume()`，tuple payload 支持扁平 `k.resume(v0, v1, ...)` 与 `a0/a1/...` 命名实参，同时旧的单 payload `k.resume(value)` 继续兼容。多 effect type params、receiver effect op、handler arm head 的显式 effect/op type args，以及 escape continuation binder 的精确 effect-row 注入也都已收口，现有 FIFO/LIFO scheduler fixtures 继续证明 pure Scoop 代码可以把 `Continuation<T>` 存进普通 class 字段后手动推进计算。因此，从“Task 内部靠 continuation stepping”这个角度看，effect codegen 主路径已经可用。当前剩余缺口主要收窄为：在 async 组合子路径上，LLVM 后端对 escape continuation 的组合能力仍偏弱，`Task<Int>.andThen` 这类“一个 setup 里串两个 await”仍需要拆成两段 `handle`。与此同时，`-> resume` / ImmediateResume 的 stack-local fast path 现已被明确记录为 deferred optimization；当前实现继续统一使用 GC-managed full machine，而不是再让 spec 隐含承诺“已经有独立 stack-local storage 选择”。
- 影响：effect / continuation 已经不再是 “Task redesign 无法开始” 的 blocker；它已经足以支撑“Task 是普通对象、内部保存并推进 continuation”的方向。真正尚未完成的是更自然的多 suspend / 多 await 组合写法，而不是 continuation 的基本表示、resume surface 或隐含的 stack-local ABI 承诺。
- 证据：`tests/fixtures/run-pass/effect_escape_continuation_scheduler_fifo_multi_task.scoop:18-163`；`tests/fixtures/run-pass/effect_escape_continuation_scheduler_lifo_multi_task.scoop:19-155`；`tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop:1-68`；`tests/fixtures/typecheck/continuation_resume_surface_ok.scoop:1-15`；`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop:19-58`；`crates/scoopc/src/typecheck/expr/call.rs` 中 `try_infer_continuation_resume_call_expr_type`；`crates/scoopc/src/llvm/codegen/effect/mod.rs` 中 `codegen_continuation_resume_payload_value`；`SCOOP_FULL_SPEC.md` 第 5.4/5.5/5.6 节；`SCOOP_RUNTIME.md` 第 10 节。

## 2. `Task` core / stable handle wake-token 合同已收口

- 现状：`Task<T>` 现已作为普通、lazy、可手动 `poll()/step()` 的 `scoop.core.Task` 对外成立；`async {}` / `async fun` 统一 lower 到内部 task-create + step-result 主线，公开的 `scoop.task` executor surface、runtime executor implementation 与 compiler 端 `scoop.task.*` / `Executor` special-case 都已移除。`spawn/join` 当前只保留语法壳以便后续恢复 structured concurrency，但 typecheck 会统一报 `structured_concurrency_deferred`，不再把它们算作当前 `Task` core 的一部分。stable handle 合同也已补齐：reactor / callback / future executor 现统一以 `GcHandle.raw` 作为长期 wake token / identity，而 `Pinned` 只承担短时裸地址借出。
- 影响：`Task` 已经真正脱离 executor 与 structured-concurrency 前提，可以单独作为通用 pollable object 成立；当前这条不再构成剩余 blocker。后续若重新展开 executor framework 或公开 structured concurrency，应作为新的独立设计任务进入，而不是回写当前 `Task` core 合同。
- 证据：`sysroot/core.scoop`；`SCOOP_FULL_SPEC.md` 第 5.7 节；`SCOOP_RUNTIME.md` 第 11 节；`crates/scoopc/src/typecheck/expr/error.rs`；`crates/scoopc/src/typecheck/expr/infer.rs`；`crates/scoopc/src/llvm/codegen/mod.rs`；`runtime/c/scoop_task.c`；`runtime/c/scoop_gc.h`；`crates/scoop_runtime/src/abi_exports_allowlist.rs`；`crates/scoop_runtime/tests/task_spawn_join.rs`；`tests/fixtures/typecheck/spawn_deferred_is_error.scoop`；`tests/fixtures/typecheck/join_deferred_is_error.scoop`；`tests/fixtures/run-pass/async_await_string_basic.scoop`；`tests/fixtures/run-pass/task_poll_step_manual_basic.scoop`；`tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop`；`tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`。

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

## 7. 值类型语义应继续保持不可变；后续重点是增强 `with`

- 现状：spec 已明确所有 value type 都是 immutable，`with` 的语义也是“创建修改后的副本”；当前实现据此拒绝 `struct` 的 `var` 字段和值类型上的 `var` property。也就是说，这里的缺口不应再理解为“还没支持 Swift-style 可变 struct”。真正仍未完成的是 immutable-friendly 的更新与声明能力：`with` 当前虽然已经支持多段字段路径与并行冲突检查，但 base 仍只支持 `struct`，还不能泛化到 tuple / enum 等其它值类型；字段默认值这类声明便利性也仍未覆盖。
- 影响：如果把“字段级写回式 `var`”继续当成待补 feature，会把当前更接近 Valhalla-style 的 value semantics 搅混。更清晰的方向是保持整体不可变，把值对象更新统一收敛到显式 `with`，再逐步增强 `with` 的覆盖面与人体工学。
- 证据：`SCOOP_FULL_SPEC.md:45-46`；`SCOOP_FULL_SPEC.md:371-371`；`SCOOP_FULL_SPEC.md:1588-1588`；`SCOOP_FULL_SPEC.md:1628-1628`；`crates/scoopc/src/typecheck/structs.rs:3-7`；`crates/scoopc/src/typecheck/structs.rs:35-50`；`crates/scoopc/src/typecheck/properties.rs:62-72`；`crates/scoopc/src/typecheck/expr/infer.rs:2967-3041`；`crates/scoopc/src/typecheck/expr/error.rs:925-955`。

## 8. `when` 的 or-pattern 仍应先收敛到“无 binder”子集

- 现状：or-pattern 已可用于简单判别，但一旦在 `A(x) | B(x)` 这类模式里引入 binder，就会被直接拒绝；resolver 也不会在 `WhenPat::Or` 下声明 binder。与此同时，现有语法已经可以表达 `A(..) | B(..)` / `A(_) | B(_)` 这类“只判别、不绑定”的子集。
- 影响：当前真正低风险、低改动的方向应是优先支持“无 binder 的 payload or-pattern”。若未来要放开 binder-sharing，则至少需要要求“各分支 binder 集合一致且每个 binder 精确同型”，不能把 `A(x) | C(x)` 这类情况宽松合流成 `Any`。另外，暂不应把 bare `A | C` 扩成“忽略 payload”的语法糖，因为 parser 当前把大写裸名解释为 0-arg variant。
- 证据：`crates/scoopc/src/typecheck/when_pat.rs:89-103`；`crates/scoopc/src/typecheck/when_pat.rs:193-206`；`crates/scoopc/src/resolve/scopes.rs:940-950`；`crates/scoopc/src/parser/expr.rs:2077-2085`。

## 9. annotation 的 declaration model 已收口为 compile-time markers only，但多项 built-in behavior 仍未兑现

- 现状：annotation class 已明确不是一般 nominal type / class 能力的延伸；实现与规范都已收口到“主构造 `val` 参数承载编译期 marker payload”的 data-only 子集，并拒绝 supertypes、type body、type/effect params 与 `where`。当前剩余缺口主要收窄为 built-in annotation behavior：编译器已硬编码识别 `Unsafe / Safe / NoGC / Extern / Intrinsic / AllowIntrinsic / CallingConvention`，并把 `@AllowIntrinsic` 收口为 file/module gate；spec 里写到的 `@Deprecated`、`@Inline`、`@Suppress` 等编译器语义仍未完全落地。
- 影响：注解声明模型、use-site target 与 `@Target/@Retention` contract 已经统一；剩余 issue 不再是“annotation 要不要做成复杂 nominal feature”，而是 built-in annotations 的具体编译器行为仍不完整。
- 证据：`crates/scoopc/src/typecheck/annotations.rs`；`crates/scoopc/src/typecheck/builtin_annotations.rs`；`crates/scoopc/src/resolve/mod.rs`；`SCOOP_FULL_SPEC.md` 第 15 节。

## 10. 应先删除 legacy `inline` / non-local return 语义残留；新设计另议

- 现状：spec §7.2 明确写的是“`inline` 只是优化提示，没有语义效果，也不存在 non-local return”；但当前 typecheck 仍把 `inline` 当成控制流语义的一部分，允许 inline 函数的 lambda 实参里出现 non-local return，错误文案和 fixture 也仍沿用这套 legacy 模型。
- 影响：当前仓库对 `inline` 的语言模型仍然分裂。更稳的方向不是继续修补这套 legacy 规则，而是先把相关 wording / gate / fixture 从现有语言模型中移除；若以后要重新引入 non-local return / break / continue，更适合基于 effect / handler 重新设计，而不是继续绑定在 `inline` 上。
- 证据：`SCOOP_FULL_SPEC.md:1341-1348`；`crates/scoopc/src/typecheck/expr/stmt.rs:190-264`；`crates/scoopc/src/typecheck/expr/error.rs:1093-1100`；`crates/scoopc/src/typecheck/expr/mod.rs:93-97`；`tests/fixtures/typecheck/return_in_inline_lambda_ok.scoop:1-12`；`tests/fixtures/typecheck/return_in_non_inline_lambda_arg_is_error.scoop:1-15`。

## 11. FFI / ABI 仍缺少明确的 effect-impermeable 边界与 pinned token 模型

- 现状：`@CallingConvention` 仍只覆盖最小 C ABI；`@Extern` 仍要求 ABI 签名为 GC-free 值类型，并鼓励通过 `Ptr<T>` / `UIntPtr` / handle 显式桥接。这个方向本身没问题，但普通 FFI 边界在规范与实现意图上仍缺少两条更明确的约束：其一，effect / continuation / non-local control 不应穿越普通 `@Extern` ABI；其二，当前 `Pinned` 仍是 `struct Pinned(val value: Any)`，不是像 `FunPtr<F>` 那样可直接出现在 ABI 上的 word-sized opaque token，因此“pin 后直接声明 extern 参数”的模型仍未定型。
- 影响：如果没有清晰的边界契约，FFI 只能继续依赖 `UIntPtr` / `Ptr<T>` 的手工协议，类型系统看不见“这是 pinned token”；同时也难以把 `@NoGC` 收敛成“普通 FFI 接口不暴露 GC / effect 语义”的更清楚模型。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:184-191`；`crates/scoopc/src/typecheck/annotations.rs:1435-1441`；`SCOOP_FULL_SPEC.md:2804-2812`；`SCOOP_FULL_SPEC.md:2846-2847`；`sysroot/core.scoop:212-223`；`sysroot/unsafe.scoop:16-22`；`sysroot/unsafe.scoop:92-98`；`crates/scoopc/src/llvm/codegen/ty.rs:86-96`。

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
