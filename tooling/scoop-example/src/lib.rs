pub mod catalog;
pub mod render;
pub mod search;

use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

pub const DRIVER_NAME: &str = if cfg!(windows) { "sco" } else { "scoop" };
pub const COMMAND_NAME: &str = if cfg!(windows) {
    "sco example"
} else {
    "scoop example"
};

fn examples_help() -> String {
    format!(
        "示例：\n  {COMMAND_NAME} language/option\n  {COMMAND_NAME} --search \"JSON 序列化\"\n  {COMMAND_NAME} http/get --format source --file client.scoop\n\n选择其他安装根：{DRIVER_NAME} --sysroot /path/to/scoop example <topic>\n本命令只读取材料，不编译或运行程序。"
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum Format {
    #[default]
    Markdown,
    Source,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = COMMAND_NAME,
    color = clap::ColorChoice::Never,
    about = "查询配套工具链的完整示例与中文说明",
    after_help = examples_help()
)]
pub struct Args {
    /// 分类、主题 ID 或显式别名
    #[arg(conflicts_with_all = ["list", "search"])]
    pub topic: Option<String>,
    /// 列出全部已发布主题
    #[arg(long, conflicts_with = "search")]
    pub list: bool,
    /// 搜索 ID、标题、摘要、别名与关键词；多个词须全部匹配
    #[arg(long, value_name = "TEXT")]
    pub search: Option<String>,
    /// 输出格式
    #[arg(long, value_enum, default_value = "markdown")]
    pub format: Format,
    /// source 格式下选择完整文件的显示路径
    #[arg(long, value_name = "PATH")]
    pub file: Option<String>,
}

#[derive(Debug)]
pub struct Error {
    pub code: u8,
    pub message: String,
}

impl Error {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }

    pub fn content(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }
}

pub fn installation_root(executable: &Path) -> Result<PathBuf, Error> {
    let commands = executable
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "commands"));
    commands
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Error::content(format!(
                "请先将 scoop-example 安装到 <SYSROOT>/commands，再通过 {COMMAND_NAME} 查询。"
            ))
        })
}

pub fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path)
        .map_err(|e| Error::content(format!("无法读取 {}：{e}", path.display())))
}

pub fn run(root: &Path, args: &Args) -> Result<String, Error> {
    if args.file.is_some() && args.format != Format::Source {
        return Err(Error::usage("--file 仅用于 --format source。"));
    }
    if args.format == Format::Source && args.topic.is_none() {
        return Err(Error::usage("--format source 需要指定一个完整主题。"));
    }
    if args
        .search
        .as_ref()
        .is_some_and(|query| query.trim().is_empty())
    {
        return Err(Error::usage("搜索词不能为空。"));
    }
    let directory = root.join("share/scoop/examples");
    let catalog =
        catalog::Catalog::parse(&read(&directory.join("catalog.toml"))?).map_err(Error::content)?;
    render::render(root, &directory, &catalog, args)
}
