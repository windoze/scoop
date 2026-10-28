# 编译期文件／环境 intrinsic 与共享常量设计

状态：实施中。日期：2026-10-28。已完成范围与验证见 [PROGRESS.md](PROGRESS.md)；本文同时记录后续实现合同。

2026-10-29 范围调整：常量与常量表达式的进一步扩展留到后续专项，本轮不继续补常量 Elvis／when，也不新增通用常量合并机制。已完成的聚合值与 env 通路保留；本文中尚未实现的常量表达式设计仅作后续参考。四个输入 intrinsic 的定义处求值、静态嵌入、输入限制和更新仍属于本轮。

权威合同已同步到[语言规范](../specs/SCOOP-SPEC.md) 8.5.3、9.1.2、11.19、12.6、13.1，[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.10.1，以及[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 2.4～2.5。本文说明实现选择、改动面和验收顺序。

## 1. 目标与范围

首批提供 `includeStr`、`includeBytes`、`includeDir` 和 `env`。它们在定义 Cone 的语义检查中展开成类型完整的常量，文件内容／环境值嵌入产物，运行时直接使用静态值。所有 profile 保持同一语义；运行期不读源文件、目录或构建环境，不在程序启动时构造资源数组。

实现必须解决目录筛选及其新增文件更新、`Option<String>` 的完整常量值和静态数据发射。编译器应尽量合并可以安全共享的常量，但是否合并是内部优化策略。本期不引入任意用户函数的编译期解释执行、build scripts、宏展开语言或通用资源处理框架。未来编译期操作沿同一个 package 和既有 typed intrinsic registry 增加各自明确语义。

## 2. core Cone 与 package

全部声明属于 reserved core Cone `scoop:scoop.core:0.1.0`，源码建议放在 `sysroot/src/scoop.core/src/compiler/`，每个文件显式声明 `package scoop.compiler`。目录只组织源码，Cone 与 package 身份继续独立。

`scoop.compiler` 不加入 core 默认 prelude，不在根 package 设 alias 或转导出。调用方使用普通 exact／star import 或 import alias，例如 `import scoop.compiler.env`、`import scoop.compiler.*`；同名用户 API、遮蔽和歧义遵守现有规则。package 不能作为 exact import 的终点。

本次不实现 `scoop.compiler.env("SCOOP_BUILD_TAG")` 这样的包限定调用，也不实现 `::a.b.f`；调用与声明引用均通过普通 import／alias 定位，见语言规范 12.4.5 及[范围决定](../qualified-expressions/DESIGN.md)。测试覆盖 exact／star／alias import，以及“core 已依赖，但未 import 时不能裸用 env／includeStr”。现有 `currentSourceLocation`、`sizeOf` 等 API 的位置与直接调用语义不在本次迁移；`currentSourceLocation` 的不可引用诊断独立完成。

```scoop
package scoop.compiler

import Array
import Byte
import Option
import Option.*
import String

@Intrinsic("include_str")
public fun includeStr(path: String): String

@Intrinsic("include_bytes")
public fun includeBytes(path: String): Array<Byte>

@Intrinsic("include_dir")
public fun includeDir(
    path: String,
    include: Array<String> = ["**"],
    exclude: Array<String> = []
): EmbeddedDir

@Intrinsic("compile_env")
public fun env(name: String): Option<String>

public struct EmbeddedFile(val path: String, val bytes: Array<Byte>)

public struct EmbeddedDir(val files: Array<EmbeddedFile>) {
    public operator fun get(path: String): Array<Byte>? {
        for (file in files) {
            if (file.path == path) return Some(file.bytes)
        }
        return None
    }
}
```

四个函数无正文，均非泛型、非 suspend、无 receiver；用独立封闭 intrinsic kind 登记，`compile_env` 是内部名称。编译器只识别解析后的实际声明及 registry kind，不通过 package／短名重识别。EmbeddedFile／EmbeddedDir 为普通 struct，字段、方法和布局沿现有 core 元数据提供，无新增 intrinsic type。core 自身的非根 package 通过普通同 Cone import 引用根 package 的基础声明，不加载第二份 core 或增加自依赖。

includeDir 生成的 files 按路径排序且唯一；但普通 struct 允许用户构造与副本更新，因此 `get` 按数组顺序精确查找第一项，不假定任何用户构造的 files 都已排序。它不执行 glob、路径归一化或 IO。排序用于确定性输出与迭代，首批不为二分查找建立另一套隐藏状态或表示。

以下示例假定源码位于 `<Cone>/src/main.scoop`，LICENSE 与 assets 位于同一 Cone 根目录内。

```scoop
import scoop.compiler.env
import scoop.compiler.includeBytes
import scoop.compiler.includeDir
import scoop.compiler.includeStr

const val LICENSE: String = includeStr("../LICENSE")
const val BUILD_TAG: Option<String> = env("SCOOP_BUILD_TAG")
val version: String = env("SCOOP_APP_VERSION") ?: "dev"

val logo = includeBytes("../assets/logo.png")
val assets = includeDir(
    "../assets",
    include = ["**/*.html", "**/*.css", "**/*.png"],
    exclude = ["draft/**", "**/*.tmp"]
)
```

## 3. 求值时机与输入限制

String 实参复用已实现的常量表达式规则：字面量、const 引用、允许的内建运算及本组操作的已展开结果；本轮不为输入参数另补 Option Elvis／when 常量求值。上例普通 val 的 Elvis 仍沿已有运行期语义处理已嵌入的 Option。include／exclude 只接受元素为常量 String 的数组字面量，可为空；不接受运行期数组、spread 或普通函数返回的数组。默认的 `["**"]`／`[]` 在编译期消费，不沿普通调用生成数组分配。

普通函数参数、局部 val 和未确定的类型参数不成为编译期输入；例如 `fun load(path: String): String = includeStr(path)` 非法，即使所有调用都传字面量或函数标注 inline。`::env`、`::includeBytes` 及相应 FunPtr 取得同样非法，因为这些操作没有运行期 callable 后备。函数体里固定参数的直接调用、lambda 内的直接调用合法，运行时仅取得已确定值。

这些引用按合法语法完成名称与访问解析，再由实际 typed intrinsic kind 报告“不支持函数引用”的语义错误。诊断须包含真实声明名称、在编译期求值且无运行期入口的原因和原引用位置，不能退化为语法错误、未找到函数或笼统签名不符。短名、alias、re-export 及 managed／无期望类型／FunPtr 上下文保持一致；多候选诊断保留这一候选的具体失败原因。`currentSourceLocation` 同样不可引用，但原因是间接调用不提供直接调用处的源码求值来源，不能捕获引用创建点充数。公共实现要求见语言规范 13.1。诊断可以显示完整声明名，但帮助信息应提示先 import 后直接调用，不建议取消的 FQN 形式。

展开在 Export HIR 完成前进行，不推迟到单态化、MIR 优化或 codegen。凡已选源码中按正常规则需要语义检查的调用，都在定义处处理；运行期不可达分支、未实例化泛型和未使用默认参数不能把该操作推迟到调用方。被平台源码选择排除的文件不参与处理。

默认参数保留原有定义／求值双来源；本组操作使用定义来源，展开后模板中只剩已求出的常量值或静态引用。缺省参数被采用时，再按通常规则实例化其余表达式。`currentSourceLocation()` 仍观察求值来源；两者不共用“把位置改成调用者”的特殊机制。

同次 HIR 的常量试探、正常绑定和模板发布共用已经完成的输入读取。原 const 依赖图检查循环；不引入用户函数递归解释器或工作量预算。`includeStr(env("SCOOP_TEXT_FILE") ?: "../default.txt")` 依赖尚未实现的常量 Elvis，留到常量表达式专项；将来开放这类组合时，环境提供的路径同样须通过 Cone 范围检查，并记录两种实际输入。

## 4. 文件路径与内容

| 输入 | 定位／结果 |
| --- | --- |
| 相对路径 | 相对包含该调用的原源码文件实际所在目录，可含 `..`，但目标必须留在定义 Cone 内 |
| 绝对路径 | 目标路径及解析 symlink 后的实际目标都必须位于定义 Cone 内 |
| single-file | 相对解析入口 symlink 后的实际文件；其父目录也是 include 范围根，不使用 cwd 或相邻 manifest |
| 生成正文 | 沿生成目标声明的原文件与定义 Cone 取得基准；无实际来源则诊断 |
| includeStr | regular file 的严格 UTF-8 String，保留 BOM、换行、U+0000 |
| includeBytes | regular file 的原始字节，Array<Byte> 元素保留八位位型 |

路径是普通输入 locator，空参数、NUL、目标不存在、类型不符或实际 IO 错误按调用位置诊断；`includeDir(".")` 可选源码所在目录。没有 shell、`~`、`$NAME` 或百分号环境展开；需要环境决定路径时显式组合 env 的常量结果。相对路径基准不取 CLI cwd，也不随默认参数／内联转移。

manifest 模式的范围根是定义 Cone 的根目录，即该 Cone 的 Cone.toml 所在目录；不是 workspace 根，也不因调用来自下游泛型特化或默认参数而换成消费者目录。单文件模式使用上表的实际入口父目录，生成正文使用目标声明所属的同一个 Cone。

每次 include 必须同时满足两项条件：定位后的标准化绝对路径位于 Cone 根路径内；按原路径解析全部 symlink 后的实际目标位于解析后的实际 Cone 根内。Cone 根的输入路径及其实际路径是同一根的两种定位形式；绝对路径可使用其中任一种。目录包含关系按宿主路径语义和完整分量判断，不能用字符串前缀让同级的 `cone-other` 被误认作 `cone` 的子目录。外部 symlink 即使指回 Cone 内，也不能绕过第一项位置检查。

解析必须覆盖最终文件、父目录 symlink 和链式链接；不能仅对末段做 lstat，也不能先词法消去 `link/..` 后打开与原路径语义不同的文件。Cone 根本身是 symlink 时，以实际 Cone 目录为解析后边界。读取内容或递归进入目录前完成必要检查；越界是编译错误，断链、真实权限／IO 失败和目录递归循环沿普通输入诊断。

includeDir 的根与每个遍历条目均执行相同检查，包含子目录、文件、symlink，以及 include 未选中或 exclude 排除的条目；不能通过 glob 剪枝绕过检查。合法 symlink 可以指向 Cone 内、当前 includeDir 根之外的位置，随后继续逐项检查，输出仍沿链接所在的逻辑相对路径。两个逻辑路径指向同一物理文件时保留两个条目，内容可按第 7 节共享。

错误主位置为调用的 path 实参，includeDir 另指出越界的逻辑条目；消息包含所属 Cone 根，链接越界时包含解析后目标。例如：

```text
error: include path is outside the defining Cone directory
error: include symlink target is outside the defining Cone directory
```

这里检查的是本组资源的目录边界，不扩展到 owner、权限归属或 inode 审计，不修改普通源码／native 输入的路径规则，也不建立并发替换防护或逐阶段重复检查。

String 无隐含 NUL 结尾；空文件生成合法空值。UTF-8 解码失败包含路径与失败字节偏移。原宿主路径只用于输入记录和诊断，不进入返回值；includeDir 的相对条目名称除外。

## 5. includeDir 的选择与表示

path 必须是目录。递归发现根下条目并按第 4 节逐项检查 Cone 范围，再对普通文件的完整相对路径匹配 include／exclude；include 任意一项命中且 exclude 全不命中才入选。glob 只决定嵌入哪些文件，不改变逐项范围检查。没有目录节点或空目录条目，不保存时间、权限、文件系统句柄或原绝对路径。

| pattern | 含义／例子 |
| --- | --- |
| `*` | 当前路径分量的零个或多个字符，不跨 `/` |
| `?` | 一个非 `/` Unicode 标量 |
| `[abc]`、`[a-z]`、`[!abc]` | 字符组、按 Unicode 标量排序的范围、反向字符组 |
| `**` | 只允许作为完整路径分量；单独使用匹配所有文件 |
| `**/*.png` | 任意深度的 PNG，包含根目录的 a.png |
| `icons/**` | icons 下任意深度的文件 |
| `[*]`、`[?]` | 字面元字符 |

pattern 以 `/` 分隔、大小写敏感，不做 Unicode 归一化。空 pattern、绝对／盘符路径、反斜线、NUL、空／`.`／`..` 分量、不闭合字符组以及 `a**`／`***` 等非法递归匹配均报错。大括号按普通字符处理，多个选择用 include 数组表达；不扩展 shell brace、反斜线转义或 gitignore 语法。

默认 include `["**"]`，exclude `[]`。隐藏文件和目录正常参与筛选，不读取 `.gitignore`。重复匹配按逻辑路径去重，无“最后一个 pattern 覆盖”规则。空 include 或零匹配返回空 EmbeddedDir，根仍须存在，逐项范围检查与遍历错误检查仍执行；不额外增加 allowEmpty 开关。

条目采用根内无前导 `/`、无 `.`／`..` 的 UTF-8 相对路径，按 UTF-8 byte order 排序；名称无法表示为 String 时诊断，不 lossy 改名。越界、断链、遍历错误、循环和匹配到的非普通文件类型正常报错，不将 pipe／device 当作字节文件读取。只读取选中文件的内容；排除条目的内容不影响值，其路径和链接仍受范围规则约束。

实现选择已有 [glob::Pattern](https://docs.rs/glob/latest/glob/struct.Pattern.html) 处理字符级模式，显式使用 [MatchOptions](https://docs.rs/glob/latest/glob/struct.MatchOptions.html) 的 `case_sensitive = true`、`require_literal_separator = true`、`require_literal_leading_dot = false`。目录枚举由已有输入宿主负责，匹配器不隐藏额外 IO。需要用适配层落实上表的路径形式和错误规则；库升级不得静默改变语言语义。实现采用 0.3.4，并通过 Cargo.lock 锁定；已核对上游 2026-07-21 发布记录。

## 6. env 与聚合常量

| 名称／变量状态 | env 结果 |
| --- | --- |
| 名称不以精确的 `SCOOP_` 开头 | 编译错误，不查询环境 |
| 合法名称但变量不存在 | None |
| 合法名称，存在且为空 | Some("") |
| 合法名称，存在非空合法文本 | Some(value) |
| 合法名称，存在但非合法 Unicode | 编译错误 |

第一版名字必须以 ASCII `SCOOP_` 精确开头，区分大小写，不自动加前缀或改写名称；名字仍不能为空或含 `=`／NUL。除此之外不要求额外后缀，`SCOOP_` 本身也满足前缀规则。先对已求出的常量名称完成检查，再查询环境；`env("HOME")`、`env("scoop_TAG")` 即使未设置或位于带默认值的表达式中也报编译错误，不能当作 None。错误定位 name 实参，并明确说明限制：

```text
error: compile-time environment variable name must start with `SCOOP_`
```

通过前缀检查后，实际名称查找的大小写等规则沿宿主环境。读取的是发起本次源码编译的进程环境，交叉编译也如此。父 scoop 与 scoopc 子进程保持同一继承环境，不能因内部工具的环境覆盖而改变该语义。结果不是 runtime env API，也不注入虚构的 target／包元数据变量。首版没有额外前缀列表、任意环境读取入口或放宽限制的开关；以后调整时再修订语言合同。

`String?` 就是 Option<String>；不再提供 envOrNull 或缺失即报错的 env 重载。调用方可用普通 Elvis 或模式匹配处理已嵌入的 Option；相应常量控制表达式留到后续专项。以下常量类型与表达式设计保留为后续参考，已实现部分见实施记录，不作为本轮继续扩展的要求。

`const val` 保留原声明位置与显式类型要求，按实际值而非 enum／struct／tuple 分类判断常量资格。primitive、String、Unit，以及递归由常量组成的 struct、tuple、enum 均可常量化；空 struct 也是常量。泛型看实例化后的实际字段，不把没有存入 payload 的 phantom 参数当作字段。普通方法、只读 computed property、interface conformance 和 companion 不增加实例存储，不影响该资格。

enum 只检查选中 variant 的 payload；`None: Option<SomeClass>` 不因另一个 variant 可包含对象而被拒绝。静态引用继续要求完整合法且不可变的对象图，String 和 includeBytes／includeDir 已产生的数组、目录满足此要求，因此也可用 `const val` 保存或成为聚合常量字段。运行期对象／地址、可变存储和未知类型不能直接嵌入；Ptr／FunPtr／MaybeUninit 按 intrinsic representation 判断，不能误当作源码无字段的空 struct。

常量表达式补齐 tuple／Unit、无 payload 的 enum variant 值、真实字段／tuple 位置读取、Option Elvis 和 when 模式解包；已有 const 引用和本组 intrinsic 的结果可继续组成聚合常量。when 复用普通类型／模式／穷尽性规则，pattern binding 指向已知常量，guard 与分支结果限于常量表达式。本版不求值任何普通构造调用，包括 struct 主／次构造、带 payload 的 enum variant 及其默认实参；也不解释任意用户函数或 computed getter。即使显式参数全为常量，源码 `S(...)`／`Some(...)` 仍不在本版常量表达式集合内。env／include 直接生成完整的常量值，不经源码构造器执行；这与结果类型能表示常量是两个问题。本次不新增 `S { field: value }` 或 enum 字面量语法。

分支均正常检查表达式是否合法，纯常量运算只求值实际选择的分支。本组外部输入 intrinsic 先按第 3 节展开，因此 `env("SCOOP_A") ?: env("SCOOP_B")` 中两个被检查的调用都记录输入；存在 SCOOP_A 不意味着可以略去 SCOOP_B 的语义检查和编译期读取。所选结果仍按 Elvis 正常选择，不生成运行期环境查询。

注解参数的类型集合不随 const 扩展自动增加 struct／tuple／Option／代数 enum；已有 String 注解参数可以引用已经求出的 String const，仍遵守语言规范 9.4 的值语法。

## 7. 常量存储合并（内部优化）

本轮复用已有优化，不新增通用常量合并机制；以下为后续优化的语义边界。

值相同的 const object，其运行期 identity 不保证一定相同，也不保证一定不同。不同泛型特化中的常量实体各自独立；泛型类的不同 companion application 及其中的 const 也保持各自的实体身份。编译器可以让这些常量引用同一只读对象，但不能因此合并声明、泛型 application 或 companion 对象，也不能改变类型、值、初始化及其他规定行为。

编译器应尽量合并可以安全共享的常量，范围包括普通常量与本组 intrinsic 的结果。是否合并、在哪一阶段合并，以及 debug／release 或不同消费 Cone 的存储选择均为内部策略，不是语法、源码合法性或统一对象身份的要求。优先复用现有常量表示与静态发射通路；只有实际优化需要时才扩展对应的数据结构，不为保证全程序唯一副本另建强制常量身份体系。

合并候选按完整 exact type 与内容比较。String 比较完整字节；Array<Byte> 比较长度和字节；聚合值保留全部字段、实际 variant、元素顺序及浮点原始位型。String 与 Array<Byte> 即使字节相同也不能合并整个对象；不同 nominal 类型不能因相同布局合并。用哈希加速时仍比较完整 key。

目录中的不同路径可以共享相同 bytes，目录值也可共享 files 数组，但各自路径和输入依赖必须保留。普通运行期对象构造、clone、toMutableArray 等操作仍遵守原有身份、副作用和独立副本语义；“结果值相同”本身不足以允许消除这些行为。

## 8. IR、产物和 runtime

| 层 | 必须完成的工作 |
| --- | --- |
| core／registry | 普通 package 导出、准确签名、编译期使用限制、两个普通 struct |
| 输入宿主 | 按定义 Cone 根定位与检查资源范围、目录逐项枚举／匹配、合法名称的环境读取，返回实际值和更新依赖 |
| HIR lowering | 调用绑定与常量求值、聚合值与静态引用、源码诊断 |
| HIR／meta | 完整 typed 常量值及其已有定义／引用关系；无 IO |
| MIR | 直接消费静态值／引用，不保留待读文件或待查询环境的调用 |
| LIR／codegen | 按已有 exact layout 发射数组、聚合值、静态引用及 relocation |
| .slib／link | 保存可移植常量及物理定义关系，按实际引用选择数据定义 |
| 构建／分析宿主 | 保存并检查实际输入更新，复用未变化结果 |

常量值结构覆盖已有标量、Unit、String 引用、tuple、带 payload 的 enum、普通 struct，以及本组产生的不可变数组对象。聚合节点保留完整类型和全部字段／元素；enum 另保存实际 variant。Some／None 不靠缺失 payload、匿名整数 tag 或 OptionString 特例猜出。数组条目保存完整数组／元素类型、长度和内容，引用携带 provider 与 typed 数据身份。

const property、普通表达式、静态初值、默认值和跨 Cone 模板复用同一常量表示；语言允许哪些表达式成为 const 与 IR 可以表示哪些静态数据分开。只读数组常量不是一次普通 ArrayAlloc，也不将任意不可变引用都视为可静态构造的对象。

Option<String> 沿既有 niche：None 为零引用，Some 为非 null String 对象引用；Some("") 使用真实空 String，不能与 None 合并。它不新增 Option 盒子。String／数组使用既有对象头、TD、长度和元素布局；EmbeddedFile／EmbeddedDir 使用普通值布局。

静态对象图只引用其他静态不可移动对象，沿原 image 的 immortal 登记管理，不新增 GC 根协议或 runtime interning。需要可写副本时，普通 toMutableArray 保持独立分配与 identity。

链接从代码、静态初值、其他常量的真实引用选择数据定义，并同选其登记；不能因常量记录存在就无条件保留全部常量。合并后的数据必须保留所有实际引用，不得随其中一个未使用的函数丢失。完全未引用的数据可裁剪。

可移植模板保存定义处已经求值的输入值或静态引用；实例化不重新读取文件或环境。普通预编译机器正文的消费者保留 provider 的外部引用，泛型／inline 模板在消费方物化时可以发射所需的静态对象。静态定义沿既有 immortal 表登记和复用；跨特化、跨 Cone 的同值合并属于优化，不要求新增通用常量表。源路径和环境只存在于原构建输入，不成为消费者必须打开的依赖。必要格式变更沿现有 schema／profile 机制处理，实施时才分配新 tag／版本，重建编译器与 core，不新增兼容 reader 或 runtime ABI。

## 9. 输入更新与缓存

沿现有成功响应和普通构建记录增加以下实际观察结果，不另立独立依赖证明文件：

| 输入 | 记录及复用判定 |
| --- | --- |
| 单文件 | 定义 Cone 根及其实际目录、locator、解析后目标、长度、修改时间与逻辑用途 |
| 目录选择 | Cone 根、目录根、include／exclude、匹配路径集合、所选文件 metadata、逐项范围与遍历更新需要的目录／链接解析状态 |
| env | 实际查询的合法 SCOOP_ 名字和缺失／有效文本值，缺失与空字符串分开 |

先判断源码／配置等已知输入是否更新；有可复用结果时，再消费上次保存的外部输入依赖。目录必须能发现新文件、新子目录、删除、改名及链接解析目标变化，不能仅 stat 旧匹配文件或只检查根目录 mtime。更新观察包括 glob 排除项的新增、链接改指及其 Cone 范围；不枚举越界链接所指的外部目录。结果集合、实际目标及所选内容 metadata 未变，且逐项范围仍满足时继续命中；同次已完成的路径观察直接复用。

单文件、目录根、父目录或条目链接改指后，实际目标变化使相关输入重新求值；改指 Cone 外或出现断链时，按原调用位置报告错误，不能复用旧值。目标路径及必要目录／链接状态属于更新输入，即使旧目标与新目标的长度和修改时间相同，也不能只凭内容 metadata 命中。该规则只观察本功能实际访问的路径，不扫描 Cone 外文件或重放整套语言检查。

未匹配普通文件的内容变化、未查询环境变量变化不使当前 Cone 重新编译；新增越界链接或链接改指是目录输入变化，不能归入被排除的文件内容变化。多个相同结果被合并后，改变任一原输入仍要重新求值；新结果不同则自然产生不同静态数据。已查询 SCOOP_ 变量改值、删除、从缺失变为空或非空均参与更新，不枚举其余环境。目录筛选规则来自实际解析后的调用参数，源码参数改变由原源码更新路径处理。

命中只观察输入 metadata、必要目录条目、链接解析结果和已查询环境值，不读取资源／源码正文，不回读命中产物计算内容摘要，不进行工具发现。读取资源发生在确需编译时，并供语义求值与产物发射共用。实际目标未变时，文件内容的修改时间／长度限制沿现有缓存合同，不增加内容摘要或 inode 替换检测；路径观察也不扩展为并发替换防护。

成功响应把实际输入交给父 driver，只有整次编译成功才发布新记录；失败不能执行或发布旧结果。并发沿原普通锁协调；不增加输入复制、读取稳定性循环或恢复损坏缓存的流程。

输入更新针对源码 provider。消费预编译 .slib 时，其 env 值与资源已经固定；应用自己的环境或同名文件不使预编译库重新展开。重新构建该库源码后，其常量／接口／Code 的实际变化按既有依赖机制传播。

## 10. 工具与包管理边界

build、check、doc 使用同一 HIR 语义与外部输入读取；各自结果缓存记录相同依赖。文档生成不能绕开有错误的编译期调用，也不为展示签名重复读取已完成的输入。

LSP 在正常按需分析到声明或正文时求值，并将输入观察关联到相应分析结果。资源通知／重新扫描和已查询环境变化使相关快照失效；旧快照不发布新诊断。查询不进入 LLVM、native 链接或外部生成器，不建立仅供编辑器使用的第二套 intrinsic 求值器。环境取语言服务进程；用户在父 shell 修改环境后，需要重新启动进程才能继承新值。

支持动态文件监听的客户端为实际查询过 include 的源码 Cone 注册目录监听，覆盖根目录本身及后代；包括源码依赖和读取失败的请求。定位根与实际根不同时同时监听，覆盖 Cone 内链接改指、目标更新、缺失文件补建及新增嵌套目录。普通资源通知先复用现有输入记录判断更新；成功输入未变时不重读内容、不重建项目或重发诊断。失败输入在该 Cone 的下一次资源通知时重试。实际更新推进分析 revision、取消旧快照并重新调度打开文档；监听集合随当前分析结果更新，旧配置的注册撤销。客户端不支持动态注册时仍处理其主动发送的资源通知，并在正常语义查询时检查输入更新。

源码包沿[包管理规范](../specs/SCOOP-PACKAGE-SPEC.md) 9 的普通资源打包规则包含所需文件。本组语法中的相对路径不会被 PM 当作可重写的 manifest 路径；发布者须保持源码与资源的相对布局。PM 不执行 HIR、glob 或 env 来猜测发布文件集合，不为本功能新增 build script／资源生成步骤。资源必须在定义 Cone 内，包括 symlink 的实际目标；绝对路径不会绕过范围检查，其发布可移植性仍由原打包规则负责。该限制不改变 PM 对其他输入的路径语义。

## 11. 实施步骤

1. **输入值与命名空间基础。** 复用已经完成的 struct／tuple／Unit、enum payload 常量数据通路，按 include 实际需要补充静态资源表示。明确 core 非根 package 的普通导出与非 prelude 行为。常量 Elvis／模式解包及其他通用常量表达式改由后续专项实现，不预建通用求值插件层。
2. **单文件和 env 闭环。** 增加 core 声明／registry、定义来源与 Cone 根输入接口、路径及 symlink 范围检查、env 的 SCOOP_ 名称限制、静态字节数组发射与普通更新记录；实际完成 includeStr／includeBytes／env 的源码到 .slib、消费、链接和运行。
3. **目录闭环。** 增加 includeDir、普通 EmbeddedFile／EmbeddedDir、逐项范围检查、glob 选择、匹配集合／链接更新和目录静态数据图；接入同一常量表示与产物引用。
4. **跨 Cone 和工具验收。** 验证泛型／inline／默认参数中的静态输入值、导出 Option 常量、check／doc／LSP 输入失效、包内相对资源和完整缓存更新；验证四个入口的普通导入调用，以及导入短名／alias 后的引用拒绝。不将进一步常量表达式扩展或新增合并优化作为本轮验收门槛。对实际变更格式统一升级并显式准备开发 sysroot。

各步完成代码后先按仓库要求格式化／lint，再运行相关测试；首批新 API 在其端到端能力完成后才加入已实现能力清单。不保留“暂时生成运行期读文件”的后备路径。

## 12. 验收矩阵

普通值行为放在独立测试 Cone，使用 scoop.test；core 本身不反向依赖测试库。正式 fixture 只承担诊断位置、产物／链接、外部环境、输入变化和调用次数等外部可观察合同。

| 范围 | 必须覆盖 |
| --- | --- |
| package 与解析 | exact／star／alias import；未 import 的裸名称错误；短名／alias 函数引用均拒绝；同名普通函数不获得 intrinsic 能力 |
| String／bytes | 空文件、UTF-8／BOM／CRLF／NUL、二进制高位字节；非法 UTF-8 与非文件诊断 |
| Option 输入值 | None、env 产生的 Some 空串／非空串、普通运行期 Elvis／when、跨 Cone const 消费；不增加常量控制表达式 |
| 聚合输入值 | intrinsic 结果的完整类型、实际 variant payload、静态数组／目录数据、字段读取与跨 Cone 消费；普通构造的常量求值不开放 |
| env 名称 | SCOOP_ 前缀、精确大小写、前缀本身、非法名称在宿主查询前失败；HOME 等即使缺失或带默认值也报错；仅已查询合法名称影响更新 |
| 常量限制 | 动态参数／局部 val／运行期数组／未知泛型输入错误，const 循环与错误位置；函数引用按真实 intrinsic 报专用语义错误，覆盖短名／alias／re-export 与三种引用上下文 |
| 目录选择 | 根目录及嵌套文件、** 零层匹配、多 include／exclude、重叠去重、隐藏文件、Unicode、空结果、非法 pattern |
| 普通目录 API | files 迭代、精确查找／未找到、用户构造的无序表与副本更新、相同内容的不同路径 |
| 路径来源 | manifest、single-file、不同 cwd、生成正文、默认参数与跨 Cone 定义来源；采用定义 Cone 根而非 workspace／消费方根 |
| 路径范围 | Cone 内相对／绝对路径、合法 ..、词法越界、同名前缀兄弟目录；文件／目录／父目录／链式 symlink、外部链接指回、Cone 根 symlink、link/..、断链与实际循环 |
| 目录逐项检查 | 根与深层条目、被 glob 排除的越界链接、空 include 仍检查；Cone 内但 includeDir 根外的合法链接，输出保持逻辑相对路径 |
| 静态输入发射 | 多种 T、泛型 companion、多个消费 Cone、重复 include／相同内容、inline／default 克隆的值与类型正确；不将新合并优化作为本轮要求，不断言语言保证唯一对象 |
| 表示／GC／链接 | Option niche、不可变数组图、现有 GC 下可访问、toMutableArray 独立副本、实际引用保留与无用数据裁剪 |
| 输入更新 | 内容更新、增加／删除／改名匹配文件、新嵌套目录、筛选参数变化、env 三态变化；链接改指 Cone 内／外、父目录链接变化、相同目标 metadata 下的改指、排除项新增越界链接；真实缺失输入、失败不发布、普通并发 |
| 复用 | 所有输入保留且未变时没有源码／资源正文读取及编译调用；排除文件内容、无关 env 变化不重编 |
| 工具 | check／doc 缓存失效，LSP 文件更新与诊断清除，已预编译库的值不被消费者环境重新求值 |

普通行为使用 release；静态发射、Option ABI 与跨产物合同在编译器正确性专测中覆盖 debug／release。合并优化的专项测试检查实际 typed 引用及物理数据定义数量，不以最终文件总大小推测去重；不同 profile 可以采用不同策略，普通语义测试不要求相同内容对象的 `===` 一定为 true 或 false。产物可以同时保存必要的语义 metadata 与机器数据表示，二者不等于重复静态对象。

GC stress 仅用于静态引用图的定向 GC 正确性验证。缓存验收保持源码、工具和原产物存在，以真实调用／读取和结果验证；删除目录匹配文件只用于正常输入变化语义，不作为移除源文件后强制复用的隔离验收。原始日志留在仓库外，实施记录只保留覆盖范围、结果与未闭合事项。
