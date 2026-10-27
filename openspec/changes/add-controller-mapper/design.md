# Design

## Decisions (Seba, 2026-10-27)

- Mixxx mappings are only read in. A DJ's own mapping is an openDJ file in `<config>/controllers/`, a superset of Mixxx's: it keeps the original's scripts and settings and adds openDJ options (value-scaling takeover, ranges, comments).
- Duplicate and rename instead of per-row overrides; one mapping per controller at a time; Reload Original always available.
- Mapping is done when not playing, in the large editor window; Map mode shows the main window behind it.
