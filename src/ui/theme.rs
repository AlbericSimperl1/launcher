//! Thema en CSS-generatie (frosted glass, afgeronde hoeken, zachte schaduw).

use crate::config::Appearance;
use crate::util;
use gtk4 as gtk;

pub struct Palette {
    pub fg: &'static str,
    pub dim: &'static str,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            fg: "#f5f5f7",
            dim: "#a1a1a6",
        }
    } else {
        Palette {
            fg: "#1d1d1f",
            dim: "#6e6e73",
        }
    }
}

pub fn resolve_dark(cfg: &Appearance) -> bool {
    match cfg.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => {
            let Some(settings) = gtk::Settings::default() else {
                return true;
            };
            let name = settings.gtk_theme_name().unwrap_or_default().to_lowercase();
            settings.is_gtk_application_prefer_dark_theme()
                || name.contains("dark")
                || !name.contains("light")
        }
    }
}

pub fn resolve_blur(cfg: &Appearance) -> bool {
    match cfg.blur.as_str() {
        "on" | "true" => true,
        "off" | "false" => false,
        _ => util::hyprland_blur_enabled().unwrap_or(false),
    }
}

pub fn css(cfg: &Appearance, dark: bool, blur: bool) -> String {
    let p = palette(dark);
    let alpha = if blur {
        cfg.opacity
    } else {
        cfg.fallback_opacity
    }
    .clamp(0.0, 1.0);
    let (bg, border, chip, chip_hover, sel, sep, menu_bg, error) = if dark {
        (
            format!("rgba(28, 28, 30, {alpha})"),
            "rgba(255, 255, 255, 0.16)",
            "rgba(255, 255, 255, 0.08)",
            "rgba(255, 255, 255, 0.15)",
            "rgba(255, 255, 255, 0.13)",
            "rgba(255, 255, 255, 0.10)",
            "rgba(44, 44, 46, 0.97)",
            "#ff453a",
        )
    } else {
        (
            format!("rgba(246, 246, 248, {alpha})"),
            "rgba(0, 0, 0, 0.10)",
            "rgba(0, 0, 0, 0.06)",
            "rgba(0, 0, 0, 0.11)",
            "rgba(0, 0, 0, 0.09)",
            "rgba(0, 0, 0, 0.08)",
            "rgba(250, 250, 252, 0.98)",
            "#d70015",
        )
    };
    let (fg, dim) = (p.fg, p.dim);
    let radius = cfg.radius.max(16);
    let ms = cfg.animation_ms;
    let font = &cfg.font;
    let accent = &cfg.accent;
    let shadow_a = if dark { 0.45 } else { 0.22 };

    format!(
        r#"
window.launcher {{ background: transparent; }}
window.launcher * {{ font-family: "{font}", "Mononoki Nerd Font Mono", "SF Pro Display", "Inter", "Cantarell", sans-serif; }}

.card {{
  background-color: {bg};
  color: {fg};
  border: 1px solid {border};
  border-radius: {radius}px;
  padding: 14px;
  box-shadow: 0 28px 70px rgba(0, 0, 0, {shadow_a}), 0 6px 18px rgba(0, 0, 0, 0.18);
  opacity: 0;
  transform: scale(0.96);
  transition: opacity {ms}ms ease-out, transform {ms}ms cubic-bezier(0.2, 0.9, 0.25, 1);
}}
.card.open {{ opacity: 1; transform: scale(1); }}

.search-row {{ padding: 2px 6px 2px 8px; }}
entry.search {{
  background: none; border: none; box-shadow: none; outline: none;
  color: {fg}; caret-color: {accent};
  font-size: 22px; font-weight: 300; min-height: 44px; padding: 0 6px;
}}
entry.search > text > placeholder {{ color: {dim}; }}
.crumb {{ color: {dim}; font-size: 15px; font-weight: 500; padding: 0 2px 0 6px; }}

button.mode {{
  min-width: 40px; min-height: 40px; padding: 0;
  border-radius: 13px; border: none; box-shadow: none; outline: none;
  background: {chip}; color: {fg};
  transition: background {ms}ms ease;
}}
button.mode:hover {{ background: {chip_hover}; }}
button.mode.active {{ background: {accent}; }}

.results-sep {{ background: {sep}; min-height: 1px; margin: 12px 8px 6px 8px; }}
.divider {{ background: {sep}; min-height: 1px; margin: 10px 14px; }}

.row {{ padding: 8px 12px; border-radius: 13px; transition: background 120ms ease; }}
.row.selected {{ background: {sel}; }}
.row .title {{ font-size: 15px; font-weight: 500; }}
.row .subtitle {{ font-size: 12px; color: {dim}; }}

.tile {{ padding: 12px 4px 8px 4px; border-radius: 16px; transition: background 120ms ease; }}
.tile.selected {{ background: {sel}; }}
.tile .name {{ font-size: 12px; }}

.empty {{ color: {dim}; font-size: 14px; padding: 28px 0; }}
.calc-result {{ font-size: 44px; font-weight: 200; }}
.calc-expr {{ color: {dim}; font-size: 14px; }}
.calc-hint {{ color: {dim}; font-size: 12px; }}
.calc-error {{ color: {error}; font-size: 20px; font-weight: 400; }}

popover.ctx-menu > contents {{
  background: {menu_bg}; color: {fg};
  border: 1px solid {border}; border-radius: 14px; padding: 6px;
  box-shadow: 0 12px 36px rgba(0, 0, 0, 0.35);
}}
popover.ctx-menu button {{
  background: none; border: none; box-shadow: none; outline: none;
  color: {fg}; border-radius: 9px; padding: 7px 14px; min-height: 0;
}}
popover.ctx-menu button:hover {{ background: {sel}; }}
scrolledwindow undershoot, scrolledwindow overshoot {{ background: none; }}
scrollbar slider {{ background: {chip_hover}; border-radius: 4px; min-width: 5px; }}
"#
    )
}
