use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};
use pocket_drivers::{DamageRect, InputClasses, InputFrameBuilder, InputSource};
use pocket_embassy::{Display, QuickJs, Ui};
use pocket_runtime::Cow;
use static_cell::StaticCell;
use std::sync::atomic::{AtomicU32, Ordering};
static HEARTBEATS: AtomicU32 = AtomicU32::new(0);
struct Screen;
impl Display for Screen {
    type Error = ();
    async fn present(&mut self, _: &[u16], _: u32, _: &[DamageRect]) -> Result<(), ()> {
        Timer::after(Duration::from_millis(2)).await;
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
#[embassy_executor::task]
async fn heartbeat() {
    loop {
        HEARTBEATS.fetch_add(1, Ordering::Relaxed);
        Timer::after_millis(1).await;
    }
}
#[embassy_executor::task]
async fn application() {
    let guest = QuickJs::new(
        pocket_embassy_host_check::config([32, 32]),
        128 * 1024,
        b"globalThis.frames=0;globalThis.frame=()=>{frames++}",
        Cow::Borrowed(&[]),
    )
    .unwrap();
    let mut ui = Ui::new(guest).unwrap();
    let mut display = Screen;
    let mut input = Input;
    {
        let result = ui.run(&mut input, &mut display);
        // The runner and the timeout are polled on one executor thread.
        let mut result = std::pin::pin!(result);
        let mut timeout = std::pin::pin!(Timer::after_millis(250));
        std::future::poll_fn(|cx| {
            if let std::task::Poll::Ready(error) = result.as_mut().poll(cx) {
                panic!("runner stopped: {error:?}");
            }
            timeout.as_mut().poll(cx)
        })
        .await;
    }
    ui.guest()
        .evaluate(b"if(frames<5)throw Error('runner did not advance')")
        .unwrap();
    let beats = HEARTBEATS.load(Ordering::Relaxed);
    assert!(beats > 20, "heartbeat stalled: {beats}");
    println!("PASS: Embassy UI runner with async display; heartbeat={beats}");
    std::process::exit(0);
}
fn main() {
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    EXECUTOR.init(Executor::new()).run(|spawner: Spawner| {
        spawner.spawn(heartbeat().unwrap());
        spawner.spawn(application().unwrap());
    });
}
