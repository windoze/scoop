# Ryu 最短十进制转换

来源：https://github.com/ulfjack/ryu

固定 commit：`4c0618b0e44f7ef027ebae05d2cc7812048f7c8f`（2026-02-10）。
本目录仅包含 f2s/d2s 最短转换及其必要头文件，保留上游原文。
采用随附 Apache-2.0 许可证；上游同时提供 Boost-1.0 许可。
Scoop 的文本形状由普通 core `floating_text.scoop` 处理，managed String 分配与复制复用 typed StringCopy。core 直接以 C ABI GCLeaf 调用 Ryu 的 buffered_n 入口，旧 C 格式包装已退役。

不构建通用 128 位、fixed precision 或实验性 parser。
