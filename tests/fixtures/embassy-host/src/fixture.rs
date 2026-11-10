use alloc::{boxed::Box, vec};
use pocket_drivers::{DisplayRole, InputFrameBuilder, InputSource, PixelFormat};
use pocket_embassy::display::{BufferedDisplay, Presenter};
use pocket_guest_quickjs::{Limits, QuickJsGuest};
use pocket_package::{
    embedded::{EmbeddedPackage, encode, identity},
    source::MemorySource,
};
use pocket_runtime::{Cow, InputClasses, Profile, RendererId, RuntimeConfig, abi::Handle};

pub fn config(viewport: [u32; 2]) -> RuntimeConfig {
    RuntimeConfig {
        profile_hash: [0x5a; 32],
        host_abi: pocket_drivers::spec::BINDING_GENERATION,
        heap_limit: 1024 * 1024,
        frame_buffer_budget: viewport[0] as usize * viewport[1] as usize * 2,
        profile: Profile {
            id: Cow::Borrowed("embassy-test"),
            cadences: Cow::Borrowed(&[60, 30]),
            viewport,
            logical_viewports: Cow::Owned(vec![viewport]),
            dynamic_viewport: None,
            presentations: Cow::Borrowed(&[Cow::Borrowed("native")]),
            auxiliary: None,
            auxiliary_logical: Cow::Borrowed(&[]),
            density: 1,
            animation: Cow::Borrowed("composited"),
            capabilities: Cow::Borrowed(&[]),
            required_inputs: InputClasses::BUTTONS,
        },
    }
}
pub struct Input(pub u32);
impl InputSource for Input {
    fn classes(&self) -> InputClasses {
        InputClasses::BUTTONS
    }
    fn poll(&mut self, frame: &mut InputFrameBuilder) {
        frame.buttons = self.0;
    }
}
pub const SCRIPT: &[u8] = br#"
const box = ui.createNode(0);
const status = ui.createNode(1);
ui.setProp(box, 1, 12); ui.setProp(box, 2, 12);
ui.setProp(box, 64, 4294901760);
ui.insertBefore(1, box, 0);
let turns = 0;
globalThis.frame = buttons => {
    ui.setProp(box, 64, buttons ? 4278190335 : 4294901760);
    ui.setText(status, `${buttons}:${++turns}`);
};
"#;
pub fn program(code: &[u8], generation: u32) -> (Box<Handle>, Presenter) {
    let config = config([24, 16]);
    let identity = identity("app", "embassy.fixture", "Embassy fixture").unwrap();
    let plan = r#"{"animation":"composited","profile":{"tickHz":30},"viewport":{"logical":[24,16],"presentation":"native","rasterDensity":1},"features":{}}"#;
    let package = encode(EmbeddedPackage {
        target: "embassy-test",
        host_abi: generation,
        profile_hash: config.profile_hash,
        identity: &identity,
        plan: plan.as_bytes(),
        code,
        pak: &[],
    })
    .unwrap();
    let handle = Handle::new(config).unwrap();
    let guest = unsafe {
        QuickJsGuest::new(
            handle.as_ptr(),
            Limits {
                heap_bytes: 1024 * 1024,
                stack_bytes: 128 * 1024,
                interrupt_polls: 256,
            },
        )
    }
    .unwrap();
    let (display, presenter) = BufferedDisplay::new(
        DisplayRole::Primary,
        24,
        16,
        PixelFormat::Rgb565,
        24 * 16 * 2,
    )
    .unwrap();
    handle
        .with_runtime(|runtime| {
            runtime.set_package_source(Box::new(MemorySource(package)))?;
            runtime.set_guest(Box::new(guest))?;
            runtime.add_input_source(Box::new(Input(5)))?;
            runtime.set_software_renderer(
                pocket_raster::RasterRenderer::new(1).unwrap(),
                pocket_raster::RasterRenderer::new,
            )?;
            runtime.add_display(display, RendererId::SOFTWARE)?;
            Ok(())
        })
        .unwrap();
    (handle, presenter)
}
// Like the SDK archive, the program defines the C exports once per link unit.
#[unsafe(no_mangle)]
unsafe extern "C" fn pocket_runtime_op(
    runtime: *mut pocket_drivers::abi::PocketRuntime,
    op: u16,
    args: *const core::ffi::c_void,
    length: usize,
    out: *mut pocket_drivers::abi::PocketResult,
) -> i32 {
    unsafe { pocket_runtime::abi::pocket_runtime_op(runtime, op, args, length, out) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn pocket_runtime_supports_op(
    runtime: *const pocket_drivers::abi::PocketRuntime,
    op: u16,
) -> u32 {
    unsafe { pocket_runtime::abi::pocket_runtime_supports_op(runtime, op) }
}
