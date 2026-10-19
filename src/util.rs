use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

pub fn resolve_output(
    input: &Path,
    single_input: bool,
    output: &Option<PathBuf>,
    out_dir: &Path,
    new_ext: &str,
    suffix: Option<&str>,
) -> Result<PathBuf> {
    if let Some(out) = output {
        if !single_input {
            bail!("--output 仅在传入单个输入文件时可用，批量处理请使用 --out-dir");
        }
        if let Some(parent) = out.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("无法创建输出目录: {}", parent.display()))?;
            }
        }
        return Ok(out.clone());
    }
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("无法创建输出目录: {}", out_dir.display()))?;
    let stem = input
        .file_stem()
        .ok_or_else(|| anyhow!("无法解析文件名: {}", input.display()))?
        .to_string_lossy();
    let name = match suffix {
        Some(s) if !s.is_empty() => format!("{stem}{s}.{new_ext}"),
        _ => format!("{stem}.{new_ext}"),
    };
    Ok(out_dir.join(name))
}

pub fn print_kv_table(rows: &[(String, String)]) {
    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    for (k, v) in rows {
        println!("  {:<width$} : {}", k, v, width = width);
    }
}

fn tool_path(name: &str) -> PathBuf {
    let env_key = format!("{}_PATH", name.to_ascii_uppercase());
    if let Ok(p) = std::env::var(&env_key) {
        let p = PathBuf::from(p);
        if p.is_file() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(4) {
            for candidate in [
                ancestor.join("bin").join(format!("{name}.exe")),
                ancestor.join(format!("{name}.exe")),
            ] {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from(name)
}

pub fn tool_available(name: &str) -> bool {
    std::process::Command::new(tool_path(name))
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn run_ffmpeg(args: &[String]) -> Result<()> {
    let output = std::process::Command::new(tool_path("ffmpeg"))
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("无法启动 ffmpeg 进程")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().collect();
        let start = tail.len().saturating_sub(8);
        bail!("ffmpeg 执行失败:\n{}", tail[start..].join("\n"));
    }
    Ok(())
}
