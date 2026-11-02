//! Runs Mixxx's HID mappings for the Traktor Kontrol range against the real
//! engine controls, with a stand-in device in place of the hardware.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_control::ControlRegistry;
use opendj_controllers::hid::{self, Io};
use opendj_controllers::{default_mappings_dir, Controller, Mapping};

/// A device that answers every report request with zeros and keeps what it
/// is sent. Input reports are as long as the S4 MK3's longest; feature
/// reports (calibration data, on the S2 MK1 and Z1) are 32 bytes, as theirs.
#[derive(Default)]
struct FakeDevice {
    feature_sent: RefCell<Vec<Vec<u8>>>,
}

impl Io for FakeDevice {
    fn write(&self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        self.feature_sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_input(&self, _: u8) -> Option<Vec<u8>> {
        Some(vec![0; REPORT - 1])
    }
    fn get_feature(&self, _: u8) -> Option<Vec<u8>> {
        Some(vec![0; 32])
    }
}

/// The length of the input reports fed in, with the ID.
const REPORT: usize = 128;

fn load(file: &str) -> (Arc<ControlRegistry>, Controller, Rc<FakeDevice>) {
    let reg = ControlRegistry::new();
    let _engine = opendj_engine::engine::Engine::new(&reg, 44_100.0);
    let path = default_mappings_dir().unwrap().join(file);
    let mapping = Mapping::load(&path).unwrap();
    assert!(mapping.is_hid(), "{file}");
    let device = Rc::new(FakeDevice::default());
    let c = Controller::new_hid("test", mapping, reg.clone(), device.clone())
        .unwrap_or_else(|e| panic!("{file}: {e}"));
    (reg, c, device)
}

fn settle(c: &mut Controller, ms: u64) {
    let start = Instant::now();
    for i in 0..=ms {
        c.poll(start + Duration::from_millis(i));
    }
}

#[test]
fn every_bundled_hid_mapping_loads_and_takes_reports() {
    let dir = default_mappings_dir().unwrap();
    let mappings = hid::list_mappings(&dir);
    assert!(mappings.len() >= 9, "{}", mappings.len());
    let mut failures = Vec::new();
    for (name, path, _) in mappings {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let (_reg, mut c, _device) = load(&file);
        settle(&mut c, 50);
        let _ = c.take_reports();
        // Every report ID with all bits clear, then all set, as if every
        // button were pressed and every fader at the top.
        for id in 0..=0x05u8 {
            for fill in [0x00u8, 0xFF, 0x00] {
                let mut report = vec![fill; REPORT];
                report[0] = id;
                c.receive_report(&report);
                c.poll(Instant::now());
            }
        }
        settle(&mut c, 50);
        let _ = c.take_reports();
        c.shutdown();
        // Mixxx's F1 script calls a shutdown function it doesn't have.
        let errors: Vec<String> = c
            .errors()
            .into_iter()
            .filter(|e| !e.starts_with("KontrolF1.shutdown(): not a function"))
            .collect();
        if !errors.is_empty() {
            failures.push(format!("{name}: {errors:#?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn get(reg: &ControlRegistry, group: &str, key: &str) -> f64 {
    reg.get(group, key)
        .unwrap_or_else(|| panic!("{group},{key} missing"))
        .get()
}

/// The S2 MK3's play button and channel fader, as its reports carry them,
/// reach the decks, and its lights go out as output reports.
#[test]
fn traktor_kontrol_s2_mk3() {
    let (reg, mut c, _device) = load("Traktor Kontrol S2 MK3.hid.xml");
    settle(&mut c, 50);
    let reports = c.take_reports();
    assert!(!reports.is_empty(), "init lights the controller");
    assert!(
        reports.iter().all(|r| r.len() > 1),
        "each report has an ID and data: {reports:x?}"
    );

    // Buttons: report 0x01. The parser ignores the first report it sees.
    let mut buttons = vec![0u8; 22];
    buttons[0] = 0x01;
    c.receive_report(&buttons);
    let idle = buttons.clone();
    // PLAY on deck 1: bit 0x08 of byte 2, counting the ID as byte 0.
    buttons[0x02] |= 0x08;
    c.receive_report(&buttons);
    c.receive_report(&idle);
    settle(&mut c, 30);
    assert_eq!(get(&reg, "[Channel1]", "play"), 1.0, "PLAY starts deck 1");
    let _ = c.take_reports();
    // The engine sets the indicator while playing; do it by hand here.
    reg.get("[Channel1]", "play_indicator").unwrap().set(1.0);
    settle(&mut c, 30);
    let lights = c.take_reports();
    assert!(!lights.is_empty(), "the PLAY light follows");

    // Faders and knobs: report 0x02, 16-bit little-endian values.
    let mut analog = vec![0u8; 40];
    analog[0] = 0x02;
    c.receive_report(&analog);
    analog[0x03] = 0xFF;
    analog[0x04] = 0x7F;
    c.receive_report(&analog);
    settle(&mut c, 30);
    let v = get(&reg, "[Channel1]", "volume");
    assert!(v > 0.3, "deck 1's channel fader moved the volume: {v}");
    assert_eq!(c.errors(), Vec::<String>::new());
}
