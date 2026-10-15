# Architecture notes: dictation pipeline

> **Fork notice.** This repository is a fork of [cjpais/Handy](https://github.com/cjpais/Handy), distributed under the MIT License. The original copyright (c) 2025 CJ Pais is retained in `LICENSE`, and the README still links to and credits upstream. The Handy name and brand assets are not covered by the MIT grant; this fork uses its own branding.

Map of the path from hotkey to pasted text, by file and function (no line numbers; they rot). Paths are under `src-tauri/src/` unless noted.

## 1. Hotkey

- `shortcut/mod.rs` (`init_shortcuts`, `register_shortcut`) registers bindings through one of two backends: `shortcut/tauri_impl.rs` (tauri global-shortcut) or `shortcut/handy_keys.rs`.
- Both backends call `shortcut/handler.rs::handle_shortcut_event`. Transcribe bindings are forwarded to the `TranscriptionCoordinator` (`send_input`); the cancel binding goes to `utils::cancel_current_operation`.
- `transcription_coordinator.rs` is a state machine (`CoordinatorState::on_input`) that applies the activation mode (toggle, push-to-talk, hold-or-toggle), debounce, and "press while still processing" queuing. It emits effects that `run_effect` turns into `start()` / `stop()`, which look up the binding in `actions.rs::ACTION_MAP` and call `ShortcutAction::start/stop`.
- CLI flags and signals reach the same place via `signal_handle.rs::send_transcription_input` -> `TranscriptionCoordinator::send_external_input`.

## 2. Recording start

`actions.rs::TranscribeAction::start`:

- kicks off `TranscriptionManager::initiate_model_load` and `AudioRecordingManager::preload_vad` on a thread;
- refuses to open the mic if no model can be loaded;
- for streaming models calls `TranscriptionManager::start_stream` (live decode fed from the capture sink);
- calls `AudioRecordingManager::try_start_recording` (`managers/audio.rs`), plays the start sound, shows the overlay.

The `AppContext` capture for Xerra belongs here (cheap, non-blocking; see "Xerra additions").

## 3. VAD

`managers/audio.rs` owns the VAD in a `VadSlot` (`Unloaded` / `Idle` / `Lent`). `create_detector` builds Silero (`audio_toolkit/vad/silero.rs`, GGUF in `resources/models/`) or Earshot (`audio_toolkit/vad/earshot.rs`), wrapped by `audio_toolkit/vad/smoothed.rs::SmoothedVad` in `create_capture_vad`. `try_start_recording` lends the VAD to a `CaptureSink` (`audio_toolkit/audio/sink*`), which gates frames, computes levels for the overlay, and feeds the live stream. `handy-recorder` does device capture and 16 kHz resampling. VAD runs while recording, not after.

## 4. Recording stop and STT

`actions.rs::TranscribeAction::stop` (spawns an async task):

1. `AudioRecordingManager::stop_recording` returns the `Vec<f32>` samples (checks the cancel generation).
2. WAV is saved concurrently via `audio_toolkit::save_wav_file` (for history).
3. `managers/transcription.rs::TranscriptionManager::transcribe_recording` -> `finalize_live` -> `complete`. **This is where STT text is returned.** Inside `complete`'s closure (near the `apply_custom_words` call) the raw text is post-treated: Chinese script conversion, `apply_custom_words` fuzzy correction (skipped if the model took the vocabulary natively, see `vocabulary_for_model`), `remove_filler_words`, `normalize_transcription_output`. So text reaching `stop` is already de-filled and custom-word corrected.

## 5. Post-processing (upstream LLM feature)

- `actions.rs::process_transcription_output(app, text, post_process)` is called from `stop`. If `post_process` is true it runs `post_process_transcription`, which picks the provider/model/prompt from settings (`post_process_*` fields), then calls `llm_client.rs::send_chat_completion` or `send_chat_completion_with_schema` (structured output), or Apple Intelligence on macOS arm64. Also strips `<think>` blocks (`strip_think_block`).
- It returns `ProcessedTranscription { final_text, post_processed_text, post_process_prompt }`.
- The flag comes from the binding: `ACTION_MAP` has `"transcribe"` (`post_process: false`), `"transcribe_with_post_process"` (`post_process: true`), plus a cancel action. It is a separate shortcut (and `--toggle-post-process` CLI flag), not a setting on the normal binding.
- History: `HistoryManager::save_entry` stores the raw text plus the post-processed text and prompt.

## 6. Paste

Still in `stop`: if `final_text` is empty the overlay is hidden and nothing is pasted. Otherwise it runs `utils::paste` (re-exported from `clipboard.rs::paste`) on the main thread via `run_on_main_thread`, re-checking cancellation first. `clipboard.rs` implements the methods (clipboard + key chord via enigo or wtype/dotool/ydotool/xdotool on Linux, direct typing, etc.), restores the clipboard, and optionally auto-submits (`send_return_key`). `paste_tx/` (`mod.rs`, `macos.rs`, `windows.rs`) is the receipt-sequenced paste transaction (debug-gated beta).

## 7. Settings: persistence to React

- Rust: `settings.rs::AppSettings` (serde, defaults per field so partial stores load), stored with `tauri-plugin-store` in `settings_store.json` (`SETTINGS_STORE_PATH`, path via `portable::store_path`). Read with `get_settings(&AppHandle)`, written with `write_settings`.
- Commands: most settings have a dedicated `change_*_setting` command (e.g. in `shortcut/mod.rs`, `commands/*.rs`) registered in `lib.rs` inside `collect_commands![...]` on the `tauri_specta::Builder`. In debug builds this exports `src/bindings.ts` (do not hand-edit).
- Events: the backend emits `settings-changed` after changes (e.g. in `shortcut/mod.rs`).
- React: `src/stores/settingsStore.ts` (Zustand) loads via `commands.getAppSettings`/`getDefaultSettings`, maps each key to its command in a `settingUpdaters` table, and listens for `settings-changed`. `src/hooks/useSettings.ts` is the component-facing hook.
- Adding a setting therefore touches: `AppSettings` + default, a command + `collect_commands!`, the updater table in `settingsStore.ts`, and `en/translation.json`.
- API keys use a `SecretMap` (`post_process_api_keys`).

## Xerra additions

- **`cleanup/`** (`src-tauri/src/cleanup/`): the OpenRouter cleanup pass. System prompt in `cleanup/system_prompt.md`; style selection and request building live alongside it. Evaluated offline with `bun scripts/cleanup-eval.ts` against `tests/cleanup_cases.yaml`.
- **Hook point:** between STT text and paste, i.e. in `actions.rs::TranscribeAction::stop` around `process_transcription_output` (or inside it, before `final_text` is returned). Because history saves the raw text and `final_text` is what is pasted, cleanup should replace `final_text` and not touch the stored raw transcription. It must be cancellable (`complete_unless_cancelled`) and should show a processing overlay state like post-processing does. On any failure, fall back to the uncleaned text.
- **App context:** `src-tauri/src/app_context.rs` (`AppContext`, `capture()`), being built separately. Intent: capture the frontmost app and window title at recording start (not at paste time, since focus may move), use it to pick a style and fill the `APP`/`WINDOW` lines of the prompt, and skip sending the window title when `secure_input` is set.
