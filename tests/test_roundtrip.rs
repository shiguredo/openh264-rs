use shiguredo_openh264::{
    Decoder, EncodeOptions, Encoder, EncoderConfig, FrameType, Openh264Library, RateControlMode,
};

/// OpenH264 ライブラリをロードする
fn load_library() -> Openh264Library {
    let path = std::env::var("OPENH264_PATH").expect("OPENH264_PATH env var is not found");
    Openh264Library::load(path).expect("failed to load OpenH264 library")
}

/// SMPTE カラーバー風の I420 フレームを生成する
///
/// 7 色の縦ストライプ（白/黄/シアン/緑/マゼンタ/赤/青）を
/// BT.601 で YUV に変換し I420 形式で返す。
fn generate_colorbar_i420(width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // SMPTE カラーバーの RGB 値（白/黄/シアン/緑/マゼンタ/赤/青）
    let bars: [(u8, u8, u8); 7] = [
        (235, 235, 235), // 白
        (235, 235, 16),  // 黄
        (16, 235, 235),  // シアン
        (16, 235, 16),   // 緑
        (235, 16, 235),  // マゼンタ
        (235, 16, 16),   // 赤
        (16, 16, 235),   // 青
    ];

    let mut y_plane = vec![0u8; width * height];
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let mut u_plane = vec![0u8; uv_width * uv_height];
    let mut v_plane = vec![0u8; uv_width * uv_height];

    for y in 0..height {
        for x in 0..width {
            let bar_index = x * 7 / width;
            let (r, g, b) = bars[bar_index];

            // BT.601 RGB -> YCbCr
            let rf = r as f64;
            let gf = g as f64;
            let bf = b as f64;
            let yv = (0.257 * rf + 0.504 * gf + 0.098 * bf + 16.0).clamp(16.0, 235.0) as u8;
            y_plane[y * width + x] = yv;

            // UV は 2x2 ブロック単位（左上ピクセルで代表する）
            if y % 2 == 0 && x % 2 == 0 {
                let u = (-0.148 * rf - 0.291 * gf + 0.439 * bf + 128.0).clamp(16.0, 240.0) as u8;
                let v = (0.439 * rf - 0.368 * gf - 0.071 * bf + 128.0).clamp(16.0, 240.0) as u8;
                let uv_row = y / 2;
                let uv_col = x / 2;
                u_plane[uv_row * uv_width + uv_col] = u;
                v_plane[uv_row * uv_width + uv_col] = v;
            }
        }
    }

    (y_plane, u_plane, v_plane)
}

/// ダミー I420 フレームを生成する
///
/// Y プレーンはフレーム番号に応じたグラデーション、UV プレーンは 128 固定。
fn generate_dummy_i420(
    width: usize,
    height: usize,
    frame_index: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut y_plane = vec![0u8; width * height];
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);

    for y in 0..height {
        for x in 0..width {
            y_plane[y * width + x] = ((x + y + frame_index * 7) % 256) as u8;
        }
    }

    let u_plane = vec![128u8; uv_width * uv_height];
    let v_plane = vec![128u8; uv_width * uv_height];

    (y_plane, u_plane, v_plane)
}

/// プレーン同士の PSNR を計算する（dB）
///
/// 値が大きいほど入力と出力が近い。一般に 30dB 以上あれば視覚的に良好。
fn psnr(original: &[u8], decoded: &[u8]) -> f64 {
    assert_eq!(original.len(), decoded.len());
    let mut mse_sum: f64 = 0.0;
    for i in 0..original.len() {
        let diff = original[i] as f64 - decoded[i] as f64;
        mse_sum += diff * diff;
    }
    let mse = mse_sum / original.len() as f64;
    if mse == 0.0 {
        return f64::INFINITY;
    }
    10.0 * (255.0_f64 * 255.0 / mse).log10()
}

/// デコード結果の YUV プレーン（ストライド除去済み）
struct DecodedFrame {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

/// デコードフレームからストライド分のパディングを除去して YUV プレーンを抽出する
fn extract_yuv(frame: &shiguredo_openh264::DecodedFrame) -> DecodedFrame {
    let width = frame.width();
    let height = frame.height();
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);

    let y_stride = frame.y_stride();
    let u_stride = frame.u_stride();
    let v_stride = frame.v_stride();

    let mut y_data = Vec::with_capacity(width * height);
    for row in 0..height {
        y_data.extend_from_slice(&frame.y_plane()[row * y_stride..row * y_stride + width]);
    }

    let mut u_data = Vec::with_capacity(uv_width * uv_height);
    for row in 0..uv_height {
        u_data.extend_from_slice(&frame.u_plane()[row * u_stride..row * u_stride + uv_width]);
    }

    let mut v_data = Vec::with_capacity(uv_width * uv_height);
    for row in 0..uv_height {
        v_data.extend_from_slice(&frame.v_plane()[row * v_stride..row * v_stride + uv_width]);
    }

    DecodedFrame {
        y: y_data,
        u: u_data,
        v: v_data,
    }
}

/// エンコード→デコードのラウンドトリップを実行し、デコード結果の YUV プレーンを返す
fn roundtrip(
    lib: &Openh264Library,
    config: EncoderConfig,
    frames: &[(Vec<u8>, Vec<u8>, Vec<u8>)],
) -> Vec<DecodedFrame> {
    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");
    let options = EncodeOptions::default();

    // エンコード: 全フレームの Annex B データを結合する
    let mut bitstream = Vec::new();
    let mut encoded_count = 0;
    for (y, u, v) in frames {
        if let Some(encoded) = encoder.encode(y, u, v, &options).expect("failed to encode") {
            // SPS/PPS を Annex B 形式で先頭に付加する
            for sps in &encoded.sps_list {
                bitstream.extend_from_slice(&[0, 0, 0, 1]);
                bitstream.extend_from_slice(sps);
            }
            for pps in &encoded.pps_list {
                bitstream.extend_from_slice(&[0, 0, 0, 1]);
                bitstream.extend_from_slice(pps);
            }
            bitstream.extend_from_slice(&encoded.data);
            encoded_count += 1;
        }
    }
    assert!(encoded_count > 0, "no encoded frames were produced");

    // デコード: NAL ユニット単位で分割してデコーダーに渡す
    let mut decoder = Decoder::new(lib.clone()).expect("failed to create decoder");
    let mut decoded_frames = Vec::new();

    let nalu_list = split_annex_b(&bitstream);
    for nalu in &nalu_list {
        if let Some(frame) = decoder.decode(nalu).expect("failed to decode") {
            decoded_frames.push(extract_yuv(&frame));
        }
    }

    // フラッシュ
    if let Some(frame) = decoder.finish().expect("failed to finish") {
        decoded_frames.push(extract_yuv(&frame));
    }

    decoded_frames
}

/// Annex B ストリームをスタートコード (0x00000001 または 0x000001) で分割する
fn split_annex_b(data: &[u8]) -> Vec<Vec<u8>> {
    let mut nalus = Vec::new();
    let mut positions = Vec::new();

    // スタートコードの位置を検出する
    let mut i = 0;
    while i < data.len() {
        if i + 3 < data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            positions.push(i);
            i += 4;
        } else if i + 2 < data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            positions.push(i);
            i += 3;
        } else {
            i += 1;
        }
    }

    // 各 NAL ユニットを切り出す（スタートコード付き）
    for (idx, &start) in positions.iter().enumerate() {
        let end = if idx + 1 < positions.len() {
            positions[idx + 1]
        } else {
            data.len()
        };
        nalus.push(data[start..end].to_vec());
    }

    nalus
}

/// カラーバーを使ったラウンドトリップで PSNR を検証するヘルパー
///
/// Y プレーンは `min_psnr_y_db` 以上、U/V プレーンは `min_psnr_uv_db` 以上を要求する。
fn roundtrip_colorbar(
    config: EncoderConfig,
    num_frames: usize,
    min_psnr_y_db: f64,
    min_psnr_uv_db: f64,
) {
    let lib = load_library();
    let width = config.width;
    let height = config.height;

    let (y, u, v) = generate_colorbar_i420(width, height);
    let frames: Vec<_> = (0..num_frames)
        .map(|_| (y.clone(), u.clone(), v.clone()))
        .collect();

    let decoded_frames = roundtrip(&lib, config, &frames);

    assert_eq!(
        decoded_frames.len(),
        num_frames,
        "decoded {} frames, expected {num_frames}",
        decoded_frames.len()
    );

    for (i, decoded) in decoded_frames.iter().enumerate() {
        let psnr_y = psnr(&y, &decoded.y);
        let psnr_u = psnr(&u, &decoded.u);
        let psnr_v = psnr(&v, &decoded.v);
        eprintln!("frame {i}: PSNR Y={psnr_y:.1} dB, U={psnr_u:.1} dB, V={psnr_v:.1} dB");
        assert!(
            psnr_y >= min_psnr_y_db,
            "frame {i}: Y PSNR {psnr_y:.1} dB < {min_psnr_y_db} dB"
        );
        assert!(
            psnr_u >= min_psnr_uv_db,
            "frame {i}: U PSNR {psnr_u:.1} dB < {min_psnr_uv_db} dB"
        );
        assert!(
            psnr_v >= min_psnr_uv_db,
            "frame {i}: V PSNR {psnr_v:.1} dB < {min_psnr_uv_db} dB"
        );
    }
}

/// Constrained Baseline + Quality モードのラウンドトリップ（PSNR 検証）
#[test]
fn roundtrip_constrained_baseline_quality() {
    let config = EncoderConfig {
        rate_control_mode: Some(RateControlMode::Quality),
        intra_period: Some(30),
        ..EncoderConfig::new(320, 240, 1_000_000, 30, 1)
    };
    roundtrip_colorbar(config, 30, 25.0, 20.0);
}

/// CABAC + Bitrate モードのラウンドトリップ（PSNR 検証）
#[test]
fn roundtrip_cabac_bitrate() {
    let config = EncoderConfig {
        rate_control_mode: Some(RateControlMode::Bitrate),
        entropy_coding_mode: Some(shiguredo_openh264::EntropyCodingMode::Cabac),
        intra_period: Some(30),
        ..EncoderConfig::new(320, 240, 1_000_000, 30, 1)
    };
    roundtrip_colorbar(config, 30, 25.0, 20.0);
}

/// IDR 強制挿入のラウンドトリップ
#[test]
fn roundtrip_force_idr() {
    let lib = load_library();
    let width = 320;
    let height = 240;
    let num_frames = 15;

    let config = EncoderConfig {
        rate_control_mode: Some(RateControlMode::Quality),
        intra_period: Some(300),
        ..EncoderConfig::new(width, height, 1_000_000, 30, 1)
    };

    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");
    let mut bitstream = Vec::new();
    let mut idr_count = 0;

    for i in 0..num_frames {
        let (y, u, v) = generate_dummy_i420(width, height, i);
        let options = if i == 10 {
            EncodeOptions { force_idr: true }
        } else {
            EncodeOptions::default()
        };

        if let Some(encoded) = encoder
            .encode(&y, &u, &v, &options)
            .expect("failed to encode")
        {
            if encoded.frame_type == FrameType::Idr {
                idr_count += 1;
            }
            for sps in &encoded.sps_list {
                bitstream.extend_from_slice(&[0, 0, 0, 1]);
                bitstream.extend_from_slice(sps);
            }
            for pps in &encoded.pps_list {
                bitstream.extend_from_slice(&[0, 0, 0, 1]);
                bitstream.extend_from_slice(pps);
            }
            bitstream.extend_from_slice(&encoded.data);
        }
    }

    assert!(
        idr_count >= 2,
        "expected at least 2 IDR frames, got {idr_count}"
    );

    // デコードで復号できることを確認する
    let mut decoder = Decoder::new(lib).expect("failed to create decoder");
    let mut decoded_count = 0;
    for nalu in &split_annex_b(&bitstream) {
        if decoder.decode(nalu).expect("failed to decode").is_some() {
            decoded_count += 1;
        }
    }
    if decoder.finish().expect("failed to finish").is_some() {
        decoded_count += 1;
    }
    assert_eq!(decoded_count, num_frames);
}

/// fps_numerator / fps_denominator がゼロの場合にエラーを返す
#[test]
fn encoder_rejects_zero_fps() {
    let lib = load_library();

    // Encoder::new で fps_numerator = 0
    let config = EncoderConfig::new(64, 64, 100_000, 0, 1);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // Encoder::new で fps_denominator = 0
    let config = EncoderConfig::new(64, 64, 100_000, 30, 0);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // set_frame_rate でゼロ
    let config = EncoderConfig::new(64, 64, 100_000, 30, 1);
    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");
    assert!(encoder.set_frame_rate(0, 1).is_err());
    assert!(encoder.set_frame_rate(30, 0).is_err());

    // set_config でゼロ
    let config_zero = EncoderConfig::new(64, 64, 100_000, 0, 1);
    assert!(encoder.set_config(config_zero).is_err());
}

/// 動的パラメーター変更後にエンコード・デコードが正常に動作する
#[test]
fn dynamic_parameter_change() {
    let lib = load_library();
    let config = EncoderConfig::new(320, 240, 500_000, 30, 1);
    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");

    // 初期解像度でエンコード
    let (y, u, v) = generate_dummy_i420(320, 240, 0);
    let encoded = encoder
        .encode(&y, &u, &v, &EncodeOptions::default())
        .expect("failed to encode");
    assert!(encoded.is_some());

    // ビットレート変更
    encoder
        .set_bitrate(1_000_000)
        .expect("failed to set bitrate");

    // フレームレート変更
    encoder
        .set_frame_rate(60, 1)
        .expect("failed to set frame rate");

    // 解像度変更
    encoder
        .set_resolution(160, 120)
        .expect("failed to set resolution");

    // 変更後の解像度でエンコード→デコードが成功する
    let (y, u, v) = generate_dummy_i420(160, 120, 1);
    let encoded = encoder
        .encode(&y, &u, &v, &EncodeOptions::default())
        .expect("failed to encode after resolution change");
    assert!(encoded.is_some());
    let encoded = encoded.expect("checked above: encode should not return None");

    // 解像度変更後は新しい SPS が必要なので IDR になる
    assert_eq!(encoded.frame_type, FrameType::Idr);
    let mut bitstream = Vec::new();
    for sps in &encoded.sps_list {
        bitstream.extend_from_slice(&[0, 0, 0, 1]);
        bitstream.extend_from_slice(sps);
    }
    for pps in &encoded.pps_list {
        bitstream.extend_from_slice(&[0, 0, 0, 1]);
        bitstream.extend_from_slice(pps);
    }
    bitstream.extend_from_slice(&encoded.data);

    let mut decoder = Decoder::new(lib).expect("failed to create decoder");
    let nalus = split_annex_b(&bitstream);
    let mut decoded = false;
    for nalu in &nalus {
        if let Some(frame) = decoder.decode(nalu).expect("failed to decode") {
            assert_eq!(frame.width(), 160);
            assert_eq!(frame.height(), 120);
            decoded = true;
        }
    }
    if let Some(frame) = decoder.finish().expect("failed to finish") {
        assert_eq!(frame.width(), 160);
        assert_eq!(frame.height(), 120);
        decoded = true;
    }
    assert!(decoded, "no frame was decoded after resolution change");
}

/// width / height がゼロの場合にエラーを返す
#[test]
fn encoder_rejects_zero_dimensions() {
    let lib = load_library();

    // Encoder::new で width = 0
    let config = EncoderConfig::new(0, 64, 100_000, 30, 1);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // Encoder::new で height = 0
    let config = EncoderConfig::new(64, 0, 100_000, 30, 1);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // set_resolution でゼロ
    let config = EncoderConfig::new(64, 64, 100_000, 30, 1);
    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");
    assert!(encoder.set_resolution(0, 64).is_err());
    assert!(encoder.set_resolution(64, 0).is_err());

    // set_config でゼロ
    let config_zero = EncoderConfig::new(0, 64, 100_000, 30, 1);
    assert!(encoder.set_config(config_zero).is_err());
}

/// レベル 5.2 の最大フレームサイズ (36864 マクロブロック) を超える解像度はエラーを返す
///
/// 超える解像度は OpenH264 のエンコーダー初期化で巨大なバッファが確保され、
/// OOM の原因になるため、バリデーションで事前に拒否する。
#[test]
fn encoder_rejects_oversized_dimensions() {
    let lib = load_library();

    // 4096x2304 (レベル 5.2 の最大フレームサイズちょうど) は通る
    let config = EncoderConfig::new(4096, 2304, 2_000_000, 30, 1);
    assert!(Encoder::new(lib.clone(), config).is_ok());

    // 1 ピクセルでも超えるとエラー
    let config = EncoderConfig::new(4096, 2305, 2_000_000, 30, 1);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // 極端に大きな解像度もエラー
    let config = EncoderConfig::new(65535, 65535, 2_000_000, 30, 1);
    assert!(Encoder::new(lib.clone(), config).is_err());

    // set_resolution でも同じ制限が適用される
    let config = EncoderConfig::new(64, 64, 100_000, 30, 1);
    let mut encoder = Encoder::new(lib.clone(), config).expect("failed to create encoder");
    assert!(encoder.set_resolution(4096, 2304).is_ok());
    assert!(encoder.set_resolution(4096, 2305).is_err());
}

/// QP 値の範囲外や min_qp > max_qp の場合にエラーを返す
#[test]
fn encoder_rejects_invalid_qp() {
    let lib = load_library();

    // max_qp が 51 を超える
    let config = EncoderConfig {
        max_qp: Some(52),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_err());

    // min_qp が 51 を超える
    let config = EncoderConfig {
        min_qp: Some(52),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_err());

    // min_qp > max_qp
    let config = EncoderConfig {
        min_qp: Some(30),
        max_qp: Some(20),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_err());

    // 正常な QP 範囲は通る
    let config = EncoderConfig {
        min_qp: Some(10),
        max_qp: Some(40),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_ok());
}

/// SliceMode のゼロ値の場合にエラーを返す
#[test]
fn encoder_rejects_zero_slice_mode() {
    let lib = load_library();

    // FixedCount(0)
    let config = EncoderConfig {
        slice_mode: Some(shiguredo_openh264::SliceMode::FixedCount(0)),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_err());

    // SizeConstrained(0)
    let config = EncoderConfig {
        slice_mode: Some(shiguredo_openh264::SliceMode::SizeConstrained(0)),
        ..EncoderConfig::new(64, 64, 100_000, 30, 1)
    };
    assert!(Encoder::new(lib.clone(), config).is_err());
}
