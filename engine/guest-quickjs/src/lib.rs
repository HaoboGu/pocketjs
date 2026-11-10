//! A Rust owner for the C guest; the program supplies its QuickJS build and runtime exports.
#![no_std]
extern crate alloc;
#[path = "alloc.rs"]
mod allocator;

use alloc::string::String;
use core::{
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};
use pocket_drivers::abi::{AdapterError, PocketGuestV1, PocketRuntime};
use pocket_runtime::{Guest, GuestEnvironment, GuestFrame, OpContext, RuntimeError, abi::CGuest};

/// Limits selected by the program. Interrupt polls are not a wall-clock deadline.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub heap_bytes: usize,
    pub stack_bytes: usize,
    pub interrupt_polls: u32,
}
#[derive(Debug)]
pub enum CreateError {
    InvalidLimits,
    Allocation,
    Binding(AdapterError),
}

unsafe extern "C" {
    fn pocket_quickjs_realm_create(heap_limit: usize, stack_limit: usize) -> *mut c_void;
    fn pocket_quickjs_realm_binding(realm: *mut c_void) -> *mut c_void;
    fn pocket_quickjs_realm_context(realm: *mut c_void) -> *mut c_void;
    fn pocket_quickjs_realm_budget(realm: *mut c_void, polls: u32);
    fn pocket_quickjs_realm_error(realm: *mut c_void) -> *const c_char;
    fn pocket_quickjs_realm_destroy(realm: *mut c_void);
    fn pocket_guest_quickjs_vtable() -> *const PocketGuestV1;
}

/// Owns a JS realm and binding, not a Runtime, package, input encoder or renderer.
/// Keep the guest on its owner task; it is neither Send nor Sync.
pub struct QuickJsGuest {
    binding: CGuest,
    realm: NonNull<c_void>,
    interrupt_polls: u32,
}
impl QuickJsGuest {
    /// Construct the guest before registering it with the canonical runtime.
    ///
    /// # Safety
    /// `runtime` must be a live `pocket_runtime::abi::Handle` on this task and
    /// outlive this guest. Register the guest in that handle, or use its matching
    /// OpContext for a program-owned embedding. In that case, borrowed package
    /// bytes in GuestEnvironment must stay valid and unchanged through shutdown.
    /// Link one set of runtime C exports.
    pub unsafe fn new(runtime: *mut PocketRuntime, limits: Limits) -> Result<Self, CreateError> {
        if runtime.is_null()
            || limits.heap_bytes == 0
            || limits.stack_bytes == 0
            || limits.interrupt_polls == 0
        {
            return Err(CreateError::InvalidLimits);
        }
        let realm = NonNull::new(unsafe {
            pocket_quickjs_realm_create(limits.heap_bytes, limits.stack_bytes)
        })
        .ok_or(CreateError::Allocation)?;
        let binding = match unsafe {
            CGuest::new(
                pocket_guest_quickjs_vtable(),
                pocket_quickjs_realm_binding(realm.as_ptr()),
                runtime,
            )
        } {
            Ok(binding) => binding,
            Err(error) => {
                unsafe { pocket_quickjs_realm_destroy(realm.as_ptr()) };
                return Err(CreateError::Binding(error));
            }
        };
        Ok(Self {
            binding,
            realm,
            interrupt_polls: limits.interrupt_polls,
        })
    }
    pub fn set_interrupt_budget(&mut self, polls: u32) {
        self.interrupt_polls = polls;
        self.budget();
    }
    /// The binding's last failure. Runtime operations report RuntimeError::Guest.
    pub fn last_error(&self) -> String {
        unsafe { CStr::from_ptr(pocket_quickjs_realm_error(self.realm.as_ptr())) }
            .to_string_lossy()
            .into_owned()
    }
    /// Borrowed JSContext for program-owned native extensions. Expires at Drop.
    pub fn context_ptr(&mut self) -> *mut c_void {
        unsafe { pocket_quickjs_realm_context(self.realm.as_ptr()) }
    }
    /// Borrowed pocket_guest_quickjs for its existing log/phase hooks.
    /// Configure hooks before Runtime::boot initializes the guest.
    pub fn binding_ptr(&mut self) -> *mut c_void {
        unsafe { pocket_quickjs_realm_binding(self.realm.as_ptr()) }
    }
    fn budget(&self) {
        unsafe { pocket_quickjs_realm_budget(self.realm.as_ptr(), self.interrupt_polls) }
    }
}
impl Guest for QuickJsGuest {
    fn initialize(
        &mut self,
        ops: OpContext,
        environment: &GuestEnvironment<'_>,
    ) -> Result<(), RuntimeError> {
        self.budget();
        self.binding.initialize(ops, environment)
    }
    fn evaluate(&mut self, code: &[u8]) -> Result<(), RuntimeError> {
        self.budget();
        self.binding.evaluate(code)
    }
    fn evaluate_bytecode(&mut self, code: &[u8]) -> Result<(), RuntimeError> {
        self.budget();
        self.binding.evaluate_bytecode(code)
    }
    fn frame(&mut self, input: &GuestFrame<'_>) -> Result<(), RuntimeError> {
        self.budget();
        self.binding.frame(input)
    }
    fn collect(&mut self) -> Result<(), RuntimeError> {
        self.budget();
        self.binding.collect()
    }
    fn shutdown(&mut self) {
        self.binding.shutdown();
    }
}
impl Drop for QuickJsGuest {
    fn drop(&mut self) {
        self.binding.shutdown();
        unsafe { pocket_quickjs_realm_destroy(self.realm.as_ptr()) };
    }
}
