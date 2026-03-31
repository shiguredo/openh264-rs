# fps が u32 を超えると encode() でタイムスタンプ計算が壊れる

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`fps_numerator` / `fps_denominator` は非ゼロしか検証していないが、`encode()` では `self.fps_numerator as u32` に縮めて `Duration` の除算に使っている。`u32::MAX` を超える値は切り詰められ、`2^32` の倍数なら 0 になって除算パニックになる。`set_frame_rate()` でも同じ値をそのまま保持するため、公開 API から再現可能。

## 根拠

`Duration::from_secs(...) / self.fps_numerator as u32` という計算で、`as u32` はサイレントに切り詰める。`fps_numerator` が `2^32` の倍数（例: `4294967296`）の場合、`as u32` が 0 になりゼロ除算パニックが発生する。

## 再現手順

1. `EncoderConfig` の `fps_numerator` に `u32::MAX + 1` を設定する
2. `Encoder::new()` でエンコーダーを生成する
3. `encode()` を呼ぶとパニックする

## 解決方法

`validate_config()` と `set_frame_rate()` に `u32::try_from()` による範囲チェックを追加した。範囲外の値は `Error::InvalidParameter` を返す。
