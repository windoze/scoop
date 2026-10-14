# M46 实施记录

基线：`e548b9ebe`。按用户要求先完成整个 M46，再开始 M38；M38 的已有设计修改保持原样。

## M46-1：稳定解码视图

- 语言规范 11.13.2 明确稳定输入、重复标量／字段读取、独立 unkeyed 游标及局部 end；core 增加普通 isNull requirement。
- JSON 解码去除 selected、consumed、pending 和全树消费状态；保留完整 parser 检查与序列长度检查。keys 每次提供独立快照。
- 独立 fixture 覆盖标量错误后重试、missing/null/value、重复字段、end 后读取、独立游标；组合 fixture 覆盖合成候选、default 次数、tuple 精确长度及 moving GC。缺失 isNull 的实现有准确编译诊断。
- 同步既有 decoder 实现及旧一次性消费断言。没有修改 runtime、IR、产物格式或编译器合成逻辑。
- 本机 cargo fmt、全工作区 clippy、debug/release 三个 CLI 工具构建通过。相关 59 个 fixture 全部闭合：初轮 25 项通过后切到优化版编译器，其余 34 项中 31 项直接通过，3 项新用例修正 lambda 裸 return 后复验通过；每个运行型新用例同时执行普通和 moving GC。
- 原始报告：[初轮](validation/decoder-initial-darwin.json.gz)、[剩余项](validation/decoder-remaining-darwin.json.gz)、[修正复验](validation/decoder-retry-darwin.json.gz)。初轮中断不计通过；三项源码语法错误及后续通过均保留。未改 Rust／runtime，本批不运行无关 Rust 全量；Linux 完整验证放在本机功能完成之后。

## 后续

M46-2～6 尚未实施。M38 尚未开始。

## 构建目录

开始时 target 占用约 111 GB；使用 cargo clean 清理全部可重建产物，Cargo 报告移除 241,742 个文件、154.8 GiB 逻辑字节。重新构建时禁用 incremental，首次重建后目录约 3.8 GB。
