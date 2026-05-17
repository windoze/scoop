# Closure Capture 语义修正与配套类型系统设计

本文档来源于 2026-05-17 的设计讨论。目的是修正 Scoop 当前 closure capture 实现与语言 spec 的偏离，并为相关库类型（`RefCell` / `Box` / `Atomic` 一族）提供配套的类型系统底座（`sealed interface AnyRef` / `AnyValue`）。

本文档只描述设计与修复路线，不实现任何代码改动。

---

## 1. 背景：当前实现违反 spec

### 1.1 Scoop closure capture 的应有语义

Scoop 的 `var x = ...; x = ...` 是 **rebinding** 语义，不是 in-place mutation——`x` 是一个可重新指向的 binding 槽位，赋值是把这个槽位换内容，不是修改某个堆 cell。基于这一点，closure 捕获遵循以下规则：

1. closure 捕获恒为**构造点 by-value snapshot**：
   - value type：复制值；
   - ref type：复制 ref 指针（managed pointer 的 copy）。
2. closure env 字段在构造之后**不可变**——env 不持有可被 lambda 写入的槽位。
3. lambda 体内对 captured 名字的引用，**等价于该 lambda 自己的 call-frame 本地变量**：
   - 入口处由 env 字段 load 一次进入 call-frame alloca；
   - 外层 `val x` → 内层 immutable 本地，可读不可 rebind；
   - 外层 `var x` → 内层 mutable 本地，可读可 rebind；
   - rebind 仅作用于本次调用的 call frame，**不影响外层、不在多次调用之间持久**。
4. ref type 的 capture：closure 持有 ref 副本：
   - 可以通过该 ref 读/写堆对象（对象状态对外层与 closure 共享，因为同一个堆对象）；
   - 不能 rebind 让外层 ref 指向另一个对象（rebind 只动本次调用 frame 的副本）。
5. 想要"closure 与外层共享 mutable 状态"或"closure 实例自带跨调用持久状态"——**必须显式使用 `RefCell<T>` / `Atomic<...>` 等库类型**。Scoop **不做隐式 boxing**。

### 1.2 与 Kotlin 的差异

Kotlin 对捕获的 `var` 隐式 ref-box：
- closure 多次调用之间，rebind 持久；
- 与外层共享同一份状态。

Scoop 不做隐式 boxing：
- 默认 capture per-call 重置；
- 想要 Kotlin 那种行为请显式 `val n = RefCell(0)` + `{ n.value = n.value + 1; n.value }`。

文档/诊断需要明确告知用户这条差异——尤其当用户写出形如 makeCounter（捕获 `var` 期望累加）的 Kotlin 习语时。

### 1.3 当前实现的偏离

| 位置 | 现状 | spec 应有 |
|---|---|---|
| `mir/lower/fn_lowering_basic.rs:712-731` | 外层 `var x` 被捕获时改写为 `Rvalue::CaptureBoxNew` | 外层 var 用普通 alloca，不改写 |
| `mir/lower/fn_lowering_call.rs:636-650` | mutable capture 走 `MirTransportKind::CaptureBox` | 所有 capture 走值/ref 普通 transport |
| `mir/mod.rs` | 存在 `Rvalue::CaptureBoxNew/Get/Set`、`MirTransportKind::CaptureBox` | 应当删除 |
| `mir/closure_simplify.rs:407, 452` 等 | 处理 CaptureBox arm | 应当删除 |
| `llvm/codegen/mir_body/terminator.rs:397-404` | LLVM 落 box | 应当删除 |
| `llvm/codegen/effect_lowered/value.rs:117-148` | 同上 | 应当删除 |
| `llvm/codegen/composite_transport.rs` | CaptureBox 分支 | 应当删除 |
| `llvm/reachability.rs:518-520, 595-597, 748-749, 817` | CaptureBox 分支 | 应当删除 |
| `llvm/codegen/closure/mod.rs:97-101` | guard 拒绝 mutable capture | 修复后 guard 不可达，改 `unreachable!` 进 INTERNAL_BUG_SENTINEL_HITS |
| `llvm/codegen/closure/mod.rs:675` | 内层绑定写死 `mutable: false` | 改为 `mutable: cap.mutable` |
| `llvm/codegen/effect_lowered/...` 内层 capture binding | 类似 | 等价处理 |
| `tests/fixtures/mir_lowered/aggregate_transport.scoop` 等 | MIR dump 含 `CaptureBoxNew` / `MirTransportKind::CaptureBox` | regen |
| `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop` | EXPECT-EXIT=30 在新旧语义下都成立，无区分能力 | 保留，另补区分性 fixture |
| frontend gate | 无任何针对 closure capture 的拒绝路径 | 不需要新加（默认 snapshot 是合法语义） |

### 1.4 修复后的 LLVM closure 路径几乎无需重写

per-call reset 的副产品：

- closure env 在构造之后 **uniformly immutable**——没有 GC barrier、escape、并发 publication 等 per-instance 写场景的烦恼；
- lambda 内 captured 名字的写入是普通 call-frame alloca 写，与"普通函数体里写本地 var"完全同构；
- `closure/mod.rs:625-678` 现有的 inner-side codegen 结构本来就是对的——每次 lambda 调用时为每个 capture 建一个新的 entry alloca，从 env 字段 load 一次填进去，天然 per-call reset。
- 唯一的 bug 是 line 675 写死 `mutable: false`。改成 `mutable: cap.mutable` 之后，外层 var 捕获就能在 lambda 内被 rebind（写本次调用 frame 的私有 alloca，下次调用从 env reload 重置）。
- 修复后 line 97-101 的 `(not supported yet)` guard **真的可达不到**，可以改 `unreachable!`。

MIR / effect-lowered 路径同样应用"per-call alloca + env-load"模式，删除一切 box 间接。

---

## 2. 配套类型系统：`sealed interface`

### 2.1 动机

`Atomic` 一族需要在 generic bound 上区分"T 是 ref 类型"还是"T 是 value 类型"——前者直接用 atomic-ref intrinsic，后者必须先 box。要让用户层 API 干净（`Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>`），就得引入"按 T 形态分类"的类型 marker。

为避免引入新关键字，复用 `sealed`（与未来可能出现的"sealed class hierarchy"等概念一致）+ `interface`，组成 `sealed interface` 这一独立构造。

### 2.2 语法层面

- `sealed interface I` 是一个独立构造，与普通 `interface I` 正交。
- **body 必须为空**：任何成员声明（`fun` / `val` / `var` / 嵌套类型）触发 frontend reject，错误码示例 `scoop::typecheck::sealed_interface_must_be_empty`。
- sealed interface 名只能作为 **generic type bound**：
  - 允许：`class C<T: AnyRef>`、`fun f<T: AnyRef>(): T`、`<T: AnyRef + Hashable>` 等组合 bound；
  - 禁止以下用法（frontend reject）：
    - binding type：`val v: AnyRef = ...`
    - param type：`fun f(x: AnyRef)`
    - return type：`fun f(): AnyRef`
    - type argument：`val xs: List<AnyRef>`
    - runtime type test：`when (x) { is AnyRef -> ... }`
    - runtime cast：`x as AnyRef`、`x as? AnyRef`
    - 显式实现：`class C : AnyRef`、`struct S : AnyValue`
- 起步阶段限定 sealed interface **只能在 sysroot 模块定义**——用户模块定义 sealed interface 触发 frontend reject。理由：用户定义的 sealed interface 没有 auto-implementation 规则，永远不可满足，等于无意义。未来如有需要再放开。
- 互斥 bound：`<T: AnyRef + AnyValue>` 是 uninhabited，typecheck 阶段直接报错，错误信息提示"两个 marker 互斥，不存在同时满足者"。
- **sealed interface 之间允许继承，但只能继承其它 sealed interface**：
  - 允许：`sealed interface PlainValue : AnyValue`、多重继承 `sealed interface I : J, K`（marker 无成员，不存在 diamond 问题）；
  - 禁止（frontend reject）：继承普通 `interface`（会偷渡成员，破坏 body-empty 不变量）、继承 `class` / `struct` / 其它非接口类型；
  - 禁止自循环 `sealed interface I : I` 与构造性循环（`A : B; B : A`）；
  - 禁止从互斥 marker 同时继承——即子 marker 不可同时蕴涵两个不相交的祖先 marker（例如 `sealed interface I : AnyRef, AnyValue` 必拒），typecheck 在定义点就检测，错误信息复用 `<T: AnyRef + AnyValue>` 的同款提示。

### 2.3 编译器层面

- 每个 nominal 类型 typecheck 完成后，按其 kind 由 typecheck pass 自动登记其满足的 sealed marker：
  - `class` → `AnyRef`
  - `struct` / `tuple` / `enum` / 内建标量（Int / Bool / Float / ...）→ `AnyValue`
  - 互斥不交（任何类型只属于其中之一）。
- 该登记仅写入 **comptime nominal-subtyping table** / type metadata，**不进入** instance layout / class itable / class vtable / type descriptor 等任何 runtime 数据结构。
- bound 在 monomorphization 期 erase；运行时 `AnyRef` / `AnyValue` 不可观测。
- 因为 body 为空，没有 vtable/itable slot 需要分配；所以这条新增构造对 runtime 完全免费。
- **sealed marker 间继承在登记阶段做传递闭包**：当类型 X 被登记为某 sealed marker M 时，X 自动满足 M 的全部直接与传递 supertype sealed marker。这一闭包在 typecheck 完成 nominal kind 登记的同一阶段一次性算好，写入同一份 comptime metadata；bound 解析直接查询登记集合，不需要在解析时再走 supertype 链。

### 2.4 为什么必须 body 空

一旦 sealed interface 允许带成员：
1. **runtime 成本**：每个实现者要往 itable 里塞函数指针，runtime 数据结构要变；
2. **auto-implementation 复杂度**：编译器要替每个合适类型生成方法体——这是 Rust `derive` / Swift `Hashable` 那一整套独立语言特性；
3. **bound-only 限制不再合理**：成员让接口"有用"，用户会自然想 `val x: I = ...` 做多态分发。

所以"sealed = empty-body marker"是不可破坏的不变量。未来如果引入"带成员、编译器派生"的 trait（Comparable / Hashable / Numeric / ...）将以**独立新构造**引入（命名待定，例如 `derived interface`），不复用 `sealed interface` 语义。这样：
- 现有 `sealed interface` 的语义永远不变；
- 用户看到 `sealed interface` 时不需要被告知"什么时候它就重了"；
- 两类构造在源码上一眼可分。

### 2.5 sysroot 起步定义

```scoop
// 起步只这两个，扁平结构
sealed interface AnyRef
sealed interface AnyValue
```

注意：sealed interface 之间继承机制（见 2.2）**起步就要落地实现**——本轮虽然只引入扁平的 `AnyRef` / `AnyValue` 两个 marker，但继承语法、传递闭包登记、互斥/循环/非 sealed supertype 的 frontend reject 都要在 T-A 一起完成。这样未来加细分子 marker（例如 `sealed interface PlainValue : AnyValue` 表示"value type 且不含 GC ref，可用作 stackAlloc / Ptr / @NoGC 的约束"，或者 `Struct` / `Tuple` / `Enum` / `Primitive` / `WordFit` 等）就只是 sysroot 加一行 + 把 auto-impl 规则加细，不再触碰 typecheck/codegen。

PlainValue 等具体子 marker **本轮不引入**，按需后补。

### 2.6 命名

- sealed marker：**`AnyRef` / `AnyValue`**——与 Kotlin/Scala 的 `Any` 风格一致，避开与 sysroot 内部类型 kind 名 `RefTypeKind` / `ValueTypeKind` 在用户视角的语义混淆。
- 注意核 sysroot 是否已有同名类型；如冲突，需要在落地前调整。

---

## 3. 配套库类型：mutable & atomic shared-state primitives

| 类 | bound | 用途 | 实现概要 | 成本 |
|---|---|---|---|---|
| `RefCell<T>` | T 无约束 | 单线程共享 mutable cell | class，单 var 字段 `value: T` | 一次 GC 分配（cell 本身） |
| `Box<T>` | T 无约束 | 不可变堆 wrapper（间接层） | class，单 val 字段 `value: T` | 一次 GC 分配（构造时） |
| `AtomicInt` | — | 原子 Int | class + atomic-int field intrinsic | 1 cmpxchg |
| `AtomicBool` | — | 原子 Bool | class + atomic-int field intrinsic（i8 / i32 视 ABI 而定） | 1 cmpxchg |
| `Atomic<T: AnyRef>` | `AnyRef` | 原子 ref（≈ Java `AtomicReference<T>`） | class + atomic-ref field intrinsic + GC barrier | 1 cmpxchg + barrier |
| `AtomicValue<T: AnyValue>` | `AnyValue` | 原子任意 value type | 内部 `Atomic<Box<T>>`，每次 store 构造新 Box | 1 GC alloc + 1 cmpxchg + barrier |

### 3.1 接口形态（示意，最终 API 以实现为准）

```scoop
// 单线程
class RefCell<T>(initial: T) {
    var value: T = initial
}

class Box<T>(val value: T)

// 原子
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

### 3.2 起步未列入

- `AtomicFloat32` / `AtomicFloat64`
- `AtomicInt32` / `AtomicInt64`（如有 Int 之外的细分整型需求）

按需后补。

### 3.3 关键依赖

- **`class` 允许 var 字段**：已确认支持。
- **atomic-ref intrinsics**：作用于 class ref-typed 字段的 `__atomic_ref_load / store / cas`，配合 GC barrier 协议。需要在落地前摸底现有 atomic-int intrinsic 的形态，看 ref 版需要新增多少。

---

## 4. 三条 spec 之间的关系

- **Closure capture spec** 是问题起点：默认 snapshot、per-call reset、不做隐式 boxing。
- 因为不做隐式 boxing，"用户想要共享 mutable 状态"的出口被推到**库类型**——`RefCell<T>` / `Atomic<...>`。
- 因为 `Atomic` 一族需要在 generic bound 上区分 ref 与 value，引入 `sealed interface AnyRef` / `AnyValue` 作为类型系统底座。
- 三件事互相支撑：closure capture spec 决定了"为什么需要 RefCell / Atomic"；sealed marker interfaces 决定了"Atomic 一族怎么干净地表达"；库类型本身决定了"用户拿到的 API 是什么样"。

---

## 5. 任务清单

代码不动，仅列出顺序与边界。后续按 phase 分发。

### 5.1 先决摸底（不一定是任务，只需事实清单）

1. atomic-ref intrinsics 现状：是否已有作用于 class ref-typed 字段的 `__atomic_ref_load/store/cas`？GC barrier 协议如何？只缺哪些？
2. sysroot 是否已有同名 `Ref` / `Value` / `AnyRef` / `AnyValue` 类型——核名空冲突。
3. `pipeline_user_visible_failure_policy.rs` 里 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 在删除 CaptureBox 一整套之后会发生哪些变化——预估改动面。

### 5.2 实现任务（按依赖顺序）

#### T-A. 引入 `sealed interface` 语法 + 自动登记 + 继承机制

- parser 接受 `sealed interface I`，以及带 supertype 的形式 `sealed interface I : J`、`sealed interface I : J, K`；
- typecheck pass 按 nominal kind 自动登记 marker；继承关系参与登记的传递闭包（见 2.3）；
- frontend reject 边界（见 2.2）：
  - body 非空；
  - bound-only 各禁用位置（binding/param/return type、type argument、`is`/`as`、显式实现）；
  - 用户模块定义；
  - 继承非 sealed interface（普通 `interface` / `class` / `struct` 等）；
  - 自循环 / 构造性循环；
  - 同时继承互斥 marker。

#### T-B. sysroot 添加 `AnyRef` / `AnyValue`

- 两条 sealed interface 定义；
- 配合 T-A 的自动登记规则。

#### T-C. 删除 CaptureBox 一整套

需要删除/修改的位置（见 1.3 表）：

- `mir/mod.rs`：删 `Rvalue::CaptureBoxNew/Get/Set`、`MirTransportKind::CaptureBox`、相关 transport metadata。
- `mir/lower/fn_lowering_basic.rs:712-731`：删除 outer var 改写为 box 的逻辑，改为普通 alloca。
- `mir/lower/fn_lowering_call.rs:636-650`：删除 CaptureBox transport 分支，统一用值/ref transport。
- `mir/closure_simplify.rs:407, 452` 等：删 CaptureBox arm。
- `llvm/codegen/mir_body/terminator.rs:397-404`：删 `codegen_mir_capture_box_*` 三个调用入口。
- `llvm/codegen/effect_lowered/value.rs:117-148`：同上。
- `llvm/codegen/composite_transport.rs`：删相关分支与 metadata。
- `llvm/reachability.rs:518-520, 595-597, 748-749, 817`：删 CaptureBox 分支。
- `mir/dump.rs`、`mir/escape.rs`、`mir/inline.rs`、`mir/summary.rs`、`mir/materialize/...`、`hir/lower/...` 等位置中所有引用 CaptureBox 的 arm 一并删。

注意：**保留 HIR `Capture.mutable` 字段**——它仍然需要传递外层 mutability 给 codegen，决定内层 binding 是否可 rebind。

#### T-D. 修 closure inner-mutable bug

- `llvm/codegen/closure/mod.rs:675`：`mutable: false` 改 `mutable: cap.mutable`。
- `llvm/codegen/closure/mod.rs:97-101`：guard 改 `unreachable!`，加进 `pipeline_user_visible_failure_policy.rs::INTERNAL_BUG_SENTINEL_HITS`。
- `llvm/codegen/effect_lowered/...` 内层 capture binding：等价修复（per-call alloca + env load 模式）。
- `llvm/codegen/mir_body/...` 路径如有内层 capture 绑定相关 codegen，同等修复。

#### T-E. sysroot 添加 `RefCell<T>` / `Box<T>`

普通 class，依赖现有 class 机制（var 字段已支持）。

#### T-F. sysroot 添加 `Atomic` 一族

- `AtomicInt` / `AtomicBool`：复用现有 atomic-int field intrinsic。
- `Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>`：依赖 atomic-ref intrinsics（先决摸底 1 可能产生新的 intrinsic 任务）；后者内部用 `Atomic<Box<T>>`。
- `<T: AnyRef>` / `<T: AnyValue>` bound 依赖 T-A、T-B。

#### T-G. fixtures 重整

- regen 受 CaptureBox 删除影响的 mir_lowered fixtures（移除 `CaptureBoxNew` / `MirTransportKind::CaptureBox` 字样）。
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

#### T-H. 更新 audit 基线

- `pipeline_user_visible_failure_policy.rs::STALE_UNSUPPORTED_MAIN_BODY_COUNTS`：CaptureBox 删除后许多 `UnsupportedMainBody` 站点消失，重算 expected counts。
- `pipeline_user_visible_failure_policy.rs::INTERNAL_BUG_SENTINEL_HITS`：增补 `closure/mod.rs` 的 `unreachable!` 行号。
- 文档：在 `FRONTEND_REJECT_SURFACES` 表里登记新增的 sealed-interface frontend rejects（错误码、定义位置、fixture 标记）。

### 5.3 任务依赖关系

```
T-A (sealed interface 语法 + 登记)
  └─> T-B (AnyRef / AnyValue 定义)
        └─> T-F (Atomic 一族)
              ↑
              T-E (RefCell / Box)

T-C (删 CaptureBox 一整套)
  └─> T-D (修 inner-mutable bug)
        └─> T-G (fixtures)
              └─> T-H (audit 基线)
```

T-A/B/E/F 与 T-C/D 可并行；T-G 在两条线都完成后做；T-H 收尾。

---

## 6. 兼容性 / 迁移影响

- 现有 fixture `mir_lowered/aggregate_transport.scoop:closureCapture` 的 `read()` 行为变化：旧实现下 closure 内 `counter = counter + box.value` 影响外层 counter；新实现下只影响本次调用 frame。该 fixture 的 `return read()` 只调用一次 `read()`，返回值在两种语义下数值上一致（都是 `1 + box.value`），但 MIR dump 内容大变（无 `CaptureBoxNew`）。
- 现有 fixture `run-pass/closure_env_composite_capture_basic.scoop` 的 EXPECT-EXIT=30 在新旧语义下都成立——它只调用 `f()` 一次，未观察外层是否被影响。无回归，但语义无区分能力，需要补区分性 fixture（见 T-G）。
- Kotlin 习惯用户在迁移时会发现 makeCounter 模式不再 work；需要在 release notes / 文档里给出"改用 RefCell"的迁移指引。

---

## 7. 决议汇总（已确认）

| 决议项 | 选择 |
|---|---|
| sealed marker 命名 | `AnyRef` / `AnyValue` |
| closure mode A 的私有 slot | per-call reset（不持久） |
| sealed interface 是否限制无成员 | 限制：body 必须为空 |
| sealed interface 定义位置 | 起步限定 sysroot only |
| Atomic 命名分工 | `AtomicInt` / `AtomicBool` / `Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>` |
| `AtomicValue<T>::cas` 的 expected 类型 | `Box<T>`（pointer-identity） |
| 起步是否做 `AtomicFloat*` / `AtomicInt32/64` | 否（按需后补） |
| 起步是否做 sub-marker（Struct/Tuple/Enum/Primitive/PlainValue） | 否（YAGNI；但继承机制本轮就实现，未来加细分 marker 不需动 typecheck） |
| sealed interface 之间是否允许继承 | 允许，但只能继承其它 sealed interface；本轮 T-A 完成语法 + 传递闭包登记 + 相关 frontend reject |
| 现有 `closure_env_composite_capture_basic.scoop` | 保留，另补区分性 fixture |
