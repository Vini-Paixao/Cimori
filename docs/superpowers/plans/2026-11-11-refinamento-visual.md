# Cimori visual integration implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task. Track verification here and in the plan ledger.

**Goal:** Ship the proposed Cimori appearance in the real Windows executable, including media artwork, AI avatar, provider logos and icon controls.
**Architecture:** Keep Rust/tiny-skia and native Win32 windows. Shared geometry owns drawing and hit targets; new focused visual modules replace the old expanded layouts. Media artwork is read and decoded on the media worker, with bounded waits and caches.
**Tech Stack:** Rust, windows 0.58 (WinRT/WIC), tiny-skia, Inter.
**Spec:** `docs/superpowers/specs/2026-11-11-refinamento-visual-design.md`.

## Global constraints

- Preserve existing services, preferences, hooks, permissions and actions; no framework migration.
- Colors: charcoal #17191D, graphite #20232A, porcelain #F5F1EB, mist #A7ADB7, apricot #F18B63.
- No permanent animation or UI polling. Artwork: 8 MiB encoded, 4096 maximum axis, 16 megapixels, 512 maximum normalized axis.
- No credential modifications or Windhawk changes. The user wants the actual installed executable updated.
- After validating the installed result, the user authorized a local commit of the completed work. No merge or push requested.

## Review focus

- Long strings and many outputs must not place hit targets outside the pill.
- Commands must remain fully visible or rejected via existing `command_fits`.
- Stale artwork must never follow a different song/app; missing artwork does not break transport.
- Per-monitor scale must affect draw and input equally; tooltips/hover should not start idle timers.
- Existing notification reply, reminder input, mixer drag and chat cancellation stay functional.

### Task 1: shared layout and visual foundation

Files: `crates/isle-render/src/{layout,pages,theme,icons,avatar,polish}.rs`, `renderer.rs`, `window.rs`, `src/app.rs`.
Interfaces: `layout::expanded_size(TabPage) -> (f32,f32,f32)`, `layout::expanded_hitboxes(canvas,w,h,top,state) -> Vec<HitBox>`; existing UiAction retained.
- [x] Add failing integration tests for eight navigable tabs (Tray included, Shelf omitted), bounded targets, mic icon target and media transport.
- [x] Run `cargo test -p isle-render --test visual_layout --offline`; confirm failures.
- [x] Implement shared layout, new shell, icons, hover/pressed feedback and static vector avatar, with native reduced-motion preference.
- [x] Run renderer tests and inspect fresh previews.

### Task 2: all expanded views

Files: `crates/isle-render/src/polish.rs`, focused `*_view.rs`, `chat.rs`, `chat_render.rs`, provider path assets.
Interfaces: views consume shared hitboxes; no service protocol changes.
- [x] Implement icon navigation and all views, preserving every action (including all ten control tiles).
- [x] Render AI chat/quotas, control, agenda, weather, media, notifications, focus modes and tray, plus empty/error states.
- [x] Test input/hitbox invariants and inspect at 100%, 125%, 150%, 200%.

### Task 3: actual Windows media artwork

Files: `crates/isle-core/src/{artwork,state}.rs`, `crates/isle-platform-win/src/{media,artwork}.rs`, Cargo features, `src/app.rs`, renderer image cache.
Interfaces: `Artwork {width:u32,height:u32,rgba:Vec<u8>}` behind shared ownership; identity tied to app/title/artist/album.
- [x] Add failing tests for byte/dimension limits, stale identity clearing and missing/error cases.
- [x] Read Thumbnail with bounded cancellable waits on worker; WIC normalize and cache; bounded post-track retries.
- [x] Draw actual artwork in media page/compact bar; neutral fallback on absence.
- [x] Run focused tests including PNG/JPEG decoding on Windows.

### Task 4: verify and install for monitor validation

Files: preview example, validation report, `dist/isle-win.exe`; installed app `C:\Tools\isle-win\isle-win.exe`.
- [x] Run workspace tests and release build; obtain one independent whole-change review and address important findings.
- [x] Capture actual Rust previews, not HTML; inspect every view and states at requested scales.
- [x] Back up installed exe, restart only the long-running island process, copy verified release, confirm installed hash/process/log.
- [x] Report actual runtime evidence separately from tests/previews; record unavailable live gates.

## Execution rulings

- The user explicitly expects the redesigned real executable and asked to validate it on their monitor. Proceed with implementation without another approval round.
- Work directly in the existing `feat/isle-win` checkout: source files are clean, and the draft assets/spec belong to this task. No Git writes or new worktree needed for this local executable handoff.
- Exact view dimensions may be adjusted to preserve all existing controls and readable content. Shared hitboxes and tests govern placement.
