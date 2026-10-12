use std::{env, ffi::c_void, mem::size_of};

const PERIOD_MS: u32 = 1;
const CONFIGURATION_VARIABLE: &str = "RUBICON_WINDOWS_TIMER_RESOLUTION";
const PROCESS_POWER_THROTTLING: i32 = 4;
const POWER_THROTTLING_VERSION: u32 = 1;
const IGNORE_TIMER_RESOLUTION: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
struct PowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessInformation(
        process: *mut c_void,
        information_class: i32,
        information: *mut c_void,
        information_size: u32,
    ) -> i32;
    fn SetProcessInformation(
        process: *mut c_void,
        information_class: i32,
        information: *const c_void,
        information_size: u32,
    ) -> i32;
}

fn power_throttling_state() -> Result<PowerThrottlingState, String> {
    let mut state = PowerThrottlingState {
        version: POWER_THROTTLING_VERSION,
        control_mask: 0,
        state_mask: 0,
    };

    // SAFETY: The repr(C) buffer matches PROCESS_POWER_THROTTLING_STATE.
    // The current-process pseudo handle is valid and must not be closed.
    let result = unsafe {
        GetProcessInformation(
            GetCurrentProcess(),
            PROCESS_POWER_THROTTLING,
            (&mut state as *mut PowerThrottlingState).cast(),
            size_of::<PowerThrottlingState>() as u32,
        )
    };

    if result == 0 {
        return Err(format!(
            "Reading process timer policy failed: {}",
            std::io::Error::last_os_error(),
        ));
    }

    Ok(state)
}

fn set_power_throttling_state(state: &PowerThrottlingState) -> Result<(), String> {
    // SAFETY: Windows reads the correctly sized repr(C) buffer synchronously.
    // This changes only the calling process's policy.
    let result = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            PROCESS_POWER_THROTTLING,
            (state as *const PowerThrottlingState).cast(),
            size_of::<PowerThrottlingState>() as u32,
        )
    };

    if result == 0 {
        return Err(format!(
            "Setting process timer policy failed: {}",
            std::io::Error::last_os_error(),
        ));
    }

    Ok(())
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

/// Keeps the Windows timer-resolution request active for the server lifetime.
pub(super) struct TimerResolutionGuard {
    previous_policy: PowerThrottlingState,
}

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

        Self::enable().map(Some)
    }

    fn enable() -> Result<Self, String> {
        let previous_policy = power_throttling_state()?;
        let requested_policy = PowerThrottlingState {
            control_mask: previous_policy.control_mask | IGNORE_TIMER_RESOLUTION,
            state_mask: previous_policy.state_mask & !IGNORE_TIMER_RESOLUTION,
            ..previous_policy
        };

        // Windows 11 may ignore timeBeginPeriod for an inactive process.
        // Preserve execution-speed policy and explicitly honor timer requests.
        set_power_throttling_state(&requested_policy)?;

        // SAFETY: WinMM accepts a period in milliseconds and retains no Rust pointers.
        let result = unsafe { timeBeginPeriod(PERIOD_MS) };

        if result != 0 {
            let failure = format!(
                "Windows timer resolution request failed: timeBeginPeriod({PERIOD_MS}) returned {result}."
            );
            set_power_throttling_state(&previous_policy)
                .map_err(|restore| format!("{failure} {restore}"))?;
            return Err(failure);
        }

        Ok(Self { previous_policy })
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

        if let Err(error) = set_power_throttling_state(&self.previous_policy) {
            eprintln!("Restoring process timer policy failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IGNORE_TIMER_RESOLUTION, PowerThrottlingState, TimerResolutionGuard,
        power_throttling_state, set_power_throttling_state,
    };

    struct RestorePolicy(PowerThrottlingState);

    impl Drop for RestorePolicy {
        fn drop(&mut self) {
            set_power_throttling_state(&self.0).expect("Restore original test-process policy");
        }
    }

    #[test]
    fn timer_guard_honors_resolution_and_restores_previous_policy() {
        let original = power_throttling_state().expect("Read original process policy");
        let _restore = RestorePolicy(original);
        let ignored = PowerThrottlingState {
            control_mask: original.control_mask | IGNORE_TIMER_RESOLUTION,
            state_mask: original.state_mask | IGNORE_TIMER_RESOLUTION,
            ..original
        };
        set_power_throttling_state(&ignored).expect("Simulate ignored timer requests");

        let guard = TimerResolutionGuard::enable().expect("Enable precise timers");
        let active = power_throttling_state().expect("Read active process policy");
        assert_eq!(active.control_mask, ignored.control_mask);
        assert_eq!(
            active.state_mask,
            ignored.state_mask & !IGNORE_TIMER_RESOLUTION
        );

        drop(guard);
        assert_eq!(
            power_throttling_state().expect("Read restored policy"),
            ignored
        );
    }
}
