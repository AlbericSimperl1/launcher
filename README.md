# rust-launcher

Apple-achtige launcher voor **CachyOS + Hyprland (Wayland)**, 100% Rust: `gtk4` + `gtk4-layer-shell`,
`tokio` voor achtergrondtaken, `nucleo` voor fuzzy matching. Frosted glass via Hyprland-blur,
afgeronde hoeken, zachte schaduw en fade/scale-animaties.

## Architectuur

```
Hyprland keybind ──► `rust-launcher` (D-Bus single instance) ──► daemon toggelt het venster
                                   │
   main.rs   GTK-app + CLI (--daemon/--show/--hide/--toggle), tokio-runtime, logging
   app.rs    Launcher: venster (layer-shell overlay), toetsen, modi, state ↔ UI, mpsc-berichten
   ui/       bar (zoekbalk + vlaggen), results (lijst/calc/leeg), grid (apps + contextmenu),
             theme (CSS), icons (Lucide-achtige SVG's)
   modes/    files · calc · browser · apps (pure logica, geen GTK)
   tokio     bestandsindex, fuzzy filter, bookmarks, .desktop-scan → Msg → GTK-mainloop
```

## Installatie (CachyOS)

```bash
sudo pacman -S --needed base-devel rust gtk4 gtk4-layer-shell pkgconf inter-font
cd rust-launcher
cargo build --release          # zonder warnings
cargo install --path .         # installeert naar ~/.cargo/bin/rust-launcher
mkdir -p ~/.config/rust-launcher
cp config.example.toml ~/.config/rust-launcher/config.toml
```

Zorg dat `~/.cargo/bin` in je `PATH` staat (of gebruik het volledige pad in de Hyprland-config).
`rusqlite` gebruikt de gebundelde SQLite (vereist `base-devel`), voor Firefox-bookmarks.

## Hyprland

`~/.config/hypr/hyprland.conf`:

```ini
# Daemon bij het inloggen (laat apps/bookmarks snel openen) + toggle op Super+Space
exec-once = rust-launcher --daemon
bind = SUPER, space, exec, rust-launcher
```

Gebruik je Noctalia's eigen launcher-binding op `Super+Space`? Verwijder of wijzig die dan.

Blur + layerrules (namespace is `rust-launcher`):

```ini
# Hyprland >= 0.53
layerrule = blur on, match:namespace rust-launcher
layerrule = ignore_alpha 0.3, match:namespace rust-launcher
layerrule = no_anim on, match:namespace rust-launcher

# Hyprland < 0.53
# layerrule = blur, rust-launcher
# layerrule = ignorealpha 0.3, rust-launcher
# layerrule = noanim, rust-launcher
```

`ignorealpha` zorgt dat alleen de kaart (niet de transparante rest van het scherm-overlay) geblurd wordt.
Zorg dat `decoration { blur { enabled = true } }` aanstaat. Met `blur = "auto"` detecteert de launcher dit
via `hyprctl` en gebruikt anders `fallback_opacity` (donkere, vrijwel dichte achtergrond).

## Gebruik

| Actie | Toets |
|---|---|
| Openen / sluiten | `rust-launcher` (toggle), `Esc` |
| Modus via vlag | `f␣` bestanden · `c␣` calculator · `b␣` browser · gewoon typen = apps |
| Modus via knop | klik op een icoon naast de zoekbalk |
| Navigeren | pijltjes; `→`/`←` in bestanden: map in / één niveau terug |
| Bestanden | `Enter` xdg-open · `Ctrl+Enter` file manager · `Shift+Enter` editor · `Ctrl+H` verborgen |
| Calculator | `Enter` kopieert + leegt · `Ctrl+Enter`/`Tab` voegt resultaat in · `ans` = vorig resultaat |
| Apps | `Enter`/klik start · rechtermuisknop: pin/verberg · `Ctrl+P` pin |

CLI: `--daemon`, `--show`, `--hide`, `--toggle`, `--unhide-all` (verborgen apps terugzetten).

## Bestanden

- Config: `~/.config/rust-launcher/config.toml` (zie `config.example.toml`)
- State (pins/verborgen): `~/.config/rust-launcher/state.json`
- Log: `~/.local/state/rust-launcher/launcher.log` (`RL_LOG=debug` voor meer detail)

## Modules

- `config.rs` — serde/TOML-config met defaults.
- `state.rs` — pins en verborgen apps (JSON); `ans` en geschiedenis blijven in het geheugen.
- `desktop.rs` — `.desktop`-parser en app-start via GIO.
- `bookmarks.rs` — Firefox-forks (`places.sqlite`, gekopieerd incl. WAL) en Chromium-`Bookmarks`.
- `modes/calc.rs` — eigen parser: `+ - * / % ^`, haakjes, `sqrt sin cos tan log ln pow abs …`, `pi e ans`.
- `modes/files.rs` — BFS-index, nucleo-matching, openen via xdg-open / file manager / editor.
- `modes/browser.rs` — URL-detectie, zoek-URL, openen in nieuw tabblad.
- `modes/apps.rs` — filter + pins boven, divider, rest alfabetisch.
- `ui/*` — zoekbalk, resultaten, grid, thema (CSS) en iconen.
