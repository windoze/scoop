## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
${name}
scalars=2, bytes=4
A|中|
```

## 关键说明

只有带 f 前缀的字符串插值；普通字符串中的 `${name}` 保留原文。String 迭代产生 Unicode 标量 Char；StringBuilder 将字符和分隔符累积为新字符串。

"A中" 有两个标量，UTF-8 编码有四个字节。charLength、字节数组 size 及常用索引使用 Long。用户看到的字素簇可能由多个标量组成，例如组合音标；不要把本例的标量计数概括成所有文字的显示宽度。

## 常见误用

不要把字符位置当作 UTF-8 字节偏移，也不要假设遍历 Char 就是在遍历字素簇。处理字素和规范化需选择对应的文本库 API。
