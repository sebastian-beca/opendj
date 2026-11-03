//! The Instruments source in the library rail (`ui/instruments.slint`):
//! the Groove Box's kits, the ticked instrument plugins and the inputs,
//! dragged onto a deck to play through its channel. Dropping is handled on
//! the screen (`InstrumentsUi.drop`), which sets the deck's controls; the
//! engine does the rest (`[ChannelN],live_input`, `[GrooveN],enabled`).

use opendj_engine::kits;
use slint::{ComponentHandle, DataTransfer, ModelRc, SharedString, VecModel};

use crate::audio_plugins::Found;
use crate::{AppWindow, InstrumentsUi, LiveItem, Preferences};

/// What dragging an item carries: `opendj-live:<kind>:<index>`.
const LIVE_PAYLOAD: &str = "opendj-live:";

/// Kinds of item, as `LiveItem.kind`.
const KIT: i32 = 1;
const PLUGIN: i32 = 2;
const LAST_KIND: i32 = 4;

fn payload(kind: i32, index: i32) -> String {
    format!("{LIVE_PAYLOAD}{kind}:{index}")
}

/// The item a drag's text names, as `kind * 1000 + index`.
fn parse(text: &str) -> Option<i32> {
    let (kind, index) = text.strip_prefix(LIVE_PAYLOAD)?.split_once(':')?;
    let (kind, index): (i32, i32) = (kind.parse().ok()?, index.parse().ok()?);
    ((1..=LAST_KIND).contains(&kind) && (0..1000).contains(&index)).then_some(kind * 1000 + index)
}

pub fn install(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    ui.on_payload(|kind, index| DataTransfer::from(SharedString::from(payload(kind, index))));
    ui.on_item(|data| data.plain_text().ok().and_then(|t| parse(&t)).unwrap_or(-1));
    let weak = app.as_weak();
    ui.on_open_settings(move |page, tab| {
        if let Some(app) = weak.upgrade() {
            app.global::<Preferences>().invoke_show_tab(page, tab);
        }
    });
    let kits: Vec<LiveItem> = kits::KITS
        .iter()
        .enumerate()
        .map(|(i, k)| LiveItem {
            kind: KIT,
            index: i as i32,
            name: format!("{} kit", k.name).into(),
            detail: k.about.into(),
        })
        .collect();
    ui.set_kits(ModelRc::new(VecModel::from(kits)));
}

/// Lists the ticked instrument plugins, in the Instrument input's order,
/// under the names its picker shows.
pub fn show_plugins(app: &AppWindow, instruments: &[Found], names: &[String]) {
    let items: Vec<LiveItem> = instruments
        .iter()
        .zip(names)
        .enumerate()
        .map(|(i, (f, name))| LiveItem {
            kind: PLUGIN,
            index: i as i32,
            name: name.as_str().into(),
            detail: if f.vendor.is_empty() {
                f.format.into()
            } else {
                format!("{}  ·  {}", f.vendor, f.format).into()
            },
        })
        .collect();
    app.global::<InstrumentsUi>()
        .set_plugins(ModelRc::new(VecModel::from(items)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_read_back_as_their_items() {
        assert_eq!(parse(&payload(1, 2)), Some(1002));
        assert_eq!(parse(&payload(2, 0)), Some(2000));
        assert_eq!(parse(&payload(4, 0)), Some(4000));
        assert_eq!(parse("opendj-row:3"), None);
        assert_eq!(parse("opendj-live:9:0"), None);
        assert_eq!(parse("opendj-live:1:x"), None);
    }
}
