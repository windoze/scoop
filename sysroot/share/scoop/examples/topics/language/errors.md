## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
count=3
finished
count must be positive
finished
```

## 关键说明

parseCount 使用 std 的 String.toInt，再拒绝非正计数。此处选用可解析的 "3" 和 "0"，使成功与业务校验失败的输出稳定。try 中的异常由匹配 catch 处理；finally 在两条路径都执行。

Exception.message 是 String?，打印时使用 `?:` 提供缺失说明。若希望把格式错误当成普通缺失，可选择 `toIntOrNone` 并显式处理 Option；这不会自动处理后面的业务限制。

本例只捕获 IllegalArgumentException。若改为非数字输入，应依据 toInt 的 NumberFormatException 合同处理，而不是用此处的 catch 吞掉所有失败。

## 常见误用

不要把 message 当成非可选 String，或假设任意解析 API 都返回 Option。finally 用于必须发生的收尾；不要在其中用新的 return 掩盖原结果或异常。
