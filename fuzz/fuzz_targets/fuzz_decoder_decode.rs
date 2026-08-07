//! fuzzing ターゲット: 任意バイト列を `Decoder::decode()` に渡し、
//! パニック・クラッシュしないことを確認する
//!
//! OpenH264 デコーダー自体が壊れたビットストリームに対してクラッシュしない
//! ことと、FFI 境界のバリデーションの漏れを検証する。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_openh264::{Decoder, Openh264Library};
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
    let Ok(mut decoder) = Decoder::new(lib.clone()) else {
        return;
    };

    // 任意バイト列を 1〜16 バイトのチャンクに分割してデコードする。
    // NAL 境界を意識しない分割で、デコーダーのバッファリングを揺さぶる。
    let chunk_size = (data.len() % 16).max(1);
    for chunk in data.chunks(chunk_size) {
        let _ = decoder.decode(chunk);
    }
    let _ = decoder.finish();
});
