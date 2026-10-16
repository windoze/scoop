# M51 实施进度

状态：实施中。开始日期：2026-10-17。基线：`8dad7d42e`（M50 合并）。范围与验收要求见 [设计](DESIGN.md)。

## 分批状态

1. Native 接入：先补齐目标条件 include，准备 OpenSSL 3.5.9 vendor 与目标资源，验证实际摘要、签名及 TLS 握手。
2. 密码与密钥：待实施。
3. 证书与 JWS：待实施。
4. 共享 TLS 引擎与同步流：待实施。
5. M49 异步 TLS：待实施。
6. 三目标组合及外部消费者验收：待执行。

## 已完成批次

- 目标条件 native include：复用现有目标谓词，按声明顺序选择生成头文件目录；预处理、选项与对象更新使用同一集合。manifest 解析记录升级为 v3。格式化与 release workspace clippy 通过；native manifest 9 项、正式 CLI 3 项／5 变体／9 进程通过，覆盖新条件路径、既有外部 include 和非法空路径诊断。首次 TOML 位置解码失败已定向修正并复测。

## 验证与工作目录

本机逐功能格式化、lint 和定向验证；最后在 nuc12 的 `~/repos/scoop` 执行 Linux 完整验证。日志、报告及工作目录使用两端 `~/.cache/scoop-m51`，临时目录显式设为其中的 `scratch`，不使用 `/tmp`。定期清理无用 target 产物，保留正在复用的 release 工具。M47 文档修改不属于本任务。
