# 日志示例

[hello.scoop](hello.scoop) 使用预编译 facade 和 console provider，展示默认格式、惰性过滤、消息构造失败和自定义格式。已安装工具链中可运行：

```sh
scoop run hello.scoop --release
```

仓库开发先按[准备脚本说明](../../../../scripts/README.md)准备 sysroot，并设置 SCOOP_SYSROOT。示例前半部分写 stderr，末尾自定义格式写 stdout。
