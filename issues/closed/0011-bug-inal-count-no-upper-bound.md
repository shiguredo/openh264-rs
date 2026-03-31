# iNalCount を上限検証せず from_raw_parts の長さに使っている

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`iNalCount <= 0` は弾いているが、正の異常値はそのまま `layer_info.iNalCount as usize` で `pNalLengthInByte` の長さに使っている。FFI 側が壊れた件数を返すと配列境界外を読む未定義動作になる。

## 根拠

`pNalLengthInByte` は FFI 側が内部確保した配列を指しているため、Rust 側から実際の配列長を検証する手段がない。しかし H.264 の仕様上、1 レイヤーあたりの NAL 数が極端に大きくなることはありえないため、合理的な上限 (65536) を設けて異常値を弾くことで防御する。

## 解決方法

`iNalCount > MAX_NAL_COUNT (65536)` の場合に `Error::InvalidParameter` を返すようにした。
