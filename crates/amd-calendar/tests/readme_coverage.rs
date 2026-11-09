//! The README coverage table is documentation of the registry, not a second
//! source of truth. This test fails when they disagree, so a venue cannot be
//! flipped to verified (or have its window changed) without the table following.

use amd_calendar::{VENUES, registry::SUN_THU};

fn session_cell(v: &amd_calendar::Venue) -> String {
    let windows: Vec<String> = v
        .sessions
        .iter()
        .map(|w| {
            format!(
                "{:02}:{:02}–{:02}:{:02}",
                w.open_min / 60,
                w.open_min % 60,
                w.close_min / 60,
                w.close_min % 60
            )
        })
        .collect();
    let mut cell = windows.join(", ");
    if v.trading_days == SUN_THU {
        cell.push_str(" (Sun–Thu)");
    }
    cell
}

#[test]
fn readme_coverage_table_matches_registry() {
    let readme = include_str!("../../../README.md");
    let rows: Vec<Vec<&str>> = readme
        .lines()
        .filter(|l| l.starts_with("| "))
        .map(|l| l.trim_matches('|').split('|').map(str::trim).collect())
        .collect();

    for v in VENUES {
        let row = rows
            .iter()
            .find(|r| r.first() == Some(&v.code.as_str()))
            .unwrap_or_else(|| panic!("README coverage table has no row for {}", v.code));
        assert_eq!(row[3], v.currency.as_str(), "{}: currency", v.code);
        assert_eq!(row[4], session_cell(v), "{}: session window", v.code);
        let verified = if v.sessions_verified { "✅" } else { "—" };
        assert_eq!(row[5], verified, "{}: verified flag", v.code);
    }
}
