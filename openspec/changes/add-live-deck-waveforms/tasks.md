# Tasks

## 1. Live trace

- [x] 1.1 Engine: band levels of each live deck's audio (after trim, before EQ), 1000 slices a second, handed to the app without locks
- [x] 1.2 App: the last seconds as a waveform drawn up to the play line on the deck's scrolling waveform; empty to the right
- [x] 1.3 The leading deck's beat grid and bar numbers on the live lane, at its playing tempo

## 2. Note roll

- [x] 2.1 Instrument decks: notes played as bars, fitted to the range in view, held notes lit; faint trace under it
- [x] 2.2 Rows of the current scale tinted, once Scale mode exists (`rework-instruments` 4.x)

## 3. Step lanes

- [x] 3.1 Groove Box decks: eight lanes, hits behind and ahead of the play line, fills marked, the queued groove's hits from the next bar (no lane names: a lane's sound can change with the kit)

## 4. Macro lanes

- [x] 4.1 A switch on instrument decks' waveform for the knobs as automation lines; 2-Deck only (the page of eight knobs the deck shows)
- [x] 4.2 Macro lanes at full height, a colour per knob, only the knobs that moved in view drawn; Groove Box decks keep their step lanes, without the switch
