//! Up Next's own mixes: a row can pick the length and style of the mix
//! into its track, over the panel's (Automix's LENGTH and STYLE). The
//! picks are kept by track id and handed to Automix when they change.

use std::collections::HashMap;

use super::Browser;
use crate::UpNextUi;

/// What the panel's LENGTH menu calls each length, in `automix::LENGTHS` order.
const LENGTHS: [&str; 7] = [
    "0 s", "12 s", "15 s", "30 s", "16 bars", "32 bars", "Phrases",
];
/// And STYLE, in `automix::STYLES` order.
const STYLES: [&str; 4] = ["Smooth", "Bass swap", "Filter", "Cut"];

/// A row's length and style, as indices; None follows the panel.
pub(super) type Picks = (Option<usize>, Option<usize>);

/// "32 bars · Cut": the picks, leaving out what follows the panel.
pub(super) fn label(picks: Picks) -> String {
    let parts: Vec<&str> = [
        picks.0.and_then(|l| LENGTHS.get(l)).copied(),
        picks.1.and_then(|s| STYLES.get(s)).copied(),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(" · ")
}

impl Browser {
    pub(super) fn install_up_next(
        ui: &UpNextUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_set_mix(move |row, length, style| {
            w(&|b| b.set_row_mix(row.max(0) as usize, length, style))
        });
    }

    /// The picks for the mix into the track at library index `index`.
    pub(super) fn mix_into(&self, index: usize) -> Picks {
        self.library
            .get(index)
            .and_then(|t| self.mixes.get(&t.id))
            .copied()
            .unwrap_or((None, None))
    }

    /// Picks the mix into Up Next's row `row`: `length` and `style` are 0
    /// for the panel's, else 1 + the index.
    fn set_row_mix(&mut self, row: usize, length: i32, style: i32) {
        let Some(&index) = self.side[0].get(row) else {
            return;
        };
        let id = self.library[index].id;
        let pick = |v: i32| (v > 0).then(|| v as usize - 1);
        let picks = (pick(length), pick(style));
        if picks == (None, None) {
            self.mixes.remove(&id);
        } else {
            self.mixes.insert(id, picks);
        }
        self.mixes_changed = true;
        self.refresh_side();
    }

    /// The picks, when they changed since last asked, for Automix.
    pub fn take_mixes(&mut self) -> Option<HashMap<usize, Picks>> {
        std::mem::take(&mut self.mixes_changed).then(|| self.mixes.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_leave_out_the_panels() {
        assert_eq!(label((Some(5), Some(3))), "32 bars · Cut");
        assert_eq!(label((None, Some(1))), "Bass swap");
        assert_eq!(label((Some(0), None)), "0 s");
        assert_eq!(label((None, None)), "");
    }
}
