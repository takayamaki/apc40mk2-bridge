# APC40mk2 Bridge

[English](README.en.md)

APC40 mk2 の LED フィードバックを復活させる MIDI ブリッジアプリ。

Windows MIDI Services 2026 のバグにより、Resolume Avenue から APC40 mk2 への MIDI 出力（LED 制御）が途切れる問題を回避します。

## 仕組み

```
Resolume (MIDI出力) → loopMIDI Port → [APC40mk2 Bridge] → APC40 mk2 (USB)
Resolume (MIDI入力) ← APC40 mk2 (USB)  ※ 直結のまま
```

Resolume の MIDI 出力先を loopMIDI の仮想ポートに変更し、本アプリがそのメッセージを受信して APC40 mk2 に即時転送します。
入力（パッド押下→クリップトリガー）は APC 直結のままなので、既存のマッピングはそのまま使えます。

ブリッジが動く理由: 本アプリは WinMM `midiOutShortMsg`（即時送信）を使い、問題のあるストリームスケジューラを経由しません。

## 必要なもの

- [loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) — 仮想 MIDI ポートドライバ
- APC40 mk2
- Resolume Avenue / Arena

## セットアップ

1. **loopMIDI** をインストールし、ポートを1つ作成（デフォルト名 `loopMIDI Port` のまま）
2. **APC40mk2 Bridge** を起動（システムトレイに常駐）
3. トレイアイコンを右クリック → **Input Port** で `loopMIDI Port` を選択
4. トレイアイコンを右クリック → **Output Port** で `APC40 mkII` を選択
5. **Start Bridge** を押す → Alt Ableton モードの SysEx が自動送信され、転送開始
6. **Resolume** の MIDI 設定:
   - 出力: `loopMIDI Port` に変更
   - 入力: `APC40 mkII`（そのまま）

## 機能

- **システムトレイ常駐**: ウィンドウなしで動作。コンテキストメニューから操作
- **MIDI ポート選択**: 入力・出力ポートをメニューから選択
- **SysEx モード切替**: Mode 0 (Generic) / Mode 1 (Ableton) / Mode 2 (Alt Ableton)
- **自動再接続**: APC が切断されても、再接続時に自動復帰
- **デバッグモニタ**: 転送中の MIDI メッセージをリアルタイム表示
- **midiStreamOut テスト**: Windows MIDI Services のストリームスケジューラ動作検証用

## ビルド

```bash
pnpm install
pnpm tauri build
```

### 必要な環境

- Rust (rustup)
- Node.js + pnpm
- Visual Studio Build Tools 2022
- Windows SDK

## ライセンス

[MIT](LICENSE)
