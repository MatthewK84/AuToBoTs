# Startup and shutdown

Startup is config, spec load, log open, then socket. A failed step does not skip ahead to the socket. The link is not constructed if the spec did not load.

Shutdown is stop setpoints, send the recovery mode, flush the log, then exit. The mode command is on the sink before the flush returns.

| Phase | Order |
| --- | --- |
| Startup | config, spec load, log open, socket |
| Shutdown | stop setpoints, recovery mode, flush log, exit |

A spec load error is the acceptance case. The link stays unconstructed.
