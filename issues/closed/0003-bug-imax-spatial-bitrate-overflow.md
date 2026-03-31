# iMaxSpatialBitrate が target_bitrate * 2 でオーバーフローする

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`target_bitrate` 自体には `c_int` 上限チェックが入っているが、空間レイヤー設定では `config.target_bitrate * 2` をそのまま `c_int` にキャストしている。`c_int::MAX / 2` を超える値で初期化すると、`iMaxSpatialBitrate` が負値にラップして OpenH264 に渡る。

## 根拠

`apply_config_to_param()` 内の `layer.iMaxSpatialBitrate = (config.target_bitrate * 2) as c_int` で、`target_bitrate` が `c_int::MAX / 2` (1,073,741,823) を超えると `target_bitrate * 2` が `c_int::MAX` を超えて負値に切り詰められる。

## 再現手順

1. `EncoderConfig` の `target_bitrate` に `1_073_741_824` (= `c_int::MAX / 2 + 1`) を設定する
2. `Encoder::new()` でエンコーダーを生成する
3. `iMaxSpatialBitrate` が負値で OpenH264 に渡される

## 解決方法

`validate_config()` で `target_bitrate` の上限を `c_int::MAX / 2` に引き下げた。エラーメッセージに `iMaxSpatialBitrate = target_bitrate * 2` が `c_int` に収まる必要がある旨を明記。
