//! App-grid: filteren en pins/verborgen apps toepassen.

use crate::desktop::DesktopApp;
use crate::state::State;
use crate::util::Fuzzy;

/// Geeft (vastgepinde apps, overige apps) terug voor de huidige zoekterm.
pub fn build_view(apps: &[DesktopApp], state: &State, query: &str) -> (Vec<DesktopApp>, Vec<DesktopApp>) {
    let visible: Vec<&DesktopApp> = apps.iter().filter(|a| !state.is_hidden(&a.id)).collect();
    let query = query.trim();

    let mut pinned: Vec<DesktopApp> = state
        .pinned
        .iter()
        .filter_map(|id| visible.iter().find(|a| &a.id == id).map(|a| (*a).clone()))
        .collect();

    if query.is_empty() {
        let rest = visible
            .into_iter()
            .filter(|a| !state.is_pinned(&a.id))
            .cloned()
            .collect();
        return (pinned, rest);
    }

    let mut fz = Fuzzy::new(query);
    let score = |a: &DesktopApp, fz: &mut Fuzzy| -> Option<u32> {
        let by_name = fz.score(&a.name).map(|s| s.saturating_mul(2));
        let by_kw = fz.score(&a.keywords);
        by_name.max(by_kw)
    };

    let mut scored_pinned: Vec<(u32, DesktopApp)> = pinned
        .drain(..)
        .filter_map(|a| score(&a, &mut fz).map(|s| (s, a)))
        .collect();
    scored_pinned.sort_by(|a, b| b.0.cmp(&a.0));

    let mut scored_rest: Vec<(u32, DesktopApp)> = visible
        .into_iter()
        .filter(|a| !state.is_pinned(&a.id))
        .filter_map(|a| score(a, &mut fz).map(|s| (s, a.clone())))
        .collect();
    scored_rest.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));

    (
        scored_pinned.into_iter().map(|(_, a)| a).collect(),
        scored_rest.into_iter().map(|(_, a)| a).collect(),
    )
}
