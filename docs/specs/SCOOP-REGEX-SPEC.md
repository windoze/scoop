# Scoop 正则表达式库规范

版本：0.1，已实现（2026-10-22）。提供普通 Cone `scoop:scoop.regex:0.1.0`，依赖 core 和 std。设计与实施路线见 [regex 设计](../regex/INVESTIGATION.md)，完成情况见[实施记录](../regex/PROGRESS.md)。

本库通过普通 C ABI 使用 PCRE2-8 10.49，不修改语言、runtime 或编译 pipeline。适用既有[语言规范](SCOOP-SPEC.md) §11.4、§11.18、§13.4、§13.11、[运行时规范](SCOOP-RUNTIME-SPEC.md) §3.4、§3.5、§3.8、[实现规范](SCOOP-IMPL-SPEC.md)的 native library 与 sysroot 合同。

## 1. 引擎与字符串

- 使用 PCRE2 原生模式语法，固定 UTF-8；默认设置 UTF、UCP、NEVER_BACKSLASH_C，以及 ANY 换行和 UNICODE BSR。模式内合法选项继续生效。
- 不启用 ALLOW_LOOKAROUND_BSK，不接受任意二进制输入。`\C` 禁用，以维持捕获端点为合法 UTF-8 字符边界。
- String 允许 NUL，输入始终使用指针加显式字节长度。公开索引和长度使用 Long Unicode 标量计数；范围为 `[start, endExclusive)`。
- 不自动 normalization、full case folding 或重写模式；`\w`、`\b`、`\X` 等沿用该 PCRE2 版本的语义和限制。上游问题不在 Scoop 补丁、匹配后处理或另一引擎中绕过。
- 首版无 partial matching、重叠搜索、callout、自定义字符表、模式缓存、regex literal 或编译期模式检查。

## 2. 编译与查询

`RegexOption` 是实现 Equals<RegexOption> 和 Hash 的普通 enum：IgnoreCase、Multiline、DotMatchesAll、Comments、AsciiClasses、DuplicateNames。AsciiClasses 仅切换字符类和相关单词边界，不关闭 UTF，也不改变 IgnoreCase。构造器保存选项的独立快照。

```scoop
public class Regex public constructor(
    pattern: String,
    options: Set<RegexOption> = emptySet()
) : Closeable {
    public val pattern: String
    public val options: Set<RegexOption>
    public val isClosed: Boolean
    public fun matches(input: String): Boolean
    public fun containsMatchIn(input: String): Boolean
    public fun matchEntire(input: String): MatchResult?
    public fun matchAt(input: String, index: Long): MatchResult?
    public fun find(input: String, startIndex: Long = 0L): MatchResult?
    public fun findAll(input: String, startIndex: Long = 0L): Sequence<MatchResult>
    public override fun close()
    public companion object {
        public fun escape(literal: String): String
    }
}

public fun String.toRegex(options: Set<RegexOption> = emptySet()): Regex
public fun String.toRegexOrNone(options: Set<RegexOption> = emptySet()): Regex?
```

声明仅示意公开形状；库源码正常提供方法体和属性初始化。

`matches`/`matchEntire` 从零偏移执行，并要求执行到输入末尾，分别返回 Boolean/匹配结果；使用 match-time ANCHORED 和 ENDANCHORED。`containsMatchIn` 只判断是否存在匹配。`matchAt` 从指定起点锚定执行，`find` 从起点向后搜索。输入始终是完整字符串，不通过切片实现起点。

起点合法范围包含 `0..input.length`；非法位置抛 IndexOutOfBoundsException。`\K` 重设返回的 0 组起点，不改变上述执行合同；例如 `foo\Kbar` 对 `foobar` 的 matchEntire 返回 `bar`。

PCRE2 match 返回值大于等于零均为成功；零仅表示捕获向量容量不足，Boolean 路径仍返回 true。只有 NOMATCH 返回 false/None，其他负值映射执行错误。需要捕获时按模式创建完整 match-data。

## 3. 结果快照

```scoop
public struct MatchRange(val start: Long, val endExclusive: Long)
public struct MatchGroup(val value: String, val range: MatchRange)
public class MatchResult {
    public val value: String
    public val range: MatchRange
    public val groups: MatchGroups
}
public class MatchGroups {
    public val size: Long
    public operator fun get(index: Long): MatchGroup?
    public operator fun get(name: String): MatchGroup?
    public fun getAll(name: String): List<MatchGroup?>
}
```

- groups 包含 0 组，0 组与 MatchResult.value/range 相同。未参与的合法组返回 None，参与的空捕获返回有值的空串及空区间。
- 组号越界抛 IndexOutOfBoundsException，未知名字抛 IllegalArgumentException。
- DuplicateNames/`(?J)` 下，按组号排序，名字下标返回第一个参与的组；全部未参与则为 None。getAll 返回该名字的全部槽，保留未参与的 None。分支重置共用组号时仅一个槽；量词捕获沿用引擎最后保留的值。
- 结果是不可变 Scoop 快照，下一次匹配或关闭 Regex 不影响既有结果，包括名字查询。组名元数据复制成 Scoop 数据后共享，不访问已释放的 code。
- lookaround 捕获可以位于 0 组之外，端点不必随组号递增。

## 4. 惰性迭代

findAll 返回可重复创建独立 iterator 的 Sequence。入口校验起点和关闭状态，首次 next 开始匹配；耗尽后持续 None。使用 `pcre2_next_match` 返回的位置与 flags 推进，不自行实现空匹配算法。例如 `.*?` 对 `ab` 依次得到空串、a、空串、b、空串；CRLF 采用引擎所选换行模式的推进规则。

iterator 在 next 之间只保留 Regex、输入和位置／选项／索引辅助等 Scoop 数据；每次 next 创建并释放 native match-data，不保留输入指针。提前停止不需要关闭 Sequence。消费期间须保持 Regex 打开；新的匹配操作在关闭后抛 IllegalStateException。

## 5. 替换、分割与转义

```scoop
public fun Regex.replace(input: String, replacement: String): String
public fun Regex.replace(input: String, transform: (MatchResult) -> String): String
public fun Regex.replaceFirst(input: String, replacement: String): String
public fun Regex.replaceFirst(input: String, transform: (MatchResult) -> String): String
public fun Regex.replaceTemplate(input: String, replacement: String): String
public fun Regex.replaceFirstTemplate(input: String, replacement: String): String
public fun Regex.split(input: String, limit: Long = 0L): List<String>
public fun Regex.splitToSequence(input: String, limit: Long = 0L): Sequence<String>
```

String replacement 默认按字面量处理。Template 入口使用 PCRE2 默认 replacement 语法，包括 `$1`、`${name}`、`$$`；不启用 extended。直接使用 pcre2_substitute，已知未参与组按空串展开，未知组报模板错误。无匹配时返回原串，不预校验未使用的引用。

回调替换在结束 native 调用和输入借用、复制结果之后调用普通 Scoop lambda，不通过引擎 callout 回调。允许分配、重入和抛异常，不允许挂起；异常传播，不返回部分输出，不回滚已发生的回调副作用。若回调关闭当前 Regex，后续需要匹配时按已关闭对象处理。

split 不插入分隔表达式的捕获；保留首尾和连续分隔的空段。limit=0 不限，正数限制段数且末段为余下文本，负数抛 IllegalArgumentException。空输入返回单个空串；空模式分割 ab 为 `["", "a", "b", ""]`。splitToSequence 惰性产生同样的分段，复用匹配推进。

escape 只生成模式字面量引用，正确处理输入中的 `\E`；不增加通用 escape 或 replacement 转义接口。

## 6. 错误与资源

| 情况 | 结果 |
| --- | --- |
| 模式语法错误 | RegexSyntaxException：pattern、标量 offset、nativeCode、detail |
| replacement 模板错误 | RegexReplacementException：nativeCode、detail、可选标量 offset |
| 引擎执行失败 | RegexMatchException：kind、nativeCode、detail；kind 区分 MatchLimit、DepthLimit、HeapLimit、JitStackLimit、Memory、Other |
| native 分配失败 | RegexMatchException，Memory；不作为语法错误或未匹配 |
| 越界／非法实参／已关闭 | IndexOutOfBoundsException／IllegalArgumentException／IllegalStateException |

三个 Regex 异常继承 Exception；toRegexOrNone 仅转换 RegexSyntaxException。诊断 detail 从 native 复制，不承诺固定英文文案。

Regex 实现 Closeable，close 幂等；使用与关闭同一实例由调用者串行化。编译/JIT 完成后的模式可以被并发只读查询，每次调用使用独立 match-data/context；不共享可变 Matcher。release block 只释放 native handle，遵守既有 GC-free 合同，不承诺回收时点。构造、结果复制、回调失败均释放其已取得的临时资源。

JIT 在构造、对象发布前尝试；JIT 不适用、不可用或生成失败时保持已编译模式的解释器能力。公共匹配调用 pcre2_match；match-time ANCHORED/ENDANCHORED 的入口使用解释器。JIT 不改变回溯复杂度。引擎限制原样映射，不提供硬超时、自动取消或跨库资源预算。

## 7. 接入与验证

C 适配不访问 Scoop 对象头。编译、匹配和替换使用 NativeSafe，输入通过 String.withUtf8Bytes 借用；跨调用不保存借用指针。合法 String 可使用 NO_UTF_CHECK，起点必须来自受检标量转换或引擎有效结果。

一次查询／iterator 的 byte↔scalar 定位按需建立并复用；不为每个捕获重新扫描输入，不用逐字符下标定位。Boolean 查询不建立结果索引表。复制捕获直接使用有效字节端点，不经反复的标量 slice。

PCRE2 作为独立 C 静态库交付，只构建 8-bit 引擎及所需 JIT，按[实现规范](SCOOP-IMPL-SPEC.md)的当前完整 target 目录准备，包括 Darwin 的 AArch64／amd64。Windows 按所选 Microsoft／LLVM-MinGW 环境及 CRT 分别构建；Microsoft 环境使用动态 UCRT（`/MD`），C 适配显式声明 `PCRE2_STATIC`。维护脚本将上游静态 archive 安装为该 target 下的 `libpcre2-8.a`，保留实际 COFF 内容，不与其他环境共用。普通应用使用已有 sysroot 与 native.libraries，不下载或重建依赖；已验证范围见[目标扩展实施记录](../target-refactor/PROGRESS.md)。

测试使用所属 Cone 的 release scoop.test；覆盖本库选项、索引、快照、迭代、替换、分割、异常映射和生命周期。禁止新 fixture、底层补缺的正反例、上游 Unicode 兼容性／完备性测试、无关 HTTP 组合、全量回归或普通功能 GC stress。实际底层缺口先在对应层修复并独立提交，再继续本库；不在库中绕过。
