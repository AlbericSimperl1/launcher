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
        _ => util::hyprland_blur_enabled()
            .unwrap_or_else(|| std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()),
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
    let radius = cfg.radius;
    let inner = (radius as f64 * 0.6).round() as u32;
    // let ms = cfg.animation_ms;
    let font = &cfg.font;
    let accent = &cfg.accent;
    let shadow_a = if dark { 0.45 } else { 0.22 };

    format!(
        r#"
window.launcher {{ background: transparent; }}
window.launcher * {{ font-family: "{font}", "SF Pro Display", "Inter", "Cantarell", sans-serif; }}

.card {{
  background-color: {bg};
  color: {fg};
  border: 1px solid {border};
  border-radius: {radius}px;
  padding: 14px;
  box-shadow: 0 28px 70px rgba(0, 0, 0, {shadow_a}), 0 6px 18px rgba(0, 0, 0, 0.18);
}}
.search-row {{ padding: 0 4px 0 6px; }}
entry.search {{
  background: none; border: none; box-shadow: none; outline: none;
  color: {fg}; caret-color: {accent};
  font-size: 18px; font-weight: 300; min-height: 36px; padding: 0 4px;
}}
entry.search > text > placeholder {{ color: {dim}; }}
.crumb {{ color: {dim}; font-size: 13px; font-weight: 500; padding: 0 2px 0 4px; }}

button.mode {{
  min-width: 32px; min-height: 32px; padding: 0;
  border-radius: {inner}px; border: none; box-shadow: none; outline: none;
  background: {chip}; color: {fg};
}}
button.mode:hover {{ background: {chip_hover}; }}
button.mode.active {{ background: {accent}; }}

.results-sep {{ background: {sep}; min-height: 1px; margin: 10px 6px 6px 6px; }}
.divider {{ background: {sep}; min-height: 1px; margin: 8px 12px; }}

.row {{ padding: 6px 10px; border-radius: {inner}px; transition: background 120ms ease; }}
.row.selected {{ background: {sel}; }}
.row .title {{ font-size: 14px; font-weight: 500; }}
.row .subtitle {{ font-size: 11px; color: {dim}; }}

.tile {{ padding: 8px 2px 6px 2px; border-radius: {inner}px; transition: background 120ms ease; }}
.tile.selected {{ background: {sel}; }}
.tile .name {{ font-size: 11px; }}

.empty {{ color: {dim}; font-size: 13px; padding: 20px 0; }}
.calc-result {{ font-size: 36px; font-weight: 200; }}
.calc-expr {{ color: {dim}; font-size: 13px; }}
.calc-hint {{ color: {dim}; font-size: 11px; }}
.calc-error {{ color: {error}; font-size: 18px; font-weight: 400; }}

popover.ctx-menu > contents {{
  background: {menu_bg}; color: {fg};
  border: 1px solid {border}; border-radius: {inner}px; padding: 4px;
  box-shadow: 0 12px 36px rgba(0, 0, 0, 0.35);
}}
popover.ctx-menu button {{
  background: none; border: none; box-shadow: none; outline: none;
  color: {fg}; border-radius: {inner}px; padding: 6px 12px; min-height: 0;
}}
popover.ctx-menu button:hover {{ background: {sel}; }}
scrolledwindow undershoot, scrolledwindow overshoot {{ background: none; }}
scrollbar, scrollbar trough {{ background: none; border: none; box-shadow: none; padding: 0; margin: 0 2px; min-width: 4px; }}
scrollbar slider, scrollbar.vertical slider, scrollbar.vertical:hover slider {{ background: {chip_hover}; border: none; border-radius: 4px; min-width: 3px; min-height: 28px; margin: 0; }}
"#
    )
}
