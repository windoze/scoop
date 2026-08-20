# Scoop 小值调用 ABI 对照

循环执行一千万次外部 `@NoGC` Scoop 调用，每次将 16-byte `Pair` 的第一个字段加一，输出必须为 `10000010`。callee 位于独立 LLVM object，编译器无法通过 MIR 或 LLVM 内联删除调用。普通 Managed 循环保留 poll；两种工具使用各自配套的 runtime／sysroot。

`provider`／`consumer` 另测相同循环的普通跨 Cone 源码调用。先将 provider 构建为 `dev.m34.bench/small-provider/0.1.0/cone.slib`，再用 `--cone-path` 构建 consumer；provider 正文不会导入 consumer 内联。这组没有 native 边界协议，适合直接观察小值调用及复制成本。两个工作负载分别报告，不跨工作负载计算加速比。

M34-4b 工具使用 `indirect.ll`，M34-5a 工具使用对应 target 的 `.ll`（Linux musl 可用 GNU 文件，二者对此函数机器 ABI 相同）。用 LLVM 22.1 的 `llc -mtriple=<llvm-target> -filetype=obj -relocation-model=pic` 生成 object，再用 `ar rcs libsmall_values.a` 打包。Darwin 的 LLVM triple 使用 `aarch64-apple-macosx` 加当前 deployment 版本，以产生 `LC_BUILD_VERSION`；Linux 与普通 target 名称相同。编译 `program.scoop` 时加入该目录的 `--library-path`。

测量复用上层 `measure.py` 的 `run`／`measure` 函数，保存七次运行、输出、GC 统计、两次构建时长、实际缓存状态与产物大小。检查 `pair_step` 和 caller 的机器码；每台机器串行完成两组测量，不与构建或测试重叠。它只比较 GC-free 小值的实际调用成本，不代表含分配或内联后应用的整体收益。
