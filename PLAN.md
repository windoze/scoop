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

## 1. 仓库结构与工具链（阶段 0：工程化）

### 1.1 代码结构（Rust workspace 拆分）

- [x] `crates/scoopc/`：编译器前端 + 中端 + LLVM 后端（inkwell）（初始骨架已建立）
- [x] `crates/scoop/`：CLI（`scoop build/run/test`），负责调用 `scoopc`、链接、跑测试（已建立骨架）
- [x] `crates/scoop_runtime/`：早期运行时构建 glue（clang + C runtime）（已建立骨架）
- [x] `runtime/c/`：早期 C 运行时（GC + 基础内建 + 线程注册 + effect TLS）（已建立占位实现）
- [x] `sysroot/`：`.scoop` 形式的内建 API 声明（当前仅 `core.scoop` 最小集合；后续补齐 integers/aliases、intrinsics、unsafe/ptr、gc、io 等）
- [x] `tests/fixtures/`：所有编译期/运行期 fixtures（见 §10）（已建立最小 smoke）
- [x] `tools/`：辅助脚本（已加入 `tools/scoop_tools`：spec doctest fixtures 抽取/一致性检查 + fixtures 覆盖矩阵报告；后续扩展 golden 工具）

> 现阶段仓库还很小，可以先在单 crate 内落地；当模块多起来再迁移到 workspace。

### 1.2 基础构建与开发体验

- [x] 引入依赖：`clap`、`thiserror`、`miette`（诊断）、`tracing`（后续再引入 `inkwell`）
- [x] 统一日志：`tracing` + `tracing-subscriber`
- [ ] 提供命令行（拆分为可迭代子任务）：
  - [x] `scoop test`（fixtures harness，当前为最小 smoke）
  - [x] `scoop dump-ast`（当前为占位信息输出）
  - [ ] `scoop dump-hir` / `scoop dump-ir`（待 HIR/MIR/LLVM 落地）
  - [ ] `scoop build <main.scoop> -o <bin>`（待 codegen + 链接落地）
  - [ ] `scoop run <main.scoop>`（待 build 可用后落地）
- [x] `build.rs`：编译 `runtime/c`（强制 clang；当前通过 `crates/scoop_runtime` 实现）
- [x] CI：最小矩阵（ubuntu）跑 `cargo test --all` + `scoop test`

**本阶段 DoD**
- 能构建出 `scoop` 可执行文件（哪怕只是空壳），`scoop test` 能跑一个最小 fixture。

---

## 2. 词法/语法/AST（阶段 1：前端可解析）

### 2.1 词法分析（Lexer）

- [x] Token 集：关键字、标识符、数字、字符串、基础运算符、注解（`@`）、泛型尖括号、常用 modifier（`open/abstract/sealed`）等（见 `scoopc::syntax::lexer`）
- [x] 补齐位运算与移位运算符 token：`&` `|` `^` `~` `<<` `>>`（spec §2.3.4 / Appendix B.8）
- [x] 注释：行注释 `//`、块注释 `/* */`（当前实现为**非嵌套**；若后续需要可扩展为嵌套）
- [x] 字符串：
  - 普通字符串（`"..."`）
  - `f` 插值字符串（`f"..."`）（当前 lexer 只识别字面量边界；插值表达式留给后续 parser）
  - raw 三引号字符串（`""" ... """`）与 `f""" ... """`
  - 大括号转义（`{{` / `}}`）属于字符串内容层语义，lexer 无需特殊处理
- [x] Span（源代码位置）基础设施：`Span` + `SourceFile` 行列映射

### 2.2 语法分析（Parser）

- [x] Parser v0（最小可用）：支持 `package` / `import` / 顶层 `fun` + 基础类型声明（`class/interface/struct/enum/effect`），函数/类型体仅保证 `{ ... }` 括号平衡并记录 span
- [x] fun 签名最小解析：参数列表 + 返回类型（支持 Path/泛型参数列表/tuple/nullable 的 `TypeRef` 子集）
- [x] 工程化：拆分 `scoopc::parser` 为多文件模块（cursor/decls/types/file），避免单文件过长，便于后续语句/表达式迭代
- [ ] Kotlin-like 声明（逐步补齐）：`class/interface/struct/enum/effect/val/var/...`
  - [x] 顶层 `val`/`var`：解析声明头；initializer 暂仅保留 span（不解析表达式）
  - [ ] 类型体内部成员声明：`val`/`var`/`fun`/nested type
    - [x] `val`/`var` 成员声明头（type body）
    - [ ] `fun` 成员声明头
    - [ ] nested type 声明
- [ ] `typealias` 声明：语法解析 + AST 表示（为 sysroot 标准别名与 Kotlin 兼容铺路）
- [ ] 语句/表达式（逐步补齐）：调用、成员访问、lambda、if/when、块表达式
- [ ] 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）
- [ ] 运算符优先级（Pratt 或 precedence climbing）
- [ ] 关键歧义：struct literal vs lambda（对应 spec §12）
- [ ] 错误恢复：尽量产出更多诊断而不是第一个错误就退出（用于 IDE 与 fixtures）

### 2.3 语法树表示（AST/Parse Tree）

- [ ] 建议区分：
  - `ParseTree`（保留所有 token/节点，利于错误恢复与格式化）
  - `AST`（更语义化的节点，利于后续分析）
- [x] AST（最小骨架）：File/Package/Import/Fun/TypeDecl/TypeBody/TypeMember/Block/Ident/Param/TypeRef，节点带 span 并可回切源文本

**本阶段 DoD**
- `scoopc` 能解析大部分 spec 示例，不做类型检查也能 `dump-ast`。

---

## 3. 包与名字解析（阶段 2：可绑定符号）

### 3.1 包系统（Cone 的源级部分）

- [x] `package` 声明、`import`、通配 `*`（已支持解析 + 最小名字绑定：TypeRef 按 import/star import 解析）
- [ ] 可见性：`public/internal/private`
- [ ] 作用域：文件级、块级、类/接口/结构体内部、泛型参数作用域

### 3.2 符号表与解析

- [x] 顶层符号索引（最小子集）：基于 `package + 顶层声明名` 构建 FQN 索引并检测重复定义（见 `scoopc::resolve`）
- [ ] 两阶段/多阶段解析：
  - 先收集声明头（type/function/field signatures）
  - 再解析函数体与初始化表达式
- [x] import 解析与名字绑定（最小子集）：对 fun/val 顶层签名里的 `TypeRef::Path` 做存在性解析（含 star import）
- [ ] `typealias` 名字解析：alias 作为 type-level symbol 纳入索引；冲突与可见性诊断
- [ ] 作用域：块级/类型体/泛型参数/扩展 receiver（逐步补齐）
- [ ] 同名优先级：成员/顶层/扩展（逐步补齐）

### 3.3 sysroot 注入

- [x] sysroot 文件与 loader 骨架：可发现并解析 `sysroot/*.scoop`（当前实现见 `scoopc::sysroot`）
- [x] 编译流程注入：通过 `scoopc::session::Session` 默认加载 sysroot，并在 `build_top_level_index` 中纳入名字解析环境
- [ ] sysroot：补齐内建标量类型的“可见声明”（spec §2.3.4 / runtime §3）
  - `Int/UInt`：word-sized（随 target 指针宽度变化，Swift 约定）
  - 固定位宽整数：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 说明：这些类型是语言 builtin（布局/语义由编译器固定），但它们的可见声明由 sysroot 提供

**本阶段 DoD**
- 能在无类型检查情况下做 name resolution，并对未定义符号给出准确 span 的错误。

---

## 4. 类型系统（阶段 3：先类型检查再优化）

### 4.1 类型表示（核心）

- [ ] 区分引用类型 vs 值类型（spec §2）
- [ ] 内建整数模型（spec §2.3.4 / runtime §3）
  - `Int/UInt` 的 bit width = target pointer size
  - 固定位宽整数类型与类型大小/对齐（为 FFI/序列化提供稳定布局）
  - 整数运算语义：wrap-around、算术/逻辑右移、shift count mask（避免 target 相关 UB）
- [ ] `typealias` 语义：类型层展开（用于 `Byte/UIntPtr` 等 sysroot 标准别名；循环 alias 报错）
- [ ] `Unit`、tuple、`Option<T>`（`T?` sugar）
- [ ] 函数类型（含 effect row）：`(A, B) -> T / E` 与 receiver function type（spec §7.5）
- [ ] 类型参数、约束（上界/下界）、声明处变型（spec §3、Appendix B）

### 4.2 声明类型：class/interface/struct/enum/effect

- [ ] class：继承、虚表/方法分发（先单继承）
- [ ] interface：多实现、默认方法（可先限制默认方法 codegen）
- [ ] struct：布局（字段顺序/对齐），不可变，值语义
- [ ] enum（rich enum）：tag + union 布局（先不做 niche 优化，后续再加）
- [ ] effect：像 interface 一样声明操作签名

### 4.3 Boxing 与 Any

- [ ] 值类型装箱到 interface/`Any`（spec §2.5）
- [ ] 先实现“语义正确”，性能优化（如 O(n) 显式转换）后置

### 4.4 模式匹配与 smart cast（spec §4）

- [ ] `when` 表达式（穷尽性检查可分阶段做）
- [ ] `is` / `!is` + smart cast（至少覆盖 `val` 的流敏感类型收窄）
- [ ] `as` / `as?`（按 spec：`as` 失败走 `Raise.raise(RuntimeError.ClassCastFailed)`）

### 4.5 值类型更新（`with` 表达式）（spec §2.6）

- [ ] 语义：并行更新（RHS 都基于原值求值，无顺序依赖）
- [ ] path 解析：`a.b.c: value`（字段路径必须存在且类型匹配）
- [ ] lowering：生成“拷贝 + 覆盖字段”的构造逻辑（对嵌套 path 生成中间拷贝）

### 4.6 变量绑定与解构（spec §9 + Kotlin-like）

- [ ] `val`/`var`：
  - 不可变/可变规则
  - `var` 的赋值类型检查
- [ ] 解构绑定（destructuring）：
  - tuple/enum/struct 的 `val (a, b) = expr`
  - `when` 分支中的解构 pattern
- [ ] 控制流基础：`if/while/for/return/break/continue`（非局部 return 不支持）

### 4.7 属性系统（spec §10）

- [ ] 类属性：
  - 默认 getter/setter（生成 backing field）
  - 自定义 accessor + `field` 关键字规则
  - 如果 accessor 不引用 `field` → 不生成 backing field
- [ ] 值类型属性：
  - 仅允许 getter-only computed property
- [ ] 扩展属性：
  - 编译为静态 getter/setter（receiver 作为第一个参数）
- [ ] 委托属性（delegated properties）：
  - `by` 语法解析与 lowering（生成 `$delegate` 字段 + 转发到 `getValue/setValue`）
  - `PropertyMeta` 生成（编译期常量/元数据；与 §13 comptime/反射联动）

### 4.8 函数声明细节（spec §7）

- [ ] `inline`：仅作为优化提示（不改变语义）
- [ ] 扩展函数：
  - 解析与分发规则（静态分发、member 优先）
  - codegen：receiver 作为第一个参数的普通函数

**本阶段 DoD**
- `scoopc` 能对一批无泛型/少量泛型的示例做类型检查（含 struct/enum/Option/when/is/as）。

---

## 5. 类型推断（阶段 4：约束求解）

对齐 spec §14：constraint generation + solving（非 HM W）。

- [ ] 约束表示：`τ1 <: τ2`、相等、行约束（effects）
- [ ] LUB（if/when 分支）
- [ ] lambda 推断：参数类型下推、返回类型与 effect row 推断（见 spec §14.7.2）
- [ ] 错误报告：把“推断失败”映射到具体源 span 与最小可读解释

**本阶段 DoD**
- 能跑 `tests/fixtures/infer/**`：涵盖 if/when/lambda/泛型调用推断的 compile-pass/compile-fail。

---

## 6. 效果系统（阶段 5：先 `Raise`，再完整三种 arm）

### 6.1 静态层：effect row + 多态 + 推断

- [ ] 语法：
  - 函数/函数类型的 `/ RowExpr`
  - `eff` 作为上下文关键字：`<eff E = Pure>`、`eff E1+E2`（按当前 spec）
  - `+` 并集、`Pure` 空行
- [ ] 规则：
  - required effects（未处理效果检测，spec §14.7.1）
  - public 默认 `/ Pure` 的强制约束
  - private/internal 可推断 effect row
  - overriding：`R_over ⊆ R_base`
  - entry point 必须 `Pure`
- [ ] 语法糖：
  - `try/catch/finally` → `handle { } with { Raise.raise -> } finally { }`
  - `!!`、`as` 失败 → `Raise.raise(RuntimeError.…)`

### 6.2 动态层：handler stack dispatch（Appendix A）

- [ ] 运行时必须维护 **handler stack**（按“最近匹配 handler”分发）
- [ ] arm body 在 dispatch scope 之外执行（避免 self-capture）

### 6.3 Codegen/Lowering：分三步落地

1) **非恢复 `->`（flag-based unwinding）**
   - [ ] TLS：`__scoop_effect_active` + perform slot
   - [ ] `perform` 写 slot + set flag + return
   - [ ] 调用链传播：检查 flag，沿栈向外返回
   - [ ] handler 边界消费 slot 并清 flag，然后执行 arm；`finally` 正确执行；必要时 re-raise

2) **立即恢复 `-> resume`（栈 state machine）**
   - [ ] 把 handle body 分段（perform/call 边界）
   - [ ] lifted locals（跨段变量提升）
   - [ ] while-loop 调度 state
   - [ ] `resume(value)` 必须恰好一次（编译期/运行期双保险）

3) **逃逸 continuation `, k ->`（堆 state machine + continuation 对象）**
   - [ ] continuation 捕获 handler stack（fiber-local 语义）
   - [ ] 支持跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（见 spec §5.5）
   - [ ] one-shot：原子状态位保证并发下只能成功一次

**本阶段 DoD**
- compile-pass + run-pass 覆盖 `Raise`、`try/catch/finally`、自定义 effect + handle，以及一个最小 async/await demo。

---

## 7. 中间表示与单态化（阶段 6：为 LLVM 做准备）

### 7.1 HIR/MIR 设计

- [ ] HIR：保留大部分结构但已解析/已类型化
- [ ] MIR：显式控制流（基本块）、显式临时变量、显式 drop/cleanup（用于 `finally`/effect unwinding）

### 7.2 泛型单态化（monomorphization）

- [ ] 为每个具体实例生成专用 IR（含 `eff` 参数实例化）
- [ ] 缓存键：符号 + type args + effect row args
- [ ] 支持“预编译常见实例”（对齐 Cone 的 pre-specialize）

### 7.3 闭包与函数值

- [ ] lambda → `{ env_struct, fn_ptr }` 形式
- [ ] 捕获变量布局与 GC trace 信息生成
- [ ] effectful function type 的调用约定统一化

**本阶段 DoD**
- 纯子集（无 class 虚分发也可）能 lowering 到 MIR，并能生成可链接 `.o`（下一阶段）。

---

## 8. LLVM 后端（阶段 7：inkwell codegen）

### 8.1 LLVM Module/Pass 管线

- [ ] 目标三元组与数据布局（target machine）
- [ ] 基本优化 pass（O0/O1/O2 可选）
- [ ] 调试信息（DWARF）可后置

### 8.2 数据布局与 ABI

- [ ] 值类型（struct/tuple/enum）按 LLVM struct layout 映射
- [ ] 引用类型：对象头（type descriptor 指针 + flags + size 等）
- [ ] interface/虚表：最小可行实现（先只支持接口方法调用与装箱）

### 8.3 与 GC 的接口（推荐：shadow stack 精确根集）

为了避免早期实现 LLVM `gc.statepoint` 的复杂度，建议先实现 **shadow stack**：

- [ ] TLS：`scoop_gc_tls.current_frame`
- [ ] 每个函数 prologue 建立 `GcFrame`（包含 prev 指针 + roots 数组）
- [ ] 在需要的地方把 GC 引用写入 roots slot（局部变量活跃区）
- [ ] 分配触发 GC 时，runtime 扫描所有线程的 frame 链得到根集

> 优点：实现难度低、语义清晰、可逐步演进到移动 GC；缺点：需要编译器插桩，性能一般，但足够 bootstrap。

**本阶段 DoD**
- 生成的二进制可运行（至少支持整数运算、函数调用、打印、Option/enum 基本构造）。

---

## 9. 早期运行时（C + clang）（阶段 8：可执行与可观测）

### 9.1 最小运行时组件

- [ ] 启动入口：`main`/平台 glue，初始化 TLS（GC + effect）
- [ ] 分配器：`scoop_alloc(size, type_desc)`
- [ ] GC（先易后难）：
  - v0：非移动 mark-sweep（实现简单，pin/unpin 可先是 no-op 或计数）
  - v1：可选移动/压缩（实现 pin/unpin 语义）
- [ ] 类型描述（type descriptor）：
  - pointer bitmap 或 trace 回调
  - 用于扫描对象内的引用字段（struct/enum/closure env）
- [ ] 线程注册：新线程必须注册到 runtime 以便 GC stop-the-world 扫描其 shadow stack

### 9.2 effect runtime（C 或编译器插桩）

- [ ] TLS：handler stack 指针、perform slot、flag
- [ ] 最小原语：push/pop handler frame、读写 perform slot、原子 one-shot continuation

### 9.3 与 clang 的构建集成

- [ ] `runtime/c` 用 clang 编译成静态库/对象
- [ ] `scoopc` 链接时自动把 runtime 拉进来
- [ ] fixtures 中提供 `--emit-llvm`/`--emit-obj`/`--emit-asm` 选项方便排查

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
    typecheck/           # 类型检查：compile-pass / compile-fail
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

每个 fixture 采用“单文件 + 注释指令”的形式（类似 LLVM lit 或 Rust compiletest）：

- [x] `// EXPECT: pass|fail`
- [x] `// EXPECT-ERROR: <substring>`（当前为子串匹配；后续可升级为 regex）
- [x] `// EXPECT-AST: <file>`（parse fixtures：AST snapshot / golden）
- [x] `// RUN-STDOUT: <file>`
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
- [x] T0106b1：run-pass phase：引入可注入的进程执行器（捕获 stdout）并补齐单测
- [ ] T0106b2（BLOCKED，待 T0807）：接入 `scoop run`（T0807）真正“编译 + 运行” fixture，并断言 stdout + 增加 1 个可执行 fixture（stderr 后续补齐）
- [ ] 支持超时、退出码断言（fixtures 指令：`TIMEOUT`/`EXPECT-EXIT`）
- [ ] 对 GC 压测类测试，支持 `SCOOP_GC_STRESS=1` 之类的环境变量切换（让 CI 可控）

### 10.5 Fuzz/性质测试（可选但很有价值）

- [x] lexer/parser fuzz（避免崩溃，保证错误恢复）
- [ ] IR lowering fuzz（随机小 AST → 不崩溃）
- [ ] GC 压测（随机分配/释放/跨线程）

### 10.6 覆盖矩阵（建议维护）

为每个 spec 章节至少准备：
- 1 个 compile-pass
- 1 个 compile-fail（覆盖常见误用）
- 若涉及运行期语义（GC/effect/async），再加 1 个 run-pass

工具（仅报告，不强制 fail）：
- `cargo run -p scoop_tools -- fixtures-matrix check`

---

## 11. `@NoGC` / `@Unsafe` / `@Extern`（阶段 9：实现“系统编程通道”）

- [ ] 通用注解系统（spec §15）：
  - 解析注解声明（`annotation class`）与注解使用（`@Name(...)`）
  - 注解 target（函数/类型/字段/参数/表达式块等）与合法性检查
  - 注解仅编译期存在（不进运行时布局）
  - 内建注解：`@Intrinsic/@Extern/@Inline/@Deprecated`（具体名字按 sysroot 定义）
- [ ] `@Unsafe`：
  - 函数级与块级 `@Unsafe { ... }`
  - 非 unsafe context 禁止：指针运算/unsafe 原语/调用 `@Unsafe` 函数/调用 `@Extern`
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

fixtures：
- `tests/fixtures/cone/*`：
  - 打包后消费编译的 API 兼容性
  - IR 版本兼容（旧版本可读）

---

## 13. 编译期执行与反射（阶段 11：comptime）

- [ ] `const fun` 解释器（先支持 value types/纯计算）
- [ ] `comptime { ... }` 执行上下文（限制 effect：必须 `Pure`）
- [ ] 反射 intrinsics：`fieldsOf/nameOf/sizeOf` 等（先从 sysroot 声明开始）

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
- [ ] `typealias`（纯类型层语法糖；基础实现已因 sysroot 标准别名前置）
- [ ] Ranges/progressions 与 `for` 迭代协议
- [ ] 基础集合与常用操作（`map/filter/fold` 等更多是库工作，但需要类型推断与泛型单态化支撑）

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

fixtures：
- 运行期 GC fixtures 必须在“C GC”和“Scoop GC”两套实现下都能跑（同一套测试，不同 runtime 实现）。

---

## 16. 风险点与建议的优先级

- **高风险/高复杂度**：effect（尤其 `, k ->` + 跨线程）、GC（移动/压缩 + pin/unpin）、类型推断（subtyping + effect rows）
- **建议优先级**：
  1) 先把 fixtures 与诊断体系立住（否则后期难以迭代）
  2) 先做“语义正确”的实现（优化后置）
  3) effect 先 `Raise`/`->`，再扩展 `-> resume`、`, k ->`
  4) GC 先非移动，再移动（pin/unpin 在移动 GC 上才真正有意义）
