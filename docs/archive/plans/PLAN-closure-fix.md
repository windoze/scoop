# Scoop：closure capture 语义修正 + sealed interface + 配套库类型 计划

> 生成时间：2026-05-18
> 设计基线：[`CLOSURE_FIX.md`](./CLOSURE_FIX.md)（2026-05-17 设计讨论）
> 当前状态：待开始
> 上一轮（core / stdlib reshape）已完成；归档于 [`docs/archive/plans/PLAN-core-reshape.md`](./docs/archive/plans/PLAN-core-reshape.md) 与 [`docs/archive/plans/TODO-core-reshape.md`](./docs/archive/plans/TODO-core-reshape.md)。
> 行号说明：下文以本计划生成时点的文件路径与符号名为准；后续若行号漂移，优先按文件路径、符号名、fixture 名定位。

## 0. 工作原则

- 当前 closure capture 实现违反 spec：外层 `var x` 被捕获时编译器隐式 box 化，使得"closure 多次调用之间 rebind 持久"。Spec 要求**默认 by-value snapshot + per-call reset**，等同于一次复制再绑定到本次 call frame；想要持久状态必须显式 `RefCell<T>` / `Atomic<...>`。
- 仓库尚无已发布版本，**不保留任何前向兼容性**：MIR Rvalue / Transport variant、LLVM closure env layout、sysroot 声明都允许一次性改换。
- "intrinsic 是否保留"沿用前一轮判定标准：**编译器在该 callsite 是否生成实质代码**。CaptureBox 一整套（New / Get / Set + Transport）属于编译器后门，本轮按"删除而非修复"处理。
- 因为 Scoop 不做隐式 boxing，"用户想要共享 mutable 状态"的出口被推到**库类型**——`RefCell<T>` / `Atomic<...>`。这就要求类型系统能在 generic bound 上区分"T 是 ref 类型"还是"T 是 value 类型"；引入 `sealed interface AnyRef` / `AnyValue` 作为底座。
- `sealed interface` 是一个独立构造，与普通 `interface` 正交。**body 必须为空**——此条不可破坏，是其与未来 derived interface（带成员、编译器派生）的根本差别。
- `sealed interface` 起步只能在 sysroot 模块定义；用户模块定义触发 frontend reject。理由：用户定义的 sealed marker 没有 auto-implementation 规则，永远不可满足，等于无意义。
- `sealed interface` 之间允许继承，但只能继承其它 sealed interface；本轮**起步就要落地继承机制**（语法 + 传递闭包登记 + 互斥 / 循环 / 非 sealed supertype 的 frontend reject），否则未来加细分 marker（如 `PlainValue`）需要触动 typecheck。
- per-call reset 是 closure 的副产品：closure env 在构造之后 uniformly immutable，没有 GC barrier / escape / 并发 publication 等 per-instance 写场景的烦恼。lambda 内 captured 名字的写入是普通 call-frame alloca 写，与"普通函数体里写本地 var"完全同构。这意味着**LLVM closure 路径几乎无需重写**，只需删除 CaptureBox 后门 + 修一个 `mutable: false` 写死的 bug。
- 本轮大量触及 fixture：fixture 的迁移原则与上一轮一致（"能跑就保留并改 import / 能合并就合并 / 不能跑就删"），但**仓库内不允许保留任何 failing fixture**（与 core reshape 同条铁律）。任何"暂时性 failing fixture"必须在产生它的任务完成记录中列出，并指明哪个后续任务负责处理。

## 1. 当前判断

### 1.1 closure capture 当前实现的偏离（与 spec 对照）

| 位置 | 现状 | spec 应有 |
|---|---|---|
| `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:720-724, 796` | 外层 `var x` 被捕获时改写为 `Rvalue::CaptureBoxNew` + 写回 `Rvalue::CaptureBoxSet` | 外层 var 用普通 alloca，不改写 |
| `crates/scoopc/src/mir/lower/fn_lowering_call.rs:637-639` | mutable capture 走 `MirTransportKind::CaptureBox` | 所有 capture 走值/ref 普通 transport |
| `crates/scoopc/src/mir/mod.rs:2381-2398` | 存在 `Rvalue::CaptureBoxNew/Get/Set` 三个 variant | 删除 |
| `crates/scoopc/src/mir/transport.rs:17, 189` | `MirTransportKind::CaptureBox` + `CaptureBoxTransportMetadata` | 删除 |
| `crates/scoopc/src/llvm/codegen/closure/mod.rs:97-101` | guard 拒绝 mutable capture（`UnsupportedMainBody { kind: "mutable capture (not supported yet)" }`） | 修复后 guard 不可达；改 `unreachable!` 进 `INTERNAL_BUG_SENTINEL_HITS` |
| `crates/scoopc/src/llvm/codegen/closure/mod.rs:675` | 内层 binding 写死 `mutable: false` | 改为 `mutable: cap.mutable` |
| `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:107-108, 136` | effect-lowered 路径的 CaptureBox 分支 | 删除 |
| `crates/scoopc/src/llvm/codegen/composite_transport.rs:14, 101, 166-168, 272, 477, 701, 713, 805` | composite transport 的 CaptureBox 分支 | 删除 |
| `crates/scoopc/src/llvm/reachability.rs:518-520, 595-597, 748-749, 817` | reachability 分析的 CaptureBox 分支 | 删除 |
| `crates/scoopc/src/mir/{closure_simplify, escape, inline, summary, dump}.rs` 等多处 | CaptureBox arm | 全部删除 |
| `tests/fixtures/mir_lowered/aggregate_transport.mir:138-174` 等 | MIR dump 含 `CaptureBoxNew` / `MirTransportKind::CaptureBox` | regen |
| `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop` (EXPECT-EXIT=30) | 在新旧语义下都成立（`f` 只调一次），无区分能力 | 保留，另补区分性 fixture |
| frontend gate | 无任何针对 closure capture 的拒绝路径 | 不需要新加（默认 snapshot 是合法语义） |

CaptureBox 一整套在仓库中合计 **139 处出现**（含 mir / llvm / effect_lowered / pipeline / effect_facts 多个子系统）。完整清单见 [`TODO.md`](./TODO.md) 任务 C2-T01。

### 1.2 修复后的 LLVM closure 路径几乎无需重写

- closure env 在构造之后 **uniformly immutable**——env 字段只读不写。
- lambda 内 captured 名字的写入是普通 call-frame alloca 写，与"普通函数体里写本地 var"完全同构。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:625-678` 现有 inner-side codegen 结构本来就是对的——每次 lambda 调用时为每个 capture 建一个新的 entry alloca，从 env 字段 load 一次填进去，天然 per-call reset。
- 唯一的 bug 是 line 675 写死 `mutable: false`。改成 `cap.mutable` 之后，外层 `var` 捕获就能在 lambda 内被 rebind（写本次调用 frame 的私有 alloca，下次调用从 env reload 重置）。
- 修复后 line 97-101 的 `(not supported yet)` guard **真的可达不到**，改 `unreachable!` 即可。
- MIR / effect-lowered 路径同样应用"per-call alloca + env-load"模式，删除一切 box 间接。

### 1.3 sealed interface 的当前底座

- `crates/scoopc/src/ast/mod.rs:560` `enum Modifier::Sealed` 已存在；`enum TypeKind::Interface` 已存在。
- `crates/scoopc/src/parser/cursor.rs:549, 568` `Keyword::Sealed` / `Keyword::Interface` 已是 keyword。
- 现有 parser 已能解析 `sealed interface I {}`（作为 `TypeDecl { modifiers: [Sealed], kind: Interface }`），但 typecheck 不识别"sealed marker"概念——它会被当作普通带 `sealed` 修饰的 interface 处理。
- 没有"按 nominal kind 自动登记 marker"机制——本轮新建。
- 没有"sealed interface 之间继承的传递闭包登记"机制——本轮新建。

### 1.4 当前 `Atomic` 一族缺位

- `sysroot/scoop.unsafe/unsafe.scoop:151-175`：内部 `__AtomicInt = Int` typealias + `__atomicIntLoad/Store/CompareExchange` 三 intrinsic（仅作 word-sized lvalue 上的原子操作，未对接 GC barrier / ref 类型）。
- 没有面向用户的 `Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>` / `AtomicInt` / `AtomicBool` 类型。
- `RefCell<T>` / `Box<T>` 也不存在。
- 这意味着 closure capture spec 落地后，"想要共享 mutable 状态"的用户在过渡期没有现成出口；本轮必须把这一组库类型一并落地。

## 2. 设计目标

1. **closure capture 默认语义与 spec 对齐**：构造点 by-value snapshot；env 字段构造后不可变；lambda 体内 captured 名字等价于本次 call frame 的本地变量；外层 `val` → 内层 immutable 本地、外层 `var` → 内层 mutable 本地（rebind 仅作用于本次调用 frame）；ref type capture 共享堆对象但不共享 binding 槽位。
2. **删除 CaptureBox 一整套**：Rvalue::CaptureBoxNew/Get/Set、MirTransportKind::CaptureBox 与配套 metadata，从 MIR / LLVM / effect_lowered / pipeline / reachability / closure_simplify / escape / inline / summary / dump / materialize 等所有路径一次性删除。
3. **修复 closure inner-mutable bug**：`closure/mod.rs:675` 写死 `mutable: false` 改为 `mutable: cap.mutable`；effect-lowered 路径的等价位置同步修复。
4. **引入 `sealed interface` 独立构造**：与普通 `interface` 正交；body 必须为空；只能用作 generic bound；起步限定 sysroot only；支持 `sealed interface` 之间继承（含传递闭包、互斥/循环/非 sealed supertype 的 frontend reject）。
5. **sysroot 起步落地 `AnyRef` / `AnyValue` 两个 marker**：扁平结构；类型 typecheck 完成后按 nominal kind 自动登记到 comptime nominal-subtyping table；不进入 instance layout / itable / vtable / type descriptor 等任何 runtime 数据结构；bound 在 monomorphization 期 erase。
6. **落地 mutable & atomic shared-state 库类型**：`RefCell<T>` / `Box<T>`（普通 class，T 无 bound）；`AtomicInt` / `AtomicBool`（复用现有 atomic-int field intrinsic）；`Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>`（依赖新 atomic-ref intrinsic + GC barrier；`AtomicValue<T>` 内部用 `Atomic<Box<T>>`，`cas` 签名采用 `expected: Box<T>` 而非 `expected: T`）。
7. **frontend reject 边界齐全**：sealed interface 各类禁用位置（binding/param/return type、type argument、`is`/`as`、显式实现）、用户模块定义、继承非 sealed interface、自循环 / 构造性循环、同时继承互斥 marker——每条对应一个错误码 + 一个 EXPECT-ERROR fixture。
8. **fixture 矩阵齐全**：per-call reset 正样本、外层不受影响正样本、ref type capture + 通过 ref 改对象、`RefCell` 显式共享（makeCounter 模式）、`AtomicInt` / `Atomic<MyClass>` / `AtomicValue<MyStruct>` 基本 load/store/cas、以及 11 条 frontend reject 反样本（详见 [`TODO.md`](./TODO.md) C4-T01）。
9. **审计基线更新**：`pipeline_user_visible_failure_policy.rs::STALE_UNSUPPORTED_MAIN_BODY_COUNTS` / `INTERNAL_BUG_SENTINEL_HITS` / `FRONTEND_REJECT_SURFACES` 三表全部按本轮的删除 / 新增逐条对账。
10. **文档更新**：`SCOOP_FULL_SPEC.md` 增加 closure capture 默认语义条目；`MANAGED_ABI.md` 不需要改（CaptureBox 不属于 managed ABI surface）；CLOSURE_FIX.md 标注"实现进度跟踪移交至 PLAN.md / TODO.md"。

## 3. closure capture spec（最终语义）

### 3.1 默认 by-value snapshot

- closure 捕获恒为**构造点 by-value snapshot**：
  - value type：复制值；
  - ref type：复制 ref 指针（managed pointer 的 copy）。
- closure env 字段在构造之后**不可变**——env 不持有可被 lambda 写入的槽位。

### 3.2 lambda 体内的捕获名

lambda 体内对 captured 名字的引用，**等价于该 lambda 自己的 call-frame 本地变量**：

- 入口处由 env 字段 load 一次进入 call-frame alloca；
- 外层 `val x` → 内层 immutable 本地，可读不可 rebind；
- 外层 `var x` → 内层 mutable 本地，可读可 rebind；
- rebind 仅作用于本次调用的 call frame，**不影响外层**、**不在多次调用之间持久**。

### 3.3 ref type capture

closure 持有 ref 副本：

- 可以通过该 ref 读 / 写堆对象（对象状态对外层与 closure 共享，因为是同一个堆对象）；
- 不能 rebind 让外层 ref 指向另一个对象（rebind 只动本次调用 frame 的副本）。

### 3.4 显式共享出口

想要"closure 与外层共享 mutable 状态"或"closure 实例自带跨调用持久状态"——**必须显式使用 `RefCell<T>` / `Atomic<...>` 等库类型**。Scoop **不做隐式 boxing**。

### 3.5 与 Kotlin 的差异

Kotlin 对捕获的 `var` 隐式 ref-box：

- closure 多次调用之间，rebind 持久；
- 与外层共享同一份状态。

Scoop 不做隐式 boxing：

- 默认 capture per-call 重置；
- 想要 Kotlin 那种行为请显式 `val n = RefCell(0)` + `{ n.value = n.value + 1; n.value }`。

文档 / 诊断需要明确告知用户这条差异——尤其当用户写出形如 `makeCounter`（捕获 `var` 期望累加）的 Kotlin 习语时。

## 4. sealed interface 设计

### 4.1 语法层面

- `sealed interface I` 是一个独立构造，与普通 `interface I` 正交。AST 上仍是 `TypeDecl { modifiers: [Sealed], kind: Interface }`，typecheck 阶段把"含 `Sealed` modifier 的 interface"识别为 sealed marker 并走独立路径。
- **body 必须为空**：任何成员声明（`fun` / `val` / `var` / 嵌套类型）触发 frontend reject，错误码 `scoop::typecheck::sealed_interface_must_be_empty`。
- sealed interface 名只能作为 **generic type bound**。允许：`class C<T: AnyRef>`、`fun f<T: AnyRef>(): T`、`<T: AnyRef + Hashable>` 等组合 bound。禁止的用法（每条对应一个 frontend reject 错误码）：
  - binding type：`val v: AnyRef = ...` → `scoop::typecheck::sealed_interface_not_allowed_as_binding_type`
  - param type：`fun f(x: AnyRef)` → `...not_allowed_as_param_type`
  - return type：`fun f(): AnyRef` → `...not_allowed_as_return_type`
  - type argument：`val xs: List<AnyRef>` → `...not_allowed_as_type_argument`
  - runtime type test：`when (x) { is AnyRef -> ... }` → `...not_allowed_in_runtime_test`
  - runtime cast：`x as AnyRef` / `x as? AnyRef` → `...not_allowed_in_runtime_cast`
  - 显式实现：`class C : AnyRef` / `struct S : AnyValue` → `...not_allowed_as_explicit_supertype`
- 起步阶段限定 sealed interface **只能在 sysroot 模块定义**——用户模块定义 sealed interface 触发 frontend reject，错误码 `scoop::typecheck::sealed_interface_user_definition_not_allowed`。理由：用户定义的 sealed marker 没有 auto-implementation 规则，永远不可满足，等于无意义。未来如有需要再放开。
- 互斥 bound：`<T: AnyRef + AnyValue>` 是 uninhabited，typecheck 阶段直接报错，错误码 `scoop::typecheck::sealed_interface_mutually_exclusive_bound`，错误信息提示"两个 marker 互斥，不存在同时满足者"。
- **sealed interface 之间允许继承，但只能继承其它 sealed interface**：
  - 允许：`sealed interface PlainValue : AnyValue`、多重继承 `sealed interface I : J, K`（marker 无成员，不存在 diamond 问题）；
  - 禁止（每条对应一个 frontend reject）：
    - 继承普通 `interface`（会偷渡成员，破坏 body-empty 不变量）→ `scoop::typecheck::sealed_interface_supertype_must_be_sealed`
    - 继承 `class` / `struct` / 其它非接口类型 → 同上错误码
    - 自循环 `sealed interface I : I` 与构造性循环（`A : B; B : A`）→ `scoop::typecheck::sealed_interface_inheritance_cycle`
    - 同时继承互斥 marker（例如 `sealed interface I : AnyRef, AnyValue` 必拒）→ 复用 `scoop::typecheck::sealed_interface_mutually_exclusive_bound` 同款提示，typecheck 在定义点就检测

### 4.2 编译器层面

- 每个 nominal 类型 typecheck 完成后，按其 kind 由 typecheck pass 自动登记其满足的 sealed marker：
  - `class` → `AnyRef`
  - `struct` / `tuple` / `enum` / 内建标量（`Int` / `Bool` / `Float` / ...）→ `AnyValue`
  - 互斥不交（任何类型只属于其中之一）。
- 该登记仅写入 **comptime nominal-subtyping table** / type metadata，**不进入** instance layout / class itable / class vtable / type descriptor 等任何 runtime 数据结构。
- bound 在 monomorphization 期 erase；运行时 `AnyRef` / `AnyValue` 不可观测。
- 因为 body 为空，没有 vtable / itable slot 需要分配；新增构造对 runtime 完全免费。
- **sealed marker 间继承在登记阶段做传递闭包**：当类型 X 被登记为某 sealed marker M 时，X 自动满足 M 的全部直接与传递 supertype sealed marker。这一闭包在 typecheck 完成 nominal kind 登记的同一阶段一次性算好，写入同一份 comptime metadata；bound 解析直接查询登记集合，不需要在解析时再走 supertype 链。

### 4.3 为什么必须 body 空

一旦 sealed interface 允许带成员：

1. **runtime 成本**：每个实现者要往 itable 里塞函数指针，runtime 数据结构要变；
2. **auto-implementation 复杂度**：编译器要替每个合适类型生成方法体——这是 Rust `derive` / Swift `Hashable` 那一整套独立语言特性；
3. **bound-only 限制不再合理**：成员让接口"有用"，用户会自然想 `val x: I = ...` 做多态分发。

所以"sealed = empty-body marker"是不可破坏的不变量。未来如果引入"带成员、编译器派生"的 trait（Comparable / Hashable / Numeric / ...）将以**独立新构造**引入（命名待定，例如 `derived interface`），不复用 `sealed interface` 语义。

### 4.4 sysroot 起步定义

```scoop
package scoop.core

// 起步只这两个，扁平结构
sealed interface AnyRef
sealed interface AnyValue
```

注意：sealed interface 之间继承机制（见 §4.1）**起步就要落地实现**——本轮虽然只引入扁平的 `AnyRef` / `AnyValue` 两个 marker，但继承语法、传递闭包登记、互斥 / 循环 / 非 sealed supertype 的 frontend reject 都要在 C1-T01 一起完成。这样未来加细分子 marker（例如 `sealed interface PlainValue : AnyValue` 表示"value type 且不含 GC ref"）就只是 sysroot 加一行 + 把 auto-impl 规则加细，不再触碰 typecheck / codegen。

PlainValue 等具体子 marker **本轮不引入**，按需后补。

### 4.5 命名

- sealed marker：**`AnyRef` / `AnyValue`**——与 Kotlin/Scala 的 `Any` 风格一致，避开与 sysroot 内部类型 kind 名 `RefTypeKind` / `ValueTypeKind` 在用户视角的语义混淆。
- 摸底任务（C0-T01）已确认：现有 sysroot 只有 `Any` / `Unit` / `Nothing` 等基础类型，没有 `AnyRef` / `AnyValue` 同名类型；可直接落地。

## 5. 配套库类型：mutable & atomic shared-state primitives

### 5.1 总表

| 类 | bound | 用途 | 实现概要 | 成本 |
|---|---|---|---|---|
| `RefCell<T>` | T 无约束 | 单线程共享 mutable cell | class，单 var 字段 `value: T` | 一次 GC 分配（cell 本身） |
| `Box<T>` | T 无约束 | 不可变堆 wrapper（间接层） | class，单 val 字段 `value: T` | 一次 GC 分配（构造时） |
| `AtomicInt` | — | 原子 Int | class + atomic-int field intrinsic | 1 cmpxchg |
| `AtomicBool` | — | 原子 Bool | class + atomic-int field intrinsic（i8 / i32 视 ABI 而定） | 1 cmpxchg |
| `Atomic<T: AnyRef>` | `AnyRef` | 原子 ref（≈ Java `AtomicReference<T>`） | class + atomic-ref field intrinsic + GC barrier | 1 cmpxchg + barrier |
| `AtomicValue<T: AnyValue>` | `AnyValue` | 原子任意 value type | 内部 `Atomic<Box<T>>`，每次 store 构造新 Box | 1 GC alloc + 1 cmpxchg + barrier |

### 5.2 接口形态（最终 API 以实现为准）

```scoop
package scoop.core

// 单线程
class RefCell<T>(initial: T) {
    var value: T = initial
}

class Box<T>(val value: T)

// 原子（class 内字段 + atomic 操作 method）
class AtomicInt(initial: Int) {
    fun load(): Int
    fun store(v: Int): Unit
    fun cas(expected: Int, desired: Int): Bool
    fun exchange(v: Int): Int
    // fetchAdd / fetchSub 等按需
}

class AtomicBool(initial: Bool) { /* 类似 */ }

class Atomic<T: AnyRef>(initial: T) {
    fun load(): T
    fun store(v: T): Unit
    fun cas(expected: T, desired: T): Bool   // pointer-identity CAS
    fun exchange(v: T): T
}

class AtomicValue<T: AnyValue>(initial: T) {
    fun load(): T
    fun store(v: T): Unit
    fun snapshot(): Box<T>                            // 拿到当前 box 用于乐观 CAS
    fun cas(expected: Box<T>, desired: T): Bool       // 通过 box pointer-identity CAS
    fun exchange(v: T): T
}
```

`AtomicValue<T>` 的 `cas` 签名采用 `expected: Box<T>` 而非 `expected: T`，避免"value-equality CAS"的常见踩坑——用户必须先 `snapshot()` 拿到 box pointer 才能发起 CAS，与 Java 的 `AtomicReference.compareAndSet` 同一思路。

### 5.3 起步未列入

- `AtomicFloat32` / `AtomicFloat64`
- `AtomicInt32` / `AtomicInt64`（如有 Int 之外的细分整型需求）

按需后补。

### 5.4 关键依赖

- **`class` 允许 var 字段**：上一轮 reshape 已确认支持。
- **atomic-ref intrinsic**：作用于 class ref-typed 字段的 `__atomic_ref_load / store / cas`，配合 GC barrier 协议。摸底任务（C0-T01）需要勘测：现有 atomic-int intrinsic 形态（`sysroot/scoop.unsafe/unsafe.scoop:165-175`）能否直接外推到 ref 字段；GC barrier 协议在 `crates/scoopc/src/llvm/codegen/gc.rs` 的 write barrier 入口；只缺哪些符号 / lowering 路径。该清单是 C3-T02（Atomic 一族）的前置输入。

## 6. 顺序总览

```
C0-T01 (baseline + 摸底)
  ├─> C1-T01 (sealed interface 语法 + 登记 + 继承机制)
  │     └─> C1-T02 (AnyRef / AnyValue 定义)
  │           └─> C3-T02 (Atomic 一族)
  │                 ↑
  │                 C3-T01 (RefCell / Box)
  │
  └─> C2-T01 (删 CaptureBox 一整套)
        └─> C2-T02 (修 inner-mutable bug)
              └─> C4-T01 (fixtures 重整)
                    └─> C4-T02 (audit 基线更新)
                          └─> C5-T01 (spec / 文档更新)
```

- C1-T01 / C1-T02 / C3-T01 / C3-T02 与 C2-T01 / C2-T02 可并行——它们是两条互不交叉的实现线（前者是类型系统 + 库类型，后者是 closure 修复）。
- C4-T01（fixtures）与 C4-T02（audit 基线）必须在两条线都完成后做：fixture 既需要新 closure 语义（per-call reset 正样本），也需要 sealed interface frontend reject 反样本。
- C5-T01 收尾。

## 7. 分阶段计划

### C0. 冻结 baseline + 摸底先决

参考：

- [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §5.1
- 当前 `tests/fixtures/run-pass/`、`tests/fixtures/typecheck/`、`tests/fixtures/llvm/`、`tests/fixtures/mir_lowered/`、`tests/fixtures/runtime_gc/` 全集
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs::STALE_UNSUPPORTED_MAIN_BODY_COUNTS / INTERNAL_BUG_SENTINEL_HITS / FRONTEND_REJECT_SURFACES`
- `sysroot/scoop.core/`、`sysroot/scoop.unsafe/` 当前内容

目标：

- 把"现在能跑通"的 fixture 集合写成一份白名单；C2 / C3 / C4 各 P 阶段完成后回放。
- 列出所有 closure-dependent fixture（经 grep `closure` / `var.*=.*\{` 模式收集）；按"per-call reset 区分能力"分类（保留并补区分性 fixture / 保留无影响 / 受 CaptureBox 删除影响需 regen）。
- 摸底三件先决事实（CLOSURE_FIX.md §5.1 列）：
  1. atomic-ref intrinsic 现状——是否已有作用于 class ref-typed 字段的 `__atomic_ref_load/store/cas`？GC barrier 协议如何？只缺哪些？
  2. sysroot 是否已有同名 `Ref` / `Value` / `AnyRef` / `AnyValue` 类型——核名空冲突。
  3. `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 在删除 CaptureBox 一整套之后会发生哪些变化——预估改动面（哪些条目消失、是否需要替换为 INTERNAL_BUG_SENTINEL_HITS 行）。

### C1. 类型系统底座

#### C1-T01. 引入 `sealed interface` 语法 + 自动登记 + 继承机制

实现 §4 全部内容：

- parser 接受 `sealed interface I`，以及带 supertype 的形式 `sealed interface I : J`、`sealed interface I : J, K`（当前 parser 已能解析，需补识别 + 校验）；
- typecheck pass 按 nominal kind 自动登记 marker；继承关系参与登记的传递闭包；
- frontend reject 边界（详见 §4.1）：body 非空 / bound-only 各禁用位置 / 用户模块定义 / 继承非 sealed interface / 自循环 / 构造性循环 / 同时继承互斥 marker。

#### C1-T02. sysroot 添加 `AnyRef` / `AnyValue`

- 两条 sealed interface 定义，落入 `sysroot/scoop.core/core.scoop`（或独立文件 `sysroot/scoop.core/sealed_markers.scoop`，由实现时决定）。
- 配合 C1-T01 的自动登记规则。

### C2. closure capture 修复

#### C2-T01. 删除 CaptureBox 一整套

需要删除 / 修改的位置（详细行号见 [`TODO.md`](./TODO.md)）：

- `crates/scoopc/src/mir/mod.rs:2381-2398`：删 `Rvalue::CaptureBoxNew/Get/Set`。
- `crates/scoopc/src/mir/transport.rs:17, 189`：删 `MirTransportKind::CaptureBox` + `CaptureBoxTransportMetadata`。
- `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:720-724, 796`：删除 outer var 改写为 box 的逻辑，改为普通 alloca。
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:637-639`：删除 CaptureBox transport 分支，统一用值/ref transport。
- `crates/scoopc/src/mir/closure_simplify.rs:407-408, 452`、`escape.rs:373-374, 420`、`inline.rs:306-308 等多处`、`summary.rs:434/530/534/680-682/837/847`、`materialize/{validation, rewrite}.rs`、`dump.rs:669-671, 1119-1147`：删 CaptureBox arm。
- `crates/scoopc/src/llvm/codegen/mir_body/{terminator, mod, operand, value_args}.rs`：删 `codegen_mir_capture_box_*` 相关入口。
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:107-108, 136`：同上。
- `crates/scoopc/src/llvm/codegen/composite_transport.rs`：删相关分支与 metadata。
- `crates/scoopc/src/llvm/reachability.rs:518-520, 595-597, 748-749, 817`：删 CaptureBox 分支。
- `crates/scoopc/src/effect_facts/builder.rs`、`crates/scoopc/src/effect_lowered/{frame, segment, materialize/classification}.rs`、`crates/scoopc/src/pipeline/{mir_stage, llvm_codegen_stage}.rs`：CaptureBox 引用一并清。

注意：**保留 HIR `Capture.mutable` 字段**（`crates/scoopc/src/hir/mod.rs:573-578`）——它仍然需要传递外层 mutability 给 codegen，决定内层 binding 是否可 rebind。

#### C2-T02. 修 closure inner-mutable bug

- `crates/scoopc/src/llvm/codegen/closure/mod.rs:675`：`mutable: false` 改 `mutable: cap.mutable`。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:97-101`：guard 改 `unreachable!("upstream pipeline gate guarantees lambda lowering reaches here only with all-immutable captures via per-call reset semantics")`，加进 `INTERNAL_BUG_SENTINEL_HITS`。等价：guard 完全删除（因 per-call reset 后 mutable capture 是合法路径，不需要 guard）。**任务开工时择一，记入完成记录**。
- `crates/scoopc/src/llvm/codegen/effect_lowered/...` 内层 capture binding：等价修复（per-call alloca + env load 模式）。
- `crates/scoopc/src/llvm/codegen/mir_body/...` 路径如有内层 capture 绑定相关 codegen，同等修复。

### C3. 配套库类型

#### C3-T01. sysroot 添加 `RefCell<T>` / `Box<T>`

普通 class，依赖现有 class 机制（var 字段已支持）。落入 `sysroot/scoop.core/`。

#### C3-T02. sysroot 添加 `Atomic` 一族

- `AtomicInt` / `AtomicBool`：复用现有 atomic-int field intrinsic（位置由 C0-T01 摸底确认）。
- `Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>`：依赖 atomic-ref intrinsic（C0-T01 摸底可能产生新的 intrinsic 任务，作为子任务列入 C3-T02 开工记录）；后者内部用 `Atomic<Box<T>>`。
- `<T: AnyRef>` / `<T: AnyValue>` bound 依赖 C1-T01、C1-T02。
- 暂定落入 `sysroot/scoop.core/atomic.scoop` 新文件；如 `sysroot/scoop.core/` 当前文件粒度更适合，可放进现有 file（实现时决定）。

### C4. fixtures + audit

#### C4-T01. fixtures 重整

- regen 受 CaptureBox 删除影响的 mir_lowered fixtures（移除 `CaptureBoxNew` / `MirTransportKind::CaptureBox` 字样），主要文件：`tests/fixtures/mir_lowered/aggregate_transport.mir:138-174` 及 C0-T01 baseline 中标记"受 CaptureBox 删除影响"的全部条目。
- 新增正样本 fixtures：
  - **per-call reset**：`var x; val f = { x = x + 1; x }; val a = f(); val b = f();` 验证 `a == b`。
  - **外层不受影响**：closure 内 rebind 后 outer 仍为原值。
  - **ref type capture + 通过 ref 改对象**：closure 改对象状态，外层可见。
  - **RefCell 显式共享**：makeCounter 模式，多次调用累加。
  - **AtomicInt** / **Atomic\<MyClass\>** / **AtomicValue\<MyStruct\>**：基本 load / store / cas。
- 新增反样本 fixtures（每条对应一个 frontend reject）：
  - `val v: AnyRef = ...`
  - `fun f(x: AnyRef)`
  - `fun f(): AnyRef`
  - `val xs: List<AnyRef>`
  - `class C : AnyRef`
  - `struct S : AnyValue`
  - `when (x) { is AnyRef -> ... }`
  - `x as AnyRef`
  - 用户模块中 `sealed interface UserMarker`
  - `sealed interface NotEmpty { fun foo() }`
  - `<T: AnyRef + AnyValue>`
- `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop` 保留——它在新旧语义下都成立（`f` 只调一次），无回归；但因无区分能力需以"per-call reset"正样本补足。

#### C4-T02. 更新 audit 基线

- `pipeline_user_visible_failure_policy.rs::STALE_UNSUPPORTED_MAIN_BODY_COUNTS`（line 146-311）：CaptureBox 删除后许多 `UnsupportedMainBody` 站点消失，重算 expected counts。
- `pipeline_user_visible_failure_policy.rs::INTERNAL_BUG_SENTINEL_HITS`（line 462-481）：增补 `closure/mod.rs` 的 `unreachable!` 行号（如 C2-T02 选 `unreachable!` 方案）。
- `pipeline_user_visible_failure_policy.rs::FRONTEND_REJECT_SURFACES`（line 390-426）：登记 11+ 条新增的 sealed-interface frontend rejects（错误码、定义位置、fixture 标记）。

### C5. 文档 / spec 更新

- `SCOOP_FULL_SPEC.md`：增加 closure capture 默认语义条目（§3 全部内容的 spec 化简短版）；引用 `RefCell` / `Atomic` 作为显式共享出口。
- `SCOOP_FULL_SPEC.md`：增加 `sealed interface` 章节（§4 全部内容的 spec 化简短版）。
- 在 sealed interface 章节中给出与 Kotlin 的差异说明（§3.5 内容）。
- 在 release notes / migration guide 给出"Kotlin makeCounter 模式如何迁移到 RefCell"的迁移指引。
- `CLOSURE_FIX.md` 在文件头部加一行"实现进度跟踪移交至 PLAN.md / TODO.md"（CLOSURE_FIX.md 本身保留为设计讨论的历史记录）。

依赖说明：

- C0 早于其他全部阶段——锁定基线 + 摸底事实。
- C1-T01 早于 C1-T02——AnyRef / AnyValue 依赖 sealed interface 语法 + 自动登记。
- C1-T02 早于 C3-T02——Atomic 的 bound 依赖 marker 已落地。
- C2-T01 早于 C2-T02——先把 CaptureBox 整套删掉，再修 inner-mutable bug（顺序反过来会留半成品 IR）。
- C3-T01（RefCell / Box）独立于 closure 主线；与 C2 并行。
- C4-T01 / C4-T02 必须在 C1-T02、C2-T02、C3-T02 都落地后做。
- C5-T01 最后，所有改动稳定后一次性更新文档。

## 8. 兼容性 / 迁移影响

- 现有 fixture `mir_lowered/aggregate_transport.mir::closureCapture` 的 `read()` 行为变化：旧实现下 closure 内 `counter = counter + box.value` 影响外层 counter；新实现下只影响本次调用 frame。该 fixture 的 `return read()` 只调用一次 `read()`，返回值在两种语义下数值上一致（都是 `1 + box.value`），但 MIR dump 内容大变（无 `CaptureBoxNew`）。**预期**：MIR snapshot 必须 regen，运行结果不变。
- 现有 fixture `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop` 的 `EXPECT-EXIT=30` 在新旧语义下都成立——它只调用 `f()` 一次，未观察外层是否被影响。无回归，但语义无区分能力，需要补区分性 fixture（见 C4-T01）。
- Kotlin 习惯用户在迁移时会发现 makeCounter 模式不再 work；需要在 release notes / 文档里给出"改用 RefCell"的迁移指引。
- HIR `Capture.mutable` 字段保留——下游 codegen / effect_lowered 仍读它来决定内层 binding mutability。任何在 C2-T01 中"顺手把 mutable 字段一并删掉"的尝试都是错的，必须保留。
- `runtime/c/scoop_gc.h` / `scoop_gc.c` 的 GC trace 协议在本轮**不变**——closure env 仍是 GC managed object，env 字段仍含 ref / composite 元素需要 trace；删除的只是"box cell 作为 GC object"的特殊 trace 路径（如有）。摸底任务确认 box cell 是否有独立的 trace 协议，相关代码路径在 C2-T01 一并清理。

## 9. 风险

- **CaptureBox 删除的扇出面**：139 处出现分布在 mir / llvm / effect_lowered / pipeline / effect_facts 多个子系统。一次性删除可能触发大量编译错误且彼此交织难以定位。建议 C2-T01 开工时按子系统分组提交（mir 一组、llvm 一组、effect_lowered 一组、pipeline 一组），每组提交后跑 `cargo build` 确认 type-level 闭合，再跑 fixture baseline。
- **inner-mutable bug 与 effect-lowered 路径的对称性**：closure 在 effect-lowered 路径上的内层 binding（`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`）需要等价修复。如果只修 `closure/mod.rs:675` 而漏改 effect-lowered 等价位置，effect-using closure 中的 `var` rebind 会沉默地继续走旧 box 路径——但 box 已删除——直接 panic。C2-T02 必须明确包含 effect-lowered 路径的同步修复。
- **HIR `Capture.mutable` 误删风险**：CaptureBox 删除诱惑很大，"不再用 mutable 字段了"的直觉是错的——下游 codegen 仍读它。HIR snapshot fixture（如 `tests/fixtures/hir/closure_capture_var.hir`）会暴露字段是否仍存在；C2-T01 完成后 grep `Capture.*mutable` 必须仍有命中。
- **sealed interface 自动登记的传递闭包对 typecheck 性能的影响**：当 sysroot 加细分子 marker 后，每个 nominal 类型都要查询登记集合的传递闭包。算法选择不当（每次重新走 supertype 链）会让 typecheck 退化。本轮已写明"传递闭包在登记阶段一次性算好"，C1-T01 实现时必须严格按此模式（写入同一份 comptime metadata，bound 解析直接查询），不允许"懒算 supertype"。
- **frontend reject 错误码命名风险**：本轮新增 11+ 条 frontend reject 错误码，命名空间膨胀。建议复用 `scoop::typecheck::sealed_interface_*` 前缀，与现有 `scoop::typecheck::struct_field_must_be_val` 等同款风格保持一致。
- **fixture 大批量新增的回归审查**：C4-T01 一次性新增 5+ 正样本 + 11 反样本，回归矩阵会显著扩大。要先对 C0-T01 baseline 做明确的"受影响 fixture 清单"，避免 C4-T01 期间新 fixture 与 baseline 漂移混淆。
- **CLOSURE_FIX.md §5.1 摸底事实可能过期**：原始 §5.1 列了 3 件先决摸底事实（atomic-ref intrinsic 现状 / sysroot 名空冲突 / `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 影响面）。从 2026-05-17 设计到 2026-05-18 实施有 1 天间隔，期间核心代码未动，但 C0-T01 仍需把这些事实重新核对一次再开工。
- **`AtomicValue<T>` 内部 `Atomic<Box<T>>` 的循环依赖嫌疑**：`Box<T>` 是 class（在 C3-T01 落地）；`AtomicValue<T>` 在 C3-T02 落地。`AtomicValue.cas(expected: Box<T>, ...)` 的签名要求 `Box<T>` 已可见。C3-T02 的实现顺序：先确认 `Box<T>` 已 sysroot 加载，再写 `AtomicValue<T>`；其内部 `Atomic<Box<T>>` field 让 `Atomic<T: AnyRef>` 已落地（C3-T02 内部子任务顺序：`AtomicInt` / `AtomicBool` → `Atomic<T: AnyRef>` → `AtomicValue<T: AnyValue>`）。
- **per-call reset 与 `__scoop_gc_collect` 测试 helper 的交互**：现有 `tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop` / `gc_move_enum_maybe_ref_closure_capture_basic.scoop` 等 GC 路径 fixture 验证"closure 持有 ref 在 GC 时正确 trace"。new 语义下 closure env 的 ref 字段仍需 trace；删除 CaptureBox 后 GC trace 协议**不应**变化（box cell trace 删除即可，env 字段 trace 不动）。C4-T01 期间这组 fixture 必须保持通过；如果 fail，意味着 GC trace 协议被 C2-T01 误删了某条路径。
