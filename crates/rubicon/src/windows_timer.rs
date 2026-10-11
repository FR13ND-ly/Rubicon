use std::env;

const PERIOD_MS: u32 = 1;
const CONFIGURATION_VARIABLE: &str = "RUBICON_WINDOWS_TIMER_RESOLUTION";

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

/// Keeps the Windows timer-resolution request active for the server lifetime.
pub(super) struct TimerResolutionGuard;

impl TimerResolutionGuard {
    /// Defaults to 1 ms; `default` disables Rubicon's resolution request.
    pub(super) fn from_environment() -> Result<Option<Self>, String> {
        match env::var(CONFIGURATION_VARIABLE) {
            Ok(value) if value == "default" => return Ok(None),
            Ok(value) if value == "1ms" => {}
            Err(env::VarError::NotPresent) => {}
            _ => {
                return Err(format!(
                    "{CONFIGURATION_VARIABLE} must be '1ms' or 'default'."
                ));
            }
        }

        // SAFETY: WinMM accepts a period in milliseconds and retains no Rust pointers.
        let result = unsafe { timeBeginPeriod(PERIOD_MS) };

        if result != 0 {
            return Err(format!(
                "Windows timer resolution request failed: timeBeginPeriod({PERIOD_MS}) returned {result}."
            ));
        }

        Ok(Some(Self))
    }
}

impl Drop for TimerResolutionGuard {
    fn drop(&mut self) {
        // SAFETY: Each guard pairs one successful request with the same period.
        let result = unsafe { timeEndPeriod(PERIOD_MS) };

        if result != 0 {
            eprintln!(
                "Windows timer resolution release failed: timeEndPeriod({PERIOD_MS}) returned {result}."
            );
        }
    }
}
