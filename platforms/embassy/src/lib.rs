#![no_std]
extern crate alloc;
#[path = "alloc.rs"]
mod c_alloc;
mod quickjs;
pub use quickjs::{Error, QuickJs};

use alloc::vec::Vec;
use embassy_time::{Instant, Timer};
use pocket_drivers::{DamageRect, InputFrame, InputFrameBuilder, InputSource};
use pocket_render::damage::{DamageTarget, DamageTracker};
use pocket_runtime::{GuestFrame, RuntimeError, input::InputEncoder, timing::TickSchedule};

/// An RGB565 display. On return or cancellation, DMA must stop borrowing pixels.
#[allow(async_fn_in_trait)]
pub trait Display {
    type Error;
    async fn present(
        &mut self,
        pixels: &[u16],
        width: u32,
        regions: &[DamageRect],
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum FrameError<E> {
    Runtime(Error),
    Display(E),
}
impl<E> From<Error> for FrameError<E> {
    fn from(value: Error) -> Self {
        Self::Runtime(value)
    }
}
impl<E> From<RuntimeError> for FrameError<E> {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value.into())
    }
}

/// Wait for a host-selected deadline. None sleeps until another future wakes it.
pub async fn wait_until(deadline_us: Option<u64>) {
    match deadline_us {
        Some(deadline) => Timer::at(Instant::from_micros(deadline)).await,
        None => core::future::pending::<()>().await,
    }
}

/// Software rendering with async display submission. The caller owns the executor.
pub struct Ui {
    guest: QuickJs,
    input: InputEncoder,
    pixels: Vec<u16>,
    damage: DamageTracker<8>,
    retry: bool,
}
impl Ui {
    pub fn new(guest: QuickJs) -> Result<Self, Error> {
        let [width, height] = guest.viewport();
        let count = (width as usize)
            .checked_mul(height as usize)
            .ok_or(RuntimeError::InvalidConfiguration)?;
        if count
            .checked_mul(2)
            .is_none_or(|bytes| bytes > guest.framebuffer_budget())
        {
            return Err(RuntimeError::InvalidConfiguration.into());
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        pixels.resize(count, 0);
        Ok(Self {
            guest,
            input: InputEncoder::new(),
            pixels,
            damage: DamageTracker::new(),
            retry: true,
        })
    }
    pub fn guest(&mut self) -> &mut QuickJs {
        &mut self.guest
    }
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }

    pub async fn frame<D: Display>(
        &mut self,
        input: &InputFrame<'_>,
        display: &mut D,
    ) -> Result<(), FrameError<D::Error>> {
        let context = self.guest.op_context();
        let encoded = self.input.encode(&mut context.core_mut()?.ui, input)?;
        self.guest.frame(&GuestFrame {
            buttons: encoded.buttons,
            analog: encoded.analog,
            touches: &encoded.touches,
            hits: &encoded.hits,
            touch_surfaces: &encoded.surfaces,
            right_analog: encoded.right_analog,
            axis_deltas: input.relative,
            motion: input.motion,
            text: input.text,
        })?;
        let [width, height] = self.guest.viewport();
        let target = DamageTarget::new(width, height, 1, 0);
        let damage = {
            let mut state = context.core_mut()?;
            state.ui.tick();
            state.ui.draw();
            if self.retry {
                self.damage.invalidate();
            }
            let words = &state.ui.current_draw_list().words;
            let damage = self
                .damage
                .prepare(&state.ui, words, target)
                .map_err(|_| RuntimeError::InvalidState)?;
            pocket_raster::render_scaled_rgb565_regions(
                &state.ui,
                words,
                &mut self.pixels,
                1,
                damage.regions(),
            );
            self.damage.commit(&state.ui, words, target);
            damage
        };
        // A failed or cancelled transfer forces a complete repaint on retry.
        self.retry = true;
        if !damage.regions().is_empty() {
            display
                .present(&self.pixels, width, damage.regions())
                .await
                .map_err(FrameError::Display)?;
        }
        self.retry = false;
        Ok(())
    }
    pub async fn run<D: Display>(
        &mut self,
        input: &mut impl InputSource,
        display: &mut D,
    ) -> Result<(), FrameError<D::Error>> {
        let hz = self.guest.cadence();
        let mut schedule =
            TickSchedule::new(hz, Instant::now().as_micros(), 2_000_000 / u64::from(hz))?;
        let mut frame = InputFrameBuilder::new();
        loop {
            wait_until(Some(schedule.deadline()?)).await;
            if schedule.poll(Instant::now().as_micros())?.is_some() {
                frame.clear();
                input.poll(&mut frame);
                self.frame(&frame.frame(), display).await?;
            }
        }
    }
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn pocket_embassy_uptime_us() -> u64 {
    Instant::now().as_micros()
}
