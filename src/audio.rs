use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::util::{print_kv_table, resolve_output, run_ffmpeg, tool_available};
use crate::AudioCommand;

const OUTPUT_EXTS: &[&str] = &["wav", "ogg", "flac", "mp3"];
const NATIVE_EXTS: &[&str] = &["wav"];

pub fn run(cmd: AudioCommand) -> Result<()> {
    match cmd {
        AudioCommand::Convert {
            inputs,
            format,
            output,
            out_dir,
            bitrate,
            sample_rate,
            channels,
        } => convert(&inputs, &format, &output, &out_dir, bitrate, sample_rate, channels),
        AudioCommand::Volume {
            inputs,
            gain,
            factor,
            normalize,
            target_lufs,
            output,
            out_dir,
        } => volume(&inputs, gain, factor, normalize, target_lufs, &output, &out_dir),
        AudioCommand::Trim {
            inputs,
            start,
            duration,
            end,
            fade_in,
            fade_out,
            output,
            out_dir,
        } => trim(&inputs, start, duration, end, fade_in, fade_out, &output, &out_dir),
        AudioCommand::Merge {
            inputs,
            output,
            bitrate,
        } => merge(&inputs, &output, bitrate),
        AudioCommand::Loop {
            inputs,
            crossfade,
            output,
            out_dir,
        } => make_loop(&inputs, crossfade, &output, &out_dir),
        AudioCommand::Info { inputs } => info(&inputs),
    }
}

struct AudioData {
    samples: Vec<Vec<f32>>,
    sample_rate: u32,
}

impl AudioData {
    fn channels(&self) -> usize {
        self.samples.len()
    }
    fn frames(&self) -> usize {
        self.samples.first().map(|c| c.len()).unwrap_or(0)
    }
    fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }
}

fn ext_lower(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn decode(path: &Path) -> Result<AudioData> {
    let file = File::open(path).with_context(|| format!("无法打开音频: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    let ext = ext_lower(path);
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("无法识别音频格式: {}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("未找到音频轨: {}", path.display()))?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| anyhow::anyhow!("无法读取采样率: {}", path.display()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("无法创建解码器")?;

    let mut channels: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => bail!("读取数据包失败: {e}"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => bail!("解码失败: {e}"),
        };
        append_decoded(&decoded, &mut channels);
    }
    if channels.is_empty() || channels[0].is_empty() {
        bail!("未解码出任何音频数据: {}", path.display());
    }
    Ok(AudioData {
        samples: channels,
        sample_rate,
    })
}

fn append_decoded(buf: &AudioBufferRef, channels: &mut Vec<Vec<f32>>) {
    let spec = *buf.spec();
    let frames = buf.frames() as u64;
    if frames == 0 {
        return;
    }
    let mut sb = SampleBuffer::<f32>::new(frames, spec);
    sb.copy_interleaved_ref(buf.clone());
    let n_ch = spec.channels.count();
    if channels.len() != n_ch {
        channels.resize_with(n_ch, Vec::new);
    }
    let data = sb.samples();
    for (i, &s) in data.iter().enumerate() {
        channels[i % n_ch].push(s);
    }
}

fn resample(data: AudioData, target_rate: u32) -> Result<AudioData> {
    if target_rate == data.sample_rate || data.frames() == 0 {
        return Ok(data);
    }
    use rubato::{FftFixedInOut, Resampler};
    let n_ch = data.channels();
    let chunk = 2048usize;
    let ratio = target_rate as f64 / data.sample_rate as f64;
    let mut rs = FftFixedInOut::<f32>::new(data.sample_rate as usize, target_rate as usize, chunk, n_ch)
        .context("无法创建重采样器")?;
    let total = data.frames();
    let mut out: Vec<Vec<f32>> = vec![Vec::new(); n_ch];
    let mut pos = 0usize;
    while pos < total {
        let end = (pos + chunk).min(total);
        let mut block: Vec<Vec<f32>> = Vec::with_capacity(n_ch);
        for ch in 0..n_ch {
            let mut v = data.samples[ch][pos..end].to_vec();
            if v.len() < chunk {
                let last = *v.last().unwrap_or(&0.0);
                v.resize(chunk, last);
            }
            block.push(v);
        }
        let processed = rs
            .process(&block, None)
            .context("重采样失败")?;
        let is_last = end == total;
        for (ch, out_ch) in processed.iter().enumerate() {
            if is_last {
                let remaining_frames = total - pos;
                let want = (remaining_frames as f64 * ratio).round().max(0.0) as usize;
                let take = out_ch.len().min(want);
                out[ch].extend_from_slice(&out_ch[..take]);
            } else {
                out[ch].extend_from_slice(out_ch);
            }
        }
        pos = end;
    }
    Ok(AudioData {
        samples: out,
        sample_rate: target_rate,
    })
}

fn convert_channels(data: AudioData, target: usize) -> Result<AudioData> {
    let src = data.channels();
    if src == target {
        return Ok(data);
    }
    if target == 1 && src == 2 {
        let (l, r) = (data.samples[0].clone(), data.samples[1].clone());
        let n = l.len().min(r.len());
        let mono: Vec<f32> = (0..n).map(|i| (l[i] + r[i]) * 0.5).collect();
        return Ok(AudioData {
            samples: vec![mono],
            sample_rate: data.sample_rate,
        });
    }
    if target == 2 && src == 1 {
        let m = data.samples[0].clone();
        return Ok(AudioData {
            samples: vec![m.clone(), m],
            sample_rate: data.sample_rate,
        });
    }
    bail!("暂不支持 {src} 声道到 {target} 声道的转换");
}

fn encode(data: &AudioData, out: &Path, _bitrate: Option<&str>) -> Result<()> {
    match ext_lower(out).as_str() {
        "wav" => encode_wav(data, out),
        other => bail!(
            "输出格式 {other} 需要 ffmpeg。请将 ffmpeg.exe 放到 game-asset-cli.exe 同目录（或其 bin/ 子目录）、加入 PATH，或设 FFMPEG_PATH"
        ),
    }
}

fn transcode_ffmpeg(
    input: &Path,
    out: &Path,
    bitrate: Option<&str>,
    sample_rate: Option<u32>,
    channels: Option<u8>,
) -> Result<()> {
    // -vn：剥离封面图等视频流，避免 ffmpeg 将其编成 theora 混入 ogg（纯音频工具永不带视频流）
    let mut args = vec!["-y".into(), "-i".into(), input.to_string_lossy().into_owned(), "-vn".into()];
    if let Some(b) = bitrate {
        args.push("-b:a".into());
        args.push(b.into());
    }
    if let Some(sr) = sample_rate {
        args.push("-ar".into());
        args.push(sr.to_string());
    }
    if let Some(c) = channels {
        args.push("-ac".into());
        args.push(c.to_string());
    }
    args.push(out.to_string_lossy().into_owned());
    run_ffmpeg(&args)
}

fn finalize_out(wav: &Path, desired: &Path, bitrate: Option<&str>) -> Result<PathBuf> {
    let dext = ext_lower(desired);
    if dext == "wav" {
        if wav != desired {
            std::fs::rename(wav, desired).ok();
        }
        return Ok(desired.to_path_buf());
    }
    if !tool_available("ffmpeg") {
        bail!("输出 {dext} 需要 ffmpeg（未找到，仅能输出 WAV）。请配置 ffmpeg（见 convert 帮助）");
    }
    transcode_ffmpeg(wav, desired, bitrate, None, None)?;
    std::fs::remove_file(wav).ok();
    Ok(desired.to_path_buf())
}

fn save_processed(
    data: &AudioData,
    input: &Path,
    single: bool,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<PathBuf> {
    let desired = resolve_output(input, single, output, out_dir, &ext_lower(input), None)?;
    let desired = if OUTPUT_EXTS.contains(&ext_lower(&desired).as_str()) {
        desired
    } else {
        desired.with_extension("wav")
    };
    let tmp = std::env::temp_dir().join(format!(
        "gac_{}_{}.wav",
        std::process::id(),
        input.file_stem().and_then(|s| s.to_str()).unwrap_or("out")
    ));
    encode_wav(data, &tmp)?;
    finalize_out(&tmp, &desired, None)
}

fn encode_wav(data: &AudioData, out: &Path) -> Result<()> {
    let spec = hound::WavSpec {
        channels: data.channels() as u16,
        sample_rate: data.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(out, spec)
        .with_context(|| format!("无法创建 WAV 文件: {}", out.display()))?;
    let frames = data.frames();
    for i in 0..frames {
        for ch in &data.samples {
            let v = (ch[i].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(v).context("写入 WAV 采样失败")?;
        }
    }
    writer.finalize().context("完成 WAV 写入失败")?;
    Ok(())
}

fn load_pipeline(input: &Path, sample_rate: Option<u32>, channels: Option<u8>) -> Result<AudioData> {
    let mut data = decode(input)?;
    if let Some(sr) = sample_rate {
        data = resample(data, sr)?;
    }
    if let Some(c) = channels {
        data = convert_channels(data, c as usize)?;
    }
    Ok(data)
}

fn convert(
    inputs: &[PathBuf],
    format: &str,
    output: &Option<PathBuf>,
    out_dir: &Path,
    bitrate: Option<String>,
    sample_rate: Option<u32>,
    channels: Option<u8>,
) -> Result<()> {
    let format = format.to_ascii_lowercase();
    if !OUTPUT_EXTS.contains(&format.as_str()) {
        bail!("不支持的输出格式: {format}。支持: {}", OUTPUT_EXTS.join(", "));
    }
    let single = inputs.len() == 1;
    if NATIVE_EXTS.contains(&format.as_str()) {
        for input in inputs {
            let data = load_pipeline(input, sample_rate, channels)?;
            let out = resolve_output(input, single, output, out_dir, &format, None)?;
            encode(&data, &out, bitrate.as_deref())?;
            println!(
                "已转换: {} ({} Hz, {} ch) -> {}",
                input.display(),
                data.sample_rate,
                data.channels(),
                out.display()
            );
        }
        return Ok(());
    }
    if !tool_available("ffmpeg") {
        bail!("输出 {format} 需要 ffmpeg（当前未找到，仅能输出 WAV）。请将 ffmpeg.exe 放到 game-asset-cli.exe 同目录（或其 bin/ 子目录）、加入 PATH，或设 FFMPEG_PATH");
    }
    for input in inputs {
        let out = resolve_output(input, single, output, out_dir, &format, None)?;
        transcode_ffmpeg(input, &out, bitrate.as_deref(), sample_rate, channels)?;
        println!("已转换: {} -> {}", input.display(), out.display());
    }
    Ok(())
}

fn db_to_factor(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

fn measure_lufs(data: &AudioData) -> f64 {
    let mut sum_sq = 0.0f64;
    let mut count = 0u64;
    for ch in &data.samples {
        for &s in ch {
            let v = s as f64;
            sum_sq += v * v;
            count += 1;
        }
    }
    if count == 0 || sum_sq <= 0.0 {
        return -70.0;
    }
    -0.691 + 10.0 * (sum_sq / count as f64).log10()
}

fn apply_gain(data: &mut AudioData, factor: f64) {
    for ch in &mut data.samples {
        for s in ch.iter_mut() {
            *s = (*s * factor as f32).clamp(-1.0, 1.0);
        }
    }
}

fn volume(
    inputs: &[PathBuf],
    gain: Option<f64>,
    factor: Option<f64>,
    normalize: bool,
    target_lufs: f64,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let base_factor = match (gain, factor) {
        (Some(_), Some(_)) => bail!("--gain 与 --factor 只能二选一"),
        (Some(g), None) => Some(db_to_factor(g)),
        (None, Some(f)) => Some(f),
        (None, None) => None,
    };
    if base_factor.is_none() && !normalize {
        bail!("请指定 --gain、--factor 或 --normalize 中的至少一项");
    }
    let single = inputs.len() == 1;
    for input in inputs {
        let mut data = load_pipeline(input, None, None)?;
        if let Some(f) = base_factor {
            apply_gain(&mut data, f);
        }
        if normalize {
            let cur = measure_lufs(&data);
            let diff = target_lufs - cur;
            apply_gain(&mut data, db_to_factor(diff));
        }
        let out = save_processed(&data, input, single, output, out_dir)?;
        println!("已调整音量: {} -> {}", input.display(), out.display());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn trim(
    inputs: &[PathBuf],
    start: f64,
    duration: Option<f64>,
    end: Option<f64>,
    fade_in: Option<f64>,
    fade_out: Option<f64>,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    if duration.is_some() && end.is_some() {
        bail!("--duration 与 --end 只能二选一");
    }
    if start < 0.0 {
        bail!("--start 不能为负数");
    }
    let single = inputs.len() == 1;
    for input in inputs {
        let data = load_pipeline(input, None, None)?;
        let sr = data.sample_rate as f64;
        let total = data.frames();
        let start_f = (start * sr).round() as usize;
        if start_f >= total {
            bail!("--start {start}s 超出音频时长 {:.2}s", data.duration());
        }
        let end_f = match (duration, end) {
            (Some(d), None) => (start_f as f64 + d * sr).round() as usize,
            (None, Some(e)) => {
                if e <= start {
                    bail!("--end 必须大于 --start");
                }
                ((e * sr).round() as usize).min(total)
            }
            (None, None) => total,
            _ => unreachable!(),
        }
        .min(total);
        let seg: Vec<Vec<f32>> = data
            .samples
            .iter()
            .map(|ch| ch[start_f..end_f].to_vec())
            .collect();
        let mut seg_data = AudioData {
            samples: seg,
            sample_rate: data.sample_rate,
        };
        let seg_len = seg_data.duration();
        if let Some(fi) = fade_in {
            if fi > 0.0 {
                let n = ((fi * sr).round() as usize).min(seg_data.frames());
                for ch in &mut seg_data.samples {
                    for (i, s) in ch.iter_mut().enumerate().take(n) {
                        *s *= i as f32 / n.max(1) as f32;
                    }
                }
            }
        }
        if let Some(fo) = fade_out {
            if fo > 0.0 {
                if fo >= seg_len {
                    bail!("--fade-out 不能大于等于截取时长");
                }
                let n = (fo * sr).round() as usize;
                let frames = seg_data.frames();
                for ch in &mut seg_data.samples {
                    for k in 0..n {
                        let idx = frames - n + k;
                        ch[idx] *= 1.0 - (k as f32 / n.max(1) as f32);
                    }
                }
            }
        }
        let out = save_processed(&seg_data, input, single, output, out_dir)?;
        println!("已截取: {} -> {}", input.display(), out.display());
    }
    Ok(())
}

fn merge(inputs: &[PathBuf], output: &Path, bitrate: Option<String>) -> Result<()> {
    if inputs.len() < 2 {
        bail!("merge 至少需要 2 个输入文件");
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建输出目录: {}", parent.display()))?;
        }
    }
    let mut parts: Vec<AudioData> = Vec::with_capacity(inputs.len());
    for input in inputs {
        parts.push(decode(input)?);
    }
    let target_sr = parts[0].sample_rate;
    let target_ch = parts[0].channels();
    let mut merged: Vec<Vec<f32>> = vec![Vec::new(); target_ch];
    for part in parts {
        let mut p = part;
        if p.sample_rate != target_sr {
            p = resample(p, target_sr)?;
        }
        if p.channels() != target_ch {
            p = convert_channels(p, target_ch)?;
        }
        for (c, ch) in p.samples.into_iter().enumerate() {
            merged[c].extend_from_slice(&ch);
        }
    }
    let data = AudioData {
        samples: merged,
        sample_rate: target_sr,
    };
    let tmp = std::env::temp_dir().join(format!("gac_{}_merge.wav", std::process::id()));
    encode_wav(&data, &tmp)?;
    let out = finalize_out(&tmp, output, bitrate.as_deref())?;
    println!(
        "已拼接 {} 个音频（{:.2} 秒）-> {}",
        inputs.len(),
        data.duration(),
        out.display()
    );
    Ok(())
}

fn make_loop(
    inputs: &[PathBuf],
    crossfade: f64,
    output: &Option<PathBuf>,
    out_dir: &Path,
) -> Result<()> {
    let single = inputs.len() == 1;
    for input in inputs {
        let mut data = load_pipeline(input, None, None)?;
        if crossfade > 0.0 {
            let sr = data.sample_rate as f64;
            let n = ((crossfade * sr).round() as usize).min(data.frames() / 2);
            if n > 0 {
                let frames = data.frames();
                for ch in &mut data.samples {
                    for k in 0..n {
                        let t = k as f32 / n as f32;
                        let head = ch[k];
                        let tail = ch[frames - n + k];
                        ch[k] = head * t + tail * (1.0 - t);
                    }
                    ch.truncate(frames - n);
                }
            }
        }
        let out = save_processed(&data, input, single, output, out_dir)?;
        println!("已制作循环: {} -> {}", input.display(), out.display());
    }
    Ok(())
}

fn info(inputs: &[PathBuf]) -> Result<()> {
    for input in inputs {
        let ext = ext_lower(input);
        let data = decode(input)?;
        let meta = std::fs::metadata(input).ok();
        let kbps = meta
            .as_ref()
            .map(|m| (m.len() as f64 * 8.0) / data.duration().max(0.001) / 1000.0);
        println!("文件: {}", input.display());
        let rows = vec![
            ("容器/格式".to_string(), if ext.is_empty() { "-".into() } else { ext }),
            ("采样率".to_string(), format!("{} Hz", data.sample_rate)),
            ("声道".to_string(), format!("{}", data.channels())),
            ("时长".to_string(), {
                let d = data.duration();
                let mins = (d as u64) / 60;
                let secs = d - (mins * 60) as f64;
                format!("{d:.3} 秒 ({mins:02}:{secs:05.2})")
            }),
            ("估算码率".to_string(), kbps.map(|b| format!("{b:.0} kbps")).unwrap_or_else(|| "-".into())),
            ("文件大小".to_string(), meta.map(|m| human_size(m.len() as f64)).unwrap_or_else(|| "-".into())),
        ];
        print_kv_table(&rows);
        println!();
    }
    Ok(())
}

fn human_size(bytes: f64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
