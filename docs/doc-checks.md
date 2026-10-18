# Document and source-text checks

The repository checks that its notes, plans, indexes, scripts, and sources say what the code does: a hazard row names its test, the SCI holds the lock hash, the ICD names the setpoint mask, the watchdog source never names the interpreter. These checks are tests. They are not flight code.

They live in `crates/rta-docs`, not in `rta-host`. No binary depends on `rta-docs`, so nothing in it is linked into a flight build. `cargo test --locked --workspace` (CI `compile`) runs every one of them, so a stale note still fails CI.

| Where | What |
| --- | --- |
| `crates/rta-docs/src/*.rs` | The evidence tables and their checks: trace, SCI hashes, lockfile, planning, packet, requirements, tools, claims, JSF baseline, safety scale, common mode, coverage gate, panic policy, degraded matrix, ICD lists, learning hazards |
| `crates/rta-docs/src/host/<module>.rs` | Checks that name one `rta-host` module: its note, its manifest, or its source text |

`rta-host` keeps its functional tests. A test there may read `spec/monitor.lola`, `config/*.toml`, or a file under `crates/rta-host/fixtures` as input data. It does not read `docs/`, `scripts/`, `.github/`, a manifest, or its own source text.

The trace check still looks for each `trace: Hn` tag next to its test in the flight sources, so a traced test stays in its flight module.

`rta-switch`, `rta-filter`, and `rta-tracker` still hold a few document checks in their test modules. Those are compiled only under `cfg(test)` and are not moved here.

## Property tests and fuzzing

`crates/rta-host/tests/properties.rs` holds proptest properties for the parsing and decision paths: `read::apply`, the raw `COMMAND_ACK` reader and the frame decoder, config validation, endpoint parsing, the switch, the startup gate, setpoint encoding, and the PX4 mode decode. Each property runs a fixed number of cases on a fixed seed with no failure file, so a run is the same on every machine.

`fuzz/` holds cargo-fuzz targets for the inputs that come from outside the process: `read_apply`, `mavlink_frames` (the frame decoder and the raw `COMMAND_ACK` reader), and `config_parse` (config text and the endpoint string). It is its own workspace, built with nightly, and is not part of `cargo test`. Seeds are in `fuzz/corpus/<target>`.

```
cargo +nightly fuzz run mavlink_frames fuzz/corpus/mavlink_frames -- -max_total_time=60
```

CI job `fuzz-smoke` runs each target for a fixed number of runs on a fixed seed. It is a smoke check that the targets build and the seeds do not crash, not a fuzzing campaign.
