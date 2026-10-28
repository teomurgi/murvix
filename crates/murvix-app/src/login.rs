//! User-controlled login activation of the main app, not the server host.
//!
//! Querying status is read-only. Registration is only called by explicit setup
//! or Settings actions; native approval/revocation is never repaired at launch.

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "ServiceManagement's Objective-C API is called through the system runtime"
)]
mod macos {
    use std::ffi::{CStr, c_char, c_void};
    use std::ptr;

    type Object = *mut c_void;
    type Selector = *mut c_void;

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Selector;
        fn class_getClassMethod(class: Object, selector: Selector) -> Object;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> Object;
        fn objc_autoreleasePoolPop(pool: Object);
    }

    struct Pool(Object);

    impl Pool {
        fn new() -> Self {
            // SAFETY: the token is kept on this thread and popped once on drop.
            Self(unsafe { objc_autoreleasePoolPush() })
        }
    }

    impl Drop for Pool {
        fn drop(&mut self) {
            // SAFETY: this is the token from this thread's matching push.
            unsafe { objc_autoreleasePoolPop(self.0) };
        }
    }

    fn selector(name: &CStr) -> Selector {
        // SAFETY: the runtime copies the selector's valid, terminated name.
        unsafe { sel_registerName(name.as_ptr()) }
    }

    // These signatures match the declared Objective-C methods used below.
    // All receivers remain alive within the current autorelease pool.
    unsafe fn object(receiver: Object, name: &CStr) -> Object {
        // SAFETY: objc_msgSend must be cast to the exact method ABI.
        let send = unsafe {
            std::mem::transmute::<
                unsafe extern "C" fn(),
                unsafe extern "C" fn(Object, Selector) -> Object,
            >(objc_msgSend)
        };
        // SAFETY: callers use only object/pointer-returning zero-argument methods.
        unsafe { send(receiver, selector(name)) }
    }

    unsafe fn integer(receiver: Object, name: &CStr) -> isize {
        // SAFETY: NSInteger is pointer-sized on both supported macOS ABIs.
        let send = unsafe {
            std::mem::transmute::<
                unsafe extern "C" fn(),
                unsafe extern "C" fn(Object, Selector) -> isize,
            >(objc_msgSend)
        };
        // SAFETY: callers use only NSInteger-returning zero-argument methods.
        unsafe { send(receiver, selector(name)) }
    }

    fn service_class() -> Result<Object, String> {
        // SAFETY: class lookup does not instantiate or register any service.
        let class = unsafe { objc_getClass(c"SMAppService".as_ptr()) };
        // Check the selector too, so older macOS versions fail without sending
        // an unrecognized selector. SMAppService first shipped in macOS 13.
        if class.is_null()
            // SAFETY: class is non-null, and this only inspects method metadata.
            || unsafe { class_getClassMethod(class, selector(c"mainAppService")) }.is_null()
        {
            return Err("Login activation requires macOS 13 or later.".into());
        }
        Ok(class)
    }

    fn main_service() -> Result<Object, String> {
        let class = service_class()?;
        // SAFETY: mainAppService is the checked class property. Its object lives
        // through the caller's autorelease pool and represents the main app.
        let service = unsafe { object(class, c"mainAppService") };
        if service.is_null() {
            Err("ServiceManagement could not find the main application.".into())
        } else {
            Ok(service)
        }
    }

    pub(super) fn status() -> String {
        let _pool = Pool::new();
        let service = match main_service() {
            Ok(service) => service,
            Err(error) => return format!("unavailable — {error}"),
        };
        // SAFETY: status is the SMAppServiceStatus (NSInteger) property.
        match unsafe { integer(service, c"status") } {
            0 => "notRegistered — Murvix does not open at login.".into(),
            1 => "enabled — Murvix opens at login.".into(),
            2 => "requiresApproval — Allow Murvix in Login Items Settings to open it at login."
                .into(),
            3 => "notFound — macOS could not find the login application.".into(),
            other => format!("unknown ({other}) — macOS returned an unrecognized login status."),
        }
    }

    /// Called only for an explicit user action, never as launch-time repair.
    pub(super) fn set_enabled(enabled: bool) -> Result<(), String> {
        if enabled {
            crate::cli_install::persistent_integration_allowed()
                .map_err(|error| error.to_string())?;
        }
        let _pool = Pool::new();
        let service = main_service()?;
        // SAFETY: status is the SMAppServiceStatus (NSInteger) property.
        let current = unsafe { integer(service, c"status") };
        match (enabled, current) {
            (true, 1) | (false, 0) => return Ok(()),
            // Re-registering after revocation is not an approval mechanism.
            // Preserve the user's choice and direct them to the native UI.
            (true, 2) => return Err(
                "Login activation requires approval. Open Login Items Settings to allow Murvix."
                    .into(),
            ),
            _ => {}
        }
        let method = if enabled {
            c"registerAndReturnError:"
        } else {
            c"unregisterAndReturnError:"
        };
        let mut error = ptr::null_mut();
        // SAFETY: both methods return an 8-bit BOOL and take an autoreleasing
        // NSError **. Its storage and any returned object outlive this call.
        let success = unsafe {
            let send = std::mem::transmute::<
                unsafe extern "C" fn(),
                unsafe extern "C" fn(Object, Selector, *mut Object) -> i8,
            >(objc_msgSend);
            send(service, selector(method), &raw mut error) != 0
        };
        if success {
            Ok(())
        } else {
            let action = if enabled { "enable" } else { "disable" };
            Err(format!(
                "Could not {action} login activation: {}",
                native_error(error)
            ))
        }
    }

    fn native_error(error: Object) -> String {
        if error.is_null() {
            return "ServiceManagement returned no error details.".into();
        }
        // SAFETY: failed register/unregister supplies an NSError alive in this
        // autorelease pool. These are its documented property types.
        unsafe {
            let description = string(object(error, c"localizedDescription"));
            let domain = string(object(error, c"domain"));
            let code = integer(error, c"code");
            format!("{description} ({domain}, {code})")
        }
    }

    unsafe fn string(value: Object) -> String {
        if value.is_null() {
            return "Unknown native error".into();
        }
        // SAFETY: value is an NSString from NSError. UTF8String returns a
        // null-terminated borrowed pointer valid until the pool drains.
        let bytes = unsafe { object(value, c"UTF8String") }.cast::<c_char>();
        if bytes.is_null() {
            return "Unknown native error".into();
        }
        // SAFETY: the NSString-owned buffer is valid for this immediate copy.
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned()
    }

    pub(super) fn open_settings() -> Result<(), String> {
        let _pool = Pool::new();
        let class = service_class()?;
        let method = selector(c"openSystemSettingsLoginItems");
        // SAFETY: this only checks availability of the native class method.
        if unsafe { class_getClassMethod(class, method) }.is_null() {
            return Err("Login Items Settings is unavailable on this macOS version.".into());
        }
        // SAFETY: the documented class method takes no arguments and returns
        // void. Apple provides no success/error result for opening this panel.
        unsafe {
            let send = std::mem::transmute::<
                unsafe extern "C" fn(),
                unsafe extern "C" fn(Object, Selector),
            >(objc_msgSend);
            send(class, method);
        }
        Ok(())
    }
}

/// The native registration state, refreshed without registering anything.
pub(crate) fn status() -> String {
    #[cfg(target_os = "macos")]
    return macos::status();
    #[cfg(not(target_os = "macos"))]
    "unavailable — Login activation is only supported on macOS.".into()
}

/// Explicit user opt-in/out. Revoked approval must be restored in macOS Settings.
pub(crate) fn set_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::set_enabled(enabled);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Login activation is only supported on macOS.".into())
    }
}

/// Opens Apple's Login Items panel; it does not alter registration or approval.
pub(crate) fn open_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::open_settings();
    #[cfg(not(target_os = "macos"))]
    Err("Login Items Settings is only supported on macOS.".into())
}
