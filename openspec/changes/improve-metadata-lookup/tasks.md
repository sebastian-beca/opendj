# Tasks

## 1. Look-up hit rate

- [x] 1.1 Search as tagged, then without brackets and guests, then by words; drop matches under 50%
- [x] 1.2 Treat placeholder tags as empty; use the file name for a missing artist or title
- [x] 1.3 Genres from MusicBrainz (recording, else artist as an unticked guess); year from the first release
- [x] 1.4 Pace requests per service, retry once, tell not found from unreachable

## 2. Reviewed fixes

- [x] 2.1 Fill from file name opens the review table
- [x] 2.2 File-name suggestions join the look-up results where the match gave no name
- [x] 2.3 Missing or poor metadata says what the flagged tracks lack
- [x] 2.4 Update website/book/src/guide/collection-health.md
- [ ] 2.5 Try a batch on Seba's library with and without an AcoustID key (online APIs are blocked in the cloud container)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
