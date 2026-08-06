//! fuzzing ターゲット: 任意の `EncoderConfig` で `Encoder::new()` を呼び、
//! パニック・クラッシュしないことを確認する
//!
//! バリデーション (`validate_config`) と OpenH264 のエンコーダー初期化の
//! 組み合わせに対する耐性を検証する。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_openh264::{Encoder, EncoderConfig, Openh264Library};
use std::sync::OnceLock;

/// OpenH264 ライブラリを一度だけロードする
///
/// `OPENH264_PATH` 環境変数が未設定の場合は `None` を返し、ターゲットの処理を
/// スキップする (ライブラリなしでも fuzz ビルド自体は通るようにする)。
fn library() -> Option<&'static Openh264Library> {
    static LIB: OnceLock<Option<Openh264Library>> = OnceLock::new();
    LIB.get_or_init(|| {
        let path = std::env::var("OPENH264_PATH").ok()?;
        Openh264Library::load(path).ok()
    })
    .as_ref()
}

/// 任意バイト列から `EncoderConfig` を組み立てる
///
/// 入力が 12 バイト未満の場合は `None` を返す。0 や極端な値を含むため、
/// バリデーションのエラーパスと正常パスの両方をカバーする。
fn build_config(data: &[u8]) -> Option<EncoderConfig> {
    if data.len() < 12 {
        return None;
    }
    let width = u16::from_le_bytes([data[0], data[1]]) as usize;
    let height = u16::from_le_bytes([data[2], data[3]]) as usize;
    let target_bitrate = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let fps_numerator = u16::from_le_bytes([data[8], data[9]]) as usize;
    let fps_denominator = u16::from_le_bytes([data[10], data[11]]) as usize;
    Some(EncoderConfig::new(
        width,
        height,
        target_bitrate,
        fps_numerator,
        fps_denominator,
    ))
}

fuzz_target!(|data: &[u8]| {
    let Some(lib) = library() else {
        return;
    };
    let Some(config) = build_config(data) else {
        return;
    };
    let _ = Encoder::new(lib.clone(), config);
});
