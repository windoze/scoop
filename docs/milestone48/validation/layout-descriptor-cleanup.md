# layout／descriptor／physical 集中清理（实施中）

基线：`9edf9060`。本轮变更仍在工作区，等待剩余 M48 清理与最终组合验收；不宣称功能完成。

## 已修改

- 读取保存的 layout 值存储后建立索引，字段、tuple、enum、class、box、array 按 typed 引用取已有存储，保留保存的偏移、扫描与实例几何；删除 slib 的 MIR→layout 重建模块。
- descriptor 从保存字段读取布局、继承、分派、定义、注册与诊断名称；删除 MIR descriptor 重建、诊断目录重建和候选改写工厂。
- physical import 不再保存完整语义合同副本，从已读取的 provider 导出取得实际合同；删除整套 semantic projection 深比较。`cross-cone-layout-abi` 从 15 升为 16。Code contribution 使用独立的既有五字段合同编码，产物 import 使用四字段引用，runtime ABI 不变。
- 删除无实际调用的 C bridge production reader，defined owner／undefined use／ODR directory 重建比较 API，以及 writer 再次计算对象 fingerprint 的复核。
- HIR／MIR／LIR foundation 不再完整编码原始数据与重建结果并比较。引用及实际结构读取保留。
- 同步退役独占比较／改写测试；保留生产端布局、实际对象、格式及真实语言 fixture。class ancestry 副本与无调用的 depth／projection API 删除，源码继承规则由原阶段处理。

代码／规范共 147 文件，新增 1,091 行、删除 5,319 行，净删 4,228 行。新增 reader 文件均不超过 180 行。统计不含用户的 M38／M45 文档改动，也不含本记录。

## 当前检查

`cargo fmt --all` 与 `cargo clippy --workspace --all-targets` 最终通过、无警告。中间接口衔接错误已修复。

按用户最新要求，本轮没有运行 Rust／CLI／runner 功能测试，没有连接 Linux。最终本机完整复验和 Linux GNU／musl 验收必须等剩余清理完成后统一执行。此次 lint 通过不能替代该验收。现有 round-trip 已迁移到保存数据 reader 或保留格式断言；[显式删除／改名清单](../CLEANUP-TESTS.tsv)。

`target/tmp` 为空，无已结束 fixture 目录可清理；保留近期增量编译缓存。

## 继续

继续集中处理 HIR／strong-profile 剩余重复职责、ProgramLink identity／foundation 边界、dump 结果复用和缺失观察的最小阶段。随后本机全量与性能复验，通过后统一 Linux 验收及功能提交。M48 尚未完成，M38 未推进。
