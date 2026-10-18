# Watchdog path

The watchdog thread does not call the interpreter, the filter, or the tracker. It reads the published tick and, on a miss, asks the mode sender for a recovery command.

## Shared resources

It still shares two things with the eval path.

| Resource | Why it is shared | What it does not share |
| --- | --- | --- |
| Socket | The mode sender writes the recovery command on the same link. | The interpreter, the filter, and the tracker. |
| Clock | The miss window is `now_ms` from the monotonic clock. | Wall time. A step back does not shrink the window. |

The runtime runs it on its own thread, `rta-watchdog`, in `crates/rta-host/src/runtime.rs`. It sends on its own clone of the socket handle with its own frame buffer and sequence. It does not wait for the tick thread. It drives the same recovery latch as the tick thread (`docs/recovery.md`), under one lock, so no attempt goes out twice. It drives the latch only while the tick thread is stalled.

A hung evaluator no longer stalls the tick thread; the deadline in `docs/deadline.md` reverts it inside one tick. The watchdog stays the backstop for a tick thread that stalls anywhere else.

A killed eval thread leaves the last published tick where it was. The watchdog fires from that tick. It does not need the interpreter to be running.
