# HTTP server 实施记录

完整范围见 [DESIGN.md](DESIGN.md)，公共合同见 [server 规范](../specs/SCOOP-HTTP-SERVER-SPEC.md)。本记录只保存范围、结果和待闭合事项，不保存原始日志。

## 当前状态

- 已完成阶段 A：路由、请求 Context、中间件与结果执行。
- 已完成阶段 B 的字段／Cookie／JSON／form 提取；正在接入 H1／TLS 和生命周期。
- 后续仍需完整实现阶段 B／C 的 H1、TLS、H2、超时与停机，以及 D1～D7 的七项能力；服务端 mTLS／动态 SNI、HTTP/3 不在本次范围内。
- 每个功能验证后独立提交；本机采用定向 release Cone 测试，避免重复全量测试。
- Linux 工作目录为 `nuc12:~/repos/scoop-http-server-work`；最终串行运行一组最小库验证，不跑完整套件或大规模性能测试。
- 本机辅助工作及原始日志位于 `~/scoop-work/http-server`，所有命令的临时目录显式放在其 scratch 下，不使用系统 `/tmp`。
- 2026-10-24 清理旧 `target/fixture-cache` 和 `target/debug/incremental`，保留 release 命令及开发 sysroot。

## 已验证

- 阶段 A：本机 release Cone 测试 18／18 通过；覆盖路由优先级与分组、HEAD／OPTIONS、单次 body、短写、提交回调、映射失败、两个应用的共享状态隔离及 Context 恢复。
- `scoop doc --no-deps --deny-doc-warnings` 通过，公开 API 文档可生成；源码检查及 `git diff --check` 通过。
- 字段与结构化提取：新增 4 个定向 release 测试全部通过，覆盖重复字段／Cookie、删除范围、媒体类型、codec 错误分类、form 加号和未支持内容编码；API 文档检查通过。
- HTTP Expect 消息工具：完整解析同时区分 Continue 和未知要求，5 个定向 release 测试通过，原布尔接口复用同一解析；API 文档检查通过。
- 目前只完成内存分派验证；真实网络、TLS、H2 与 Linux 验证随接入能力完成，未计为已通过。
