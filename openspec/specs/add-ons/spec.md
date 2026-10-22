# add-ons Specification

## Purpose
How openDJ is extended: the fixed plugin points (store search, inbox hints, library import, rail sources, settings pages), add-ons as separate programs, and the rail sources add-ons provide.

## Requirements

### Requirement: Plugin points
A plugin SHALL be able to search a store and send the DJ to buy (never taking payment or playing audio), recognise its files in the purchase inbox, import another app's library, provide a rail source with saved searches and downloads, and have its own Settings page.

#### Scenario: Settings page
- **WHEN** an add-on declares settings
- **THEN** Settings shows a page for it, saved under `plugins.<id>`

### Requirement: Add-ons as programs
Every executable in `<config>/addons/`, plus any named in `OPENDJ_ADDONS`, SHALL be started at launch and spoken to over stdin/stdout, one JSON message per line; the first request is `initialize`. Errors SHALL be `network`, `format` or `unsupported`. Installing an add-on SHALL NOT need rebuilding openDJ.

#### Scenario: Drop-in add-on
- **WHEN** an add-on program is copied into the add-ons folder and openDJ restarts
- **THEN** its source shows in the rail

### Requirement: Isolation
Each plugin SHALL run on its own thread in the app, so a slow or failing add-on never holds up the UI or the audio.

#### Scenario: Hung add-on
- **WHEN** an add-on stops answering
- **THEN** the decks and the UI keep working

### Requirement: Search sources
An add-on source SHALL show its saved searches down the left (newest first, named as the add-on names them, kept across restarts, Clear all), results as they arrive, and a download per result; finished downloads SHALL join the collection. The Downloading bar SHALL open the downloads list with cancel, retry failed and clear finished.

#### Scenario: Download joins library
- **WHEN** a download from an add-on source finishes
- **THEN** the file is in the Collection

### Requirement: Download folder setting
A source's settings SHALL be able to pick its download folder.

#### Scenario: Custom folder
- **WHEN** a download folder is chosen for a source
- **THEN** its downloads land there

### Requirement: Licence-sensitive add-ons stay outside
Add-ons that fetch content of unclear licence (such as Soulseek) SHALL live outside the openDJ repository, with the DJ responsible for what they download.

#### Scenario: Soulseek not bundled
- **WHEN** openDJ is installed from a release
- **THEN** no Soulseek add-on is included
