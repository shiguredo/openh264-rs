//! PBT 共通ヘルパー
//!
//! `tests/` 配下の PBT テストで共有するユーティリティ。

use shiguredo_openh264::{EncodedFrame, Openh264Library};
use std::sync::OnceLock;

/// PBT 用に OpenH264 ライブラリを一度だけロードする
pub fn library() -> &'static Openh264Library {
    static LIB: OnceLock<Openh264Library> = OnceLock::new();
    LIB.get_or_init(|| {
        let path = std::env::var("OPENH264_PATH").expect("OPENH264_PATH env var is not found");
        Openh264Library::load(path).expect("failed to load OpenH264 library")
    })
}

/// エンコード済みフレームから Annex B ストリームを構築する
///
/// SPS/PPS をスタートコード付きで先頭に付加し、続けてフレームデータを結合する。
pub fn build_bitstream(encoded: &EncodedFrame) -> Vec<u8> {
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
    bitstream
}

/// Annex B ストリームをスタートコード (0x00000001 または 0x000001) で分割する
pub fn split_annex_b(data: &[u8]) -> Vec<Vec<u8>> {
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
