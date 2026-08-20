# M8 设计：异常

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M8。目标：`try` / `catch` / `finally` / `throw`（spec 11.7、runtime spec 第 5 章，基于 LLVM landingpad），并把 M3 以来的 trap 路径（`!!`、数组越界、`as` 失败）改接真实异常。

> M25 更新：本设计确定的语言语义、MIR/LIR CFG 与 LLVM landingpad 形态继续有效；3.4、3.5、4 中依赖 `__cxa_*` / `__gxx_personality_v0` 的实现选择已由 `docs/milestone25/DESIGN.md` 的 Scoop exception record、personality 与 catch 状态协议取代。

## 0. 范围说明

- **try 只做语句形态**（Kotlin 的 try 表达式 `val x = try {...}` 缓做，见第 6 章）；
- **未捕获异常** = 进程终止（runtime 默认处理：打印类型名 + abort，见 spec 11.7）；
- **边界规则**（runtime spec 第 5 章已定）：异常不得穿越 C ABI frame（UB；M8 无检查手段，文档约定）；能否穿越 Scoop ABI FFI frame 维持"初版禁止"的暂定规则；
- **数组越界的异常类型是 spec 缺口**：spec 11.7 未列出——M8 顺带修订 spec，新增 `IndexOutOfBoundsException`（见第 7 章文档同步项）；
- 整数除零：当前是 LLVM `sdiv` 的 UB——M8 加除零检查并抛 `ArithmeticException`（spec 11.7 已列）。

## 1. 语言子集

```
class MyError(val code: Int) : Exception(Some("my error"))

fun read(): Int {
    val x: Int? = None
    return x!!                       // 抛 UnwrapException
}

fun main() {
    try {
        read()
        println("unreachable")
    } catch (e: UnwrapException) {
        println("caught unwrap")
    } catch (e: Exception) {
        println("caught other")
    } finally {
        println("finally")
    }
    val a = [1, 2]
    try {
        println(a[5])                // 抛 IndexOutOfBoundsException
    } catch (e: IndexOutOfBoundsException) {
        println("caught index")
    }
    try {
        throw MyError(42)            // 自定义异常
    } catch (e: MyError) {
        println(e.code)
    } finally {
        println("done")
    }
}
```

新增（相对 M7）：

- `throw expr`：expr 类型必须是 `Throwable` 的子类型（否则诊断）；
- `try { } catch (e: T) { } (catch (e: T2) { })* (finally { })?`：catch 按声明顺序匹配（is_instance 判定），首个匹配者执行；无 catch 时 finally 必须有；
- `Throwable` / `Exception(message: String?)` / `UnwrapException` / `ClassCastException` / `ArithmeticException` / `IndexOutOfBoundsException`：core 库 class（见 2.1）；
- 自定义异常：用户 class 继承 `Throwable` / `Exception`（M6 的类继承直接支持）；
- catch 的遮蔽检查：前一个 catch 的类型是后一个的父类型（含相等）时，后者不可达——诊断 "unreachable catch block"（Kotlin 是警告，M8 取错误，见 5.1）；
- `!!` → `UnwrapException`；数组越界 → `IndexOutOfBoundsException`；`as` 失败 → `ClassCastException`；整数除零 → `ArithmeticException`（四条 M3/M5/M6 trap 路径全部改接）。

## 2. core 库新增（`sysroot/lib/scoop.core/src/throwable.scoop`）

```
class Throwable

class Exception(val message: String?) : Throwable

class UnwrapException : Exception(Some("unwrap on None"))
class ClassCastException : Exception(Some("invalid cast"))
class ArithmeticException : Exception(Some("arithmetic error"))
class IndexOutOfBoundsException : Exception(Some("array index out of bounds"))
```

- `Throwable` 是最小异常根（spec 11.7）；M8 不要求 `printStackTrace` 等设施；
- `Exception(message)` 的位置参数构造；子类用固定消息委托（基类委托实参为常量——M6 的"委托实参不可引用构造属性"限制不影响）。

## 3. 各 stage 设计

### 3.1 AST / parser

- `throw` 表达式（优先级同 `return` 的语句形态——`throw` 在语句位解析为表达式语句的一部分，或独立语句种类，实现时统一）；
- `try { } catch (ident: Type) { } finally { }`：catch 参数必须有类型标注（M8 无推断）。

### 3.2 HIR

- `StatementKind::Try { body, catches: Vec<CatchClause>, finally: Option<Block> }`；`CatchClause { local: LocalId, ty: TypeId, body }`；
- `throw`：操作数类型必须 is_subtype 于 `Throwable`（core 必须定义，缺失是 core 配置错误）；类型为 `Nothing`；
- catch：类型必须 is_subtype 于 `Throwable`（否则诊断）；遮蔽检查（见 1）；catch local 进分支作用域；
- `try` 的语句级表达式形态沿用现有规则。

**后续规则（取代本设计3.1对形态的待定描述及上文“类型为`Nothing`”）：** 以当前`SCOOP-SPEC`第5章、8.7和`SCOOP-IMPL-SPEC` 2.11为准，现行实现子集中的`throw`是jump statement；它终止当前路径且不参与外围控制表达式的结果类型合并，但不构造`Nothing`类型的expression。源码可命名的`Nothing`及一般jump expression属于后续语言子集；本段保留为M8当时的历史设计记录，不再构成当前MIR必须引入`Nothing` type id的要求。

### 3.3 MIR

- 结构化 `StatementKind::Try` 保留（MIR 不做控制流展开）；`throw` → MIR 的 `Throw(expr)` 语句/终结节点；
- `!!` 的 trap 结构、数组越界检查、`as` 失败、除零检查**从"调 `scoop_rt_trap`"改为"构造对应异常对象并 `Throw`**（异常构造 = 普通 ClassInit）；
- 除零检查：`SDiv` 左操作数除数为零时走 Throw（`ArithmeticException`）；除零检查本身在 try 内时也要走 invoke（见 3.4）。

### 3.4 LIR

- **invoke 化**：try 体内（含嵌套）任何可能抛出的操作（函数调用、间接调用、数组越界检查、除零检查、`as` 检查、`!!`）都以 `Invoke { symbol/normal: BlockId, unwind: BlockId }` 发射（新增 `Invoke` 与 `InvokeIndirect` 指令；间接调用同理）；
- **landingpad**：unwind 目标块以 `LandingPad { record, raw }` 捕获 ABI unwind record 与原始异常指针，写入函数级 EH spill 后跳到普通 dispatch 块；dispatch 以 `BeginCatch` 取得异常对象，catch 匹配 = 现有 IsInstance 检查链（与 `when` 决策序列同构）。正常离开 handler 发射 `EndCatch`；handler 内新异常与重抛先进入 handler/exit pad，保存替代异常、执行 `EndCatch`，再跳到同函数外层 try 的 dispatch/cleanup continuation；没有同函数外层时以 `Resume` 继续传播。pad 是否带 catch-all clause 由 continuation 最终是否到达同函数 handler 决定；
- **finally**：正常路径在 try 结尾内联执行一次；异常路径在 landingpad 末尾（catch 处理完或 Rethrow 前）执行一次（cleanup 形态）；`try` 内的 `return` 在返回前先执行 finally（复制或跳转共享——选一种并在注释说明）；finally 内再抛异常的嵌套情形 M8 不做特殊处理（沿 unwind 自然传播）；
- **personality**：函数带 try 的函数标记 personality 符号（`scoop_eh_personality`），codegen 发射。

### 3.5 codegen

- `Invoke`/`InvokeIndirect` → LLVM `invoke` + 正常/异常块；`LandingPad` → catch-all `landingpad`，`CleanupPad` → cleanup `landingpad`，二者只捕获 record/raw；`BeginCatch` → `__cxa_begin_catch(raw)`，`EndCatch` → `__cxa_end_catch()`，`Resume` → LLVM `resume`；
- personality = C 实现的 `scoop_eh_personality`；
- `scoop_rt_throw` / `scoop_rt_rethrow` → `__cxa_throw`（空 type_info）/ `__cxa_rethrow`；
- 链接加 `c++abi`（macOS/Linux 的 __cxa_throw/begin_catch/end_catch 来源——driver 链接参数更新）。

## 4. runtime 新增

- `void scoop_rt_throw(const void *obj)`：分配 ABI 异常缓冲、按 TypeDescriptor 大小复制对象并登记外部 GC 根，再以负责移除该根的 destructor 调 `__cxa_throw(buffer, NULL, destructor)`（完整契约见 runtime spec 第 5 章）；
- `void scoop_rt_rethrow(void)`：`__cxa_rethrow()`；
- `__cxa_begin_catch` / `__cxa_end_catch` 由 codegen 直调；runtime 不再增加同义薄封装；
- `scoop_eh_personality`（C 实现，GCC/LLVM personality 协议）；
- 进程启动注册 terminate 处理：未捕获异常打印 `uncaught exception: <type name>` 后 abort；type name 从异常对象 TypeDescriptor 的 `name` 字段读取（该字段的 ABI 契约见 runtime spec 2.2）。

## 5. 临时决策（及退役里程碑）

1. **catch 遮蔽是错误**（Kotlin 为警告）：诊断基础设施无警告级别，取错误；有警告机制后可降级。
2. **未捕获 = 打印类型名 + abort**：按 spec 11.7，采用 TypeDescriptor 的稳定 `name` 字段，并已同步 runtime spec 2.2。
3. **finally 内再抛异常**：沿当前 unwind 自然传播（M8 不做路径分析）。
4. **try 表达式形态缓做**（见第 6 章）。

## 6. 明确不做

- try 作为表达式（`val x = try {...}`）；`use`/资源管理语法；异常穿越 FFI frame 的规则细化（维持 runtime spec 的暂定）；
- `printStackTrace` 与异常链（cause）；
- catch 参数的类型推断（必须显式标注）。

## 7. 文档同步项

- **spec 11.7**：新增 `IndexOutOfBoundsException`（数组越界）；明确四个内建异常的抛出点（`!!`、越界、`as`、除零）；
- **runtime spec 2.2**：TypeDescriptor 增加稳定的 `name` 字段，供未捕获异常诊断使用；
- **runtime spec 第 5 章**：M8 落地后把"抛出入口、unwind 机制（landing pad / personality function，具体方案由实现定）"细化为 `__cxa_throw` + `scoop_eh_personality` 的既定方案。

## 8. 测试计划

- **独立 fixture**（`tests/fixtures/m8-exceptions/`）：同函数 throw/catch、多 catch 类型匹配顺序、finally 正常路径、finally 异常路径、跨函数传播（caller catch callee 的 throw）、自定义异常类、四个内建异常的捕获（`!!`/越界/`as`/除零）；
- **组合 fixture**：try 内调用含重载/泛型/数组/装箱的代码；嵌套 try；catch 后用 when 继续处理；
- **negative fixture**：throw 非 Throwable；catch 非 Throwable；catch 遮蔽（父类在前）；缺类型的 catch；
- **未捕获 fixture**：`EXPECT-TRAP` 的语义改为"进程异常终止 + stderr 快照"（未捕获路径）；
- golden dump 锁定 invoke/landingpad 结构。
