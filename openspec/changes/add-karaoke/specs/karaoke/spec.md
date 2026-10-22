## Purpose

Hosting karaoke: playing CDG and MP4 karaoke files with lyrics on a second screen, and managing a singer queue.

## ADDED Requirements

### Requirement: Karaoke files
CDG+MP3 and MP4 karaoke files SHALL play with their lyrics shown in a separate window.

#### Scenario: CDG
- **WHEN** a CDG pair is loaded
- **THEN** lyrics show in sync

### Requirement: Singer queue
A singer queue SHALL hold singers with their song and key change, applied on load.

#### Scenario: Key per singer
- **WHEN** a singer's song is queued at -2
- **THEN** it loads shifted down two semitones
