# M52 实施记录

基线：`43ff43117`。先完整完成 M52，再继续 M53。

已确认范围：M47 尚未定稿，延后决定；本期分发现有 `scoop`、`scoopc`、`scoop-link` 三个宿主命令，不以 generator 为前置。M53 采用 llhttp 9.4.3，并明确已知方法集合限制。

实施与验收按功能记录；原始日志、构建缓存和发布归档保存在仓库外。M52 总体验收尚未完成。

## Runtime 索引

- schema 4 按索引目录解析相对对象路径；生产记录保留完整编译信息，消费只检查目标、runtime ABI 和最低部署版本。构建记录切换为 `runtime-v4`。
- 本机 `cargo fmt --all`、`cargo clippy -p scoop -p scoop-linker --all-targets` 通过；三个配套命令以 release 构建通过。
- release 定向测试通过：runtime 构建／缓存 3 项；复制到带空格目录后真实链接运行 1 项。覆盖兼容编译器／SDK、错误目标、过低 macOS 部署版本、实际输入变化、缺失输入及失败不发布。
- Linux 完整回归留到本机各功能闭合后统一执行。


## 共享 native 源码

- manifest native 源文件允许归一化后的前置 `..`；Scoop 源码路径约束不变。native 对象键采用独立路径类型，父路径贯通对象生产和 `.slib` 消费。
- 格式化与相关 crate 的全部 target lint 通过；manifest 单元测试 61 项、路径身份定向测试 8 项通过。
- 本机正式 CLI 验证通过：8 个相关 negative fixture；2 个新增正向 fixture、3 个变体，覆盖共享 C 源码与头文件更新、跨 Cone C++／泛型、release 与 moving GC。首次正向验证发现的产物路径类型限制和用例泛型语法错误均已修复并复验。
