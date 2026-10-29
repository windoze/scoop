# libbacktrace

上游：https://github.com/ianlancetaylor/libbacktrace

固定 commit：`0b9b49cf4a2c9229fc052d6716e1528b2f23e91a`（2026-09-03）。许可见 LICENSE。

只保留实际使用的 DWARF／Mach-O／ELF、文件访问和 mmap 分配源码；不编译其栈采集入口，runtime 沿既有 unwind provider 采集。config.h 为当前三个 64-bit target 的固定配置，backtrace-supported.h 对应线程安全 mmap 实现。不依赖外部 zlib／zstd，ELF 调试段解压沿上游内置实现。

dwarf.c 有两项局部修补：DWARF 5 的绝对文件路径不再拼接编译目录；`scoop$` linkage name 不覆盖 DW_AT_name／abstract origin 的源码名称，其他语言的名称优先级不变。LLVM 中的精确 linkage name 保持原样。

state 在进程内共享，首次打印时初始化。模块集合发生变化时 runtime 保守退回历史模块标识／地址，不复用错误的地址索引；不为每个 Throwable 创建无法释放的 libbacktrace state。
