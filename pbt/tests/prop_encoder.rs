//! エンコーダーの Property-Based Testing
//!
//! - ラウンドトリップ: ランダムな設定・フレームでエンコード → デコード → 寸法一致
//! - バリデーション: `Encoder::new()` が Ok を返す条件と入力値の整合性

mod helpers;

use helpers::library;
use proptest::prelude::*;
use shiguredo_openh264::{Decoder, EncodeOptions, Encoder, EncoderConfig};

/// エンコード可能な最小解像度 (16x16) から 64x64 までの偶数解像度を生成する
///
/// OpenH264 は幅・高さのどちらかが 16 未満のフレームをエンコードできない
/// (EncodeFrame がエラーコード 5 を返す)。また、奇数サイズのストリームは
/// デコーダーが偶数に切り捨てて出力するため、ラウンドトリップで寸法が
/// 一致するのは偶数サイズのみ (仕様由来の制約)。
fn dimension() -> impl Strategy<Value = usize> {
    (8usize..=32).prop_map(|v| v * 2)
}

/// ランダムな I420 プレーンを生成する
///
/// 純粋なランダムノイズは圧縮効率が最悪で、低ビットレート設定では
/// OpenH264 のビットストリームサイズ制限 (MinCr) を超えて EncodeFrame が
/// エラーを返す。そのため、位相と傾きをランダムにしたランプパターンで
/// 生成する (フレームごとに内容が変わり、圧縮も可能)。
fn random_i420(width: usize, height: usize) -> impl Strategy<Value = (Vec<u8>, Vec<u8>, Vec<u8>)> {
    let y_len = width * height;
    let uv_len = width.div_ceil(2) * height.div_ceil(2);
    (any::<u8>(), any::<u8>()).prop_map(move |(phase, step)| {
        let mut y = vec![0u8; y_len];
        for (i, b) in y.iter_mut().enumerate() {
            *b = phase.wrapping_add((i as u8).wrapping_mul(step));
        }
        let u = vec![100u8; uv_len];
        let v = vec![200u8; uv_len];
        (y, u, v)
    })
}

/// ラウンドトリップに使う設定とフレームの組を生成する
fn roundtrip_case() -> impl Strategy<Value = (EncoderConfig, (Vec<u8>, Vec<u8>, Vec<u8>))> {
    (dimension(), dimension()).prop_flat_map(|(width, height)| {
        let config = (50_000usize..=1_000_000, 1usize..=60, 1usize..=30).prop_map(
            move |(bitrate, fps_num, fps_den)| {
                EncoderConfig::new(width, height, bitrate, fps_num, fps_den)
            },
        );
        (config, random_i420(width, height))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// ランダムな設定・フレームのラウンドトリップでデコード結果の寸法が入力と一致する
    #[test]
    fn roundtrip_preserves_dimensions((config, (y, u, v)) in roundtrip_case()) {
        let lib = library();
        let mut encoder =
            Encoder::new(lib.clone(), config.clone()).expect("encoder should be created");

        let encoded = encoder
            .encode(&y, &u, &v, &EncodeOptions::default())
            .expect("encode should succeed")
            .expect("encoded frame should be produced");

        // SPS/PPS とフレームデータを結合して Annex B ストリームを構築する
        let bitstream = helpers::build_bitstream(&encoded);

        let mut decoder = Decoder::new(lib.clone()).expect("decoder should be created");
        let decoded = decoder
            .decode(&bitstream)
            .expect("decode should succeed")
            .expect("decoded frame should be produced");

        prop_assert_eq!(decoded.width(), config.width);
        prop_assert_eq!(decoded.height(), config.height);
    }

    /// Encoder::new() が Ok を返すなら幅・高さ・fps がすべて非ゼロである
    #[test]
    fn new_result_matches_validation_rules(
        width in 0usize..=2048,
        height in 0usize..=2048,
        target_bitrate in 0usize..=2_000_000_000,
        fps_numerator in 0usize..=5_000_000_000,
        fps_denominator in 0usize..=5_000_000_000,
    ) {
        let config = EncoderConfig::new(width, height, target_bitrate, fps_numerator, fps_denominator);
        if let Ok(_encoder) = Encoder::new(library().clone(), config) {
            // 幅・高さ・fps がゼロの設定は validate_config で必ずエラーになる
            prop_assert!(width > 0);
            prop_assert!(height > 0);
            prop_assert!(fps_numerator > 0);
            prop_assert!(fps_denominator > 0);
        }
    }
}
