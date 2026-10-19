use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::util::{print_kv_table, resolve_output};
use crate::ImageCommand;

pub fn run(cmd: ImageCommand) -> Result<()> {
    match cmd {
        ImageCommand::Convert {
            inputs,
            format,
            quality,
            output,
            out_dir,
        } => convert(&inputs, &format, quality, &output, &out_dir),
        ImageCommand::Resize {
            inputs,
            width,
            height,
            scale,
            filter,
            output,
            out_dir,
        } => resize(&inputs, width, height, scale, &filter, &output, &out_dir),
        ImageCommand::Crop {
            inputs,
            x,
            y,
            width,
            height,
            output,
            out_dir,
        } => crop(&inputs, x, y, width, height, &output, &out_dir),
        ImageCommand::TrimAlpha {
            inputs,
            threshold,
            padding,
            output,
            out_dir,
        } => trim_alpha(&inputs, threshold, padding, &output, &out_dir),
        ImageCommand::Pad {
            inputs,
            width,
            height,
            pot,
            anchor,
            background,
            output,
            out_dir,
        } => pad(&inputs, width, height, pot, &anchor, background, &output, &out_dir),
        ImageCommand::Slice {
            input,
            tile_width,
            tile_height,
            cols,
            rows,
            margin,
            spacing,
            out_dir,
            prefix,
        } => slice(
            &input, tile_width, tile_height, cols, rows, margin, spacing, &out_dir, &prefix,
        ),
        ImageCommand::RemoveColor {
            inputs,
            color,
            tolerance,
            soften,
            output,
            out_dir,
        } => remove_color(&inputs, color, tolerance, soften, &output, &out_dir),
        ImageCommand::Pack {
            inputs,
            output,
            cols,
            spacing,
            margin,
            background,
            cell,
            json,
        } => pack(&inputs, &output, cols, spacing, margin, background, cell, json),
        ImageCommand::Quantize {
            inputs,
            colors,
            dither,
            output,
            out_dir,
        } => quantize(&inputs, colors, dither, &output, &out_dir),
        ImageCommand::Info { inputs } => info(&inputs),
    }
}

fn open_image(path: &Path) -> Result<DynamicImage> {
    image::ImageReader::open(path)
        .with_context(|| format!("无法打开图像: {}", path.display()))?
        .decode()
        .with_context(|| format!("无法解码图像: {}", path.display()))
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_else(|| "png".into())
}

fn save_with(img: &DynamicImage, out: &Path, quality: Option<u8>) -> Result<()> {
    let ext = ext_of(out);
    match ext.as_str() {
        "jpg" | "jpeg" => {
            let mut file = std::fs::File::create(out)
                .with_context(|| format!("无法创建文件: {}", out.display()))?;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut file,
                quality.unwrap_or(90),
            );
            img.to_rgb8()
                .write_with_encoder(encoder)
                .context("JPEG 编码失败")?;
        }
        "webp" => {
            let file = std::fs::File::create(out)
                .with_context(|| format!("无法创建文件: {}", out.display()))?;
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(file);
            img.write_with_encoder(encoder).context("WebP 编码失败")?;
        }
        _ => {
            img.save(out)
                .with_context(|| format!("保存失败: {}", out.display()))?;
        }
    }
    Ok(())
}

fn parse_hex_color(s: &str) -> Result<Rgba<u8>> {
    let hex = s.trim_start_matches('#');
    let parse = |range: std::ops::Range<usize>| -> Result<u8> {
        u8::from_str_radix(&hex[range], 16).map_err(|_| anyhow!("无效颜色值: {s}，应为 RRGGBB 或 RRGGBBAA"))
    };
    if hex.len() == 6 {
        Ok(Rgba([parse(0..2)?, parse(2..4)?, parse(4..6)?, 255]))
    } else if hex.len() == 8 {
        Ok(Rgba([parse(0..2)?, parse(2..4)?, parse(4..6)?, parse(6..8)?]))
    } else {
        bail!("无效颜色值: {s}，应为 RRGGBB 或 RRGGBBAA")
    }
}

fn parse_filter(name: &str) -> Result<FilterType> {
    match name.to_ascii_lowercase().as_str() {
        "nearest" => Ok(FilterType::Nearest),
        "triangle" => Ok(FilterType::Triangle),
        "catmullrom" => Ok(FilterType::CatmullRom),
        "gaussian" => Ok(FilterType::Gaussian),
        "lanczos3" => Ok(FilterType::Lanczos3),
        _ => bail!("未知采样算法: {name}。支持: nearest、triangle、catmullrom、gaussian、lanczos3"),
    }
}

fn out_path(
    input: &Path,
    single: bool,
    output: &Option<PathBuf>,
    out_dir: &Path,
    ext: &str,
) -> Result<PathBuf> {
    resolve_output(input, single, output, out_dir, ext, None)
}

fn convert(
    inputs: &[PathBuf],
    format: &str,
    quality: Option<u8>,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    const FORMATS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tga", "ico", "gif", "pnm"];
    if !FORMATS.contains(&format.to_ascii_lowercase().as_str()) {
        bail!("不支持的图像格式: {format}。支持: {}", FORMATS.join(", "));
    }
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let out = out_path(input, single, output, out_dir, format)?;
        save_with(&img, &out, quality)?;
        println!("已转换: {} -> {}", input.display(), out.display());
    }
    Ok(())
}

fn resize(
    inputs: &[PathBuf],
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f64>,
    filter: &str,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let filter = parse_filter(filter)?;
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let (w, h) = img.dimensions();
        let resized = if let Some(s) = scale {
            if s <= 0.0 {
                bail!("--scale 必须大于 0");
            }
            let nw = ((w as f64) * s).round().max(1.0) as u32;
            let nh = ((h as f64) * s).round().max(1.0) as u32;
            img.resize_exact(nw, nh, filter)
        } else {
            match (width, height) {
                (Some(tw), Some(th)) => img.resize_exact(tw, th, filter),
                (Some(tw), None) => {
                    let nh = ((h as f64) * (tw as f64) / (w as f64)).round().max(1.0) as u32;
                    img.resize_exact(tw, nh, filter)
                }
                (None, Some(th)) => {
                    let nw = ((w as f64) * (th as f64) / (h as f64)).round().max(1.0) as u32;
                    img.resize_exact(nw, th, filter)
                }
                (None, None) => bail!("请指定 --width、--height 或 --scale 中的至少一项"),
            }
        };
        let ext = output.clone().map(|_| ()).map(|_| ext_of(output.as_ref().unwrap()));
        let ext = ext.unwrap_or_else(|| ext_of(input));
        let out = out_path(input, single, output, out_dir, &ext)?;
        save_with(&resized, &out, None)?;
        println!(
            "已缩放: {} ({}x{}) -> {} ({}x{})",
            input.display(),
            w,
            h,
            out.display(),
            resized.width(),
            resized.height()
        );
    }
    Ok(())
}

fn crop(
    inputs: &[PathBuf],
    x: u32,
    y: u32,
    width: Option<u32>,
    height: Option<u32>,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let (w, h) = img.dimensions();
        let cw = width.unwrap_or(w.saturating_sub(x));
        let ch = height.unwrap_or(h.saturating_sub(y));
        if x >= w || y >= h {
            bail!("裁剪起点 ({x},{y}) 超出图像范围 {w}x{h}: {}", input.display());
        }
        let cw = cw.min(w - x);
        let ch = ch.min(h - y);
        let cropped = img.crop_imm(x, y, cw, ch);
        let out = out_path(input, single, output, out_dir, &ext_of(input))?;
        save_with(&cropped, &out, None)?;
        println!("已裁剪: {} -> {} ({}x{} @ {},{})", input.display(), out.display(), cw, ch, x, y);
    }
    Ok(())
}

fn trim_alpha(
    inputs: &[PathBuf],
    threshold: u8,
    padding: u32,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut min_x = w;
        let mut min_y = h;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;
        for (x, y, p) in rgba.enumerate_pixels() {
            if p[3] > threshold {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        if !found {
            bail!("图像全部为透明像素（阈值 {threshold}）: {}", input.display());
        }
        min_x = min_x.saturating_sub(padding);
        min_y = min_y.saturating_sub(padding);
        max_x = (max_x + padding).min(w - 1);
        max_y = (max_y + padding).min(h - 1);
        let cropped = img.crop_imm(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1);
        let out = out_path(input, single, output, out_dir, &ext_of(input))?;
        save_with(&cropped, &out, None)?;
        println!(
            "已裁剪透明边缘: {} ({}x{}) -> {} ({}x{})",
            input.display(),
            w,
            h,
            out.display(),
            cropped.width(),
            cropped.height()
        );
    }
    Ok(())
}

fn next_pot(v: u32) -> u32 {
    v.max(1).next_power_of_two()
}

fn anchor_offset(anchor: &str, cw: u32, ch: u32, iw: u32, ih: u32) -> Result<(i64, i64)> {
    let cx = ((cw - iw) / 2) as i64;
    let cy = ((ch - ih) / 2) as i64;
    let right = (cw - iw) as i64;
    let bottom = (ch - ih) as i64;
    match anchor.to_ascii_lowercase().as_str() {
        "center" => Ok((cx, cy)),
        "top-left" => Ok((0, 0)),
        "top" => Ok((cx, 0)),
        "top-right" => Ok((right, 0)),
        "left" => Ok((0, cy)),
        "right" => Ok((right, cy)),
        "bottom-left" => Ok((0, bottom)),
        "bottom" => Ok((cx, bottom)),
        "bottom-right" => Ok((right, bottom)),
        _ => bail!("未知锚点: {anchor}。支持: center、top-left、top、top-right、left、right、bottom-left、bottom、bottom-right"),
    }
}

fn pad(
    inputs: &[PathBuf],
    width: Option<u32>,
    height: Option<u32>,
    pot: bool,
    anchor: &str,
    background: Option<String>,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let bg = match &background {
        Some(c) => parse_hex_color(c)?,
        None => Rgba([0, 0, 0, 0]),
    };
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let (iw, ih) = img.dimensions();
        let (cw, ch) = if pot {
            (next_pot(iw), next_pot(ih))
        } else {
            match (width, height) {
                (Some(w), Some(h)) => (w, h),
                (Some(w), None) => (w, ih),
                (None, Some(h)) => (iw, h),
                (None, None) => bail!("请指定 --width/--height 或 --pot"),
            }
        };
        if cw < iw || ch < ih {
            bail!("目标画布 {cw}x{ch} 小于原图 {iw}x{ih}（pad 只扩展不裁剪）: {}", input.display());
        }
        let (dx, dy) = anchor_offset(anchor, cw, ch, iw, ih)?;
        let mut canvas = RgbaImage::from_pixel(cw, ch, bg);
        image::imageops::overlay(&mut canvas, &img.to_rgba8(), dx, dy);
        let out = out_path(input, single, output, out_dir, &ext_of(input))?;
        DynamicImage::ImageRgba8(canvas)
            .save(&out)
            .with_context(|| format!("保存失败: {}", out.display()))?;
        println!("已扩展画布: {} ({}x{}) -> {} ({}x{})", input.display(), iw, ih, out.display(), cw, ch);
    }
    Ok(())
}

fn slice(
    input: &Path,
    tile_width: Option<u32>,
    tile_height: Option<u32>,
    cols: Option<u32>,
    rows: Option<u32>,
    margin: u32,
    spacing: u32,
    out_dir: &Path,
    prefix: &str,
) -> Result<()> {
    let img = open_image(input)?;
    let (w, h) = img.dimensions();
    let inner_w = w.saturating_sub(margin * 2);
    let inner_h = h.saturating_sub(margin * 2);
    if inner_w == 0 || inner_h == 0 {
        bail!("外边距 {margin} 过大，图像 {w}x{h} 去除边距后无有效区域");
    }
    let tw = match (tile_width, cols) {
        (Some(tw), _) => tw,
        (None, Some(c)) => {
            let total_spacing = spacing.saturating_mul(c.saturating_sub(1));
            if c == 0 || inner_w <= total_spacing || (inner_w - total_spacing) % c != 0 {
                bail!("有效宽度 {inner_w}（去除边距与间距后）无法按 {c} 列均分");
            }
            (inner_w - total_spacing) / c
        }
        (None, None) => unreachable!(),
    };
    let th = match (tile_height, rows) {
        (Some(th), _) => th,
        (None, Some(r)) => {
            let total_spacing = spacing.saturating_mul(r.saturating_sub(1));
            if r == 0 || inner_h <= total_spacing || (inner_h - total_spacing) % r != 0 {
                bail!("有效高度 {inner_h}（去除边距与间距后）无法按 {r} 行均分");
            }
            (inner_h - total_spacing) / r
        }
        (None, None) => unreachable!(),
    };
    if tw == 0 || th == 0 {
        bail!("瓦片尺寸必须大于 0");
    }
    let step_x = tw + spacing;
    let step_y = th + spacing;
    let n_cols = (inner_w + spacing) / step_x;
    let n_rows = (inner_h + spacing) / step_y;
    if n_cols == 0 || n_rows == 0 {
        bail!("瓦片尺寸 {tw}x{th}（含间距 {spacing}）大于有效区域 {inner_w}x{inner_h}");
    }
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("无法创建输出目录: {}", out_dir.display()))?;
    let mut count = 0;
    for row in 0..n_rows {
        for col in 0..n_cols {
            let x = margin + col * step_x;
            let y = margin + row * step_y;
            let tile = img.crop_imm(x, y, tw, th);
            let out = out_dir.join(format!("{prefix}_{row}_{col}.png"));
            tile.save(&out).with_context(|| format!("保存失败: {}", out.display()))?;
            count += 1;
        }
    }
    let used_w = margin * 2 + n_cols * step_x - spacing;
    let used_h = margin * 2 + n_rows * step_y - spacing;
    if used_w != w || used_h != h {
        println!("提示: 图像尺寸 {w}x{h} 切割后右侧/底部有剩余区域（已使用 {used_w}x{used_h}），剩余部分已忽略");
    }
    println!("已切割 {} ({}x{}) 为 {} 个瓦片（{} 行 x {} 列，单个 {}x{}，边距 {}，间距 {}）-> {}",
        input.display(), w, h, count, n_rows, n_cols, tw, th, margin, spacing, out_dir.display());
    Ok(())
}

fn remove_color(
    inputs: &[PathBuf],
    color: Option<String>,
    tolerance: u8,
    soften: bool,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let mut rgba = img.to_rgba8();
        let target = match &color {
            Some(c) => parse_hex_color(c)?,
            None => *rgba.get_pixel(0, 0),
        };
        let tol = tolerance as i32;
        for p in rgba.pixels_mut() {
            let dr = p[0] as i32 - target[0] as i32;
            let dg = p[1] as i32 - target[1] as i32;
            let db = p[2] as i32 - target[2] as i32;
            let dist = ((dr * dr + dg * dg + db * db) as f64).sqrt();
            if dist <= tol as f64 {
                p[3] = 0;
            } else if soften && dist <= (tol as f64) * 2.0 {
                let ratio = (dist - tol as f64) / tol.max(1) as f64;
                let alpha = (ratio.clamp(0.0, 1.0) * 255.0).round() as u8;
                p[3] = p[3].min(alpha);
            }
        }
        let out = out_path(input, single, output, out_dir, "png")?;
        DynamicImage::ImageRgba8(rgba)
            .save(&out)
            .with_context(|| format!("保存失败: {}", out.display()))?;
        println!(
            "已抠图: {} -> {}（颜色 #{:02X}{:02X}{:02X}，容差 {}）",
            input.display(), out.display(), target[0], target[1], target[2], tolerance
        );
    }
    Ok(())
}

fn pack(
    inputs: &[PathBuf],
    output: &Path,
    cols: Option<u32>,
    spacing: u32,
    margin: u32,
    background: Option<String>,
    cell: Option<u32>,
    json: bool,
) -> Result<()> {
    let n = inputs.len();
    if n == 0 {
        bail!("pack 至少需要 1 个输入文件");
    }
    let mut imgs = Vec::with_capacity(n);
    let mut max_side = 0u32;
    for input in inputs {
        let img = open_image(input)?;
        let (w, h) = img.dimensions();
        max_side = max_side.max(w).max(h);
        imgs.push(img);
    }
    let cell_size = cell.unwrap_or(max_side).max(1);
    let cols = cols.unwrap_or_else(|| (n as f64).sqrt().ceil() as u32).max(1);
    let rows = ((n as u32) + cols - 1) / cols;
    let width = margin * 2 + cols * cell_size + (cols - 1) * spacing;
    let height = margin * 2 + rows * cell_size + (rows - 1) * spacing;
    let bg = match &background {
        Some(c) => parse_hex_color(c)?,
        None => Rgba([0, 0, 0, 0]),
    };
    let mut canvas = RgbaImage::from_pixel(width, height, bg);
    let mut frames: Vec<serde_json::Value> = Vec::with_capacity(n);
    for (i, img) in imgs.iter().enumerate() {
        let col = i as u32 % cols;
        let row = i as u32 / cols;
        let x = margin + col * (cell_size + spacing);
        let y = margin + row * (cell_size + spacing);
        let scaled = if img.width() != cell_size || img.height() != cell_size {
            img.resize_exact(cell_size, cell_size, FilterType::Lanczos3)
        } else {
            img.clone()
        };
        image::imageops::overlay(&mut canvas, &scaled.to_rgba8(), x as i64, y as i64);
        frames.push(serde_json::json!({
            "file": inputs[i].file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            "x": x, "y": y, "w": cell_size, "h": cell_size,
        }));
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建输出目录: {}", parent.display()))?;
        }
    }
    DynamicImage::ImageRgba8(canvas)
        .save(output)
        .with_context(|| format!("保存失败: {}", output.display()))?;
    println!(
        "已打包 {} 张图 -> {}（{}x{}，{} 行 x {} 列，单元 {}px，间距 {}，边距 {}）",
        n, output.display(), width, height, rows, cols, cell_size, spacing, margin
    );
    if json {
        let meta = serde_json::json!({
            "image": output.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            "width": width, "height": height,
            "tile": cell_size, "cols": cols, "rows": rows,
            "spacing": spacing, "margin": margin,
            "frames": frames,
        });
        let json_path = output.with_extension("json");
        std::fs::write(&json_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("无法写入元数据: {}", json_path.display()))?;
        println!("已输出元数据: {}", json_path.display());
    }
    Ok(())
}

fn quantize(
    inputs: &[PathBuf],
    colors: u32,
    dither: bool,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let colors = colors.clamp(2, 256);
    let levels = ((colors as f64).cbrt().round().max(2.0)) as u32;
    let single = inputs.len() == 1;
    for input in inputs {
        let img = open_image(input)?;
        let mut rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let step = 255.0 / (levels - 1) as f32;
        let mut err = vec![[0f32; 3]; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let p = rgba.get_pixel_mut(x, y);
                let mut new_rgb = [0u8; 3];
                for c in 0..3 {
                    let v = p[c] as f32 + err[idx][c];
                    let snapped = (v / step).round().clamp(0.0, (levels - 1) as f32) * step;
                    new_rgb[c] = snapped.clamp(0.0, 255.0) as u8;
                    let e = v - snapped;
                    if dither {
                        if x + 1 < w { err[idx + 1][c] += e * 7.0 / 16.0; }
                        if y + 1 < h {
                            if x > 0 { err[idx + w as usize - 1][c] += e * 3.0 / 16.0; }
                            err[idx + w as usize][c] += e * 5.0 / 16.0;
                            if x + 1 < w { err[idx + w as usize + 1][c] += e * 1.0 / 16.0; }
                        }
                    }
                }
                p[0] = new_rgb[0];
                p[1] = new_rgb[1];
                p[2] = new_rgb[2];
            }
        }
        let out = out_path(input, single, output, out_dir, "png")?;
        DynamicImage::ImageRgba8(rgba)
            .save(&out)
            .with_context(|| format!("保存失败: {}", out.display()))?;
        println!("已减色到 ≤{} 色: {} -> {}", colors, input.display(), out.display());
    }
    Ok(())
}

fn info(inputs: &[PathBuf]) -> Result<()> {
    for input in inputs {
        let img = open_image(input)?;
        let (w, h) = img.dimensions();
        let meta = std::fs::metadata(input).ok();
        println!("文件: {}", input.display());
        let rows = vec![
            ("尺寸".to_string(), format!("{w} x {h} px")),
            ("色彩类型".to_string(), format!("{:?}", img.color())),
            ("透明通道".to_string(), if img.color().has_alpha() { "有".into() } else { "无".into() }),
            ("2 的幂尺寸".to_string(), if w.is_power_of_two() && h.is_power_of_two() { "是".into() } else { "否".into() }),
            ("文件大小".to_string(), meta.map(|m| {
                let kb = m.len() as f64 / 1024.0;
                if kb >= 1024.0 { format!("{:.1} MB", kb / 1024.0) } else { format!("{kb:.1} KB") }
            }).unwrap_or_else(|| "-".into())),
        ];
        print_kv_table(&rows);
        println!();
    }
    Ok(())
}
