# Drinking game sound effects

Drop-in directory for the Ring of Fire sound effects. No mp3s are committed
to the repo — this directory ships with only this README. The server reads
files from here on request; the `DRINKS_SOUNDS_DIR` env var points at this
directory (defaults to `drinks-sounds` relative to the working directory).

Allowlisted filenames (any other name 404s, even if present):

- `drink.mp3`
- `shot.mp3`
- `card-draw.mp3`
- `card-use.mp3`
- `dice-roll.mp3`
- `dice-give.mp3`

Drop mp3s here with these exact names to enable sound. Missing files 404 and
the client stays silent — no sound effect is required for the game to work.
