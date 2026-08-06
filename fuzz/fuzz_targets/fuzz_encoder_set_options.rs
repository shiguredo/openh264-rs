//! fuzzing ターゲット: 有効なエンコーダーに対して任意の値で動的パラメーター
//! 変更 (`set_bitrate()` / `set_resolution()` / `set_frame_rate()` /
//! `set_config()`) を呼び、パニック・クラッシュしないことを確認する
//!
//! バリデーションを通過した後に動的変更 API が受け付ける値の検証漏れを
//! 検出する。

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

fuzz_target!(|data: &[u8]| {
    let Some(lib) = library() else {
        return;
    };
    if data.is_empty() {
        return;
    }

    // まず有効なエンコーダーを生成する
    let config = EncoderConfig::new(64, 64, 500_000, 30, 1);
    let Ok(mut encoder) = Encoder::new(lib.clone(), config) else {
        return;
    };

    // 任意バイト列から値を取り出して動的パラメーター変更を呼び出す。
    // 入力が尽きたら先頭に戻って繰り返す (値は 0 や極端な値を含む)。
    let mut index = 0;
    let next = |index: &mut usize| -> usize {
        let value = data[*index % data.len()] as usize;
        *index += 1;
        value
    };

    let _ = encoder.set_bitrate(next(&mut index));
    let _ = encoder.set_resolution(1 + next(&mut index), 1 + next(&mut index));
    let _ = encoder.set_frame_rate(1 + next(&mut index), 1 + next(&mut index));
    let _ = encoder.set_config(EncoderConfig::new(
        1 + next(&mut index),
        1 + next(&mut index),
        next(&mut index),
        1 + next(&mut index),
        1 + next(&mut index),
    ));
});
