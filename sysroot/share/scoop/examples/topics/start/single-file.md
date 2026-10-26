## 运行

在保存文件的目录执行：

```sh
scoop run --release typed.scoop
```

## 预期结果

程序 stdout：

```text
{"name":"Ada","scores":[7,9]}
Ada: 9
no note
```

## 关键说明

`//! dep: scoop:scoop.json:0.1.0` 是依赖声明，必须出现在语言 token 之前。坐标包含 group、name、精确版本；此处使用配套系统库。单文件入口默认有 core/std，额外的 JSON 库仍需声明。

`import scoop.json.*` 把库里的名称引入当前文件；依赖声明和 import 分工不同，不能相互替代。单文件模式不读取相邻 Cone.toml；要使用 manifest 的依赖和多个文件，请改用 Cone 入口。JSON codec 的解释见 `json/typed`。

## 常见误用

不要只写 import 而省略额外依赖，也不要把依赖头放在 struct 或 import 之后。无需将父库源码复制到程序目录。
