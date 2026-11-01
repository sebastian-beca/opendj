# Tasks

## 1. HID backend
- [x] 1.1 Read `<info><devices><product>` from `.hid.xml` mappings; match devices as Mixxx's `matchProductInfo` does
- [x] 1.2 Find and open devices with `hidapi` (only the vendors mappings name; shared on macOS)
- [x] 1.3 A thread per device: read input reports, run timers and connections, write output reports
- [x] 1.4 Mixxx's `controller` HID API in the script prelude; output reports kept as `HidIoOutputReport` keeps them
- [x] 1.5 Let a file's top-level `class`/`const`/`let` replace an earlier file's `var` global, as Qt allows (S4 MK3's `Deck`)

## 2. App
- [x] 2.1 HID controllers in Settings › Controllers (Automatic or Off), with a reason when one won't open
- [x] 2.2 Controller Editor, Map mode and Quick map leave HID controllers out

## 3. Mappings and docs
- [x] 3.1 Bundle Mixxx's Traktor HID mappings and `common-hid-packet-parser.js`
- [x] 3.2 S2 MK3, S3 and S4 MK3 on the supported list as Beta, with guide pages; website lists HID mappings
- [x] 3.3 Linux udev rule, shipped in the download
- [ ] 3.4 Testers try the S2 MK3, S3 and S4 MK3 on macOS, Windows and Linux
