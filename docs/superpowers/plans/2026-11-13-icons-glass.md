# Icon packs and Liquid Glass implementation plan

**Goal:** implement the user's icon-pack and glass port on feat/fin-claude, without Git writes.
**Spec:** the task in this session and 2026-11-12-finalizacao.md global rules; the session explicitly authorizes these files on this branch.
**Architecture:** core owns the glyph/name catalog and backward-compatible preferences; a cancellable platform worker downloads/loads images; the renderer caches native glyphs and resvg rasters. Glass uses a native shaped blur window behind the layered content and static isle-inspired overlays, without capture or lens.
**Tech stack:** Rust, DirectWrite/Direct2D, resvg/usvg using tiny-skia 0.11, reqwest, Windows composition.

## Constraints and review focus
- Work only in this worktree; do not commit or format the workspace; preserve the root test fixture.
- Keep pinned upstream names/versions, missing-glyph fallback, provider logos and PT-BR UI.
- No network before an explicit online-pack choice; switching/deleting cancels obsolete work.
- Bad/missing custom pictures and 404s fall back to Windows; cache keys include physical size and color.
- Old configs preserve defaults; pin state is per monitor; shortcut minimization clears the pin.
- Blur follows the animated pill and notch, disables cleanly, and creates no idle timer/capture loop.

## Task 1 (complete before Task 2)
- [x] Add failing catalog/config tests; extract the matching mac-island kIcons and exact online URLs.
- [x] Replace polylines with cached Segoe glyph/image rendering. Add SVG tint/PNG/fallback tests.
- [x] Add cancellable worker, atomic downloads, 404 markers, progress, delete and folder picker settings.
- [x] Port arrow pinning; test multi-monitor pin/unpin and shortcut behavior.
- [x] Run workspace tests/release build and inspect previews at 100%/150%.

## Task 2
- [x] Read mac-island blur/GlassLayer and isle glass/theme/shader sources; add glass preference and tests.
- [x] Implement shaped native backdrop plus tone/sheen/glow/rim/card overlays, off by default.
- [x] Run workspace tests/release build; inspect previews at both scales; document live-only gates.

## Handoff
- [x] Review final diff and write the source mapping, omissions, verification and live checklist.
- Ruling: report is staged under docs in this worktree if sandbox cannot write the requested main-checkout path; do not bypass that boundary.
- Ruling: execute inline from the fully specified user request; no extra design approval or commits.

Task 1 gate: full workspace tests, release build and Windows/Lucide previews at 1.0/1.5 passed before starting Task 2. Native desktop interactions remain live-check items.

Final gate: 523 tests passed, release build succeeded, all review findings closed, native effect graphs/clips accepted, real Lucide download/cache probe passed with Rustls, light/dark previews inspected at both scales. Live desktop capture was blocked by the native pipe error; see docs/codex-icons-glass-report.md.
