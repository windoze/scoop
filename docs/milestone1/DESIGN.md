# M1 设计：hello world 主线

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M1。目标：把一个 hello world 程序从源码一路编到可执行文件并运行正确，打通 parser → HIR → MIR → LIR → codegen → linker 全链路。GC 为 always-leak 实现，单 Cone，不插 statepoint。

## 1. 语言子集

M1 只支持：

```
// 文件 = 若干顶层函数声明
fun main() {
    print("hello, world")
    println("!")            // println 结尾换行
}
```

- 顶层 `fun` 声明：无参数、无返回类型标注（返回 `Unit`）；函数体是 block，内含若干语句；
- 语句：表达式语句（函数调用）；
- 表达式：String 字面量（普通双引号，无插值）、函数调用（仅限直接调用已解析的顶层函数）；
- 内建函数：`print(message: String)` / `println(message: String)`（见 5.2 的临时决策）。

子集之外的一切语法由 parser/HIR 报正式诊断（有位置、有信息），不留 TODO 分支（见 ROADMAP 第 1 章的策略约定）。

## 2. 各 stage 设计

### 2.1 AST（`scoop-ast`）

- 节点用 enum + struct 表达，所有节点携带 **span**（文件内字节区间）；
- M1 节点集：`SourceFile`、`FunctionDecl`、`Block`、`CallExpr`、`StringLiteral`；
- `SourceFileId` / `NodeId` 为 newtype 包裹的 u32 arena 索引（`la-arena`），不写占位字段——M1 不需要的节点就不进 enum。

### 2.2 parser（`scoop-parser`）

- **手写 lexer + 手写递归下降 parser**，不引入 parser 生成器/组合子库。理由：Kotlin 风格语法存在上下文相关结构（模式、`..`、`v.{...}` 等），后续对错误恢复与诊断质量要求高，手写是主流实现（rustc、Kotlin 编译器）的共同选择；lexing 用 `logos` 之类反而会在字符串/插值处受限。
- M1 不做错误恢复：第一个错误即报即停（带 span）；错误恢复在后续里程碑单独设计。
- 输出：AST + 结构化诊断列表。

### 2.3 HIR（`scoop-hir` 数据 / `scoop-hir-lower` stage）

- **实体 id 体系初版**（AGENTS.md"全局唯一、类型化的实体 id"的落地起点）：
  - `hir::FunctionId`、`hir::TypeId` 等为 newtype arena 索引；M1 只有 `FunctionId`/`TypeId` 两种；
  - 全局唯一性：id 由本 Cone 的全局 arena 分配（M17 多 Cone 时再引入 Cone 前缀，届时类型不变、构造逻辑收敛在一处）。
- M1 的类型集：`Unit`、`String`。表达式的类型信息**结构上不可缺失**：`hir::Expr` 内嵌 `ty: TypeId`，不是 `Option`。
- 名称解析：顶层函数符号表 + 内建 `print`/`println`；未知名、调用非函数、参数类型不匹配均为诊断。
- 输出：HIR 函数列表（调用目标已解析为 `FunctionId`）+ 诊断。

### 2.4 MIR（`scoop-mir` 数据 / `scoop-mir-lower` stage）

M1 无泛型、无 suspend、无分派，MIR 接近直通，但按最终形态搭好骨架：

- MIR 函数/类型列表，调用标注 call kind（M1 全部是 direct）；
- name mangling 初版规则：`scoop.<fn-name>`（无参数编码）；M3 泛型落地时扩展为带类型实参的编码，规则集中在一个模块；
- 输出 MIR 列表 + 本 Cone 的 MIR meta（M1 为空表结构，但类型上存在）。

### 2.5 LIR（`scoop-lir` 数据 / `scoop-lir-lower` stage）

- LIR 是自有最小 IR（不是 LLVM 的薄封装）：`Function` / `BasicBlock` / `Instruction`（`Call`、`Ret`、取全局常量地址）、`GlobalConst`；
- 布局：M1 只有 `String` 一种运行时布局（见 3.2）；布局表在 LIR meta 中（M1 仅 String 一项）；
- M1 不插 statepoint（M9 打开），LIR 结构上为此预留的是"指令序列中的 call 标注"，而不是空字段。

### 2.6 codegen（`scoop-codegen`）

- 用 inkwell 把 LIR 机械翻译为 LLVM IR 并发射 `.o`；
- String 字面量 → 全局常量（带对象头，见 3.2）；
- 发射 `TypeDescriptor` 初版：仅 String 一个实例，字段对齐 runtime spec 2.2 的最终形态（GC 位图等字段在 M1 填零，但结构上存在）；
- 入口 shim：用户 `fun main()` 编译为 `scoop_main`；C runtime 提供 `main()` 调用它（见 3.3）。

### 2.7 链接

driver 用 `cc` crate 编译 runtime 的 C 源并缓存，随后调用系统链接器（`cc` 驱动）把 `.o` + runtime 链接为可执行文件。M17 之前不考虑跨 Cone 链接。

## 3. runtime（M1 最小集）

`runtime/` 下新增 C 源（C11）：

### 3.1 always-leak 分配

- `void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size)`：`malloc` + 写对象头，永不释放。签名与 runtime spec 3.1/4.1 的最终形态一致，M9 换 Immix 时调用方不变。

### 3.2 字符串布局

`struct ScoopString { ScoopObjectHeader header; uint64_t len; char data[]; }`（runtime spec 2.4）。字符串字面量由 codegen 以同样布局生成为全局常量，两边共享一个头文件里的布局定义。

### 3.3 入口与输出

- `void scoop_rt_print(const ScoopString *)` / `scoop_rt_println(...)`：写 stdout；
- `int main(void)`：调用 `scoop_main()`，返回 0。

## 4. 测试计划

- **端到端 fixture**：harness 扩展为"编译 + 链接 + 运行，快照 stdout"。`tests/fixtures/m1-hello/`：hello world（print/println、多语句、多函数互相调用）。
- **golden dump**：CLI 增加 `--emit=ast|hir|mir|lir`，harness 对每个 fixture 追加各阶段文本 dump 的快照（`insta`）。
- **negative fixture**：`tests/fixtures/m1-hello/errors/`：语法错误（缺括号、非法 token）与语义错误（未知函数、参数类型错）断言诊断文本（含 `file:line:col`）。
- 诊断格式统一为 `<file>:<line>:<col>: error: <message>`。

## 5. 临时决策（及其退役里程碑）

1. **always-leak GC**：M9 由 Immix 替换；分配入口签名不变。
2. **内建 `print` / `println`（已退役）**：M7 已迁移为 core 中的普通重载；其依赖的 `write` / 数值转字符串原语暂保留 `@Intrinsic`。M12 将 `write(String)` 改为直接传 managed ref的 Scoop ABI `@Extern`；其余转换原语按各自里程碑迁移。
3. **不插 statepoint**：M9 打开；LIR 的 call 标注结构已为此预留。
4. **单文件、单 Cone**：M17 引入 `Cone.toml` 与 `.slib`；M1 的实体 id arena 构造已把多 Cone 扩展点收敛在一处。
5. **name mangling 仅 `scoop.<fn>`**：M3 泛型落地时扩展参数编码，编码规则集中在 MIR 的一个模块内。

## 6. 明确不做

- 错误恢复（parser 遇首个错误即停）；
- 变量、控制流、算术——M2 起逐步加入；
- 任何 GC 语义（handle/pin/statepoint）——M9。
