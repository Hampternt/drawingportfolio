repo: Hampternt/drawingportfolio
branch: master

## Last sync

date: 2026-09-08T01:20:00Z

### Updated in this project

- Recreated the live hub (`/`) and admin (`/admin`) screens from source, light theme intact.
- New dark hub on the Hampter Design System: grid-texture hero, Ctrl+K palette bar, six section tiles.
- New dark admin: sidebar + panes for Posts (caption/tags/visibility), New post and owner-only Accounts.
- Added two new sections ahead of the repo: a CV page built from the user's own CV, and an employer-facing "How this site works" page written from `CLAUDE.md` and `docs/design.md`.

## Screen map

| Screen | Built from |
| --- | --- |
| `Current Hub.dc.html` | `templates/hub/hub.html`, `templates/base.html`, `src/routes/hub.rs`, `static/style.css` (lines 1–345) |
| `Current Admin.dc.html` | `templates/admin.html`, `src/routes/admin.rs` (`admin_post_card_html`), `static/style.css` |
| `Hub.dc.html` | `templates/hub/hub.html`, `templates/base.html`, `static/palette.js`, `CLAUDE.md` |
| `Admin.dc.html` | `templates/admin.html`, `src/routes/admin.rs`, `templates/users.html`, `templates/artportfolio/partials/card_edit_popover.html`, `templates/partials/post_card.html` |
| `CV.dc.html` | `uploads/cv-jesper-lovland.html` (user-supplied CV, content verbatim) |
| `How it works.dc.html` | `CLAUDE.md`, `docs/design.md`, `src/routes/*.rs` route summaries, `static/palette.js` |

## Notes

- New routes these two pages would need: `GET /cv` and `GET /docs` (or `/how-it-works`) — plus a nav link, a `HubCard`, and a `palette.js` COMMANDS entry each, per the repo's "adding a new section" checklist.
- The CV still carries two unfilled fields from the source document: `[ARBEIDSGIVER]` and `[ÅRSTALL]`, highlighted in amber on the page.
