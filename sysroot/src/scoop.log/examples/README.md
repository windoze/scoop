# 自定义日志 provider 示例

[memory.scoop](memory.scoop) 只依赖 facade，实现将事件累积到原子字符串中的简单 provider，展示接口扩展和日志闭包失败的记录结果。

使用已安装并包含日志库的 sysroot 运行：

```sh
scoop run memory.scoop --release
```

仓库开发先按[准备脚本说明](../../../../scripts/README.md)准备 sysroot 并设置 SCOOP_SYSROOT。控制台输出示例见 [scoop.log.console](../../scoop.log.console/examples/README.md)。
