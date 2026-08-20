# M30 调研：浮点语义、LLVM 与 128 位数值

日期：2026-10-06。对应[设计文档](DESIGN.md)。本文记录设计前的仓库观察、上游资料和独立工具链探针；没有实施 Scoop 的 Float/Double 或 128 位支持，也没有执行 M30 正式验收。

## 1. 浮点比较与 Hash 的参考

| 语言 | 原生浮点普通比较 | 显式全序 | 原生浮点 Hash |
| --- | --- | --- | --- |
| Rust | `PartialEq` / `PartialOrd`，NaN 不等于自身且无序 | `total_cmp`，区分正负零与完整 NaN 表示 | f32/f64 不实现 Hash，也不实现 Eq/Ord |
| Swift | 普通 `== < <= > >=` 保持 IEEE，NaN 是 Comparable 全序要求的例外 | `isTotallyOrdered(belowOrEqualTo:)`，IEEE totalOrder | Float/Double 实现 Hashable；正负零的 hash 必须一致 |
| Scoop M30 选择 | IEEE，保持当前成员 equals 和派生字段比较机制 | Swift 风格非严格方法，IEEE totalOrder | 不实现 Hash |

Swift 存在浮点 hash，不代表浮点 IEEE 相等自然满足集合通常需要的等价关系。调研时 Swift 的独立样例中，NaN 仍不等于自身，以 NaN 做 Dictionary key 后用 NaN 查询不能正常匹配；Set 中重复加入 NaN 也不是自动去重。Scoop 的决定是不为 Float/Double 提供 Hash，而不是增加特殊容器规则。

Rust / Swift 两种显式全序都能区分正负零、NaN sign 和 payload。IEEE totalOrder 不是 Java 风格“统一所有 NaN 为一个排在末尾的值”；Scoop 采用前者，普通运算符不随之改变。也不照搬 Kotlin 因静态浮点类型与泛型/Any 上下文不同而改变 equality/order 的处理。

参考：[Rust f64](https://doc.rust-lang.org/std/primitive.f64.html)、[Swift FloatingPoint](https://github.com/swiftlang/swift/blob/main/stdlib/public/core/FloatingPoint.swift)、[Swift Comparable](https://github.com/swiftlang/swift/blob/main/stdlib/public/core/Comparable.swift)、[Kotlin 浮点相等](https://kotlinlang.org/docs/equality.html#floating-point-numbers-equality)。

## 2. 仓库现状与 M30 接入成本

当前整数已经具有 typed representation、同类型 operator、候选期 literal fit、const evaluator、完整 IR/meta、C storage 和 runtime 后备，可复用其阶段划分，不能直接复用其所有存储类型和运算假设。

| 观察到的现状 | 对新增数值类型的影响 |
| --- | --- |
| integer literal magnitude 与多处常量运算以 u64 为上界 | f32/f64 要独立保存十进制原值及目标 bits；Int128 还需扩展整数 magnitude 全链路 |
| 关系 operator 经 `compareTo(): Long` | NaN 无序需要专门的 typed 浮点 comparison |
| LIR / LLVM 类型映射只有现有 integer / pointer / aggregate carrier | 加入 F32/F64 必须覆盖参数、返回、phi、存储与布局，不能只扩 parser |
| canonical C storage 无 float/double | 增加实际 C 标量种类后复用 generated-C bridge |
| core 基础类型约定普遍要求 Hash | 收窄到显式实现该接口的类型，浮点不补 Hash |
| JSON parser 保留 number 原文，整数 decoder 自行精确解析 | 可直接增加两个精度的解析；无需改成统一 Double 数字树 |
| native-support 表已有 memcpy / TLS 等隐式导入 | 沿原机制加入实际 fmod 支持，不新建依赖框架 |

inkwell 0.10 已提供 f32/f64/f128/i128 LLVM 类型入口；这仅说明绑定层能表示它们。接受宿主 f64 的 `const_float` 便利接口不能作为目标浮点字面量或 binary128 的统一解析路径，应从 raw bits 构造精确常量。

## 3. 探针环境与证据边界

| 项目 | 实际观察 |
| --- | --- |
| host | macOS / Apple Silicon |
| PATH 默认 LLVM | 23.1.2 |
| 本次显式使用的 LLVM | `/opt/homebrew/opt/llvm@22/bin/llc` 与 clang，22.1.8 |
| clang resource directory | `/opt/homebrew/Cellar/llvm@22/22.1.8/lib/clang/22` |
| Apple clang | Xcode 的 `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang` |
| IR target | `arm64-apple-macosx14.0.0`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl` |
| native library 检查 | 本机 SDK 的 system tbd 与 clang_rt.osx.a 的 arm64 符号 |

浮点/整数运算探针是给 llc 输入独立 LLVM IR、读取输出汇编；C 类型探针由 clang 生成 LLVM IR 或预定义宏。Linux 结果属于 cross codegen，未在本次调研中用 Linux sysroot 实际链接和运行。Darwin helper 检查是符号可用性观察，也不等于 Scoop 已正确登记或消费对应 provider。

复现时必须显式选择 LLVM 22.1，不能因为 PATH 中有 llc 就假定版本正确。可把以下片段保存到临时 `probe.ll`，分别以三种 triple 调用 `llc -mtriple=<triple> -filetype=asm probe.ll -o -`：

```llvm
define float @rem32(float %a, float %b) {
  %r = frem float %a, %b
  ret float %r
}

define i128 @div128(i128 %a, i128 %b) {
  %r = sdiv i128 %a, %b
  ret i128 %r
}

define fp128 @add128(fp128 %a, fp128 %b) {
  %r = fadd fp128 %a, %b
  ret fp128 %r
}

define fp128 @rem128(fp128 %a, fp128 %b) {
  %r = frem fp128 %a, %b
  ret fp128 %r
}
```

这是后端能力探针，不定义 Scoop 的整数除零、signed MIN/-1 或浮点环境规则；这些必须由正常编译 pipeline 实现。

## 4. Float32 / Float64

实际 llc 观察：

| LLVM 操作 | Darwin/AArch64 | Linux/amd64 glibc / musl |
| --- | --- | --- |
| f32/f64 add、div | 对应硬件浮点指令 | 对应 SSE 浮点指令 |
| f64 sqrt intrinsic | 硬件 sqrt | 硬件 sqrt |
| f32 frem | `fmodf` | `fmodf` |
| f64 frem | `fmod` | `fmod` |
| f32/f64 → 窄 signed / unsigned saturating integer | 可生成目标代码 | 可生成目标代码 |

LLVM LangRef 明确提供 `llvm.fptosi.sat` / `llvm.fptoui.sat`：向零截断、越界饱和、NaN 为零。比裸 fptosi/fptoui 更直接地表达 M30 转换语义；后者在输入越界时可产生 poison。

现有 Darwin SDK 的 system math exports 中可见 fmodf/fmod；Linux 最终链接已包含 `-lm`。M30 仍需把 LLVM 隐式 libcalls 接入现有 native-support/产物闭包，并验证 musl 静态归档与所有动态配置。sqrt 的探针成功只记录工具链能力，不扩大本次公共 API。

编译期与文本实现建议：

| 用途 | 选择与理由 | 尚需在实施中完成 |
| --- | --- | --- |
| HIR literal / const | `rustc_apfloat`，纯 Rust、目标精度舍入、bits 及 c_fmod；不依赖 LLVM 动态库 | 锁定 crate 版本，核实转换/NaN 适配，加入边界向量 |
| runtime 最短输出 | Ryu C 的 f2s/d2s，分别支持 binary32/binary64 shortest round-trip | 固定上游 commit/许可证、接入 C 构建与规定格式 |
| JSON runtime 解析 | 目标 libc 的固定 C locale strtof_l/strtod_l，直接解析目标精度 | 验证三目标 API、完整消费、overflow/subnormal、负零与双重舍入样例 |

调研时 `rustc_apfloat` 有已发布的 `0.2.3+llvm-462a31f5a5ab`，仓库在 2026 年仍有维护；Ryu 仓库同样在 2026 年有更新。运行时采用 C 实现，不为格式化引入 Rust runtime 或 C++ runtime。Ryu 的通用扩展格式/实验性 parser 不视为已完成的 Float128 或任意长 JSON 解析方案。

参考：[LLVM 22.1.8 LangRef](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/docs/LangRef.rst)、[rustc_apfloat](https://github.com/rust-lang/rustc_apfloat)、[Ryu](https://github.com/ulfjack/ryu)。依赖的确切版本与版权文件在实际引入时记录，当前文档没有添加依赖。

## 5. Int128 / UInt128

### 5.1 LLVM 与基础 helper

三种 target 均接受 i128 IR。探针中的加法、乘法由后端展开为较窄机器操作；除余与部分浮点转换产生运行库调用：

| 操作 | 观察到的 helper |
| --- | --- |
| signed division / remainder | `__divti3` / `__modti3` |
| unsigned division / remainder | `__udivti3` / `__umodti3` |
| signed i128 → Double | `__floattidf` |
| Double → signed i128 | `__fixdfti` |

f64→i128 saturating intrinsic 也能完成 codegen，但其中仍可能需要基础转换 helper；“用了 LLVM intrinsic”不代表没有链接依赖。Scoop 的零除数和 MIN/-1 特例仍要沿现有整数语义处理，不能直接继承 helper 的异常或未定义边界。

本机 clang_rt.osx.a 的 arm64 符号中可见上述若干 ti 类 helper，SDK 的 libcompiler_rt.tbd 也导出常见 128 位整数支持。Linux 可能由目标 compiler-rt 或 libgcc 提供，必须按实际工具链核实；M30 不统一切换当前已工作的 compiler runtime provider。

### 5.2 C ABI 与编译器工作

Apple clang 与 LLVM clang 在 Darwin/AArch64 接受 `__int128`；Linux/amd64 的两个 triple 也接受。三个 C 目标的 `__SIZEOF_INT128__` 均为 16。这给 generated-C bridge 提供了可行路线，但参数拆分、返回寄存器、混合 struct 与 callback 仍需分别运行验证。

Scoop 后续实现必须同时处理：

- integer AST、范围与 const 的 u64 上限；拟议 128 位定宽 literal 可用 checked u128 magnitude 表达，不因此需要任意精度整数框架。
- 两个新的 nominal kind、带符号边界、shift mask、全部转换、字符串化和整数 Hash；不能截掉高 64 位来复用现有 fallback。
- HIR/MIR/LIR/meta 常量、指纹、静态初值、mangle、布局、boxing/array、普通调用与 generated-C storage。
- `__*ti*` 导入、实际 provider 和四种正式链接配置；包括现有 signed MIN/-1 wrapping 特例。
- 是否扩展无后缀 literal 默认阶梯、是否增加显式后缀，以及与既有 Long/ULong 的边界。这些是后续语言决定，不由 LLVM 自动给出。

结论：Int128/UInt128 比 Float128 路线明确，工作重心是贯通已有定宽整数 pipeline 和实际 helper/ABI；M30 只保留调研，不增加类型名、字面量后缀或 IR 占位 kind。

## 6. Float128 / binary128

### 6.1 LLVM 类型可用不等于平台能力完整

IEEE binary128 使用 1 位 sign、15 位 exponent、112 位 fraction，有效精度为 113 bits。LLVM 有 `fp128`，inkwell 能创建该类型，但其多数算术由软件完成。

| fp128 操作 | 观察到的 helper |
| --- | --- |
| add / sub / mul / div | `__addtf3` / `__subtf3` / `__multf3` / `__divtf3` |
| less-than 比较 | `__lttf2` |
| Double → fp128 | `__extenddftf2` |
| fp128 → Double | `__trunctfdf2` |

额外观察到目标相关的 remainder/sqrt libcall：

| triple | fp128 frem | fp128 sqrt |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | `fmodf128` | `sqrtf128` |
| x86_64-unknown-linux-musl | `fmodl` | `sqrtl` |
| arm64-apple-macosx14.0.0 | `fmodl` | `sqrtl` |

不能把后两行当作可用实现：这些目标的 C long double 不是 binary128，函数名能够解析也不说明参数、返回 ABI 和数值语义正确。必须选择明确的 binary128 helper，必要时在受支持目标中显式降级，而不是放任默认 libcall 名称决定语言语义。

### 6.2 `long double` 与 C 边界

| C 目标 | `long double` size | `LDBL_MANT_DIG` | `__float128` 探针 |
| --- | --- | --- | --- |
| Darwin/AArch64 | 8 bytes | 53 | Apple clang / LLVM clang 均拒绝该目标上的 `__float128` |
| Linux/amd64 glibc | 16 bytes | 64 | LLVM clang 接受，生成 fp128 |
| Linux/amd64 musl | 16 bytes | 64 | LLVM clang 接受，生成 fp128 |

Linux 的 16-byte long double 是含 padding 的 x87 扩展精度表示，有效精度不是 113 bits。Darwin/AArch64 的 long double 则与 Double 同精度。`__float128` 能通过 C 前端也不证明对应 libc/数学函数已安装，musl 尤其需要独立核实库支持。

本机 clang_rt.osx.a 的 arm64 符号检查未找到 `__addtf3/__subtf3/__multf3/__divtf3/__extenddftf2/__trunctfdf2` 这一组。不能假定 macOS 已安装的 compiler runtime 自动提供 binary128；手动构建 compiler-rt 子集还需要确认它在该目标上的类型支持、ABI 与构建条件。

### 6.3 库的职责与候选路线

| 组件 | 能解决的部分 | 不能直接推出的能力 |
| --- | --- | --- |
| compiler-rt / libgcc builtins | 平台支持的基本算术、比较和整数/浮点转换 helper | 完整数学函数、可靠的十进制输入输出、所有目标均有 binary128 C ABI |
| glibc `_Float128` math | 支持该类型的平台上的 f128 数学函数 | 自动覆盖 musl、Darwin，或解决 Scoop 产物/ABI |
| libquadmath | 明确的 quad 数学函数，如 fmodq/sqrtq，以及 strtoflt128/quadmath_snprintf | LLVM 默认自动使用 q 后缀、所有平台现成可构建、零额外部署成本 |
| Berkeley SoftFloat | binary128 基础算术、sqrt、比较、转换，可使用显式 bits / pointer helper | 完整超越函数、十进制文本转换，以及直接等价于 fmod 的 remainder |

compiler-rt 使用 Apache-2.0 WITH LLVM-exception；Ryu 可选 Apache-2.0 / Boost；libquadmath 和 SoftFloat 各有自身许可证，真正引入时随源代码/构建和分发方式确认，不能把它们视为同一工具链依赖。SoftFloat 发布版较成熟但发行频率低；调研时最新正式版本为 3e，上游仓库仍有后续维护。若选用它，需要处理其 rounding/exception 状态的线程隔离，而不是把可变全局状态直接暴露给并发 Scoop 程序。

后续可比较两条具体路线：

1. **先做具备 binary128 ABI/库的目标。** 以 LLVM fp128 和明确的目标 helper/math provider 为基础，首批只承诺实际验收的平台；另行设计公开转换、字面量、打印和 C ABI。即使选择 glibc，也须验证目标版本、符号和发行环境。
2. **统一软件值表示。** 用显式 128 位存储和 SoftFloat 等基础实现，helper 通过 pointer/out-parameter 传值，避免要求每个 C 前端都支持 `__float128`。Scoop 的布局、算术、文本、数学精度及 C 互操作仍需完整设计；不能把 binary128 简单重命名为两个整数后跳过语言语义。

两条路线都需要性能和正确舍入测试、NaN/zero/totalOrder 规则、格式化/解析，以及与既有 GC、泛型、`.slib` 和链接的组合。目前未构建或 benchmark 任一完整方案，M30 不作 Float128 实现承诺。

参考：[compiler-rt builtins 22.1.8](https://github.com/llvm/llvm-project/tree/llvmorg-22.1.8/compiler-rt/lib/builtins)、[GCC 浮点扩展类型](https://gcc.gnu.org/onlinedocs/gcc/Floating-Types.html)、[glibc 数学函数](https://sourceware.org/glibc/manual/latest/html_node/Mathematics.html)、[libquadmath 数学接口](https://gcc.gnu.org/onlinedocs/libquadmath/Math-Library-Routines.html)、[SoftFloat 文档](https://www.jhauser.us/arithmetic/SoftFloat-3/doc/SoftFloat.html)、[SoftFloat 源码](https://github.com/ucb-bar/berkeley-softfloat-3)。
