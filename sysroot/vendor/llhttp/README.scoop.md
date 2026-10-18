# llhttp 9.4.3

固定版本：[release/v9.4.3](https://github.com/nodejs/llhttp/tree/release/v9.4.3)。此目录保留上游发布 tag 的 `include`、`src` 与 MIT 许可证；生成的状态机保持原样，不按项目文件长度要求机械拆分。

`scoop.http` 通过普通 native.sources 编译三份 C 源码。该版本仅接受已知方法集合；不公开或开启已知存在含 NUL 不前进路径的 header_value_relaxed。具体 API 与兼容边界见 [HTTP 规范](../../../docs/specs/SCOOP-HTTP-SPEC.md)。
