# Nocturne design system

Nocturne is a quiet, compact dark interface: a near-neutral blue-grey ground, Inter at medium weight, soft 8px radii and an accent used as a line and a glow rather than a flood. Rules fade to transparent at their ends — over 48px a side — rather than stopping cleanly; short accent marks stay solid. Contrast comes from the tonal ramps, not from saturation, and photographs blend into the page with their dark values falling away.

## How to use this

- Take every color, font, spacing, radius and shadow from the token variables (`var(--color-*)`, `var(--font-*)`, `var(--space-*)`, `var(--radius-*)`, `var(--shadow-*)`). Never hard-code a hex, a font name or a px value the tokens already carry.
- Build with the classes in `nocturne-tokens.css` rather than inventing parallel ones.

## Direction

Left-aligned, asymmetric layouts. Flush-left headings; content hugs the left edge with whitespace on the right. Buttons are outlined (1px accent border on transparent), not solid-filled. Grounds stay desaturated, with soft gradient depth rather than flat fills.

## Color

A dark ground (`--color-bg` #161826) with `--color-text` #e9e9ed and a single accent #9184d9 — a blurple, at chroma that reads as an accent against the desaturated ramps (mono scheme: treat accent-2 as the same role). Each role carries a 100–900 tonal ramp generated in OKLCH on a shared perceptual lightness scale. On this dark ground use the dark steps (700–900) for tinted fills, hovers and subtle borders, 500 as the role's base, and the light steps (100–300) for text on those tints and for pressed states; prefer ramp steps over ad-hoc `color-mix()`. For elevation use `--shadow-sm/md/lg` rather than ad-hoc box-shadows.

## Type

Inter for headings over Inter for body text. Density 0.70× and radius 8px are already baked into the `--space-*` / `--radius-*` scales — use the variables, not raw numbers.

## Icons

Use Phosphor icons (https://phosphoricons.com) throughout — inline SVG.

## Interaction states

Interactive states are themed, never browser defaults: give every interactive element a `:hover` tint and a pressed state from the accent ramp (one step past the base — `--color-accent-400` on a dark ground, or a `color-mix()` tint for outlined/ghost variants), and style keyboard focus with `:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }` — never leave the default blue focus ring.

## Components

| Class | What it is |
| --- | --- |
| `.btn` with `.btn-primary`, `.btn-secondary`, `.btn-ghost`, `.btn-icon`, `.btn-block` | Actions — the primary is an accent outline, never a fill |
| `.tag` with `.tag-accent`, `.tag-accent-2`, `.tag-neutral`, `.tag-outline` | Small labels tinted from the ramps |
| `.field` + `label`, `.input`, `.radio` + `.dot`, `.seg` + `.seg-opt` | Form fields and choices on native elements — no script |
| `.card` with `.card-kicker`, `.card-title`, `.card-body`, `.card-meta`; `.elev-sm/md/lg` | Surface-filled content cards; elevation utilities |
| `.table` | Data tables with themed header and row rules |
| `.dialog-backdrop` + `.dialog` | A modal at the top elevation |
| `.hr` | A horizontal rule — present, but this system prefers whitespace; avoid it |

States are built in: hovers and pressed states come from the accent ramp, keyboard focus is the 2px accent `:focus-visible` ring, `::selection` is an accent tint, and disabled controls drop to 45% opacity. Don't restyle them per page. The accent-to-ground pair is tuned to at least 3:1 — enough for icons, large text and interface chrome, not for body copy — so for paragraph-size text in the accent use `--color-accent-300` rather than the accent itself.

## Do

- Keep chroma low outside the accent; lean on the `--color-neutral-*` steps for surfaces, borders and muted text.
- Use the compact spacing scale (density 0.7×) — this system is dense on purpose.
- Outline primary actions and let `:focus-visible` carry the accent.

## Don't

- Do not flood large areas with the accent or any saturated fill — the accent carries its chroma in lines and marks, never as a flood.
- Do not use pure black or pure white — every value comes from the ramps (shadow tokens excepted).
- Do not stack heavy shadows; on a dark ground elevation is an edge plus ambient darkness.
- Do not bolden headings past their 500 weight — hierarchy here is size and space.
