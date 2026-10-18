//! Host library. The replay binary reuses the monitor.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

pub mod bundle;
pub mod clock;
pub mod command_out;
pub mod config;
pub mod eval;
pub mod eval_worker;
pub mod event;
pub mod fail_closed;
pub mod fault;
pub mod fence;
pub mod gate;
pub mod link;
pub mod link_age;
pub mod mavlink_link;
pub mod mode;
pub mod monitor;
pub mod operator;
pub mod quarantine;
pub mod read;
pub mod reason;
pub mod recovery;
pub mod replay;
pub mod ring;
pub mod runtime;
pub mod shadow;
pub mod sitl;
pub mod spec;
pub mod stale;
pub mod startup;
pub mod tick;
pub mod tick_log;
pub mod tracker;
pub mod watchdog;
pub mod weights;
