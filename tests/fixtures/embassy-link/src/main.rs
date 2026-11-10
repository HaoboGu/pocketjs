#![no_std]
#![no_main]
extern crate alloc;

use core::{
    alloc::{GlobalAlloc, Layout},
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Waker},
};
use pocket_drivers::{DamageRect, InputFrameBuilder, InputSource};
use pocket_embassy::{Display, QuickJs, Ui};
use pocket_runtime::{Cow, InputClasses, Profile, RuntimeConfig};

// Link-only fixture: this arena and inert clock are not a board support package.
struct Bump;
#[repr(align(64))]
struct Storage([u8; 512 * 1024]);
static mut STORAGE: Storage = Storage([0; 512 * 1024]);
static NEXT: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Bump = Bump;
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = unsafe { (&raw mut STORAGE.0).cast::<u8>() as usize };
        let result = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |offset| {
            let start = (base + offset + layout.align() - 1) & !(layout.align() - 1);
            let end = start.checked_add(layout.size())?.checked_sub(base)?;
            (end <= 512 * 1024).then_some(end)
        });
        result.map_or(core::ptr::null_mut(), |offset| {
            ((base + offset + layout.align() - 1) & !(layout.align() - 1)) as *mut u8
        })
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
struct Clock;
embassy_time_driver::time_driver_impl!(static CLOCK: Clock = Clock);
impl embassy_time_driver::Driver for Clock {
    fn now(&self) -> u64 {
        0
    }
    fn schedule_wake(&self, _: u64, _: &Waker) {}
}
struct Critical;
critical_section::set_impl!(Critical);
unsafe impl critical_section::Impl for Critical {
    unsafe fn acquire() -> critical_section::RawRestoreState {}
    unsafe fn release(_: critical_section::RawRestoreState) {}
}
struct Screen;
impl Display for Screen {
    type Error = ();
    async fn present(&mut self, pixels: &[u16], _: u32, _: &[DamageRect]) -> Result<(), ()> {
        core::hint::black_box(pixels);
        Ok(())
    }
}
struct Input;
impl InputSource for Input {
    fn classes(&self) -> InputClasses {
        InputClasses::NONE
    }
    fn poll(&mut self, _: &mut InputFrameBuilder) {}
}
#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    let config = RuntimeConfig {
        profile_hash: [0x5a; 32],
        host_abi: pocket_drivers::spec::BINDING_GENERATION,
        heap_limit: 256 * 1024,
        frame_buffer_budget: 16 * 16 * 2,
        profile: Profile {
            id: Cow::Borrowed("link-test"),
            cadences: Cow::Borrowed(&[60]),
            viewport: [16, 16],
            logical_viewports: Cow::Borrowed(&[[16, 16]]),
            dynamic_viewport: None,
            presentations: Cow::Borrowed(&[Cow::Borrowed("native")]),
            auxiliary: None,
            auxiliary_logical: Cow::Borrowed(&[]),
            density: 1,
            animation: Cow::Borrowed("composited"),
            capabilities: Cow::Borrowed(&[]),
            required_inputs: InputClasses::NONE,
        },
    };
    let guest = QuickJs::new(
        config,
        64 * 1024,
        b"globalThis.frame=()=>{}",
        Cow::Borrowed(&[]),
    )
    .unwrap();
    let mut ui = Ui::new(guest).unwrap();
    let mut input = Input;
    let mut display = Screen;
    let mut future = core::pin::pin!(ui.run(&mut input, &mut display));
    let _ = core::hint::black_box(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
    );
    loop {
        core::hint::spin_loop();
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
