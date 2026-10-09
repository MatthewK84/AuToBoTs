# Switch invariants

The table in `rta-switch` covers every verdict and the fail-closed inputs the host maps to `Revert`: no verdict, a late tick, a latched interpreter fault, a missing heartbeat, and a write failure.

A `Revert` command is `Mode` or `Idle`. The complex-function request is not on that value. `Inhibit` clears the commit flag. A test fails if inhibit carries a commit. These tests do not open a socket.
