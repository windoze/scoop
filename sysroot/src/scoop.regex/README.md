# scoop.regex

基于 PCRE2-8 10.49 的 UTF-8 正则库，提供可复用模式、编号／命名捕获、惰性搜索、替换和分割。作为普通 Cone 使用：

```toml
[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.std" = "0.1.0"
"scoop:scoop.regex" = "0.1.0"
```

```scoop
import scoop.regex.*

Regex("""(?<key>[a-z]+)=(?<value>[^;]+)""").use { regex ->
    val input = "user=chen;count=12"
    for (match in regex.findAll(input)) {
        println(match.groups["key"]!!.value)
        println(match.groups["value"]!!.value)
    }
    println(regex.replaceTemplate(input, "${key}: ${value}"))
}
```

位置是 `Long` Unicode 标量索引，区间为 `[start, endExclusive)`；NUL 可以出现在输入中。`matches`/`matchEntire` 要求从头执行到尾，`matchAt` 从指定位置锚定执行，`find` 从指定位置向后搜索。起点总是针对完整输入，不切断 lookbehind 或锚点的上下文。

捕获结果是独立快照，后续搜索或关闭 Regex 后仍可使用，包括组名查询。未参与的组返回 None，参与的空捕获有值；未知名字和越界组号是调用错误。`findAll` 与 `splitToSequence` 每次创建独立 iterator，提前停止不需要清理 native 迭代器；实际匹配期间须保持 Regex 打开。

`replace`/`replaceFirst` 的 String 参数是字面文本；模板引用显式使用 `replaceTemplate`/`replaceFirstTemplate`，支持 PCRE2 默认 `$1`、`${name}`、`$$`。回调替换在结束 native 输入借用后执行，允许重入与抛异常。`split` 保留空段、不插入分隔符捕获，limit=0 不限，正数把剩余内容保留在最后一段，负数报错。空输入得到单个空串。

默认 UTF/UCP，使用 Unicode 换行规则；禁止 `\C`。模式、属性、大小写、字素簇和高级语法沿用 PCRE2，不做隐式 normalization、模式重写或上游问题补丁。`\K` 可改变报告的匹配起点；例如 `foo\Kbar` 对 `foobar` 的 matchEntire 结果为 `bar`。JIT 是可选优化，构造时尝试；不可用时使用解释器，锚定查询也沿原生路径执行。回溯模式不保证线性时间或可取消的硬超时。

Regex 实现 Closeable；优先使用 `use`，close 可重复调用。编译后的模式可并发查询，每次调用的匹配数据独立；调用者负责串行化关闭与使用。Syntax、Replacement、Match 异常区分语法、模板及执行错误，未匹配返回 false/None；`toRegexOrNone` 仅转换语法错误。

目标资源通过 [PCRE2 准备脚本](../../../scripts/README.md)与预编译 sysroot 交付；PCRE2 是独立 C 静态库，不增加 Rust 后端。完整合同见 [regex 规范](../../../docs/specs/SCOOP-REGEX-SPEC.md)。

开发验证只使用 release `scoop.test`：

```sh
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop test sysroot/src/scoop.regex --release
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop build sysroot/src/scoop.regex --release \
  -o sysroot/lib/aarch64-apple-darwin/scoop.regex.slib
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop test sysroot/src/scoop.regex/consumer-tests --release
```

consumer-tests 从 sysroot 消费预编译 Regex，覆盖公开 API 组合与只读并发使用，不重复验证编译器、链接器或上游 Unicode 算法。
