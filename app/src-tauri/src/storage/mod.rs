// storage/mod.rs — backends for where photos live.
//
// R·01: local + NAS (which is really just local from the OS's POV via mount).
// R·02: S3-compat backend behind the same trait.

pub mod local;
pub mod thumbs;
pub mod watch;
