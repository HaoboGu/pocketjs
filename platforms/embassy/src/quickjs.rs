use alloc::{boxed::Box, string::String};
use core::{
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};
use pocket_drivers::abi::{PocketGuestV1, PocketResult, PocketRuntime};
use pocket_runtime::{
    Cow, Guest, GuestEnvironment, GuestFrame, OpContext, RuntimeConfig, RuntimeError,
    abi::{self, CGuest, Handle},
};

#[derive(Debug)]
pub enum Error {
    Allocation,
    Runtime(RuntimeError),
    JavaScript(String),
}
impl From<RuntimeError> for Error {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

unsafe extern "C" {
    fn pocket_embassy_guest_create(heap_limit: usize, stack_limit: usize) -> *mut c_void;
    fn pocket_embassy_guest_binding(guest: *mut c_void) -> *mut c_void;
    fn pocket_embassy_guest_context(guest: *mut c_void) -> *mut c_void;
    fn pocket_embassy_guest_budget(guest: *mut c_void, polls: u32);
    fn pocket_embassy_guest_error(guest: *mut c_void) -> *const c_char;
    fn pocket_embassy_guest_destroy(guest: *mut c_void);
    fn pocket_guest_quickjs_vtable() -> *const PocketGuestV1;
}

/// A realm and its native scene, owned by one task. Neither is Send or Sync.
pub struct QuickJs {
    handle: Box<Handle>,
    guest: CGuest,
    engine: NonNull<c_void>,
    // The guest's immutable ArrayBuffer borrows these bytes until shutdown.
    assets: Cow<'static, [u8]>,
    viewport: [u32; 2],
    cadence: u32,
    framebuffer_budget: usize,
}
impl QuickJs {
    /// Load a trusted, compiler-produced JS/PAK pair matching host_abi.
    /// The application owns the allocator and the current task's stack.
    pub fn new(
        config: RuntimeConfig,
        stack_limit: usize,
        javascript: &[u8],
        assets: Cow<'static, [u8]>,
    ) -> Result<Self, Error> {
        Self::with_setup(config, stack_limit, javascript, assets, |_| {})
    }
    /// Configure native extensions before guest initialization. Setup runs on the owner task.
    pub fn with_setup(
        config: RuntimeConfig,
        stack_limit: usize,
        javascript: &[u8],
        assets: Cow<'static, [u8]>,
        setup: impl FnOnce(&mut Self),
    ) -> Result<Self, Error> {
        if stack_limit == 0 || config.profile.density != 1 || config.profile.auxiliary.is_some() {
            return Err(RuntimeError::InvalidConfiguration.into());
        }
        let viewport = config.profile.viewport;
        let cadence = *config
            .profile
            .cadences
            .first()
            .ok_or(RuntimeError::InvalidConfiguration)?;
        let heap_limit = config.heap_limit;
        let framebuffer_budget = config.frame_buffer_budget;
        let host = config.profile.id.clone();
        let host_abi = config.host_abi;
        let handle = Handle::new(config)?;
        let engine = NonNull::new(unsafe { pocket_embassy_guest_create(heap_limit, stack_limit) })
            .ok_or(Error::Allocation)?;
        let guest = match unsafe {
            CGuest::new(
                pocket_guest_quickjs_vtable(),
                pocket_embassy_guest_binding(engine.as_ptr()),
                handle.as_ptr(),
            )
        } {
            Ok(guest) => guest,
            Err(error) => {
                unsafe { pocket_embassy_guest_destroy(engine.as_ptr()) };
                return Err(Error::Runtime(error.into()));
            }
        };
        let mut result = Self {
            handle,
            guest,
            engine,
            assets,
            viewport,
            cadence,
            framebuffer_budget,
        };
        let context = result.op_context();
        {
            let mut state = context.core_mut()?;
            state
                .ui
                .set_viewport(viewport[0] as f32, viewport[1] as f32);
            if !state.ui.set_tick_rate(cadence) {
                return Err(RuntimeError::InvalidConfiguration.into());
            }
        }
        setup(&mut result);
        result.set_budget(256);
        let environment = GuestEnvironment {
            host: &host,
            host_abi,
            tick_hz: cadence,
            sim_hz: cadence,
            viewport,
            auxiliary_viewport: None,
            auxiliary_root: -1,
            raster_density: 1,
            heap_limit,
            pak: &result.assets,
            worker_javascript: &[],
            assets: None,
        };
        result
            .guest
            .initialize(context, &environment)
            .map_err(|_| result.exception())?;
        result.evaluate(javascript)?;
        Ok(result)
    }
    pub fn evaluate(&mut self, source: &[u8]) -> Result<(), Error> {
        self.set_budget(256);
        self.guest.evaluate(source).map_err(|_| self.exception())
    }
    pub fn frame(&mut self, input: &GuestFrame<'_>) -> Result<(), Error> {
        self.set_budget(32);
        self.guest.frame(input).map_err(|_| self.exception())
    }
    /// Bound interpreter interrupt polls; this aborts runaway JS, not a time slice.
    pub fn set_budget(&mut self, polls: u32) {
        unsafe { pocket_embassy_guest_budget(self.engine.as_ptr(), polls) };
    }
    pub fn op_context(&self) -> OpContext {
        self.handle.op_context()
    }
    pub fn viewport(&self) -> [u32; 2] {
        self.viewport
    }
    pub fn cadence(&self) -> u32 {
        self.cadence
    }
    pub fn framebuffer_budget(&self) -> usize {
        self.framebuffer_budget
    }
    /// For native extensions on this task. The pointer expires when self drops.
    pub fn context_ptr(&mut self) -> *mut c_void {
        unsafe { pocket_embassy_guest_context(self.engine.as_ptr()) }
    }
    /// For the existing guest's logging/profiling hooks; same lifetime as context_ptr.
    pub fn binding_ptr(&mut self) -> *mut c_void {
        unsafe { pocket_embassy_guest_binding(self.engine.as_ptr()) }
    }
    fn exception(&self) -> Error {
        let error = unsafe { CStr::from_ptr(pocket_embassy_guest_error(self.engine.as_ptr())) };
        Error::JavaScript(error.to_string_lossy().into_owned())
    }
}
impl Drop for QuickJs {
    fn drop(&mut self) {
        self.guest.shutdown();
        unsafe { pocket_embassy_guest_destroy(self.engine.as_ptr()) };
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn pocket_runtime_op(
    runtime: *mut PocketRuntime,
    op: u16,
    args: *const c_void,
    length: usize,
    out: *mut PocketResult,
) -> i32 {
    unsafe { abi::pocket_runtime_op(runtime, op, args, length, out) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn pocket_runtime_supports_op(runtime: *const PocketRuntime, op: u16) -> u32 {
    unsafe { abi::pocket_runtime_supports_op(runtime, op) }
}
