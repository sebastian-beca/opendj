# Controller mappings

These are Mixxx's MIDI controller mappings (`*.midi.xml`) and their
JavaScript, copied unchanged from Mixxx's `res/controllers` at commit
0fffd3d0d07b755c34c1730f7b87afadd8f2be19 (2026-10-11). HID and bulk
mappings and the scripts only they use, TypeScript sources and screen (QML)
files are left out because mixxx-rs only drives MIDI controllers so far.
`crates/controllers/tests/mappings.rs` checks that every mapping here loads.

Each file keeps its authors' copyright notices and licence. Mixxx and its
mappings are licensed under the GNU General Public License, version 2 or
later; `lodash.mixxx.js` is a build of Lodash, licensed under the MIT
licence (see its header). The authors of each mapping are listed in its
`<info><author>` element.

mixxx-rs looks for this folder next to its executable (inside the app
bundle on macOS), in the repository, or wherever `MIXXX_RS_CONTROLLERS`
points, and picks a mapping for each connected controller by name.
