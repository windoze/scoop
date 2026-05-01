# Managed ABI 设计（runtime 切分 / cone 化 / ABI 驱动的 helper lowering）

> 生成时间：2026-04-26  
> 状态：设计草案，面向当前仓库重构；不承诺对外兼容性。  
> 目的：引入一个正式的 managed ABI，使编译器能够按 ABI 而不是按特定 FQN 生成必要的调用框架，从而把所有非核心 helper 从 runtime/compiler special-case 中移出，并交给 cone 承接。

## 0. 设计目标与非目标

### 0.1 目标

1. **把“按名字 special-case helper”收敛成“按 ABI lowering helper”**。  
   现阶段很多 helper 被硬编码在编译器里，不是因为它们是语言 intrinsic，而是因为它们需要 ordinary managed call 的调用框架。

2. **把 runtime 收缩到最小 substrate**。  
   runtime 只保留 GC、分配、write barrier、pin/handle、continuation/effect substrate、raw pointer/atomic、必要 host substrate 等核心部分。

3. **允许 cone 承接过去被迫放在 runtime 里的 helper**。  
   这些 helper 可以是 Scoop 实现，也可以是 cone 自己的 native helper；关键是编译器不再按 helper 名字理解它们。

4. **复用现有 ordinary managed ABI，而不是再造一套新的调用规则**。  
   当前编译器已经有 ordinary 参数 lowering、hidden sret、caller side GC root spill、statepoint rewrite 路径；Managed ABI 的目标是把这套内部 ABI 外部化。

5. **为后续 compiler synthesis cleanup 提供统一出口**。  
   一旦 Managed ABI 成立，很多目前占据 `resolver/typecheck/codegen` 三层 special-case 的路径都可以系统性迁出。

### 0.2 非目标

- 本文不追求对外稳定 ABI。当前仓库尚无已发布版本，可以在重构期打破兼容性。
- 本文不要求 v1 立刻支持所有现有语言能力跨 ABI 边界流动。
- 本文不试图把 arbitrary C 函数自动变成“无需遵守 GC discipline 的 managed 代码”。
- 本文不在 v1 里解决 effect/continuation outward suspend 的外部 ABI。
- 本文不要求 v1 立刻把所有 runtime helper 一次性迁完；重点是先建立可验证的 ABI 机制。

## 1. 现状与问题

### 1.1 当前只有两条路

当前仓库里，工具链实际上只有两种 helper 落点：

1. **ordinary 函数路径**  
   编译器会为它生成 ordinary 参数 ABI、ordinary 返回值/hidden sret、caller side root spill，并让 LLVM statepoint pipeline 处理 safepoint。

2. **`@Extern` 路径**  
   当前 `@Extern` 从 typecheck 到 codegen 都被定义成 **C ABI native leaf**：
   - ABI 签名必须 GC-free
   - 必须是 `Pure`
   - 调用点走 `enter_native/leave_native`
   - callee 被视作 `gc-leaf-function`
   - 不允许 ordinary managed return / hidden sret / GC ref 进出

这使得一大批“不是 intrinsic，但需要 managed call 框架”的 helper 没有正式出口。

### 1.2 结果：helper 被迫进入 runtime / compiler special-case

于是当前仓库出现了大量这类现象：

- runtime 中承载了大量并非 substrate 的 helper。
- sysroot 中只是写了声明，真正行为却由 compiler dispatch 按 FQN special-case。
- resolver/typecheck/codegen 三层都维护一份同名 special-case 列表。

当前热点主要集中在：

- `crates/scoopc/src/llvm/codegen/call/dispatch.rs`
- `crates/scoopc/src/resolve/scopes.rs`
- `crates/scoopc/src/typecheck/expr/call.rs`

这些 special-case 中，有不少实际上只是：

- 返回 `String` / `Ref`
- 需要 caller side 的 statepoint / root spill
- 可能在 helper 内部触发分配 / GC

但它们并不是语言 substrate。

### 1.3 这不是“缺少一个 machine calling convention”的问题

这里缺的不是 LLVM 层面的 `callconv` 名字。

真正缺的是一套 **语言级 / lowering 级** 的 ABI 模式，用来定义：

- 参数如何 lowering
- 返回值是否允许 hidden sret
- 是否允许 GC ref 进出
- caller 是否生成 ordinary managed call 框架
- 是否插 `enter_native/leave_native`
- callee 是否视为 `gc-leaf-function`

因此，“Managed ABI”更准确地说是一个新的 **ABI mode**，而不是一个新的 machine calling convention。

### 1.4 native surface 审查补充结论

在 `unsafe_funptr_aggregate_return_tuple` 于 macOS/AArch64 失败、但 Linux/amd64 通过之后，对当前 native surface 做的复审暴露出几条更底层的问题。这些问题不应通过单点补丁修，而应并入本文的 ABI 设计。

1. **当前 callable identity 只有“函数签名”，没有“ABI 身份”**。  
   `FunctionType` 只描述 receiver/params/return/effects；一旦 `@Extern` 符号被取成 `FunPtr<F>`，调用点只剩下 `F`，不再知道它是 ordinary managed callable、还是 C ABI native callable、还是将来的 managed external callable。

2. **direct `@Extern` 与 `FunPtr` 目前没有共享同一套 native ABI classifier**。  
   现状是 direct `@Extern` 会按 native leaf 处理，而 `FunPtr` 间接调用只在参数上复用了 native arg lowering，却在 aggregate 返回值上继续套 ordinary hidden sret。这种“参数 native、返回 ordinary”的分裂模型在 x86_64 上可能偶然工作，但在 macOS/AArch64 这类 ABI 更敏感的平台上会直接暴露。

3. **`GC-free` 不是 `C ABI-safe` 的同义词**。  
   当前 `@Extern` typecheck 只要求签名为 GC-free 值类型，但这并不自动意味着 tuple、普通 nominal value type、enum 等都具备稳定的跨平台 C ABI。是否可安全过 native ABI，必须由单独的 ABI-safe 规则定义，而不能复用 GC-free 判定代替。

4. **裸函数地址不足以表达 native callable contract**。  
   现有 `FunPtr<F>` 运行时表示只是一个 word-sized address；它不能携带 ABI family、calling convention、aggregate return 规则等元数据。因此，只在 call lowering 末端根据 `F` 现推 ABI 是不充分的；设计上必须承认“native callable surface”本身需要一等 ABI 身份。

5. **测试辅助不能把编译器内部 ordinary ABI 假设伪装成 native ABI**。  
   像 “用手写 `void + out* + args` helper 模拟 native aggregate return” 这样的测试方式，只能验证 hidden sret 路径是否自洽，不能证明 `FunPtr` 的 native C ABI 正确。native surface 的验收必须尽量贴近目标平台的真实 ABI，而不是把 ordinary managed ABI 投影到 native helper 上。

这几条结论共同说明：本文不能只把 `Managed ABI` 设计成 “给 extern 多一个 managed 选项”，还必须把 **native callable ABI 的边界与身份** 一并画清楚。否则 runtime helper 从名字 special-case 迁出后，native surface 仍会继续以新的形态泄漏同类问题。

## 2. 分层模型

本文建议把系统明确切成三层：

### 2.1 Substrate

这层是 runtime / compiler 必须固定的核心能力：

- managed allocation / object header / type descriptor
- GC root 枚举、statepoint、stackmap
- write barrier
- `pin/unpin`
- `GcHandle`
- continuation / effect substrate
- raw pointer / atomics / task transport
- 必要的 host substrate（线程、互斥、条件变量等）

这层依然允许 compiler intrinsic、runtime C、LLVM-special lowering 等机制存在。

### 2.2 Managed ABI（又名 Scoop ABI）

这层承接：

- 不是 substrate
- 但又需要 ordinary managed call 框架
- 不能走当前 `@Extern` C ABI native leaf

的 helper。

典型例子：

- `Int.toString`
- `Bool.toString`
- `Char.toString`
- `Float.toString`
- `String.concat`
- `String.replace`
- `String.repeat`
- 将来的一批 `path/io/env/fs/process/time` 表面 helper

### 2.3 C ABI

这层继续保留当前 `@Extern` 的定位：

- native leaf
- ABI-safe 的 native value 参数/返回值
- `Pure`
- `enter_native/leave_native`
- 不承担 ordinary managed return / statepoint caller contract

并且这里的 “C ABI” 应同时覆盖两类入口：

- direct external symbol call（`@Extern`）
- indirect native callable（例如 `FunPtr<F>` 指向的 native function address）

两者只是“符号 vs 指针”表示不同，不应拥有彼此矛盾的参数/返回值 ABI 规则。

换句话说：

- `C ABI` 负责真正的 native FFI
- `Managed ABI` 负责 external linkage 下的 ordinary managed helper

## 3. Managed ABI v1 的核心定义

### 3.1 总定义

Managed ABI = **external linkage + ordinary managed ABI**。

也就是说：

- 外部可链接的符号
- 但参数 / 返回值 / caller 框架都复用 ordinary 函数路径
- 编译器按 ordinary managed call 处理，而不是按 C ABI extern 处理

### 3.2 v1 语义约定

Managed ABI v1 建议约定为：

1. 参数 lowering 复用现有 `ordinary_param_abi(...)`。
2. 返回值 lowering 复用 ordinary return 规则；aggregate 允许 hidden sret。
3. 允许 GC ref 进出 ABI。
4. caller side 必须生成 ordinary managed call 框架：
   - conservative root spill
   - ordinary call site
   - 由 statepoint rewrite 处理 safepoint
5. 不插 `enter_native/leave_native`。
6. callee 不标记为 `gc-leaf-function`。
7. machine callconv 初版仍使用 LLVM 默认 callconv `0`。

### 3.3 v1 范围限制

为了先把 runtime 切开，Managed ABI v1 应刻意收窄：

- 只支持顶层函数
- 不支持泛型导入/导出
- 不支持 closure / function value 参数
- 不支持 outward suspend / continuation crossing
- 初版只支持 `Pure`

这些限制不是长期目标，而是为了先把最重要的 helper 迁移通路打通。

## 4. 语法与前端表示建议

### 4.1 不建议使用 `@Extern("scoop")`

不建议把 `@Extern("scoop")` 解释成 ABI 名字，原因有两点：

1. 当前 `@Extern("...")` 的位置参数语义已经是 symbol name。
2. 当前整条 `@Extern` 语义链都绑定在 C ABI native leaf 上，重载它会让语义混乱。

### 4.2 建议新增 ABI 显式字段

推荐方向：

```scoop
@Extern(name = "cone_string_int_to_string", abi = "scoop")
fun __cone_int_to_string(value: Int): String
```

或者单独引入一个新注解；本文不强制最终语法，但要求前端/HIR 能区分：

- `ExternAbi::C`
- `ExternAbi::Scoop`

### 4.3 HIR 建议

当前 `ExternAbi` 只有 `C`。  
建议扩展为：

```text
ExternAbi {
  C,
  Scoop,
}
```

并让 `ExternFun` 继续保留：

- `abi`
- `symbol`
- `lib`
- `calling_convention`

但要注意：

- `calling_convention` 对 `ExternAbi::Scoop` 初版没有独立意义
- Managed ABI 不是 machine callconv 扩展点

### 4.4 callable ABI 身份必须是一等信息

仅有 `FunctionType` 不足以支撑系统性的 ABI 分流。

本文建议在设计上明确区分两层信息：

- **源码级函数签名**：receiver / params / return / effects
- **调用 ABI 身份**：ordinary managed、managed external、native C ABI

约束如下：

1. `@Extern` 声明必须保留 ABI 身份，而不只是 symbol name。
2. `FunPtr<F>` 若承载 native callable，就不能在 lowering 时把 ABI 身份擦除成“只剩 `F`”。
3. direct call 与 indirect call 应共享同一套 ABI classifier；不能一个入口知道 ABI、另一个入口靠局部规则猜测。
4. `calling_convention`、aggregate return 规则、receiver 参与方式，都属于 ABI 身份的一部分，而不是 callsite 局部补充信息。

本文不强制 v1 立刻把这些都做成公开语言表面，但要求内部设计先按这个边界组织，否则后续无法避免 “同一 native callable 在不同入口下被不同 lowering” 的问题。

## 5. typecheck / lowering 合同

### 5.1 `ExternAbi::C`

继续保留现有门禁：

- 签名必须是 **ABI-safe 的 native value surface**，不能只用 GC-free 近似
- `Pure`
- effect-impermeable

这里要特别强调：

- `GC-free` 只说明“不含 GC 引用”，不说明“跨平台 C ABI 稳定”。
- native ABI v1 应只接受一组明确列出的 ABI-safe 类型，而不是默认放行所有 GC-free aggregate。
- 初版推荐的安全集合应至少包括：标量整数/布尔/字符/浮点、`UIntPtr`、以及 `Ptr<T>`/`@CLayout` 且已证明 ABI-safe 的值类型。
- tuple、普通 nominal value type、enum、以及需要 hidden sret 才能表达的 aggregate，不应仅因 “GC-free” 就自动穿过 `ExternAbi::C`。

### 5.2 `ExternAbi::Scoop`

建议 v1 门禁：

- 允许 GC ref 参数 / 返回值
- 允许 ordinary aggregate 参数 / 返回值
- 初版仍要求 `Pure`
- 初版禁止 effect row 参数
- 初版禁止 outward suspend / continuation crossing
- 初版禁止泛型、closure/function-value surface

### 5.3 为什么 v1 仍然要求 `Pure`

当前 effectful ordinary call 依赖编译器已知 callee 的 `may_suspend` 信息，并在需要时生成 wrapper。  
对 external managed callee 而言，这套 outward effect ABI 目前还没有正式化。

因此 v1 先限定为 `Pure`，是为了优先服务 runtime 切分，而不是一次性设计新的 effect ABI。

### 5.4 `FunPtr<F>` 的合同补充

`FunPtr<F>` 不应被视为“绕过 ABI 设计的自由通道”。

本文建议把它明确建模为：

- 一个 **callable token**
- 它承载源码级函数签名 `F`
- 同时承载其所属的 ABI family

因此：

1. 若 `FunPtr<F>` 来源于 native surface，则其调用必须与 `ExternAbi::C` 共享同一套 ABI-safe 规则。
2. 若将来需要支持 managed external function pointer，则它应是另一条显式 ABI family，而不是复用 native `FunPtr` 语义。
3. direct `@Extern` 取地址再通过 `FunPtr` 调用，不应丢失 ABI identity / calling convention / aggregate return 规则。
4. bare `UIntPtr <-> FunPtr` round-trip 只能保留“地址”这一事实；若语言层需要跨 callsite 稳定复用完整 ABI 信息，必须在内部 lowering contract 中显式保留，而不能依赖最后一跳重新猜测。

## 6. codegen 合同

Managed ABI 的核心不是多一个 symbol kind，而是让 `declare_top_level_fun` / `codegen_top_level_fun_call_impl` 在 ABI 分流时不再把所有 `extern` 都视为 native leaf。

### 6.1 声明阶段

对于 `ExternAbi::Scoop`：

- 使用 external linkage
- 参数类型使用 ordinary ABI 计算
- 返回值允许 hidden sret
- 不标记 `gc-leaf-function`
- 不插 `enter_native/leave_native`

### 6.2 调用阶段

对于 `ExternAbi::Scoop`：

- 参数绑定使用 ordinary ABI
- 返回值路径与 ordinary 函数一致
- aggregate 返回值允许 hidden sret
- 调用点走 ordinary managed call
- caller 侧生成 conservative GC root spill
- 由 LLVM statepoint pipeline 改写该 callsite

### 6.2a native callable 的统一分流要求

对 `ExternAbi::C` 与 native `FunPtr`，codegen 必须共享 **单一 native ABI classifier**，并把下列维度作为一个整体决策：

- 参数 lowering
- receiver lowering
- 返回值 lowering
- aggregate return 是否 direct / indirect
- calling convention
- 是否插 `enter_native/leave_native`
- 是否标记 `gc-leaf-function`

禁止出现以下分裂实现：

- 参数按 native lowering，返回值按 ordinary lowering
- direct symbol call 与 indirect funptr call 使用不同的 aggregate return 规则
- declaration path 与 callsite path 各自独立决定是否 hidden sret

换句话说，native surface 的 codegen 合同必须回答的是：

- “这是哪一类 ABI callable？”

而不是：

- “这次碰巧从哪个入口进来？”

### 6.3 结果

一旦这条路径成立，编译器对 helper 的理解就从：

- “如果 FQN 是某某，就特殊 lowering”

变成：

- “如果 ABI 是 `Scoop`，就走 managed external call lowering”

这正是本文要建立的架构边界。

同时，对 native surface 来说，本文要求编译器从：

- “看到 `FunPtr` 就套用 callable-value 现有 lowering”

转成：

- “先判定该 callable 的 ABI family，再选对应的 declaration/call lowering”

## 7. external implementation 的运行时合同

Managed ABI 解决的是 **caller side 的 managed call 正确性**，不是让任意 native 代码自动成为 GC-aware 代码。

因此 external implementation 仍然必须遵守 runtime substrate 合同。

### 7.1 分配

若 implementation 需要创建 GC object / `String` / `Ref`：

- 必须通过 managed allocator / substrate 构造路径
- 不能伪造一个“长得像 GC ref 的指针”

### 7.2 跨可能 GC 的区间持有传入 ref

若 implementation：

- 接收了一个 `String` / `Ref`
- 后续还要继续使用它
- 中间又可能触发分配 / GC

则必须使用：

- `pin/unpin`
- 或 `GcHandle`
- 或其它正式 substrate 机制

不能直接假设传入裸指针在 safepoint 后仍然稳定。

### 7.3 写入 heap ref

若 implementation 向 GC heap 对象字段写入 GC ref：

- 必须走 write barrier

### 7.4 cone 视角与 native 视角

Managed ABI 的最终目标不是让手写 C 更舒服，而是让 cone 承接 helper。  
因此：

- 对 **cone 编译产物** 而言，compiler 会自动生成正确的 ordinary managed body/ABI
- 对 **native helper** 而言，仍需手动遵守 substrate 规则

这两者都可以是 Managed ABI 的实现者，但主要优化对象应该是前者。

## 8. 迁移策略

建议分三步走。

### 8.1 第一步：建立 ABI，不迁功能

先让工具链具备：

- `ExternAbi::Scoop`
- declaration path
- call lowering path
- IR / statepoint 验证能力

但先不急着清理所有 runtime helper。

### 8.2 第二步：用单一领域试点

选择 `string cone` 作为 tracer bullet：

- 范围清晰
- 既有大量非核心 helper
- 又能覆盖返回 `String`、managed allocation、caller side safepoint 等典型场景

### 8.3 第三步：逐步消掉 special-case

当 `string cone` 路径稳定后，再系统清理：

- compiler dispatch 里的 FQN 分支
- resolver/typecheck 为这些 helper 保留的 builtin 放行规则
- runtime 中仅为这些 helper 存在的薄包装

## 9. `string cone` 试点设计

### 9.1 为什么先做 `string cone`

`string cone` 最适合作为试点，原因是它天然覆盖了三类边界：

1. **纯 Scoop 即可实现的字符串算法**
2. **需要 managed ABI 但不是 substrate 的 helper**
3. **必须留在核心层的 string substrate**

如果这三层边界能在 string 上画清楚，后续 `path cone`、`io cone`、`process cone`、甚至 `sync` 上层包装都会容易很多。

### 9.2 `string cone` 的目标职责

`string cone` 应承接：

- `Int.toString`
- `Bool.toString`
- `Char.toString`
- `Float32.toString`
- `Float64.toString`
- `String.concat`
- `String.hash`
- `String.isEmpty`
- `String.replace`
- `String.repeat`
- `String.compareTo`
- 后续更多“不是 substrate 的字符串 helper”

其中：

- 能用 Scoop 写的逻辑，优先写成 Scoop
- 仍需要少量 native helper 的部分，可以作为 cone 内部实现细节

### 9.3 核心 runtime 里保留什么

`string cone` 试点完成后，核心 runtime 中与 string 相关的内容应只保留：

- `String` 物理布局
- managed allocation substrate
- 必要 type descriptor / object model 合同
- 最底层 byte-level primitive
- pin/handle/write barrier 等 substrate 规则

不应继续把 `Int.toString` 这类公共 helper 算作 runtime 核心。

### 9.4 试点实施顺序

建议顺序：

1. 先让编译器支持导入 `ExternAbi::Scoop` 顶层函数
2. 选 `Int.toString` 作为最小 tracer bullet
3. 再扩展到 `Bool/Char/Float.toString`
4. 再扩展到 `String.concat/replace/repeat/...`
5. 最后再继续缩小 compiler special-case 与 runtime helper 列表

## 10. `string cone` 的验证清单

试点不能只看“功能能跑”，必须证明编译器已经按 ABI 而不是按名字工作。

### 10.1 IR 级验证

至少需要验证：

1. 调用 imported `Scoop ABI` string helper 时，IR 中 **不出现** `enter_native/leave_native`
2. 调用点仍然走 ordinary managed call 路径
3. 含 live GC locals 的 caller 在调用点前后，statepoint rewrite 仍然保留 roots
4. 若 helper 返回 aggregate，hidden sret 路径正确

此外，native surface 需要单独验证：

5. direct `@Extern` 与 native `FunPtr` 对同一目标签名生成一致的 ABI lowering
6. 对 aggregate/native callable，不会出现 “direct call 正确、indirect call 偷套 hidden sret” 的分裂 IR

### 10.2 语义级验证

至少需要验证：

1. `Int.toString` / `Bool.toString` / `Char.toString` / `Float.toString` 返回值正确
2. 调用点前存在多个 live GC roots 时，helper 内部触发分配后这些 roots 仍然正确
3. helper 返回的 `String` 在调用后立即参与更多 GC-sensitive 操作时仍然正确

### 10.3 架构级验证

至少需要验证：

1. 迁移后的 helper 不再需要 `dispatch.rs` 中按 FQN special-case
2. 对应 public surface 不再需要 resolver/typecheck 的 member-call builtin 放行
3. runtime 中只保留真正 substrate 所需的 string 核心，而不是整批 helper
4. native callable ABI 的 declaration / direct call / indirect call 共享同一套 classifier，而不是分别维护局部规则
5. 至少对 `linux/amd64` 与 `macos/aarch64` 各跑一组 direct vs indirect parity 回归，避免把 x86_64 偶然工作的 ABI 假设误记为通用合同

### 10.4 native callable 回归原则

对 `@Extern` / `FunPtr` 的 ABI 回归，本文要求：

1. 测试 helper 尽量贴近目标平台真实 C ABI，不用 ordinary hidden sret helper 冒充 native aggregate return。
2. 若需要验证 indirect native aggregate return，应同时覆盖：
   `@Extern` direct call、取地址后的 `FunPtr` indirect call、以及不同 host 架构。
3. 任何只在单一架构通过、但 ABI 模型本身不自洽的测试，都不能作为设计正确性的证据。

## 11. 成功判据

本文方案落地后，应达到以下结果：

1. 编译器可以对 external symbol 生成 ordinary managed call 框架。
2. external helper 是否被特殊处理，不再取决于其 FQN，而取决于其 ABI。
3. `string cone` 可以承接第一批非核心 string helper。
4. runtime 的职责收缩到 substrate。
5. 后续 cone 化工作可以按领域推进，而不是继续向 runtime/compiler 追加名字驱动的 helper。

## 12. 后续扩展（v2+）

v1 稳定后，可以再考虑：

- 支持泛型导入/导出
- 支持 closure / function-value crossing
- 支持 outward effect / continuation ABI
- 给 Managed ABI 增加版本号 / feature bitmap
- 在 cone 之间建立更正式的 import/export 工具链

这些都不是 v1 的前置条件。

## 13. 一句话总结

Managed ABI 的本质不是“给 `@Extern` 多一个名字”，而是：

**把当前仅在编译器内部使用的 ordinary managed ABI 正式外部化，让编译器以后按 ABI 生成 helper 调用框架，而不是按 helper 名字生成特殊路径。**

`string cone` 是最适合的第一块试验田；如果它成功，就说明 runtime 切分和 compiler synthesis cleanup 终于有了统一的架构出口。
