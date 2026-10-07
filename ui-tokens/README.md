# ui-tokens

Design tokens for the desktop UI (and any other app that wants them). A theme is a file:

- `base.json` — spacing, radii, fonts → `--<key>`
- `themes/<id>.json` — `{ "name", "colors": {…} }` → `--color-<key>`; `dark` is the default,
  others apply via `<html data-theme="<id>">`. All themes must define the same color keys.

`cargo xtask tokens` (also part of `cargo xtask check`) writes
`apps/desktop/src/lib/tokens.css`. Commit the generated file together with the token change.
