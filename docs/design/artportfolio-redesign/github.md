repo: Hampternt/drawingportfolio
branch: master

## Last sync

date: 2026-08-07T19:10:00Z

### Updated in this project

- Recreated the current `/artportfolio` feed (light theme) as the baseline `0a`.
- Two redesign directions for the drawing feed: `1a` studio rail, `1b` contact sheet.
- Added a multi-upload tray (`1c`) with per-file visibility, reorder and shared caption.
- Visibility model extended to public / unlisted / hidden, managed inline in the feed.

## Screen map

| Screen | Repo files |
| --- | --- |
| 0a — current /artportfolio | `templates/base.html`, `templates/artportfolio/feed.html`, `src/routes/feed.rs` (`post_card_html`), `static/style.css` |
| 1a — studio direction | same as 0a + `static/palette.js` |
| 1b — contact sheet direction | same as 0a + `static/palette.js` |
| 1c — multi-upload tray | `templates/artportfolio/feed.html` (composer), `src/routes/admin.rs` (upload limits) |
