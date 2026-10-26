## 运行

在保存 greet.scoop 的目录执行：

```sh
scoop run --release greet.scoop -- Ada --count 2 --excited
scoop run --release greet.scoop -- --help
scoop run --release greet.scoop -- Ada --count 0
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
Hello, Ada!
```

第一条命令打印以上两行并返回 0；第二条输出包含 Name to greet、--count、--excited 的帮助并返回 0；第三条打印 `count must be positive` 并返回 2。此例把参数错误说明写到 stdout。

## 关键说明

main 的 args 包含 argv[0]。parseCommandLine 接收整个命令行并处理程序名，不要手动裁掉第一项后仍调用同一入口。注解定义位置参数、重复名字的选项、默认文本和 Boolean flag；Options.Companion 是显式传入的 decoder。

run 的 `--` 将后续参数传给程序，特别是以 --help 开头时，避免由 scoop 自身解释。count 的语法由 decoder 解析，正数业务限制由程序另行检查。

## 常见误用

不要将 scoop run 的选项与示例的选项混在分隔符之前。解析失败与业务失败都可返回非零；不要只看有没有打印问候而忽略退出码。
