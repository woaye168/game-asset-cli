# AGENTS.md — game-asset-cli

> 面向在本仓库工作的 AI 编程代理。

## 项目简介

游戏资源加工 CLI（音频 / 图像批处理），Rust 单二进制。设计目标：给 AI 代理在游戏开发中处理素材用——单 exe 独立运行，音频压缩格式依赖外置 ffmpeg。

## 目录结构

```
src/
  main.rs       # CLI 入口：clap 命令树定义（TopCommand / AudioCommand / ImageCommand）
  audio.rs      # 音频子命令实现（convert/volume/trim/merge/loop/info）
  image_ops.rs  # 图像子命令实现（convert/resize/crop/trim/pad/slice/pack/chroma/quantize/info）
  util.rs       # 公共工具：ffmpeg 探测与调用（run_ffmpeg/tool_available）、输出路径解析、表格打印
bin/            # 外置 ffmpeg.exe/ffprobe.exe（git 忽略，不入库）
target/         # 构建产物（git 忽略）
```

## 核心约定

- **纯 Rust 内核优先**：能用 symphonia/rubato/hound/image 完成的处理不经过 ffmpeg；ffmpeg 只用于压缩格式（ogg/flac/mp3）的最终转码。
- **ffmpeg 调用纪律**：经 `util::run_ffmpeg` 统一调用；音频转码必须带 `-vn` 剥离视频流——mp3 内嵌封面会被 ffmpeg 编成 theora 混入 ogg，导致游戏引擎无法播放（2026-10-20 实炸修复）。
- **ffmpeg 探测顺序**：`FFMPEG_PATH` 环境变量 → exe 同目录 / 其 `bin/` 子目录 → 系统 PATH。
- **批量处理**：所有加工类子命令接受多输入；单输入可用 `-o`，批量用 `--out-dir`（默认 `output/`）。
- 用户面向文档见 README.md；命令的 about 文案（main.rs）与 README 保持同步。

## 构建与验证

```bash
cargo build --release
```

改动音频转码链路后，用带封面图的 mp3 转 ogg 回归验证：输出文件头部应只含 `vorbis`，不得出现 `theora`。

## 提交规范

Conventional Commits 中文规范：`feat: / fix: / docs: / chore:` 前缀。
