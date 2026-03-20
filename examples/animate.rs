use raden::{Circle, Context, Image, PipelineRuntime, PixelFormat, Rgba32};
use shiguredo_mp4::TrackKind;
use shiguredo_mp4::Uint;
use shiguredo_mp4::boxes::{Avc1Box, AvccBox, SampleEntry, VisualSampleEntryFields};
use shiguredo_mp4::mux::{
    Mp4FileMuxer, Mp4FileMuxerOptions, Sample, estimate_maximum_moov_box_size,
};
use shiguredo_openh264::{
    EncodeOptions, EncodedFrame, Encoder, EncoderConfig, Openh264Library, Profile, RateControlMode,
};
use std::env;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::num::NonZeroU32;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const FPS: u32 = 30;
const DURATION_SECS: u32 = 5;
const TOTAL_FRAMES: u32 = FPS * DURATION_SECS;
const TIMESCALE: NonZeroU32 = NonZeroU32::MIN.saturating_add(FPS - 1);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let openh264_path =
        env::var("OPENH264_PATH").expect("OPENH264_PATH environment variable is required");

    let lib = Openh264Library::load(&openh264_path)?;
    let config = EncoderConfig {
        profile: Some(Profile::ConstrainedBaseline),
        intra_period: Some(30),
        rate_control_mode: Some(RateControlMode::Bitrate),
        ..EncoderConfig::new(WIDTH as usize, HEIGHT as usize, 1_000_000, FPS as usize, 1)
    };
    let mut encoder = Encoder::new(lib, config)?;

    let mut runtime = PipelineRuntime::new();
    let mut img = Image::new(WIDTH, HEIGHT, PixelFormat::Prgb32);

    let w = WIDTH as usize;
    let h = HEIGHT as usize;
    let mut y_buf = vec![0u8; w * h];
    let mut u_buf = vec![0u8; w.div_ceil(2) * h.div_ceil(2)];
    let mut v_buf = vec![0u8; w.div_ceil(2) * h.div_ceil(2)];

    let mut encoded_frames: Vec<EncodedFrame> = Vec::with_capacity(TOTAL_FRAMES as usize);

    let mut ball_x: f64 = 100.0;
    let mut ball_y: f64 = 100.0;
    let mut vel_x: f64 = 4.0;
    let mut vel_y: f64 = 3.0;
    let ball_radius: f64 = 30.0;

    for frame in 0..TOTAL_FRAMES {
        // raden で描画
        {
            let mut ctx = Context::new(&mut img, &mut runtime);

            // 紺色の背景
            ctx.set_fill_style(Rgba32::rgb(0, 0, 128));
            ctx.fill_all();

            // フレームごとに色相を変化させる円
            let hue = (frame as f64 / TOTAL_FRAMES as f64) * 360.0;
            let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
            ctx.set_fill_style(Rgba32::rgb(r, g, b));
            ctx.fill_circle(&Circle::new(ball_x, ball_y, ball_radius));

            ctx.end();
        }

        // ボール位置を更新（バウンド処理）
        ball_x += vel_x;
        ball_y += vel_y;
        if ball_x - ball_radius < 0.0 || ball_x + ball_radius > WIDTH as f64 {
            vel_x = -vel_x;
            ball_x = ball_x.clamp(ball_radius, WIDTH as f64 - ball_radius);
        }
        if ball_y - ball_radius < 0.0 || ball_y + ball_radius > HEIGHT as f64 {
            vel_y = -vel_y;
            ball_y = ball_y.clamp(ball_radius, HEIGHT as f64 - ball_radius);
        }

        // Prgb32 → I420 変換
        prgb32_to_i420(img.data(), w, h, &mut y_buf, &mut u_buf, &mut v_buf);

        // エンコード
        if let Some(encoded) = encoder.encode(&y_buf, &u_buf, &v_buf, &EncodeOptions::default())? {
            encoded_frames.push(encoded);
        }
    }

    // 最初の IDR フレームから SPS/PPS を取得
    let (sps, pps) = extract_sps_pps(&encoded_frames)?;

    // MP4 ファイルに書き出し
    write_mp4("output.mp4", &encoded_frames, &sps, &pps)?;

    eprintln!(
        "output.mp4 を生成しました ({} フレーム)",
        encoded_frames.len()
    );

    Ok(())
}

/// Prgb32 (premultiplied ARGB) から I420 (YUV 4:2:0) に変換する
///
/// Prgb32 のメモリレイアウト (リトルエンディアン):
///   byte[0]=B, byte[1]=G, byte[2]=R, byte[3]=A (各値は premultiplied)
fn prgb32_to_i420(
    prgb: &[u8],
    width: usize,
    height: usize,
    y_out: &mut [u8],
    u_out: &mut [u8],
    v_out: &mut [u8],
) {
    let stride = width * 4;
    let uv_width = width.div_ceil(2);

    for row in 0..height {
        for col in 0..width {
            let offset = row * stride + col * 4;
            let (r, g, b) = unpremultiply(
                prgb[offset + 2],
                prgb[offset + 1],
                prgb[offset],
                prgb[offset + 3],
            );

            // BT.601 YUV 変換
            let y = ((66 * r as i32 + 129 * g as i32 + 25 * b as i32 + 128) >> 8) + 16;
            y_out[row * width + col] = y.clamp(0, 255) as u8;
        }
    }

    // U/V は 2x2 ブロックの平均でサブサンプリング
    for row in (0..height).step_by(2) {
        for col in (0..width).step_by(2) {
            let mut sum_r: i32 = 0;
            let mut sum_g: i32 = 0;
            let mut sum_b: i32 = 0;
            let mut count: i32 = 0;

            for dy in 0..2 {
                let y = row + dy;
                if y >= height {
                    continue;
                }
                for dx in 0..2 {
                    let x = col + dx;
                    if x >= width {
                        continue;
                    }
                    let offset = y * stride + x * 4;
                    let (r, g, b) = unpremultiply(
                        prgb[offset + 2],
                        prgb[offset + 1],
                        prgb[offset],
                        prgb[offset + 3],
                    );
                    sum_r += r as i32;
                    sum_g += g as i32;
                    sum_b += b as i32;
                    count += 1;
                }
            }

            let avg_r = sum_r / count;
            let avg_g = sum_g / count;
            let avg_b = sum_b / count;

            let u = ((-38 * avg_r - 74 * avg_g + 112 * avg_b + 128) >> 8) + 128;
            let v = ((112 * avg_r - 94 * avg_g - 18 * avg_b + 128) >> 8) + 128;

            let uv_idx = (row / 2) * uv_width + (col / 2);
            u_out[uv_idx] = u.clamp(0, 255) as u8;
            v_out[uv_idx] = v.clamp(0, 255) as u8;
        }
    }
}

/// Premultiplied alpha を元に戻す
#[inline]
fn unpremultiply(r_pre: u8, g_pre: u8, b_pre: u8, a: u8) -> (u8, u8, u8) {
    if a == 0 {
        return (0, 0, 0);
    }
    if a == 255 {
        return (r_pre, g_pre, b_pre);
    }
    let a32 = a as u32;
    let r = ((r_pre as u32 * 255 + a32 / 2) / a32).min(255) as u8;
    let g = ((g_pre as u32 * 255 + a32 / 2) / a32).min(255) as u8;
    let b = ((b_pre as u32 * 255 + a32 / 2) / a32).min(255) as u8;
    (r, g, b)
}

/// エンコード済みフレームから SPS と PPS を取得する
fn extract_sps_pps(
    frames: &[EncodedFrame],
) -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    for frame in frames {
        if !frame.sps_list.is_empty() && !frame.pps_list.is_empty() {
            return Ok((frame.sps_list[0].clone(), frame.pps_list[0].clone()));
        }
    }
    Err("SPS/PPS not found in any frame".into())
}

/// Annex.B 形式を AVCC 形式に変換する
///
/// `EncodedFrame.data` には SPS/PPS が含まれないため、全 NALU を変換対象とする。
fn annexb_to_avcc(data: &[u8]) -> Vec<u8> {
    let mut avcc = Vec::new();
    let nalus = extract_nalus(data);
    for nalu in nalus {
        let len = nalu.len() as u32;
        avcc.extend_from_slice(&len.to_be_bytes());
        avcc.extend_from_slice(nalu);
    }
    avcc
}

/// Annex.B ストリームからスタートコードの位置を検索する
fn find_start_code(data: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut i = start;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 {
            if i + 3 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                return Some((i, 4));
            }
            if data[i + 2] == 1 {
                return Some((i, 3));
            }
        }
        i += 1;
    }
    None
}

/// Annex.B ストリームから NALU を抽出する
fn extract_nalus(data: &[u8]) -> Vec<&[u8]> {
    let mut nalus = Vec::new();
    let mut pos = 0;

    while let Some((sc_pos, sc_len)) = find_start_code(data, pos) {
        let nalu_start = sc_pos + sc_len;
        let nalu_end = find_start_code(data, nalu_start)
            .map(|(next_pos, _)| next_pos)
            .unwrap_or(data.len());

        if nalu_start < nalu_end {
            nalus.push(&data[nalu_start..nalu_end]);
        }

        pos = nalu_end;
    }

    nalus
}

/// MP4 ファイルに書き出す
fn write_mp4(
    path: &str,
    frames: &[EncodedFrame],
    sps: &[u8],
    pps: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let reserved_moov_size = estimate_maximum_moov_box_size(&[frames.len()]);
    let options = Mp4FileMuxerOptions {
        reserved_moov_box_size: reserved_moov_size,
        ..Default::default()
    };
    let mut muxer = Mp4FileMuxer::with_options(options)?;

    let initial_bytes = muxer.initial_boxes_bytes();
    let mut file = File::create(path)?;
    file.write_all(initial_bytes)?;
    let mut current_offset = initial_bytes.len() as u64;

    // SPS から profile/level 情報を取得
    // SPS の先頭バイトは NALU ヘッダーなので、profile は [1], compatibility は [2], level は [3]
    let avc_profile_indication = sps[1];
    let profile_compatibility = sps[2];
    let avc_level_indication = sps[3];

    let sample_entry = SampleEntry::Avc1(Avc1Box {
        visual: VisualSampleEntryFields {
            data_reference_index: VisualSampleEntryFields::DEFAULT_DATA_REFERENCE_INDEX,
            width: WIDTH as u16,
            height: HEIGHT as u16,
            horizresolution: VisualSampleEntryFields::DEFAULT_HORIZRESOLUTION,
            vertresolution: VisualSampleEntryFields::DEFAULT_VERTRESOLUTION,
            frame_count: VisualSampleEntryFields::DEFAULT_FRAME_COUNT,
            compressorname: VisualSampleEntryFields::NULL_COMPRESSORNAME,
            depth: VisualSampleEntryFields::DEFAULT_DEPTH,
        },
        avcc_box: AvccBox {
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size_minus_one: Uint::new(3), // 4 バイト長
            sps_list: vec![sps.to_vec()],
            pps_list: vec![pps.to_vec()],
            chroma_format: None,
            bit_depth_luma_minus8: None,
            bit_depth_chroma_minus8: None,
            sps_ext_list: vec![],
        },
        unknown_boxes: vec![],
    });

    for (i, frame) in frames.iter().enumerate() {
        let avcc_data = annexb_to_avcc(&frame.data);
        let is_sync = frame.frame_type == shiguredo_openh264::FrameType::Idr;

        file.write_all(&avcc_data)?;

        let sample = Sample {
            track_kind: TrackKind::Video,
            sample_entry: if i == 0 {
                Some(sample_entry.clone())
            } else {
                None
            },
            keyframe: is_sync,
            timescale: TIMESCALE,
            duration: 1,
            composition_time_offset: None,
            data_offset: current_offset,
            data_size: avcc_data.len(),
        };
        muxer.append_sample(&sample)?;

        current_offset += avcc_data.len() as u64;
    }

    let finalized = muxer.finalize()?;

    for (offset, bytes) in finalized.offset_and_bytes_pairs() {
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(bytes)?;
    }

    Ok(())
}

/// HSV から RGB に変換する (H: 0-360, S: 0-1, V: 0-1)
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = ((r1 + m) * 255.0) as u8;
    let g = ((g1 + m) * 255.0) as u8;
    let b = ((b1 + m) * 255.0) as u8;
    (r, g, b)
}
