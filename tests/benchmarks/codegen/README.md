# LLVM 管线对照

`pipeline.scoop` 的程序参数为 `ranges`、`integers`、`copies`，分别循环处理有范围检查的整数、窄范围整数、64-byte 值副本；源码 main 参数包含 argv[0]，模式位于索引 1。输出依次为 `-1`、`383`、`-1018`。`collections.scoop` 遍历托管数组并校验约束，输出 `34355544064`。

使用同一预备 sysroot、app release、runtime 配置，分别以修改前后编译器构建到独立缓存及输出。先预热，每轮交替执行两版，校验 stdout，再报告整个程序的耗时中位数与范围；启动和输出包含在内。记录最终 executable 的 `llvm-size -A` text bytes 与文件大小。不要在计时期间并行构建或测试。

编译时间以 `scoopc build <source> --profile release --sysroot <sysroot> --out-slib <output>` 直接重复编译计量，避免应用缓存命中；预热后至少三对，顺序交替。报告宿主编译器自身的构建模式；dev 编译器的绝对耗时不能当作 release 编译器性能。生成代码差异不等于端到端加速。

本批不改变 Managed poll 位置；需要测停稳尾部的频率变更另用 `../poll-response`。结果与启用范围见 `docs/milestone58/LLVM-CODEGEN.md`、`PERFORMANCE.md`。原始结果和临时产物保持仓库外。
