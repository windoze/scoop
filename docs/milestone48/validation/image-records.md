# 最终 image 使用已保存的运行时记录

本批基于 `e393aad7`，是 ProgramLink 对象重放退役的前置实现，不代表 M48 全部完成。

## 实现与范围

`single-cone-production/7` 新增 field 14：运行时指纹前缀与六组带 typed definition ID 的 canonical runtime records。编译生产端从实际生成的注册结果编码一次，并用同一份记录计算原 image fingerprint。最终链接按 ODR／物理成员选择所得 definition 集合筛选记录，计算实际最终 image fingerprint；不再通过 `VerifiedRuntimeImageFingerprint` 取得这一输入。runtime ABI、六组记录顺序与 fingerprint 公式保持不变。

新数据直接服务最终链接，没有增加内容比对、损坏恢复或另一套验证入口。原来的对象／注册／Code 重放仍由其他 ProgramLink 消费方引用，本批尚未删除，不能据此宣称 Link 已不读取机器对象。后续需用保存的 member、symbol、native 与 ODR 数据替换这些消费方，然后删除旧重放链及其独占测试。

实现分为记录选择、wire 读写与生产编码；新增文件分别为 96、92、62 行，生产编码文件为 157 行。两项单元测试独立按字节构造指纹输入，验证六张表的顺序、筛选后的条数、空选择和 wire 往返。真实程序用例覆盖 runtime 对最终 image 的读取与登记。

## 本机验收

- [workspace clippy](image-records-darwin-clippy.log.gz)、[570 项 slib 测试](image-records-darwin-slib.log.gz)、[release 构建](image-records-darwin-release.log.gz)通过。
- [实际 ProgramLink 读取测试](image-records-darwin-program.log.gz)通过，包含三个真实 executable 产物与 core。
- [10 个 CLI 用例](image-records-darwin-cli.log.gz)、[报告](image-records-darwin-cli.json.gz)通过：11 variants、142 processes。包括普通链接、稳定对象复用与缺失结果补产、空 image、普通／有序初始化、两类跨 Cone 泛型 ODR 合并、真实 ODR 冲突诊断、argv／moving GC／minor GC 和 M46 序列化。
- 首轮 slib 的 5 项失败是格式版本与固定 capability 描述指纹期望仍为旧版；按更新后的规范描述独立计算 SHA-256 并更新期望，完整 slib 复验通过。其余 565 项首轮即通过。
- 所有本机编译／测试结束后，清理 inactive `target/debug/incremental` [2,992.0 MiB](image-records-cleanup.log.gz)。

## Linux 验收

使用 `target/m48-summary-reader` 独立构建目录；历史 fixture 迁移仍使用原 `target/release`，两者不互相覆盖。[fmt](image-records-linux-fmt.log.gz)、[workspace clippy](image-records-linux-clippy.log.gz)、[release 构建](image-records-linux-release.log.gz)、[571 项 slib 测试](image-records-linux-slib.log.gz)和[实际 ProgramLink 读取测试](image-records-linux-program.log.gz)全部通过。

[GNU](image-records-gnu-cli.log.gz)与 [musl](image-records-musl-cli.log.gz)各通过同一组 10 个 CLI 用例、11 variants、142 processes；[GNU 报告](image-records-gnu-cli.json.gz)、[musl 报告](image-records-musl-cli.json.gz)逐项确认通过。执行顺序与参数见[Linux 验收脚本](image-records-linux.py.gz)。该组针对本批运行时记录与真实链接消费，不替代 M48 最终全量验收及五轮独立性能测量。
