# num-bigint 算法参考

- 上游：[rust-num/num-bigint](https://github.com/rust-num/num-bigint)。
- 核对版本：`num-bigint-0.5.1`。
- 固定 commit：`33c59ba44b7bdb09975b38a321b1b88c6a444005`。
- 适用许可：MIT，保留于 [LICENSE-MIT](LICENSE-MIT)。上游同时提供 Apache-2.0 选择；本移植选择 MIT。

Scoop 的 `src/internal/integer/division.scoop` 参考 `src/biguint/division.rs` 的正规化 Knuth D 商估计与修正；`multiplication.scoop` 参考 `src/biguint/multiplication.rs` 的 schoolbook、Karatsuba 和不平衡分割。代码按 Scoop 的 32 位 limb、ULong 中间值、不可变发布和 managed 数组模型重写。未链接 Rust num-bigint，也未移植其 trait、allocator、RNG、Toom-Cook 或平台配置。

64 位 Miller–Rabin 的七个确定性底数参考 [Jim Sinclair 的记录（2011-04-20）](https://miller-rabin.appspot.com/)。一般精度抽样复用 Scoop 的既有 `Random`。BigDecimal 舍入及宽 scale 运算是本库实现，不复制 Java BigDecimal 源码。

此目录保留实际适用的许可与归属；现有工具链打包会把它收集到 `share/licenses`，同时保留库源码。
