# M43 设计：测试库与 `scoop test`

状态：实施中。日期：2026-10-14。实际进度见[实施记录](PROGRESS.md)。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)与[路线图](../ROADMAP.md)。本文记录 M43 的目标行为、实现边界与验收要求；测试 API、manifest 字段和命令的已完成范围以实施记录为准。实施每批功能前，先按第 8 章更新对应 spec。

## 0. 目标与范围

采用 Kotlin/JUnit 的注解和断言习惯，结合 Rust/Cargo 的构建与执行方式：用户以 `@Test class` 声明测试容器，在类内以 `@Test` 标记用例，通过官方 `scoop.test` 库断言，通过 `scoop test` 完成发现、编译、链接、过滤和执行。类上的标记只声明容器，普通辅助方法不会自动成为用例。

| 参考对象 | 借鉴内容 | Scoop 的选择 |
| --- | --- | --- |
| Java / JUnit | 断言 API、测试类、初始化与清理 | 每用例新建测试类实例，提供准备与清理方法 |
| Kotlin / kotlin.test | `@Test`、`@Ignore`、`@BeforeTest`、`@AfterTest` 与断言 | 使用普通类和方法、编译期注解与清楚的调用写法 |
| Rust / Cargo | 内建测试命令、合成测试入口 | 由工具链静态发现并构建原生测试程序 |

M43 首版交付：

- 普通 `scoop:scoop.test:0.1.0` library Cone，包含测试标记、断言、失败数据和串行运行逻辑。
- 显式测试容器和类内同步测试方法，支持同 Cone 单元测试，不提供类外独立测试入口。
- 每用例新建实例，支持字段初始化、构造、init 与可选的 BeforeTest/AfterTest；本次运行的容器和用例全部串行。
- 测试专用源码、测试依赖和独立的测试构建用途。
- `scoop test`、包含容器的名称过滤、分层列表与报告、忽略、只构建、debug/release、输出捕获和生命周期失败定位。
- 真实源码构建、依赖的普通生产 `.slib` 消费、测试程序链接与三种现有 target 的运行验收。

整类一次性的生命周期、继承或嵌套的测试发现、参数化用例、协程端到端测试、每用例进程隔离、并行调度及 JUnit XML 等后续范围见第 11 章，不成为首版的隐含验收前置。协程端到端测试等待相关调度与外部事件驱动能力具备后再设计，首版不增加挂起测试入口、runTest 或挂起断言重载。M43 不增加运行期反射、通用注解处理器或测试引擎插件体系；不改变现有异常、panic、FFI、GC 和进程退出语义。

## 1. 当前基础与缺口

| 已有能力 | M43 的使用方式 |
| --- | --- |
| 语言规范 9.4 的编译期注解、Target、普通 import 与 typed identity | 绑定真实测试注解声明，检查应用目标，静态发现容器、用例和 hooks |
| 普通 class、构造与初始化、方法、函数引用与 closure | 生成包含容器关联的测试目录，按用例构造实例并调用方法，不按运行期字符串查找函数 |
| Throwable、try/catch/finally、跨 Cone 异常 | 普通断言失败与未预期异常的捕获和报告 |
| 11.12 的 SourceLocation 和缺省参数求值来源 | 报告用户调用断言的位置，复用已有 intrinsic |
| Equals、ToString、结构比较、引用相等 | 明确断言的类型约束，不增加 Any 相等或自动接口 conformance |
| source selection、exact 依赖图、构建缓存与 artifact-only link | 增加测试输入和用途，沿现有 pipeline 产生可执行程序 |

M43 开始时 CLI 仅有 build/run/link，manifest 不支持测试源码或测试依赖，executable 入口规则只选择用户 main。这些是 M43 的实际增量；已实现部分见[实施记录](PROGRESS.md)。

`scoop.test` 的断言和基本运行逻辑只需要现有 core。M42 的 `scoop.std` 可以用于组合用例，但其全部交付、系统时间库或高层协程调度不是 M43 的前置条件。

已有 startCoroutine/Continuation 可以配合显式保存与恢复验证协程原语，但不提供调度器或外部事件驱动。多数实际 suspend 计算依赖这些驱动才能继续；增加一个等待 completion 的包装不能让它们自动端到端运行。当前编译器的协程 fixtures 继续用受控恢复验证挂起、异常、finally、Context 和 GC 等局部协议，不据此扩展 M43 的用户测试 API。

## 2. 用户代码、源码集合与依赖

### 2.1 基本使用

生产文件 `src/math.scoop`：

```scoop
package example.math

fun add(left: Int, right: Int): Int = left + right

fun requirePositive(value: Int) {
    if (value <= 0) {
        throw IllegalArgumentException()
    }
}
```

测试文件 `tests/math_test.scoop`：

```scoop
package example.math

import scoop.test.Test
import scoop.test.assertEquals
import scoop.test.assertFailsWith

@Test
class MathTest {
    private val left: Int = 1

    @Test
    fun `adds two integers`() {
        assertEquals(expected = 3, actual = addToLeft(2))
    }

    @Test
    fun `rejects negative values`() {
        assertFailsWith<IllegalArgumentException> {
            requirePositive(-1)
        }
    }

    private fun addToLeft(right: Int): Int = add(left, right)
}
```

MathTest 是容器，只有两个带 Test 的方法是用例；addToLeft 是普通 helper。每个被执行的用例都使用新实例，left 按普通字段初始化规则取得值。需要准备和清理时使用第 6.1 节的 BeforeTest/AfterTest。

根 `Cone.toml`：

```toml
schema = 1

[cone]
group = "dev.example"
name = "math"
version = "0.1.0"
kind = "library"

[test-dependencies]
"scoop:scoop.test" = "0.1.0"
```

测试库安装于 `sysroot/lib/scoop.test/`，按普通 `scoop` group library 定位和构建。它没有默认导入，也不隐式加入普通构建的依赖；使用测试库的 Cone 显式声明该依赖。测试库自身的测试复用当前 Cone 的声明，不构造自依赖。

### 2.2 测试源码选择

生产源码继续使用语言规范 12.2.2 的默认 `src/` 或显式 `sources`。测试源码另设 `test.sources`，省略时收集 `tests/` 下的 `.scoop` 文件；默认测试目录不存在表示没有单元测试。

显式配置沿用现有源码选择的 path/when 规则，例如：

```toml
[[test.sources]]
path = "tests/common"

[[test.sources]]
path = "tests/linux"
when = { os = "linux" }
```

- 显式 `test.sources` 是完整测试源码清单，空数组表示关闭当前 Cone 的单元测试源码，不隐式补入 `tests/`。
- 平台筛选、路径规范化、文件身份、重复选择和错误定位复用生产 source selection。生产与测试集合重叠也是输入错误，同一文件不能同时承担两种角色。
- 普通 build/run 不读取、解析或类型检查测试文件；测试文件变化不使生产编译缓存失效。
- 测试构建将生产和测试源码放入同一 Cone 的编译输入，保留原 ConeIdentity 和完整 Cone-relative source identity。
- 生产源码定义处的名称绑定仍只使用生产声明和生产依赖；测试源码可见生产与测试声明及两组依赖。测试输入不能补齐生产代码缺失的声明、改变生产函数原有的重载选择或使生产 API 依赖测试专用库。这是前端定义处的名称候选规则，不要求完整编译生产代码两次。

首版不提供文件内条件编译或同文件测试代码区域。Test/Ignore/BeforeTest/AfterTest 应用只允许出现在测试源码集合，普通生产输入中的应用报编译错误；不能假设只排除测试容器就能同时排除它的 import 和 helper。

### 2.3 可见性

单元测试属于被测 Cone，因此自然可访问 internal 声明。顶层 private 仍只在原 source file 内可见，成员 private/protected 继续按语言规范 9.1.5 检查；同 package 不扩大这些边界。

测试身份不产生跨文件或跨 Cone 的特殊访问权限。测试文件可以拥有自己的 private helper；测试对生产 private 实现的覆盖通过现有可见 API 进行。以后若引入同文件测试区域，应单独定义完整的测试输入排除规则，不扩展成 friend-module 或访问授权机制。

### 2.4 测试依赖

`[test-dependencies]` 使用与 `[dependencies]` 相同的 exact coordinate、version 和 path/artifact locator 语法；实际定位继续由 scoop 完成，scoopc 只消费显式解析好的依赖。

- 当前测试目标使用生产依赖与测试依赖的并集；普通构建只使用生产依赖。core 继续按语言规范 12.6 绑定同一实际库，测试依赖不能替换生产代码已选用的 core。
- 同一 group:name 已在生产依赖声明时直接复用，不得在测试依赖中再次声明或覆盖版本、locator；重复声明在 manifest 处诊断。
- 普通依赖节点始终按生产模式构建，不递归启用其测试源码和测试依赖。
- 合并后的依赖仍满足现有 exact、静态、无环规则。测试 helper 库反向依赖当前被测 Cone 形成的环照常报错，不增加构建环的特殊求解机制。
- 测试依赖进入测试构建键；普通生产 `.slib` 的依赖闭包不包含仅用于测试的边。

## 3. 测试声明与静态发现

### 3.1 标记与角色

`scoop.test` 声明以下公开、不可重复的编译期注解：

| 注解 | Target | 首版语义 |
| --- | --- | --- |
| Test | CLASS、FUNCTION | 无参数；标在普通 class 上表示容器，标在该容器的实例方法上表示用例 |
| BeforeTest | FUNCTION | 无参数；当前用例实例的准备方法，每个容器至多一个 |
| AfterTest | FUNCTION | 无参数；当前用例实例的清理方法，每个容器至多一个 |
| Ignore(val reason: String) | FUNCTION | 与同一用例方法上的 Test 配对；原因使用现有编译期 String 参数规则 |

类上的 Test 不会自动标记其方法，也不生成一条额外用例。方法名、签名和所在文件名均不用于推断测试身份。类内与类外未标记的函数仍是普通 helper，可按普通语言规则带参数、返回值、泛型或使用其他合法 callable 形态；测试方法的额外签名限制不套用到 helper。

### 3.2 测试容器

首版容器为测试源码中的顶层、具名、非泛型、可实例化的普通 class。abstract/sealed class、struct、enum、interface、object/companion、annotation class，以及局部、嵌套或 inner class 均不作为首版测试容器。Test 的 CLASS target 仍需执行这些具体形态检查。

容器必须能通过普通零实参构造调用唯一选中可安全调用的构造器，允许普通默认实参；缺失必需参数、歧义或 unsafe 构造均在前端诊断。字段初始化、构造委派和 init 沿现有语言规则执行，不注入构造参数或要求框架基类。类的普通继承和接口实现保持原语义，测试发现只处理容器直接声明的标记方法，不从基类或接口收集用例与 hooks。

容器可以使用默认 internal 或显式 public/private。构造器、用例和 hooks 使用普通声明的可见性；合成适配位于相应词法域，详见第 7.1 节，不要求测试声明公开。没有用例的容器合法，不产生执行结果或实例。

### 3.3 用例与生命周期方法

Test 用例、BeforeTest 和 AfterTest 必须是已标记容器直接声明、具有 Scoop 正文的具名实例方法。不接受类外顶层测试函数、未标记类中的测试方法、局部函数、accessor、constructor 或 extern/intrinsic 方法。

- 只有普通实例 this，不带 extension receiver 或 context 参数；无值参数、无类型参数，非 suspend，返回 Unit；有默认值的参数也不接受。
- 必须可从普通 safe 上下文调用，不自动包入 unsafe context。
- 同一方法只能承担 Test、BeforeTest、AfterTest 中的一种角色；每个容器至多声明一个 BeforeTest 和一个 AfterTest。方法名称自由，不按 setUp/tearDown 等名称发现。
- Ignore 只与用例方法上的 Test 配对，不标记容器或生命周期方法。

非法容器、目标或签名、构造调用不合法、重复标记、角色冲突、同类多个同种 hook、孤立 Ignore 均为编译错误。primary 指向相应声明或注解，notes 给出实际形态和允许规则；同类重复 hook 的诊断同时指出另一处声明。Ignore、运行过滤和容器为空均不跳过语义检查。

### 3.4 静态目录与名称

前端从当前测试目标的实际测试库依赖绑定 Test/Ignore/BeforeTest/AfterTest 声明；目标就是测试库自身时使用其当前声明，后续均按 typed annotation identity 识别。import alias 和 re-export 不改变作用，同名用户注解仍只是自己的普通注解。这些标记是明确新增的编译期语义，不是通用用户注解执行接口。

只发现当前目标测试源码中的声明，不扫描依赖产物中的测试，不因泛型实例化、导入或 re-export 复制容器和用例。目录显式保存容器与用例的关联：容器持有真实 typed class/constructor 与可选的 hook 引用，用例持有所属容器、真实 typed method、源码位置、显示名和忽略状态。容器形态、构造调用和方法签名在前端检查，不在 MIR/LIR 或运行时重做。

完整用例显示名包含 Cone、package、类名、方法名和必要的文件位置，例如 `math::example.math.MathTest::adds two integers`；Unicode 与反引号名称按原名称保留。显示名用于报告和过滤，不替代实体 id，也不通过解析显示名恢复容器关联。容器按规范化 source path、声明位置排序，各容器内用例按声明位置排序；用户测试不得依赖此执行顺序。

## 4. 断言库

### 4.1 首版 API

| API | 行为与约束 |
| --- | --- |
| assertTrue / assertFalse | 检查 Boolean 条件 |
| assertEquals / assertNotEquals | expected、actual 具有同一 T，按 T : Equals<T> 的普通接口契约比较 |
| assertSame / assertNotSame | 只接受可按普通规则执行 === 的引用类型，不接受值类型 |
| assertFailsWith<E> | E : Throwable；同步调用代码块一次，返回捕获到的预期类型异常，子类匹配沿普通类型规则 |
| fail | 显式抛出断言失败，返回 Nothing |

断言在 debug/release 下都执行，不受普通优化级别关闭。expected、actual、condition 等表达式遵守正常实参求值规则，不由断言机制重新执行；比较器或用户运算符的异常是当前测试的失败。

### 4.2 比较与消息

结构 `==` 与 Equals conformance 保持分离：struct/enum/tuple 可比较，不表示满足泛型 assertEquals 的约束。结构值可写 `assertTrue(actual == expected)`；需要通用 helper 时使用显式比较器，不为断言增加结构型 bound、隐式接口实现或 Any.equals fallback。

M41 的 Float/Double 默认相等同样用于 assertEquals，包括全部 NaN 相等、正负零相等。数组内容比较和带容差的浮点比较需要独立 API 与明确合同，不混入 assertEquals 的默认语义；assertContentEquals/assertAlmostEquals 留待有对应规范的后续批次。

失败报告至少保存断言种类、自定义消息和调用位置。expected/actual 的展示使用普通 ToString 能力或显式格式化函数；不能因为渲染需要而偷偷要求所有可比较类型实现 ToString，也不能派生不存在的字符串转换。无法展示值时仍完整报告断言失败；用户格式化再次抛出时保留原失败并报告渲染问题。

### 4.3 失败与来源位置

测试库定义普通 `AssertionFailure : Throwable`，携带消息、SourceLocation 及所需失败数据。它不直接继承 Exception，避免通常只捕获业务 Exception 的代码无意吞掉断言；显式 catch Throwable 仍遵守普通语言行为。

各断言使用 `location: SourceLocation = getCurrentSourceLocation()` 取得调用处位置；内部转发显式传递 location。来源复用现有 canonical semantic source path，不依赖调试符号，不新增断言位置 intrinsic。正常优化与跨 Cone 实例化不得把位置替换成测试库内部位置。

assertFailsWith 只围绕用户代码块捕获异常。没有抛出、或抛出类型不匹配时，在捕获区之外构造 AssertionFailure，防止断言自己产生的失败被误当成预期异常；成功时返回普通 managed E，允许调用方继续检查其字段。

不提供作用于整个测试函数的 expected-exception 标记，避免准备代码抛出的异常满足预期。panic、native crash、abort 和进程退出也不能作为 assertFailsWith 的成功结果。

## 5. CLI 与报告

首版输入为 manifest Cone 的目录或 Cone.toml；省略时沿用当前目录 Cone.toml，不向父目录查找。一次命令只构建和运行该 Cone 的单元测试，不发现其他 Cone 的测试。单文件模式的 core-only 合同保持，测试命令不隐式扩展该模式。

| 命令 / 选项 | 语义 |
| --- | --- |
| scoop test [root-input] | 构建并串行运行指定 Cone 的单元测试 |
| --filter text | 对包含容器的完整用例显示名做大小写敏感的子串匹配，可按类名或类名与方法名筛选；只改变选择结果 |
| --list | 完成测试构建所需的检查，按目标、容器、用例分层列出名称、位置与忽略原因；不启动程序、构造实例或执行 initializer/hooks |
| --ignored | 只执行匹配的 Ignore 用例；默认只执行未忽略用例 |
| --no-run | 构建并链接测试程序，显示输出位置，不执行 |
| --profile debug/release、--release | 复用现有 profile 与冲突规则，默认 debug |
| --nocapture | 实时显示用例完整生命周期的输出；默认在失败报告中展示对应 stdout/stderr |

target、sysroot、依赖定位、native 工具链和缓存选项沿用现有 build 规则。不能在当前执行环境运行的 target 可使用 --no-run/--list；实际运行时报明确工具错误，不静默改为宿主目标或自动选模拟器。

--list 与 --no-run 互斥；运行专用 --nocapture 与这两个选项互斥。过滤和 --ignored 可用于 --list，在用例级选择后按容器分组。--no-run 始终构建当前 Cone 的完整测试程序，过滤条件不改变产物内容。

CLI 采用“测试目标 → 测试容器 → 测试用例”层级；测试目标为当前 Cone，容器以 package 与类名标识。示例列表如下，普通 helper 不进入列表：

```text
math
  example.math.MathTest
    adds two integers
    rejects negative values
```

选择一个容器或进一步按方法筛选，均使用现有的子串规则，不增加另一套选择语法：

```sh
scoop test --filter 'MathTest::'
scoop test --filter 'MathTest::adds two integers'
```

按类名匹配会选中其符合当前忽略策略的用例；选中整个容器仍按每用例新实例的生命周期执行。没有匹配用例的容器不启动实例或 hooks，过滤后的列表只展示仍有被选用例的容器。

首版容器、用例全部串行，每个执行的用例有开始与结束记录。结果按同一层级分组，汇总列出当前 Cone 的容器、用例数量；通过、失败和忽略以用例为统计单位，容器只汇总子用例状态，不额外计为一次通过或失败。没有测试或过滤无匹配时显式显示 0 项和原因，不伪装成已执行；没有其他错误时返回 0。

用例报告包含完整名称、源码位置、耗时、状态和失败原因；捕获到的失败标出实例初始化、BeforeTest、测试方法或 AfterTest 阶段，准备或清理失败仍归于该用例。耗时覆盖从实例初始化开始到清理结束的完整执行流程，仅用于观察，不影响测试的语言合法性。首版执行并发度固定为 1，不增加用例并发数选项，Test 保持无参数。

完成且没有失败的运行返回 0；编译、链接、断言、未预期异常、启动、shutdown 或测试进程失败返回非零。CLI 参数错误和用户中断继续使用现有工具的错误/信号约定。构建失败时不执行旧测试 binary。

## 6. 执行模型与进程边界

### 6.1 每用例实例与生命周期

每个被执行的用例在开始时新建一个容器实例，依次执行普通构造与初始化、可选的 BeforeTest、该用例方法、可选的 AfterTest；两个 hook 和用例方法使用同一个实例。初始化包括构造默认实参、委派、字段初始化和 init，顺序沿语言规范。实例不在发现、列出或选择整个容器时提前创建，也不在同类用例之间复用。

```text
创建实例 A → BeforeTest → case1 → AfterTest
整个流程结束后：
创建实例 B → BeforeTest → case2 → AfterTest
整个流程结束后：
创建实例 C → BeforeTest → case3 → AfterTest
```

以下示例显式准备并恢复测试文件中的共享状态，构造时保存的值属于当前用例实例：

```scoop
import scoop.test.AfterTest
import scoop.test.BeforeTest
import scoop.test.Test
import scoop.test.assertEquals

private var sharedCounter: Int = 0

@Test
class CounterTest {
    private val originalCounter: Int = sharedCounter

    @BeforeTest
    fun setUp() {
        sharedCounter = 0
    }

    @AfterTest
    fun tearDown() {
        sharedCounter = originalCounter
    }

    @Test
    fun `increments from zero`() {
        sharedCounter += 1
        assertEquals(expected = 1, actual = sharedCounter)
    }
}
```

一轮 scoop test 的执行串行覆盖当前 Cone 的不同容器和同一容器内的用例。上一个用例的初始化、准备、正文和清理流程结束后，才开始下一个用例的初始化。不因每用例新建实例而并行执行，也不预建后续用例实例。

未被本次选择执行的用例，包括默认由 Ignore 跳过的用例，不创建实例或调用 hooks；以 --ignored 选中的用例正常执行完整生命周期。--list/--no-run 均不运行该流程，某容器或目标没有待执行用例时无需创建实例或启动测试进程。

### 6.2 生命周期失败与清理

普通断言失败和同步调用路径上的未预期 Throwable 由 Scoop 运行器捕获，按以下规则记录并继续下一用例：

| 失败阶段 | 当前用例后续行为 |
| --- | --- |
| 实例初始化 | 用例失败；未取得完整实例，不执行 BeforeTest、测试方法或 AfterTest |
| BeforeTest | 用例失败，跳过测试方法；仍对同一实例执行 AfterTest |
| 测试方法 | 用例失败；仍对同一实例执行 AfterTest |
| AfterTest | 用例失败；若准备或测试方法已经失败，保留两次失败及各自阶段、位置，不用清理失败覆盖原失败 |

构造成功后，无论准备或测试方法正常返回还是抛出可捕获异常，已声明的 AfterTest 均执行一次。BeforeTest 应与 AfterTest 配合处理部分准备成功的情况；构造未完成时，已获取资源的清理由构造代码按普通异常规则负责。hooks 不生成独立测试结果。finally、Context 作用域和异常对象的 GC 生命周期沿既有规则执行，不在 C frame 外接管 Scoop unwind。

新实例只隔离实例字段。singleton、global、数据库或文件等共享状态不会自动复制或重置；串行防止本次运行中的用例生命周期重叠，但不能消除遗留状态的影响。用例通过构造、BeforeTest 和 AfterTest 建立已知状态并显式释放资源；启动的后台工作、线程和 callback 也应在用例结束前按现有 API 完成或清理。用例执行结束不保证实例立即被 GC 回收，不能依赖 GC 或 release hook 及时释放共享资源。

### 6.3 测试进程与输出

当前 Cone 的测试构建生成一个原生可执行文件，在该进程中串行执行选中用例的完整生命周期。目标按正常 runtime 启动，eager initialization 在用例运行前完成；用例过滤和 Ignore 不改变已加载的生产或测试声明的顶层初始化规则。首版不提供每用例进程隔离。

panic、abort、native crash、显式 exit 或启动/shutdown 失败可能结束整个目标进程，不保证执行 AfterTest。父 scoop 记录真实退出/信号，将已开始但未结束的用例记为失败，后续尚未开始的用例列为未运行；目标启动阶段的 eager 初始化失败归于目标本身，测试实例初始化中的可捕获异常则按第 6.2 节归于对应的用例。

测试程序与父进程使用独立于用户 stdout/stderr 的最小结果通道，传递用例开始、结束及目标完成信息，用例结果保留容器关联和捕获到的失败阶段。开始记录先于实例初始化，结束记录在清理结束或初始化失败处理后产生。父进程同时检查完成记录和进程状态：缺少完成记录时，即使用户调用 exit(0) 也不能判整组通过；已经报告用例通过但 shutdown 失败时仍判目标失败。这些是普通进程结果一致性规则，不建立认证、防伪或信任凭证。

断言和用例执行使用普通 Scoop 代码。必要的事件 I/O、串行用例输出捕获与单调计时可由工具端和测试库私有的普通 FFI 适配完成，走现有 native 构建/链接；不为它们新增语言关键字、测试专用 GC 操作或公共 runtime ABI。该适配只服务本次实际运行，不扩展成通用 RPC、系统库或 reporter 插件层。

stdout/stderr 的捕获覆盖串行用例从实例初始化到清理结束的完整区间，目标启动/收尾输出单独归于目标。后台线程的输出不保证对应最初创建它的用例，不能为输出归属改变线程或 stdio 的语言合同。

## 7. 编译、产物与缓存职责

| 层次 | 职责 |
| --- | --- |
| scoop | 选择当前 Cone 的源文件、解析测试依赖、构建图与缓存、调用 scoopc、链接、启动测试进程，按容器组织过滤、列表和报告 |
| scoopc / 前端 | 接收明确的生产/测试用途和源码角色，绑定测试标记，检查容器、构造和方法形态，静态发现并合成生命周期调用与测试入口 |
| HIR / MIR / LIR | 使用完整 typed class/callable、普通构造和方法调用、函数值及既有控制流表示测试生命周期；阶段只经 IR 交换数据 |
| codegen / program-link | 沿既有对象、入口、runtime image 与 GC/EH 合同生成并链接测试 executable |
| scoop.test | 公开容器、用例、生命周期与忽略标记，以及断言、分阶段失败数据和串行用例运行；普通库实现不依赖编译器实现 crate |

### 7.1 合成入口

测试用途输出具有合成的 executable root entry，library 和 executable 源 Cone 都可测试，不要求用户修改 manifest kind 或写 main。用户已有 main 保持普通声明，测试启动不自动调用它；用户在测试里显式调用时按普通函数规则处理。

生产构建仍按原有四种 main 签名选择入口。测试入口具有独立的 generated callable 身份，并被正常 typed entry 引用，不通过重命名用户 main、固定 FQN 或 native 符号扫描选择。

编译器按已检查的容器与用例目录生成普通构造、方法调用和必要的 typed 调用适配，按第 6.1～6.2 节执行实例初始化、准备、正文及清理。适配只在用例被选择执行后构造实例，不能为建立绑定方法引用而提前创建整类或全部用例的实例；不引入按名字反射调用或通用 fixture 工厂接口。

访问测试类 private/protected 构造器、方法和 hooks 的合成适配位于该类的词法域；引用文件内 private 容器的适配保留相应文件域。源码可见域和导出集合保持，适配不能因此访问其他生产声明的 private 实现。目录明确保存目标、容器与用例的归属，CLI 不解析显示字符串重建关系。仅为当前目标运行需要的测试目录不成为普通依赖发现接口。

### 7.2 构建隔离与 `.slib`

测试用途、实际测试源码、测试依赖和测试库版本进入测试编译键；target、profile、toolchain 和普通依赖指纹继续进入现有构建键。filter、ignored、nocapture 等执行选择不改变编译产物。列表/只构建所需静态目录可以随对应测试构建结果保存并复用，不从 native 符号或依赖 Export 表重建。

测试输出放在 `target/<canonical-target-triple>/<profile>/tests/` 下，不覆盖普通 build 输出。发布继续使用原子替换，实际运行本次成功构建的内容，不能因失败或并发发布执行旧版本。

单元测试进程只链接被测 Cone 的测试版本，不同时装入同 identity 的生产版本；其依赖使用普通生产版本，复用未改变的生产依赖缓存。

测试 `.slib` 作为具有测试入口的 executable root 使用，不作为可分发的生产 library dependency。编译用途、入口与依赖的必要差异通过普通 manifest/metadata 表达；具体 wire/protocol 版本随实现的实际字段变化更新，不预先冻结空 section 或建立第二套产物格式。artifact-only program-link 继续只读传入产物，不发现源码测试或补建测试库。

## 8. Spec 先行与跨文档一致性

| 文档位置 | 实施前需要补充的合同 |
| --- | --- |
| 语言规范 9.4 | Test/Ignore/BeforeTest/AfterTest 的真实声明身份、合法目标、角色与数量限制、非重复与显式测试构建语义 |
| 语言规范的测试支持章节 | scoop.test 公共 API、容器和用例形态、每用例实例与生命周期失败语义、全部串行、AssertionFailure、比较约束、调用位置与 assertFailsWith；引用既有 9.1、11.7、11.11、11.12，不把测试库塞入最小 core |
| 语言规范 12.2～12.4 | test.sources、test-dependencies、源码集合的定义处可见候选、同 Cone 单元测试范围和测试用途的入口选择 |
| 语言规范 12.5、实现规范 2.6～2.8 | 测试 executable 产物用途、依赖投影、缓存/输出隔离、CLI 的目标/容器/用例层级、选择与结果、artifact-only link |
| 实现规范 2.1～2.5、2.11 | 编译请求中的用途与输入、容器与用例目录、前端检查及生命周期适配合成、typed entry 和阶段边界 |
| 运行时规范第 5、7 章 | 仅在需要时补充测试入口如何复用现有异常及启动/退出合同的说明；不改 panic、unwind、GC 或 shutdown 语义 |

引用章节随最终落位同步更新。现行规范与本文的计划差异在对应实现前解决；不能仅因设计写过某字段就增加无实际消费者的 wire 数据或公共 ABI。

## 9. 实施批次

| 批次 | 工作 | 完成条件 |
| --- | --- | --- |
| M43-1 | 公共 API/spec；普通 scoop.test Cone、断言与失败位置 | 手写普通 main 消费测试库即可验证断言、异常和跨 Cone 位置，无测试命令前置 |
| M43-2 | test.sources、test-dependencies、生产/测试用途及定义处名称边界 | 正常构建排除测试，测试输入/依赖图与缓存区分完整；negative 诊断明确 |
| M43-3 | 容器、用例与 hooks 的发现、形态和构造检查、typed 目录、生命周期与入口合成 | library/executable 两种根可运行类内测试；每用例新实例、准备/清理与失败保留正确，生产 main 不被自动执行 |
| M43-4 | scoop test 串行编排、按容器组织过滤/列表/报告、忽略/只构建、输出和进程结果 | 容器/用例全部串行；未选中用例无实例或 hooks 副作用，各生命周期阶段、异常结束、exit(0)、启动/shutdown 失败均有正确报告 |
| M43-5 | 生产依赖产物消费、完整 CLI 与平台组合 | 同 Cone 单元测试在依赖来自源码或无源码生产 .slib 时均完成闭环，三 target 适用范围验收完成 |

每批以对应的可用能力和真实 fixture 完成，不提前增加后续测试引擎或通用框架。阶段记录如需 PROGRESS/ACCEPTANCE，在实际实施与验证时创建，不预写成功结论。

## 10. 验收矩阵

| 范围 | 必须覆盖 |
| --- | --- |
| 容器与发现 | 同类多用例、多类/多文件、package、Unicode/反引号、import alias、同名普通注解、private 容器/方法/hooks、普通零实参与默认实参构造、空容器、测试库自身无自依赖、稳定目录、不扫描依赖或继承用例 |
| 辅助代码 | 只有标记的方法成为用例；普通 helper 可带参数、返回值和泛型，不把全部 Unit 方法自动发现为测试；类外普通 helper 仍可使用 |
| 容器错误 | abstract/sealed、非 class 目标、局部/嵌套/inner、泛型容器、构造缺少必需实参/歧义/unsafe、生产源码上的测试标记 |
| 方法与标记错误 | 顶层/局部/未标记类中的用例或 hooks、extern/intrinsic/无正文、参数含默认值、泛型、extension receiver/context、suspend、错误返回类型、unsafe、重复标记、角色冲突、同类多个同种 hook、孤立 Ignore 或用于类/hooks |
| 实例与生命周期 | 每用例新实例、字段/构造/init、BeforeTest/方法/AfterTest 共用当前实例、无 hooks、只含一个 hook、构造失败不调用 hooks、准备失败跳过正文但仍清理、正文失败仍清理、清理失败及两次失败均保留 |
| 串行与共享资源 | 同容器、跨容器的完整执行流程不重叠，后续实例不预构造；共享状态由 hooks 显式恢复、不自动重置；过滤、默认忽略、空容器无实例或 hook 副作用 |
| 源码与可见性 | internal 正例；跨文件 private、成员 private 负例；生产源码不能借测试声明/依赖通过检查；选中路径、平台条件与重复输入 |
| 测试依赖 | exact 定位、普通依赖复用、重复/冲突、循环、依赖测试不启用、正常构建无测试库仍可成功 |
| 断言与异常 | 每类断言成功/失败、expected/actual 顺序、表达式与 block 仅执行一次、预期异常返回/子类/错类型/未抛、finally 与后续用例继续 |
| 类型组合 | Equals 的显式 conformance、结构 == 配合 assertTrue、tuple/value 约束负例、引用身份、NaN/±0、ToString 缺失及格式化抛出 |
| 位置与报告 | 用户断言位置、helper 显式转发、跨 Cone 默认参数、debug/release 一致、当前 Cone 下的容器/用例分组与计数、四个生命周期阶段定位、完整生命周期的耗时与输出归属、0 用例、忽略原因 |
| 入口与进程 | library 无 main、executable 已有 main、显式调用 main、正常结束、panic/crash/exit(0)、目标 eager 初始化失败与用例实例初始化失败的归属、shutdown 失败、用户中断 |
| CLI 与构建 | 按容器及容器/方法子串过滤、filter 不隐藏编译错误、list 分层且无初始化或 hooks 副作用、ignored 选择执行完整生命周期、no-run、选项冲突、不能本机运行的 target、构建失败不运行旧程序 |
| 产物与缓存 | 生产/测试缓存与输出隔离、过滤不重编译、测试改动不使生产失效、单进程不混入两份同 Cone 定义、删除依赖库源码后消费其生产 .slib、artifact-only 链接 |
| GC 与平台组合 | 当前实例和已捕获失败在 hooks 分配/GC 后仍有效，原失败与清理失败均保留；closure、泛型、Context、native 输出的代表组合；Darwin/AArch64、Linux GNU/musl 的适用运行 |

普通功能 fixture 只断言编译诊断、测试报告和运行结果；每个编译错误规则验证实际位置与信息。只用少量 typed IR 专测锁定测试入口、真实 callable 引用和必要 metadata，不为普通用例保存完整阶段快照。

实现期间遵守先格式化/lint 再验证的顺序，运行受影响 crate、fixture runner 公共规则与正式 CLI 覆盖。共享 pipeline、依赖/入口或产物格式改变后按实际影响完成仓库要求的回归；重用未变化的验证结果，不以重复全量运行替代定向验证。

现有编译器 negative、ABI、GC、链接与产物 fixtures 继续承担原职责；M43 验收使用这些基础设施，不在首版顺带迁移整个既有测试库。

## 11. 后续扩展

- integration-test（独立 Cone 集成测试）留待 multi-cone workspace 定型后另行设计，不纳入 M43 的实现与验收范围。
- BeforeAll/AfterAll 等整类一次性的准备与清理、共享实例，以及继承或嵌套的测试发现按实际需求另行设计；不改变首版每用例实例与 BeforeTest/AfterTest 的约定。
- 参数化用例优先采用普通数据提供 API，使 struct/tuple/Array 等保持完整类型，并为每条数据独立报告；首版可在函数中写普通循环，但不将每次迭代伪报为独立发现的测试。
- assertContentEquals、assertAlmostEquals 等便利断言先定义元素比较、容差、NaN/无穷等实际行为，再纳入测试库。
- 每用例进程隔离与并行执行基于已有原生测试程序按需增加。若支持并行，并发上限由命令行控制、默认 1，并覆盖从实例初始化到清理结束的完整流程；代码标记只表达测试本身的并发约束，不写死机器相关的并发数。届时需明确容器内串行与跨容器共享资源约束的范围，类内串行不等于与其他容器互斥；首版不预加并发参数或资源协调框架。
- JUnit XML 优先服务 CI；结构化结果按实际消费者需求补充，不建立 reporter 插件平台。
- 协程端到端测试待相关调度、事件循环和外部完成驱动具备后，围绕实际 API 再设计测试方式；当前只保留受控 continuation 的原语级验证，不预先决定挂起测试注解或 runTest 等入口。虚拟时间、结构化子任务和取消按实际需求另行确定。
- 文档测试、mock、属性测试、快照与覆盖率分别按真实需求排期；能作为普通库提供的能力保持库实现。
