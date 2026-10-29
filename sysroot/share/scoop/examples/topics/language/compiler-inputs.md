## 运行

按显示路径保存全部文件，保留文本资源末尾的换行。在 Cone.toml 所在目录执行：

```sh
SCOOP_EXAMPLE_VERSION=1.2.3 scoop build --release . -o embedded-example
SCOOP_EXAMPLE_VERSION=runtime ./embedded-example
```

第一条命令读取编译环境和资源；第二条直接运行产物，环境变量已改为 runtime。也可以用 `SCOOP_EXAMPLE_VERSION=1.2.3 scoop run --release .` 一次完成构建与运行。

## 预期结果

程序 stdout：

```text
version=1.2.3
Hello, Scoop!
bytes=14, first=72
file=banner.txt
file=docs/help.txt
help bytes=12
private bytes=0
```

资源中的换行由 `includeStr` 原样保留。`settings.json` 未被 include 选中，`private/notes.txt` 被 exclude 排除；按精确路径查找不存在的条目得到 None，所以本例输出 private bytes=0。

试用缺失和空值时重新构建：

```sh
env -u SCOOP_EXAMPLE_VERSION scoop run --release .
SCOOP_EXAMPLE_VERSION= scoop run --release .
```

这两次只有首行变化，分别为 `version=dev` 和 `version=`。空字符串是 `Some("")`，不会采用 Elvis 的默认值。

## 关键说明

这四个 compiler intrinsics 由 core Cone 的 `scoop.compiler` package 提供，需要显式导入。可使用 exact import、`import scoop.compiler.env as compileEnv` 或 `import scoop.compiler.*`；也可以在块内导入，见 `language/imports`。

| 调用 | 结果与用途 |
| --- | --- |
| `includeStr(path)` | `String`：嵌入严格 UTF-8 文本，保留 BOM、换行与全部内容；编码错误在编译时报出 |
| `includeBytes(path)` | `Array<Byte>`：嵌入原始字节；数组不可变，Byte 为有符号 8 位整数，原始位型保持 |
| `includeDir(path, include = ["**"], exclude = [])` | `EmbeddedDir`：嵌入匹配文件的逻辑路径和字节内容 |
| `env(name)` | `Option<String>`：读取编译器宿主进程的环境变量；不存在为 None，空值为 `Some("")` |

结果在定义它们的 Cone 编译时确定并嵌入产物，运行时不再读取这些文件或编译环境。库的泛型函数和默认参数也保留定义方编译时取得的值。不要把资源更新当成运行期热加载；需要更新嵌入值时重新构建。相同常量对象是否共享 identity 没有保证。

路径相对调用所在源码的实际目录，而非 shell 当前目录。本例从 `src/main.scoop` 用 `../assets` 到达同一 Cone 中的资源。manifest Cone 的边界是 Cone.toml 所在目录；单文件模式的边界是解析入口 symlink 后的实际文件所在目录。目标路径和解析全部 symlink 后的目标都必须在所属 Cone 内。includeDir 会检查遍历到的每一项，包括未匹配或被排除的条目；exclude 不能绕过越界 symlink 检查。

includeDir 的 include 多项取并集，然后应用 exclude；默认选择全部文件，无隐含 `.gitignore` 或隐藏文件排除。匹配针对目录内完整的相对路径，使用 `/`、区分大小写：

| pattern | 含义 |
| --- | --- |
| `*.txt` | 仅根目录中的 txt 文件 |
| `**/*.txt` | 根目录及任意子目录中的 txt 文件 |
| `private/**` | private 目录下的文件 |
| `images/icon?.[ab]` | `?` 匹配一个 Unicode 标量；字符组匹配 a 或 b |

`*` 不跨目录分量；`**` 必须是完整分量；字符组也支持 `[!...]` 排除。没有 brace 展开或反斜线转义，字面的 `*`、`?` 可写作 `[*]`、`[?]`。空 pattern、绝对 pattern、`.`／`..` 分量与反斜线不合法。`include = []` 或无匹配产生空文件表，目录仍必须存在并通过检查。

`EmbeddedDir.files` 是 `Array<EmbeddedFile>`，每项具有 String 类型的 path 和 `Array<Byte>` 类型的 bytes。路径相对 includeDir 根目录，以 `/` 分隔并按 UTF-8 字节序排列，不包含空目录。`bundled["docs/help.txt"]` 返回 `Array<Byte>?`，只做精确路径匹配，不展开 glob 或解析 `..`。

env 的名称必须以区分大小写的 `SCOOP_` 开头，且不含 `=` 或 NUL；非法名称、非法 Unicode 值是编译错误。交叉编译时读取的也是宿主编译环境。

## 常见误用

- 使用函数前写 exact、alias 或 star import。`import scoop.compiler` 不能导入 package 本身，`scoop.compiler.env(...)` 也不是可省略 import 的包限定调用写法。
- path、name 使用编译期可确定的 String；include／exclude 使用由常量 String 组成的数组字面量。普通局部 val、函数形参、运行期数组或用户函数调用不能充当输入。`includeStr(env("SCOOP_FILE") ?: "default.txt")` 所需的常量 Elvis 本版尚不支持；本例对 env 结果使用普通 val 和 Elvis 是合法的。
- `env("HOME")` 会报错，不会返回 None。需要运行期读取配置时使用相应平台 API；改变已构建程序的环境不会改变已嵌入的值。
- `::env`、`::includeStr` 等不能取得函数引用：这些 intrinsic 在编译期求值，没有运行期 callable 入口。别名也遵守此规则。`getCurrentSourceLocation` 同样不允许取函数引用，以保留调用点语义。
