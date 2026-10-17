# Time base

Ages and grant expiry use one monotonic clock. Wall time is not read.

`MonoClock` holds the last reading. A later reading that is smaller is ignored. The clock does not step back. `std::time::Instant` is monotonic. `SystemTime` is not used.

`fix_age_ms`, `link_age_ms`, and `fc_heartbeat_age_ms` are `now_ms` minus the last accept, saturating. A missing sample is not zero. It is the maximum age.

Grant expiry uses the same `now_ms` as the tick log. A grant is expired when `now_ms` is past `not_after_ms`. A wall-clock step back does not shrink an age and does not extend a grant window.

The offset between wall time and this clock is M140.9. This task does not own that conversion.
