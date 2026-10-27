# Design

## Decisions
- Keep Mixxx XML/JS as the on-disk format so mappings stay shareable with Mixxx; script-driven controls are shown read-only with a note.
- Learn binds plain controls (`<control>` entries); script functions are not learnable.
- Custom files live in `<config>/controllers/` and win over bundled ones by controller name.
