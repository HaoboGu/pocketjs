//! Deadline waits for an application's canonical Runtime loop.
use embassy_time::{Instant, Timer};
use pocket_runtime::{
    RuntimeError, Timing,
    timing::{TickSchedule, TickTiming},
};

pub struct Ticker {
    schedule: TickSchedule,
}
impl Ticker {
    /// Pass the timing selected by Runtime::boot, not the first advertised cadence.
    pub fn new(timing: Timing, max_lag_us: u64) -> Result<Self, RuntimeError> {
        Ok(Self {
            schedule: TickSchedule::new(timing.cadence_hz, Instant::now().as_micros(), max_lag_us)?,
        })
    }
    /// Reserve one due tick. The program calls Runtime::tick once, then drains its displays.
    pub async fn next(&mut self) -> Result<TickTiming, RuntimeError> {
        loop {
            wait_until(self.schedule.deadline()?).await;
            if let Some(tick) = self.schedule.poll(Instant::now().as_micros())? {
                return Ok(tick);
            }
        }
    }
}
pub async fn wait_until(deadline_us: u64) {
    Timer::at(Instant::from_micros(deadline_us)).await;
}
