# Issues

## 范围

- 本清单记录当前阶段仍然存在的“语言特性 / 编译器 / 运行时实现”缺口。
- 明确排除：纯粹“还没扩出来”的库 API 愿望单；但若某个已公开表面仍是占位实现，或被 compiler/runtime 的硬限制卡住，仍记入问题。
- 已确认过时的历史说法不再计入问题；若审计过程中发现其它文档/注释也需要同步清理，会挂在相关条目的“关联文档待更新”里。

## 1. effect / continuation 仍然只覆盖最小可执行子集

- 现状：`handle` 虽然已经支持在同一个表达式里混用 non-resuming / immediate-resume / escape-continuation arms，但 arm head 仍只接受 effect operation；effect type param 推断仍只支持单一 type param；receiver effect op 仍被拒绝；escape continuation binder 目前注入的仍是默认 effect row 的 `Continuation<T>`；`Continuation.resume` 仍只接受一个实参，并且命名实参只接受 `value = ...`。
- 影响：effect 语义已经不再是最早期的 shape-based 子集，但在 richer effect polymorphism、receiver op、以及更完整 continuation 类型语义上仍有明显缺口。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:875-883`；`crates/scoopc/src/typecheck/expr/infer.rs:947-958`；`crates/scoopc/src/typecheck/expr/infer.rs:1403-1407`；`crates/scoopc/src/typecheck/expr/call.rs:3425-3436`；`crates/scoopc/src/typecheck/expr/call.rs:3646-3680`。

## 2. `spawn` / `join` / `Task` / executor 的可执行路径仍然硬编码在 `Int` / `u64`

- 现状：`spawn { ... }` 仍要求 body 可赋给 `Int`；typecheck 把结果建模为 `Task<Int>`；HIR lowering 里的 `await` / `join` 仍直连 `__scoop_task_join_int`；runtime executor 仍是 cooperative、单队列、无取消，并且任务结果槽位是 `u64`。
- 影响：结构化并发语法已经可用，但还不是完整的 `Task<T>` / `await` / executor 语义；泛型结果类型、取消和更完整的调度模型仍未贯通。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:478-483`；`crates/scoopc/src/typecheck/expr/infer.rs:510-513`；`crates/scoopc/src/typecheck/expr/infer.rs:567-571`；`crates/scoopc/src/hir/lower/expr.rs:559-576`；`crates/scoopc/src/hir/lower/expr.rs:654-657`；`crates/scoopc/src/llvm/codegen/mod.rs:7991-8025`；`sysroot/task.scoop:44-93`；`runtime/c/scoop_task_executor.c:4-8`。

## 3. lambda 推断与 receiver lambda 仍不完整

- 现状：没有 expected function type 时，未标注类型的 lambda 参数会直接报错；当前只支持 0/1/2 参数 lambda 的 expected-type 向下传播；receiver function type 虽可进入类型系统，但 lambda body 里还不会自动注入 `this`。
- 影响：lambda 基本语法已经可用，但“先写 lambda 再让上下文推断”“更多参数形态”“receiver lambda 直接用 `this`”这些常见写法仍不完整。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:305-321`；`crates/scoopc/src/typecheck/expr/infer.rs:2049-2056`；`crates/scoopc/src/typecheck/expr/infer.rs:2058-2065`。

## 4. 调用语义仍有多处早期门禁

- 现状：函数值调用仍不支持命名实参；函数指针调用同样不支持命名实参，并明确拒绝 receiver function type 作为签名；`callee<T>` 仍不能作为一等值传递；函数签名仍只支持“至多一个 vararg，且必须为最后一个形参”；`super(...)` / `this(...)` 构造器委托调用仍只允许位置参数。
- 影响：Kotlin-like 调用规则并没有在所有 callee 形态上统一落地，调用系统仍带着较多“按 callee 形态分流”的早期特判。
- 证据：`crates/scoopc/src/typecheck/expr/call.rs:789-800`；`crates/scoopc/src/typecheck/expr/call.rs:943-978`；`crates/scoopc/src/typecheck/expr/call.rs:1560-1564`；`crates/scoopc/src/typecheck/expr/mod.rs:147-160`；`crates/scoopc/src/typecheck/expr/entry.rs:910-911`。
- 关联文档待更新：`sysroot/core.scoop`、`sysroot/collections.scoop` 里仍保留“普通成员函数调用尚未全面打通”的历史注释；member call / interface dispatch 现已有实现与回归（例如 `crates/scoopc/src/typecheck/expr/call.rs:4384-4828`、`tests/fixtures/typecheck/member_call_interface_dispatch_not_supported_is_error.scoop:1-18`）。

## 5. 泛型约束与“带实参的超类型”仍不完整

- 现状：`where` 子句仍会直接拒绝带类型实参的 nominal bound；type env 记录 direct supertypes 时仍只保存 FQN、不保存 type args；赋值/上转规则也仍只在目标类型“未带实参”时做 nominal 上转，因此 `MyIter : Iterator<Int>` 这一类参数化 interface 上转语义仍不完整，仓库里才保留了 `IntIterable` 这条专用旁路。
- 影响：泛型约束系统与子类型关系目前只能覆盖较浅的 nominal 场景，参数化接口/父类型的真实实例化语义还没贯通。
- 证据：`crates/scoopc/src/typecheck/where_clause.rs:35-40`；`crates/scoopc/src/typecheck/where_clause.rs:209-215`；`crates/scoopc/src/typecheck/type_env.rs:603-607`；`crates/scoopc/src/typecheck/assignable.rs:180-186`；`sysroot/collections.scoop:26-45`。

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

## 9. annotation class 仍只支持 data-only 子集

- 现状：annotation class 当前仍不支持继承 / 实现接口，也不支持类型体；实现上只接受“主构造参数承载数据”的最小子集。
- 影响：注解系统已经有基本声明与使用能力，但 richer annotation model 仍未落地。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:77-90`。

## 10. FFI / ABI 仍停留在 C ABI + GC-free 最小子集

- 现状：`@CallingConvention` 仍只接受 `"c"` / `"cdecl"`；`@Extern` 顶层变量与函数 ABI 签名仍要求 GC-free 值类型；`FunPtr<F>` 仍不支持 receiver function type；codegen 对函数指针调用也仍只按 C ABI 发 indirect call。
- 影响：系统互操作虽然已经打开，但 ABI 维度的表达能力仍很窄，无法覆盖更复杂的宿主互操作场景。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:145-156`；`crates/scoopc/src/typecheck/annotations.rs:176-191`；`crates/scoopc/src/typecheck/lower.rs:146-150`；`crates/scoopc/src/typecheck/expr/call.rs:974-978`；`crates/scoopc/src/llvm/codegen/mod.rs:9659-9663`。
- 关联文档待更新：`sysroot/core.scoop` 里 `@CLayout(..., packed = ...)` 仍写着“v0 仅支持 packed = 1”，但当前 typecheck/codegen 已支持 `packed` 为 `<= 16` 的 2 次幂（见 `crates/scoopc/src/typecheck/annotations.rs:1710-1714`、`crates/scoopc/src/typecheck/annotations.rs:1759-1767`、`crates/scoopc/src/llvm/codegen/ty.rs:396-407`）。

## 11. const/comptime 仍然只覆盖很窄的纯计算子集

- 现状：声明头检查仍会直接拒绝 `const fun` 上的非 `Pure` effect row 与任何 `eff` 参数；常量 evaluator 仍只覆盖字面量与一元/二元运算，不支持函数外的控制流 / effects / 循环；`const fun` 解释器当前也仍只支持同文件、按“函数名 + 参数个数”的最小选择。
- 影响：编译期执行链路已经不是空壳，但仍停留在最保守的纯函数 / 常量折叠模型，离更完整的 comptime 语义还有明显距离。
- 证据：`crates/scoopc/src/typecheck/headers.rs:34-49`；`crates/scoopc/src/typecheck/headers.rs:94-116`；`crates/scoopc/src/comptime/eval.rs:3-6`；`crates/scoopc/src/comptime/interpreter.rs:8-10`；`crates/scoopc/src/comptime/interpreter.rs:48-56`。

## 12. 跨文件 / 跨包编译链路仍有明显边界

- 现状：表达式 typecheck 里的顶层值类型表仍只收“当前文件”；单态化 lowering 仍只实例化“当前文件内”的顶层函数；扩展函数解析仍只在同包内查找。
- 影响：多文件 / 多包工程虽然已经能走通主路径，但跨文件顶层值、跨文件泛型实例化、跨包扩展分发这些能力仍不完整，语言规则还没有在 compilation-unit 维度上完全统一。
- 证据：`crates/scoopc/src/typecheck/expr/entry.rs:232-237`；`crates/scoopc/src/monomorph/lower.rs:242-245`；`crates/scoopc/src/resolve/mod.rs:432-436`。
- 关联文档待更新：`stdlib/prelude.scoop`、`stdlib/array_iter.scoop`、`stdlib/mutable_array.scoop`、`stdlib/mutable_array_iter.scoop`、`stdlib/mutable_list.scoop`、`stdlib/task.scoop`、`stdlib/test.scoop` 里仍保留“非入口文件 source-backed literals 会被 multi-file lowering 拒绝”的历史注释；HIR multi-file lowering 现在已经明确标注这条旧限制已解除（见 `crates/scoopc/src/hir/lower/mod.rs:1518-1519`）。

## 13. LLVM / runtime backend 仍然只覆盖较窄的 host-only / v0 GC 边界

- 现状：LLVM target 仍只支持 host；stackmap 解析与运行时注册仍只按 v3 / 64-bit host 的最小子集实现；moving GC 仍只支持 spill-slot roots，不支持寄存器 roots；`gc-minimal` / `gc-hosted` 检测到多线程后仍把 `collect()` 退化为 no-op；共享 type-descriptor trace 也仍要求 word 对齐扫描。
- 影响：后端与 GC 已经具备可执行主线，但目标平台、roots 形态和 backend 能力边界仍然很窄，离“泛平台 + 统一 GC 能力矩阵”还有明显差距。
- 证据：`crates/scoopc/src/llvm/target.rs:3-7`；`crates/scoopc/src/stackmap.rs:10-12`；`runtime/c/scoop_stackmap.c:741-743`；`runtime/c/scoop_stackmap.c:1225-1226`；`crates/scoopc/src/llvm/mod.rs:47-56`；`runtime/c/scoop_gc_backend_minimal.c:460-463`；`runtime/c/scoop_gc_backend_hosted.c:470-474`；`runtime/c/scoop_gc_common.c:71-73`。

## 14. 并发基础库仍是最小 host-only 子集

- 现状：`Channel<T>` 在类型表面是泛型，但 runtime 队列节点当前只承载 `u64 word`，不能安全承载 GC 引用；通道语义当前只承诺“多线程 send + 单线程 recv”；线程与同步原语仍只覆盖 host 平台；`threadSpawn(block)` / `Once.run(block)` 当前仅保证非捕获 lambda。
- 影响：并发基础能力已经可跑通基本回归，但值域、平台与 closure 形态都仍然受限，还不能视为完整的一般用途并发库。
- 证据：`sysroot/channels.scoop:18-40`；`runtime/c/scoop_channels.c:8-18`；`sysroot/thread.scoop:15-24`；`runtime/c/scoop_sync.c:8-15`。

## 15. 部分已公开 stdlib / sysroot API 仍是占位实现或仅有声明面

- 现状：`IntIterable.toArray()` 当前仍直接返回空数组；`scoop.net` 目前能看到的是 sysroot 声明面与 typecheck fixture，审计时未发现对应的 runtime / LLVM 映射实现。
- 影响：这些 API 在用户视角上已经“存在”，但语义要么明显不完整，要么仍停留在仅能通过解析 / 类型检查的阶段，容易造成“表面可用、运行期不可用”的错觉。
- 证据：`stdlib/collections_iter.scoop:18-23`；`sysroot/net.scoop:1-68`；`tests/fixtures/typecheck/std_net_tcp_api_surface_ok.scoop:3-5`。

## 16. RTTI 仍不支持泛型 / `eff` 参数化类型

- 现状：RTTI 生成对泛型或带 `eff` 参数的类型仍直接报 `unsupported_generic_type`。
- 影响：运行时类型描述符当前只能覆盖未参数化的类型，generic / effect-parameterized type 的反射与运行时可观测性还没有贯通。
- 证据：`crates/scoopc/src/rtti/mod.rs:122-124`。

## 17. MIR lowering 仍大量依赖 `Todo` 占位

- 现状：MIR 路径里，`assign lhs`、一元/二元表达式、`perform` / `handle` 的结果与 unwind、以及若干控制流出口仍直接降成 `Todo(...)`。
- 影响：当前 MIR 更像“结构回归视图”而不是一条完整可执行的中间表示；如果下一步希望把 MIR 作为优化 / 验证 / 解释的稳定基础，这一块仍有明显缺口。
- 证据：`crates/scoopc/src/mir/mod.rs:12`；`crates/scoopc/src/mir/mod.rs:269-387`；`crates/scoopc/src/mir/lower.rs:324-344`；`crates/scoopc/src/mir/lower.rs:482-544`；`crates/scoopc/src/mir/lower.rs:638-716`。
