use anchor_lang::prelude::*;

#[cfg(not(any(test, feature = "client")))]
pub fn get_recent_epoch() -> Result<u64> {
    Ok(Clock::get()?.epoch)
}

#[cfg(any(test, feature = "client"))]
pub fn get_recent_epoch() -> Result<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / (2 * 24 * 3600))
}
