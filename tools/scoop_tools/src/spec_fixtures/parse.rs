//! Markdown fenced code block 解析（极简实现）。
//!
//! 说明：
//! - 不依赖完整 Markdown 解析器（减少依赖、降低维护成本）
//! - 仅支持标准的 fenced code block：以 ``` 开始，以 ``` 结束
//! - 在 code block 内查找 `// FIXTURE:` 指令，作为是否抽取的开关

use std::path::PathBuf;

use miette::{miette, Result};

use super::GeneratedFixture;

pub fn extract(spec_text: &str) -> Result<Vec<GeneratedFixture>> {
    let mut fixtures = Vec::new();

    let mut in_block = false;
    let mut block_lines: Vec<&str> = Vec::new();

    for line in spec_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_block {
                // close
                let fixture = parse_block(&block_lines)?;
                if let Some(f) = fixture {
                    fixtures.push(f);
                }
                block_lines.clear();
                in_block = false;
            } else {
                // open（忽略 info string）
                in_block = true;
            }
            continue;
        }

        if in_block {
            block_lines.push(line);
        }
    }

    if in_block {
        return Err(miette!("规范文件存在未闭合的 fenced code block（缺少 ```）"));
    }

    fixtures.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(fixtures)
}

fn parse_block(lines: &[&str]) -> Result<Option<GeneratedFixture>> {
    let mut fixture_path: Option<PathBuf> = None;

    for line in lines.iter().take(64) {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            break;
        }
        let directive = trimmed.trim_start_matches("//").trim();
        if let Some(rest) = directive.strip_prefix("FIXTURE:") {
            fixture_path = Some(PathBuf::from(rest.trim()));
            break;
        }
    }

    let Some(rel_path) = fixture_path else {
        return Ok(None);
    };

    let mut content = String::new();
    for l in lines {
        content.push_str(l);
        content.push('\n');
    }

    Ok(Some(GeneratedFixture { rel_path, content }))
}

