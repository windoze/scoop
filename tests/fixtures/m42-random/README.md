# Random 固定向量

seeded/program/src/vectors.scoop 的 60 项期望由原作者的 SplitMix64 与 xoshiro256** 1.0 C 实现离线计算。
种子依次为 0、1、全一位型、最高位为一、42；每次非零 nextBits 取一个 64-bit 输出的高位，零宽不调用 next。
本次只取 xoshiro 的 next 核心（不编译 jump）；参考程序及输出留在 target/m42/random-reference。
正式 fixture 不运行 C 代码，也不下载网络资料。

取用的原始文件 SHA-256：

- splitmix64.c: `071795a8e29978a5cbd7015ce8f7d772e7ab4631e574e9102b748fe99105ff3d`
- xoshiro256starstar.c: `413bfcae1a0c9a0dc03f7de6c78106a67361dcce4dae15eef3195014d1dbe9ce`
