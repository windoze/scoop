# Scoop 实现路线图

版本：0.1（草案）

配套文档：`docs/specs/SCOOP-IMPL-SPEC.md`（pipeline 与各 stage 职责）、`AGENTS.md`（编码准则）。

## 1. 策略：纵向主线 + 显式语言子集

不采用"逐 stage 完整实现"的横向推进，而是先打通最小端到端主线，再逐里程碑扩大语言子集。理由：

- **风险前置**：statepoint/stackmap、landingpad、GC pin/handle、suspend 状态机是本项目的新链路，越晚碰代价越大；
- **IR 设计需要下游反馈**：每个 stage 输出的真实需求由其消费者发现，孤立地"完整"实现某个 stage 几乎必然返工；
- **测试基建一次到位**：fixture runner、golden dump、negative fixture 断言在第一个里程碑建好，后续特性直接落入现成框架。

与 AGENTS.md"不留占位符"准则的调和：每个里程碑定义一个**显式的语言子集**，子集内的规则完整实现（结构完备、无 TODO 分支）；子集外的语法由 parser/HIR 报**正式的不支持诊断**（有位置、有信息）——这是面向用户的错误报告，不是代码中的占位分支。随着里程碑推进，这些诊断逐一消除。

## 2. 里程碑

每个里程碑都是全链路可运行的（parser → HIR → MIR → LIR → codegen → 可执行文件）。

### M0 技术 spike ✅（2026-08-21 完成）

- ~~独立一次性程序验证 inkwell 的 statepoint + stackmap + landingpad 全链路（不进主线代码）~~——`spikes/llvm-gc/`（独立 workspace），inkwell 0.10 + LLVM 22.1.8 验证通过：`rewrite-statepoints-for-gc` 正常改写、`.o` 含 `__llvm_stackmaps` 与 `gcc_except_tab`；
- ~~建立 fixture runner、golden dump 设施与 CLI 骨架~~——`compiler/driver/tests/fixtures.rs`（insta 快照），`scoopc` CLI 拆为 lib + 薄 bin（clap），冒烟 fixture `tests/fixtures/m0-smoke/hello.scoop` 端到端通过。

### M1 hello world ✅（2026-08-22 完成，设计见 `docs/milestone1/DESIGN.md`）

顶层函数、`String` 字面量、`fun main`、调用 runtime 的 print。GC 用 always-leak 实现（分配即 malloc、不回收）；单 Cone；不插 statepoint。

### M2 值类型基础

struct / tuple、字段访问、`val` / `var`、if / while、结构相等。

### M3 泛型与 Option

单态化、`enum Option<T>`、`T?` 脱糖与 `?.` / `?:` / `!!`（spec 第 7 章）。Option 是核心设施，越早越好。

### M4 enum 与模式匹配

enum 变体、when 扩展模式、守卫、穷尽性、解构声明与 `..`（spec 第 4、5 章）。

### M5 数组

`Array` / `MutableArray`、字面量与推导规则（spec 第 10 章）。

### M6 字符串插值

f-string 与 `StringBuilder` 脱糖（spec 第 6 章）。

### M7 引用类型层级

class / 继承 / interface、vtable / itable 分派、装箱（spec 3、4.4、9.1；impl spec 2.9）。

### M8 异常

try / catch / finally / throw，landingpad 落地（runtime spec 第 5 章）。

### M9 真 GC

Immix 分代 GC 替换 always-leak；打开 statepoint；handle / pin 设施（runtime spec 第 3、4 章）。M9 之前的全部代码必须在 always-leak GC 下保持正确，GC 实现因此获得一个长的并行开发窗口。

### M10 协程

suspend 状态机变换、Continuation（spec 8.2；impl spec 2.3）。

### M11 FFI 注解族

`@Extern` / `@NoGC` / `@Unsafe` / `@Safe` / `@CLayout` / `@CallingConvention` / `@Global` / `@ThreadLocal` / `@InteriorMutable`、`Ptr` / `FunPtr`（spec 第 13、14 章）。

### M12 多 Cone 与 `.slib`

`Cone.toml`、依赖图、`.slib` 打包与 reader、三层 meta（impl spec 2.6）、re-export（`public import`）。

## 3. 备注

- 里程碑内的特性验收标准：独立 fixture + 组合 fixture + 相关编译错误规则的 negative fixture + 各 stage 的 golden dump（见 AGENTS.md 编码准则）。
- 里程碑顺序可按实现中发现的依赖调整，但 M0 不推迟、M3 不晚于任何依赖 `Option` 的特性。
