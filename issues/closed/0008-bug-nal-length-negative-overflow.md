# NAL 長の負値・iLayerNum の範囲を検証していない

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`encode()` では `iLayerNum` / `iNalCount` / 各 `nal_len` (すべて `c_int`) をそのまま `as usize` で変換して `from_raw_parts` やポインタ演算に使用している。負値や異常に大きい値が返された場合、巨大な `usize` になり範囲外参照でセグフォする。

## 根拠

- `iLayerNum` が負値の場合: `as usize` で巨大値 → `sLayerInfo[..巨大値]` で配列境界外アクセス
- `iNalCount` が負値の場合: `as usize` で巨大値 → `from_raw_parts` で未定義動作
- `nal_len` が負値の場合: `as usize` で巨大値 → `from_raw_parts` + `pBsBuf.add(offset)` で未定義動作
- `sLayerInfo` は固定長 128 要素なので `iLayerNum` は 0..=128 の範囲に収まるべき

## 解決方法

- `iLayerNum` を `as usize` する前に 0 以上かつ配列上限 (128) 以下であることを検証するようにした
- `iNalCount` が 0 以下の場合はスキップするようにした（既存の `== 0` チェックを `<= 0` に拡張）
- 各 `nal_len` が 0 以下の場合はスキップするようにした
