//! HARNESS-ONLY stub of the `tauri` crate.
//!
//! This crate exists solely so that `cargo check` can typecheck all 50 modules of
//! the M2M app on a machine that lacks the GTK/glib/gdk *development* packages
//! required to build the real `tauri`. It ships nothing and must never be linked
//! into a real build — `check.sh` rewrites `tauri::` to point here only inside
//! the throwaway harness tree.
//!
//! # Soundness
//!
//! This stub is typecheck-only. `check.sh` runs `cargo check --lib`, which never
//! executes any of it, so the deliberately-fabricated references below (see
//! `Manager::state`) are never dereferenced. They are `NonNull::dangling()`
//! rather than real pointers precisely so that a future `cargo run` against this
//! tree fails loudly instead of quietly reading garbage.

use serde::Serialize;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Stand-in for `tauri::Error`. Must be a real `std::error::Error` because
/// `Builder::setup` closures return `Result<(), Box<dyn Error>>` and use `?` on
/// menu/tray construction results.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

// ── Managed state ─────────────────────────────────────────────────────────────

thread_local! {
    /// The harness only ever *typechecks* state access, so nothing is ever
    /// inserted or read. The map exists so `Manager::state` has somewhere
    /// plausible to look if this is ever run.
    static MANAGED: RefCell<HashMap<TypeId, Box<dyn std::any::Any>>> =
        RefCell::new(HashMap::new());
}

/// `tauri::State<'r, T>` — a shared reference to a Tauri-managed value.
pub struct State<'r, T: Send + Sync + 'static> {
    inner: &'r T,
    _marker: PhantomData<&'r T>,
}

impl<'r, T: Send + Sync + 'static> State<'r, T> {
    pub fn inner(&self) -> &'r T {
        self.inner
    }
}

impl<'r, T: Send + Sync + 'static> Deref for State<'r, T> {
    type Target = T;
    fn deref(&self) -> &'r T {
        self.inner
    }
}

impl<'r, T: Send + Sync + 'static> Clone for State<'r, T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner, _marker: PhantomData }
    }
}

// ── Emitter ───────────────────────────────────────────────────────────────────

/// `tauri::Emitter` — implemented for every handle that can broadcast an event.
pub trait Emitter<R = ()> {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<R>;
}

// ── Handles ───────────────────────────────────────────────────────────────────

/// `tauri::AppHandle` — the application's primary handle.
#[derive(Clone)]
pub struct AppHandle {
    _private: (),
}

impl AppHandle {
    // Verified against tauri 2.11.4 `src/app.rs`: `exit` is *inherent* on
    // `AppHandle` (app.rs:574, inside `impl<R: Runtime> AppHandle<R>`), not a
    // `Manager` method. So a module calling `app_handle.exit(0)` correctly does
    // NOT need `use tauri::Manager`.
    //
    // `state`, `try_state` and `get_webview_window` ARE `Manager` methods
    // (lib.rs:729, 744, 576), so they deliberately have no inherent
    // counterpart here: giving them one would make `use tauri::Manager` look
    // unnecessary in every module that genuinely needs it, and the harness
    // would pass code that does not compile against the real crate.
    pub fn exit(&self, code: i32) {
        let _ = code;
    }
}

impl Emitter for AppHandle {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        let _ = (event, payload);
        Ok(())
    }
}

/// `tauri::Runtime` — the real crate uses this to thread a runtime type through
/// handles. The app never names it, so `()` stands in and every handle
/// implements `Manager<()>`.
pub trait Runtime: 'static {}
impl Runtime for () {}

/// `tauri::Manager` — access to app-managed state and windows.
pub trait Manager<R = ()> {
    fn state<T: Send + Sync + 'static>(&self) -> State<'_, T>;
    fn try_state<T: Send + Sync + 'static>(&self) -> Option<State<'_, T>>;

    fn get_webview_window(&self, label: &str) -> Option<WebviewWindow> {
        let _ = label;
        None
    }

    fn handle(&self) -> &AppHandle {
        unimplemented!("harness stub: never executed")
    }
}

impl Manager for AppHandle {
    /// Fabricates a reference. See the module docs: the harness is never run,
    /// so this is never dereferenced. `None` from `try_state` is likewise
    /// unreachable in practice, but the signature must be honoured for
    /// typechecking.
    fn state<T: Send + Sync + 'static>(&self) -> State<'_, T> {
        let ptr = MANAGED.with(|m| {
            m.borrow()
                .get(&TypeId::of::<T>())
                .and_then(|v| v.downcast_ref::<T>())
                .map_or_else(std::ptr::NonNull::<T>::dangling, |v| {
                    std::ptr::NonNull::from(v)
                })
        });
        State { inner: unsafe { ptr.as_ref() }, _marker: PhantomData }
    }

    fn try_state<T: Send + Sync + 'static>(&self) -> Option<State<'_, T>> {
        Some(<Self as Manager>::state(self))
    }
}

/// `tauri::WebviewWindow` — a handle to a single window.
#[derive(Clone)]
pub struct WebviewWindow {
    handle: AppHandle,
}

impl WebviewWindow {
    /// Real tauri returns `&AppHandle`, not `AppHandle` — the app passes this
    /// straight into helpers taking `&AppHandle`.
    pub fn app_handle(&self) -> &AppHandle {
        &self.handle
    }
    pub fn show(&self) -> Result<()> {
        Ok(())
    }
    pub fn hide(&self) -> Result<()> {
        Ok(())
    }
    pub fn set_focus(&self) -> Result<()> {
        Ok(())
    }
    pub fn is_visible(&self) -> Result<bool> {
        Ok(false)
    }
    pub fn is_null(&self) -> bool {
        false
    }
}

impl Emitter for WebviewWindow {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        let _ = (event, payload);
        Ok(())
    }
}

/// `tauri::App` — passed to `Builder::setup`.
pub struct App {
    handle: AppHandle,
}

impl App {
    pub fn handle(&self) -> &AppHandle {
        &self.handle
    }
}

impl Manager for App {
    fn state<T: Send + Sync + 'static>(&self) -> State<'_, T> {
        <AppHandle as Manager>::state(&self.handle)
    }
    fn try_state<T: Send + Sync + 'static>(&self) -> Option<State<'_, T>> {
        <AppHandle as Manager>::try_state(&self.handle)
    }
    fn get_webview_window(&self, label: &str) -> Option<WebviewWindow> {
        <AppHandle as Manager>::get_webview_window(&self.handle, label)
    }
}

impl Emitter for App {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        <AppHandle as Emitter>::emit(&self.handle, event, payload)
    }
}

// ── Window events ─────────────────────────────────────────────────────────────

/// `tauri::WindowEvent`. Only the variants the app matches on are named; the
/// real crate has many more and the app's `match` ends in `_ => {}`.
pub enum WindowEvent {
    CloseRequested { api: CloseRequestApi },
    Focused(bool),
    Resized(u32, u32),
    Destroyed,
}

/// `tauri::window::CloseRequestApi`
#[derive(Debug)]
pub struct CloseRequestApi;

impl CloseRequestApi {
    pub fn prevent_close(&self) {}
}

// ── Image ─────────────────────────────────────────────────────────────────────

pub mod image {
    /// `tauri::image::Image`
    #[derive(Clone)]
    pub struct Image<'a> {
        _bytes: &'a [u8],
        _w: u32,
        _h: u32,
    }

    impl<'a> Image<'a> {
        pub fn new(bytes: &'a [u8], w: u32, h: u32) -> Self {
            Self { _bytes: bytes, _w: w, _h: h }
        }
        pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, super::Error> {
            Ok(Self { _bytes: bytes, _w: 1, _h: 1 })
        }
        pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, super::Error> {
            let _ = path;
            Err(super::Error("harness stub".into()))
        }
    }
}

// ── Menu ──────────────────────────────────────────────────────────────────────

pub mod menu {
    use super::{Manager, Result, Runtime};

    pub struct MenuItem<Rt = ()>(std::marker::PhantomData<Rt>);
    pub struct Menu<Rt = ()>(std::marker::PhantomData<Rt>);

    impl<Rt> Clone for MenuItem<Rt> {
        fn clone(&self) -> Self {
            Self(std::marker::PhantomData)
        }
    }
    impl<Rt> Clone for Menu<Rt> {
        fn clone(&self) -> Self {
            Self(std::marker::PhantomData)
        }
    }

    impl<Rt> MenuItem<Rt> {
        pub fn id(&self) -> &str {
            ""
        }
    }

    /// Anything `MenuBuilder::item` accepts. Real tauri takes
    /// `&impl Into<MenuItemKind>`; both `MenuItem` and `PredefinedMenuItem`
    /// convert, and the app mixes them in one menu.
    pub trait MenuEntry<Rt> {}
    impl<Rt> MenuEntry<Rt> for MenuItem<Rt> {}
    impl<Rt> MenuEntry<Rt> for PredefinedMenuItem<Rt> {}

    /// `tauri::menu::MenuItemBuilder`
    pub struct MenuItemBuilder<Rt = ()>(std::marker::PhantomData<Rt>);

    impl<Rt: Runtime> MenuItemBuilder<Rt> {
        pub fn new() -> Self {
            Self(std::marker::PhantomData)
        }
        pub fn with_id(_id: &str, _text: &str) -> Self {
            Self(std::marker::PhantomData)
        }
        pub fn build(self, _manager: &(impl Manager<Rt> + ?Sized)) -> Result<MenuItem<Rt>> {
            Ok(MenuItem(std::marker::PhantomData))
        }
    }

    /// `tauri::menu::PredefinedMenuItem`
    pub struct PredefinedMenuItem<Rt = ()>(std::marker::PhantomData<Rt>);

    impl<Rt: Runtime> PredefinedMenuItem<Rt> {
        pub fn separator(_manager: &(impl Manager<Rt> + ?Sized)) -> Result<Self> {
            Ok(Self(std::marker::PhantomData))
        }
    }

    /// `tauri::menu::MenuBuilder`
    pub struct MenuBuilder<Rt = ()>(std::marker::PhantomData<Rt>);

    impl<Rt: Runtime> MenuBuilder<Rt> {
        pub fn new(_manager: &(impl Manager<Rt> + ?Sized)) -> Self {
            Self(std::marker::PhantomData)
        }
        pub fn item<E: MenuEntry<Rt>>(self, _item: &E) -> Self {
            self
        }
        pub fn items<E: MenuEntry<Rt>>(self, _items: &[&E]) -> Self {
            self
        }
        pub fn separator(self) -> Self {
            self
        }
        pub fn build(self) -> Result<Menu<Rt>> {
            Ok(Menu(std::marker::PhantomData))
        }
    }

    /// `tauri::menu::MenuId` — derefs to `str` and is `AsRef<str>`, so the
    /// app's `event.id().as_ref()` resolves to `&str`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MenuId(String);

    impl std::ops::Deref for MenuId {
        type Target = str;
        fn deref(&self) -> &str {
            &self.0
        }
    }
    impl AsRef<str> for MenuId {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }

    /// `tauri::menu::MenuEvent`
    pub struct MenuEvent;

    impl MenuEvent {
        pub fn id(&self) -> &MenuId {
            unimplemented!("harness stub: never executed")
        }
    }

    pub type CheckMenuItem<Rt = ()> = MenuItem<Rt>;
    pub type Submenu<Rt = ()> = Menu<Rt>;
}

// ── Tray ──────────────────────────────────────────────────────────────────────

pub mod tray {
    use super::{AppHandle, Manager, Result, Runtime};
    use super::menu::Menu;
    use crate::image::Image;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MouseButton {
        Left,
        Right,
        Middle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MouseButtonState {
        Up,
        Down,
    }

    /// `tauri::tray::TrayIconEvent`
    pub enum TrayIconEvent {
        Click { button: MouseButton, button_state: MouseButtonState, id: u32 },
        Enter { id: u32, rect: (f64, f64, f64, f64) },
        Leave { id: u32, rect: (f64, f64, f64, f64) },
    }

    /// `tauri::tray::TrayIcon`
    pub struct TrayIcon {
        _private: (),
    }

    impl TrayIcon {
        pub fn app_handle(&self) -> &AppHandle {
            unimplemented!("harness stub: never executed")
        }
    }

    /// `tauri::tray::TrayIconBuilder`
    pub struct TrayIconBuilder<Rt = ()>(std::marker::PhantomData<Rt>);

    impl<Rt: Runtime> TrayIconBuilder<Rt> {
        pub fn new() -> Self {
            Self(std::marker::PhantomData)
        }
        pub fn icon(self, _icon: Image<'_>) -> Self {
            self
        }
        pub fn menu(self, _menu: &Menu<Rt>) -> Self {
            self
        }
        pub fn tooltip(self, _tooltip: &str) -> Self {
            self
        }
        pub fn title(self, _title: &str) -> Self {
            self
        }
        pub fn on_menu_event<F: Fn(&AppHandle, crate::menu::MenuEvent) + Send + Sync + 'static>(
            self,
            _f: F,
        ) -> Self {
            self
        }
        pub fn on_tray_icon_event<
            F: Fn(&TrayIcon, TrayIconEvent) + Send + Sync + 'static,
        >(
            self,
            _f: F,
        ) -> Self {
            self
        }
        pub fn build(self, _manager: &(impl Manager<Rt> + ?Sized)) -> Result<TrayIcon> {
            Ok(TrayIcon { _private: () })
        }
    }
}

// ── async_runtime ─────────────────────────────────────────────────────────────

pub mod async_runtime {
    /// `tauri::async_runtime::spawn`
    ///
    /// Verified against tauri 2.11.4 `src/async_runtime.rs`: `spawn` takes a
    /// `Future<Output = T> + Send + 'static` and returns a `JoinHandle<T>`. It is
    /// a *function* over Tauri's own global runtime handle, not a
    /// `Runtime`-parameterised trait method, so `maintenance.rs` calls it
    /// without importing anything.
    pub fn spawn<F, R>(future: F) -> tokio::task::JoinHandle<R>
    where
        F: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        tokio::spawn(future)
    }

    /// `tauri::async_runtime::spawn_blocking`
    pub fn spawn_blocking<F, R>(f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        tokio::task::spawn_blocking(f)
    }
}

// ── Plugin stubs ──────────────────────────────────────────────────────────────
//
// The real app references these as separate crates (`tauri_plugin_dialog::init()`).
// `check.sh` rewrites those crate names to these modules.

pub mod plugin_dialog {
    use super::{Plugin, Result};
    pub fn init() -> Plugin {
        Plugin(std::marker::PhantomData)
    }
    pub struct DialogExt;
    pub fn message() -> DialogExt {
        DialogExt
    }
    impl DialogExt {
        pub fn blocking_show(self) -> Result<()> {
            Ok(())
        }
    }
}

pub mod plugin_notification {
    use super::Plugin;
    pub fn init() -> Plugin {
        Plugin(std::marker::PhantomData)
    }
}

pub mod plugin_updater {
    pub struct Builder;
    impl Builder {
        pub fn new() -> Self {
            Self
        }
        pub fn build(self) -> super::Plugin {
            super::Plugin(std::marker::PhantomData)
        }
    }
    pub struct UpdaterExt;
    pub fn check() -> Result<UpdaterExt, super::Error> {
        Ok(UpdaterExt)
    }
    impl UpdaterExt {
        pub fn dialog(self) -> Result<UpdaterExt, super::Error> {
            Ok(self)
        }
    }
    impl std::future::Future for UpdaterExt {
        type Output = std::result::Result<(), super::Error>;
        fn poll(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

/// `tauri::Plugin`
pub struct Plugin<R = ()>(std::marker::PhantomData<R>);

// ── Builder ───────────────────────────────────────────────────────────────────

/// `tauri::Builder` — every method is a no-op pass-through that fixes the
/// generic parameters and closure argument types so closure bodies typecheck.
pub struct Builder;

impl Default for Builder {
    fn default() -> Self {
        Self
    }
}

impl Builder {
    pub fn new() -> Self {
        Self
    }

    pub fn plugin<P>(self, _plugin: P) -> Self {
        self
    }

    pub fn manage<T: Send + Sync + 'static>(self, state: T) -> Self {
        MANAGED.with(|m| {
            m.borrow_mut()
                .insert(TypeId::of::<T>(), Box::new(state));
        });
        self
    }

    pub fn on_window_event<F: Fn(&WebviewWindow, WindowEvent) + Send + Sync + 'static>(
        self,
        _f: F,
    ) -> Self {
        self
    }

    pub fn setup<F: FnOnce(&mut App) -> std::result::Result<(), Box<dyn std::error::Error>>
             + Send + 'static>(
        self,
        _f: F,
    ) -> Self {
        self
    }

    /// Accepts the unit value that `generate_handler!` expands to.
    pub fn invoke_handler(self, _handler: ()) -> Self {
        self
    }

    pub fn system_tray(self, _tray: crate::tray::TrayIcon) -> Self {
        self
    }

    pub fn menu(self, _menu: crate::menu::Menu<()>) -> Self {
        self
    }

    pub fn run(self, _context: ()) -> Result<()> {
        Ok(())
    }
}

// ── Macros ────────────────────────────────────────────────────────────────────

/// `tauri::generate_handler![...]` — the real macro expands to a wrapper per
/// command that *references* each handler function.
///
/// That reference matters: without it, dead-code analysis reports every
/// `#[tauri::command]` that nothing calls from Rust as "never used". Device
/// sync, for example, is five dead-looking functions that are in fact
/// registered and reachable. `let _ = <path>;` reproduces the use, and the
/// surrounding block still evaluates to `()` for `Builder::invoke_handler`.
#[macro_export]
macro_rules! generate_handler {
    ($($t:path),* $(,)?) => {{
        $( let _ = $t; )*
    }};
}

#[macro_export]
macro_rules! generate_context {
    () => { () };
}
