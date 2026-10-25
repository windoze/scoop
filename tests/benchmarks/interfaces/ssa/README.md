# 接口值与重定位的数据流

这三个普通 release 程序补充现有 dispatch／lambdas 对照：

| 程序 | 真实工作 |
| --- | --- |
| `list-long.scoop` | 通过未知 `List<Long>` 迭代十万次，每次一千个元素。 |
| `interface-arrays.scoop` | 64 个 Child 的复制、Parent 投影和调用；定期产生独立数组副本。 |
| `class-live.scoop` | 抽象 class 调用后继续读取同一 receiver 的字段，保留真正跨调用的引用。 |

双方使用相同源码、接口布局与预编译 sysroot，只更换编译器；`scoop build <source> --release --sysroot <fixed-sysroot> --scoopc <compiler> -o <binary>`。使用上两级的 `compare.py`，一对预热、五对 AB／BA，检查输出一致。正常计时不启用 GC stress；移动正确性在 `gc-correctness/interface-ssa` fixture 中单独验证。`--emit all` 和目标 `llvm-objdump` 用于检查真实循环，原始产物和逐次样本放仓库外。
