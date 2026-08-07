//! デコーダーの Property-Based Testing
//!
//! - 複数フレームのラウンドトリップ: ランダムなフレーム系列をエンコードし、
//!   NAL ユニット単位でデコードすると全フレームが復号でき寸法が一致する

mod helpers;

use helpers::{build_bitstream, library, split_annex_b};
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

/// 複数フレームのラウンドトリップに使う設定とフレーム系列の組を生成する
fn multi_frame_case() -> impl Strategy<Value = (EncoderConfig, Vec<(Vec<u8>, Vec<u8>, Vec<u8>)>)> {
    (dimension(), dimension()).prop_flat_map(|(width, height)| {
        let config = (50_000usize..=1_000_000, 1usize..=60, 1usize..=30).prop_map(
            move |(bitrate, fps_num, fps_den)| {
                EncoderConfig::new(width, height, bitrate, fps_num, fps_den)
            },
        );
        let frames = prop::collection::vec(random_i420(width, height), 1..=3);
        (config, frames)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// ランダムなフレーム系列をエンコードし、NAL ユニット単位でデコードすると
    /// 全フレームが復号でき、デコード結果の寸法が入力と一致する
    #[test]
    fn decode_all_encoded_frames((config, frames) in multi_frame_case()) {
        let lib = library();
        let mut encoder =
            Encoder::new(lib.clone(), config.clone()).expect("encoder should be created");
        let options = EncodeOptions::default();

        // 全フレームをエンコードして Annex B ストリームを構築する
        let mut bitstream = Vec::new();
        let mut encoded_count = 0;
        for (y, u, v) in &frames {
            if let Some(encoded) = encoder
                .encode(y, u, v, &options)
                .expect("encode should succeed")
            {
                bitstream.extend_from_slice(&build_bitstream(&encoded));
                encoded_count += 1;
            }
        }
        prop_assert!(encoded_count > 0, "no frame was encoded");

        // NAL ユニット単位で分割してデコードする
        let mut decoder = Decoder::new(lib.clone()).expect("decoder should be created");
        let mut decoded_count = 0;
        for nalu in &split_annex_b(&bitstream) {
            if let Some(frame) = decoder
                .decode(nalu)
                .expect("decode should succeed")
            {
                prop_assert_eq!(frame.width(), config.width);
                prop_assert_eq!(frame.height(), config.height);
                decoded_count += 1;
            }
        }

        // フラッシュで残りのフレームを取り出す
        if let Some(frame) = decoder.finish().expect("finish should succeed") {
            prop_assert_eq!(frame.width(), config.width);
            prop_assert_eq!(frame.height(), config.height);
            decoded_count += 1;
        }

        prop_assert_eq!(decoded_count, encoded_count);
    }
}
