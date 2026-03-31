# iSpatialLayerNum を無検証でスライス境界に使っている

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`GetDefaultParams()` / `GetOption()` が返した `param.iSpatialLayerNum` をそのまま `as usize` にして `sSpatialLayers[..param.iSpatialLayerNum as usize]` を切っている。`sSpatialLayers` は固定長 4 要素であり、FFI 側の異常で負値や 5 以上が返ると即 panic する。

## 影響箇所

- `is_profile_supported()` (行 410)
- `apply_config_to_param()` (行 1161)
- `set_resolution()` (行 1381)

## 根拠

`iSpatialLayerNum` は `c_int` (i32) であり、負値なら `as usize` で巨大値になりスライス境界で panic する。5 以上でも配列長 4 を超えて panic する。3 箇所すべてが同じ危険に依存している。

## 解決方法

`spatial_layer_count()` ヘルパー関数を追加し、`iSpatialLayerNum` が 0〜`sSpatialLayers.len()` (4) の範囲に収まることを保証するようにした。範囲外の場合は 0 にクランプする。3 箇所すべてをこのヘルパー経由に置き換えた。
