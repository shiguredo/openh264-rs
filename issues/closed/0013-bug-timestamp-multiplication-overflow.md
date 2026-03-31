# タイムスタンプ計算の乗算が usize オーバーフローで panic し得る

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`encode()` で `self.frames * self.fps_denominator` が `usize` 同士の通常乗算のまま。長時間運転で `frames` が増加すると `usize` オーバーフローし、debug ビルドで panic する。

## 根拠

`fps_denominator` は `u32::MAX` まで許可済み。64-bit 環境で `frames` が約 43 億（30fps で約 4.5 年）を超えると `frames * fps_denominator` が `usize::MAX` を超える。長時間動作するアプリケーションで踏む可能性がある。

## 解決方法

タイムスタンプ計算を `u128` 経由に変更した。`frames as u128 * fps_denominator as u128 * 1000 / fps_numerator as u128` でミリ秒を直接算出し、`c_longlong` にキャストする。これにより `Duration` も不要になったため import を削除した。
