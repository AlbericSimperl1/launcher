# rust-launcher

Apple-achtige launcher voor **CachyOS + Hyprland (Wayland)**, 100% Rust: `gtk4` + `gtk4-layer-shell`,
`tokio` voor achtergrondtaken, `nucleo` voor fuzzy matching. Frosted glass via ext-background-effect (Hyprland-blur),
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

Blur vraagt de launcher zelf aan: de kaart heeft CSS `backdrop-filter: blur()`, GTK (>= 4.24) zet dat om
naar het `ext-background-effect`-protocol en Hyprland blurt precies het gebied achter de kaart (met afgeronde
hoeken). Je hebt dus **geen** `blur`/`ignore_alpha`-layerrule nodig; zet die juist niet aan, anders blurt
Hyprland het hele overlay. Alleen animatie uitzetten is handig (namespace is `rust-launcher`):

```lua
hl.layer_rule({
    match = { namespace = "rust-launcher" },
    no_anim = true
})
```

Zorg dat `decoration.blur.enabled = true` aanstaat. `blur = "off"` vraagt geen blur aan en gebruikt
`fallback_opacity`.

De blur-**sterkte** regel je met `blur_radius` en `blur_passes` in `config.toml` (0 = Hyprlands eigen
`decoration.blur.size`/`passes`). Hyprland kent geen blur-sterkte per layer, dus de launcher zet die globale
waarden via de Hyprland-socket zolang hij open is en zet ze bij sluiten terug. Crasht de launcher terwijl hij
open is, dan blijft zijn waarde staan tot je Hyprland herlaadt. Met `opacity` regel je hoeveel van de blur
door de kaart heen zichtbaar is.

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
