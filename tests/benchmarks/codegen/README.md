# LLVM 管线对照

`pipeline.scoop` 的程序参数为 `ranges`、`integers`、`copies`，分别循环处理有范围检查的整数、窄范围整数、64-byte 值副本；源码 main 参数包含 argv[0]，模式位于索引 1。输出依次为 `-1`、`383`、`-1018`。`collections.scoop` 遍历托管数组并校验约束，输出 `34355544064`。

`loops.scoop` 模式为 `fill`、`prefix`、`small`、`branch`，分别覆盖清零、前项递推、固定四步整数变换、不变条件分支；输出依次为 `0`、`33550336`、`4458563639690780672`、`8486008903794476648`。这组 8192 × 4096 的 NoGc 内核单独比较 load elimination、有限展开及 unswitch，并复测组合结果。

`string-reads.scoop` 模式为 `length`、`bytes`、`characters`，输出依次为 `11264000000`、`-499712`、`175646965760`。String 表示读取位于 core 中；对照必须分别以两版编译器重建同一 core，再用对应 sysroot 编译应用，不能只替换应用编译器。字符串包含 NUL、双／三／四字节 UTF-8。

`unbox.scoop` 的 `words`／`references` 模式分别处理四 Long 值、含引用与嵌套 tuple 的值。4096 个 box 存入数组并 full GC，随后扫描 1024 次，输出分别为 `34376515584`／`25776095232`；这使 box 实际逃逸，不能用 MIR 局部消除掩盖 C／IR 拆箱差异。两版使用同一预编译 core。

`clone.scoop` 的 `small`／`references`／`large` 模式分别复制 16 Long、16 个含引用值、8192 Long；前两项 100,000 次，后一项 512 次。输出为 `2250000`／`1600000`／`392448`。配套的 `../poll-response/clone.scoop` 单独测 wrapper 握手减少后的请求响应，不能与 production 计时混用。

`byval/provider` 与 `byval/consumer` 是普通库／应用 Cone，用于保留跨 Cone 的 64-byte byval 调用。先以固定编译器构建 provider 到 artifact root 的 `dev.m58/byval-provider/0.1.0/cone.slib`，再让两版 consumer 构建使用相同 `--artifact-dir`。输出 `-1018`。不能把非泛型 provider 正文当作本地 inline 输入；这个对照用于区分副本消除与函数调用开销。

使用同一预备 sysroot、app release、runtime 配置，分别以修改前后编译器构建到独立缓存及输出。先预热，每轮交替执行两版，校验 stdout，再报告整个程序的耗时中位数与范围；启动和输出包含在内。记录最终 executable 的 `llvm-size -A` text bytes 与文件大小。不要在计时期间并行构建或测试。

编译时间以 `scoopc build <source> --profile release --sysroot <sysroot> --out-slib <output>` 直接重复编译计量，避免应用缓存命中；预热后至少三对，顺序交替。报告宿主编译器自身的构建模式；dev 编译器的绝对耗时不能当作 release 编译器性能。生成代码差异不等于端到端加速。

本批不改变 Managed poll 位置；需要测停稳尾部的频率变更另用 `../poll-response`。结果与启用范围见 `docs/milestone58/LLVM-CODEGEN.md`、`PERFORMANCE.md`。原始结果和临时产物保持仓库外。
