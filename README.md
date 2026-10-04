# drawingportfolio

The source of a personal website: a drawing portfolio, a practice board of
drawing tasks, a fitness and food log, a crate-sorting and van-loading board,
a set of party games, and a CV. It is one Rust program (Axum, SQLite, Askama
templates and HTMX) that renders its pages on the server — the sorting board is
drawn in the browser from the day's plan — with no front-end build step.

How it is put together, the decisions behind it and how it is tested are
explained on the site itself: [How this site works](https://portfolio.dblo.net/docs).

## Running it locally

You need a Rust toolchain and, for the gates, Node.js.

```bash
cp .env.example .env            # every variable is documented in the file
SQLX_OFFLINE=true cargo run     # serves http://localhost:3000
```

The SQLite files are created and migrated on first start. `SQLX_OFFLINE=true` builds
against the committed query cache in `.sqlx/`, so no database has to exist
before the first build.

Two gates check a change:

```bash
./scripts/check.sh    # fast: compiles the workspace and checks JavaScript syntax
./scripts/verify.sh   # full: formatting, lints, the whole test suite and the board's checks
```

To run the tests alone, use `cargo test --workspace`: the root crate is also a
package, so a bare `cargo test` skips the drinks games' suite.

## How I use AI

**How I use AI.** I design the solution first — the data model, the
boundaries, the failure cases, what has to be tested — then hand Claude Code
a precise brief to implement it. Because I know the technology and where it
usually goes wrong, the brief names the right constraints up front, so the
first pass lands close and there are few rounds of correction. That keeps
both cycle time and token cost low. **Review is scaled to risk:** I weigh how
critical a change is and how likely it is to go wrong. Routine, low-stakes
work gets a quick check; anything that reaches live users, touches security
or data, or is complex enough to hide errors gets a thorough review — often
of the plan, before any code is written.
