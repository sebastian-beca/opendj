# Design

## Decisions
- Transport: stdio for assistants that launch a helper, and streamable HTTP on 127.0.0.1 with a token for ones that connect to a running app.
- Tools map onto existing controls (`[ChannelN],play`) and library calls, so no new engine paths.
- Every deck-changing call is logged in the recording's control log like any other control change.
