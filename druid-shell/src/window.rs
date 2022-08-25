// Copyright 2018 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Platform independent window types.

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::application::Application;
use crate::common_util::Counter;
use crate::dialog::{FileDialogOptions, FileInfo};
use crate::error::Error;
use crate::keyboard::KeyEvent;
use crate::kurbo::{Insets, Point, Rect, Size};
use crate::menu::Menu;
use crate::mouse::{Cursor, CursorDesc, MouseEvent};
use crate::region::Region;
use crate::scale::Scale;
use crate::text::{Event, InputHandler};
use piet_wgpu::PietText;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event_loop::{EventLoopProxy, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowBuilderExtMacOS;
use winit::window::CursorIcon;

pub enum WinitEvent {
    Idle(IdleToken),
    Timer(winit::window::WindowId, TimerToken, std::time::Duration),
    NewWindow,
}

/// A token that uniquely identifies a running timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub struct TimerToken(u64);

impl TimerToken {
    /// A token that does not correspond to any timer.
    pub const INVALID: TimerToken = TimerToken(0);

    /// Create a new token.
    pub fn next() -> TimerToken {
        static TIMER_COUNTER: Counter = Counter::new();
        TimerToken(TIMER_COUNTER.next())
    }

    /// Create a new token from a raw value.
    pub const fn from_raw(id: u64) -> TimerToken {
        TimerToken(id)
    }

    /// Get the raw value for a token.
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

/// Uniquely identifies a text input field inside a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub struct TextFieldToken(u64);

impl TextFieldToken {
    /// A token that does not correspond to any text input.
    pub const INVALID: TextFieldToken = TextFieldToken(0);

    /// Create a new token; this should for the most part be called only by platform code.
    pub fn next() -> TextFieldToken {
        static TEXT_FIELD_COUNTER: Counter = Counter::new();
        TextFieldToken(TEXT_FIELD_COUNTER.next())
    }

    /// Create a new token from a raw value.
    pub const fn from_raw(id: u64) -> TextFieldToken {
        TextFieldToken(id)
    }

    /// Get the raw value for a token.
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

//NOTE: this has a From<backend::Handle> impl for construction
/// A handle that can enqueue tasks on the window loop.
#[derive(Clone)]
pub struct IdleHandle(Arc<EventLoopProxy<WinitEvent>>);

unsafe impl Sync for IdleHandle {}
unsafe impl Send for IdleHandle {}

impl IdleHandle {
    /// Add an idle handler, which is called (once) when the message loop
    /// is empty. The idle handler will be run from the main UI thread, and
    /// won't be scheduled if the associated view has been dropped.
    ///
    /// Note: the name "idle" suggests that it will be scheduled with a lower
    /// priority than other UI events, but that's not necessarily the case.
    pub fn add_idle<F>(&self, callback: F)
    where
        F: FnOnce(&mut dyn WinHandler) + Send + 'static,
    {
    }

    /// Request a callback from the runloop. Your `WinHander::idle` method will
    /// be called with the `token` that was passed in.

    pub fn schedule_idle(&mut self, token: IdleToken) {
        self.0.send_event(WinitEvent::Idle(token));
    }
}

/// A token that uniquely identifies a idle schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub struct IdleToken(usize);

impl IdleToken {
    /// Create a new `IdleToken` with the given raw `usize` id.
    pub const fn new(raw: usize) -> IdleToken {
        IdleToken(raw)
    }
}

/// A token that uniquely identifies a file dialog request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub struct FileDialogToken(u64);

impl FileDialogToken {
    /// A token that does not correspond to any file dialog.
    pub const INVALID: FileDialogToken = FileDialogToken(0);

    /// Create a new token.
    pub fn next() -> FileDialogToken {
        static COUNTER: Counter = Counter::new();
        FileDialogToken(COUNTER.next())
    }

    /// Create a new token from a raw value.
    pub const fn from_raw(id: u64) -> FileDialogToken {
        FileDialogToken(id)
    }

    /// Get the raw value for a token.
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

/// Levels in the window system - Z order for display purposes.
/// Describes the purpose of a window and should be mapped appropriately to match platform
/// conventions.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WindowLevel {
    /// A top level app window.
    AppWindow,
    /// A window that should stay above app windows - like a tooltip
    Tooltip,
    /// A user interface element such as a dropdown menu or combo box
    DropDown,
    /// A modal dialog
    Modal,
}

/// Contains the different states a Window can be in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    Maximized,
    Minimized,
    Restored,
}

/// A handle to a platform window object.
#[derive(Clone)]
pub struct WindowHandle(Arc<winit::window::Window>, Arc<EventLoopProxy<WinitEvent>>);

impl WindowHandle {
    pub fn id(&self) -> winit::window::WindowId {
        self.0.id()
    }
    /// Make this window visible.
    ///
    /// This is part of the initialization process; it should only be called
    /// once, when a window is first created.
    pub fn show(&self) {}

    /// Close the window.
    pub fn close(&self) {}

    /// Set whether the window should be resizable
    pub fn resizable(&self, resizable: bool) {
        self.0.set_resizable(resizable);
    }

    /// Sets the state of the window.
    pub fn set_window_state(&mut self, state: WindowState) {
        match state {
            WindowState::Maximized => self.0.set_maximized(true),
            WindowState::Minimized => self.0.set_minimized(true),
            WindowState::Restored => {
                self.0.set_maximized(false);
                self.0.set_minimized(false);
            }
        }
    }

    /// Gets the state of the window.
    pub fn get_window_state(&self) -> WindowState {
        let maximized = self.0.is_maximized();
        if maximized {
            WindowState::Maximized
        } else {
            WindowState::Restored
        }
    }

    /// Informs the system that the current location of the mouse should be treated as part of the
    /// window's titlebar. This can be used to implement a custom titlebar widget. Note that
    /// because this refers to the current location of the mouse, you should probably call this
    /// function in response to every relevant [`WinHandler::mouse_move`].
    ///
    /// This is currently only implemented on Windows.
    pub fn handle_titlebar(&self, val: bool) {}

    /// Set whether the window should show titlebar.
    pub fn show_titlebar(&self, show_titlebar: bool) {}

    /// Sets the position of the window in [display points](crate::Scale), relative to the origin of the
    /// virtual screen.
    pub fn set_position(&self, position: impl Into<Point>) {
        let point: Point = position.into();
        self.0
            .set_outer_position(LogicalPosition::new(point.x, point.y));
    }

    /// Returns the position of the top left corner of the window in
    /// [display points], relative to the origin of the virtual screen.
    ///
    /// [display points]: crate::Scale
    pub fn get_position(&self) -> Point {
        let point = self
            .0
            .outer_position()
            .map(|p| Point::new(p.x.into(), p.y.into()));
        point.unwrap_or(Point::ZERO)
    }

    /// Returns the insets of the window content from its position and size in [display points].
    ///
    /// This is to account for any window system provided chrome, e.g. title bars. For example, if
    /// you want your window to have room for contents of size `contents`, then you should call
    /// [`WindowHandle::get_size`] with an argument of `(contents.to_rect() + insets).size()`,
    /// where `insets` is the return value of this function.
    ///
    /// The details of this function are somewhat platform-dependent. For example, on Windows both
    /// the insets and the window size include the space taken up by the title bar and window
    /// decorations; on GTK neither the insets nor the window size include the title bar or window
    /// decorations.
    ///
    /// [display points]: crate::Scale
    pub fn content_insets(&self) -> Insets {
        let outer_size = self.0.outer_size();
        let outer_position = self
            .0
            .outer_position()
            .map(|p| Point::new(p.x.into(), p.y.into()))
            .unwrap_or(Point::ZERO);
        let outer_rect = Size::new(outer_size.width.into(), outer_size.height.into())
            .to_rect()
            .with_origin(outer_position);

        let inner_size = self.0.inner_size();
        let inner_position = self
            .0
            .inner_position()
            .map(|p| Point::new(p.x.into(), p.y.into()))
            .unwrap_or(Point::ZERO);
        let inner_rect = Size::new(inner_size.width.into(), outer_size.height.into())
            .to_rect()
            .with_origin(inner_position);
        outer_rect - inner_rect
    }

    /// Set the window's size in [display points].
    ///
    /// The actual window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the size of the window.  The
    /// platform might choose a different size depending on its DPI or other platform-dependent
    /// configuration.  To know the actual size of the window you should handle the
    /// [`WinHandler::size`] method.
    ///
    /// [display points]: crate::Scale
    pub fn set_size(&self, size: impl Into<Size>) {
        let size: Size = size.into();
        self.0
            .set_inner_size(LogicalSize::new(size.width, size.height));
    }

    /// Gets the window size, in [display points].
    ///
    /// [display points]: crate::Scale
    pub fn get_size(&self) -> Size {
        let inner_size = self.0.inner_size();
        Size::new(inner_size.width.into(), inner_size.height.into())
    }

    /// Sets the [`WindowLevel`](crate::WindowLevel), the z-order in the Window system / compositor
    ///
    /// We do not currently have a getter method, mostly because the system's levels aren't a
    /// perfect one-to-one map to `druid_shell`'s levels. A getter method may be added in the
    /// future.
    pub fn set_level(&self, level: WindowLevel) {}

    /// Bring this window to the front of the window stack and give it focus.
    pub fn bring_to_front_and_focus(&self) {}

    /// Request that [`prepare_paint`] and [`paint`] be called next time there's the opportunity to
    /// render another frame. This differs from [`invalidate`] and [`invalidate_rect`] in that it
    /// doesn't invalidate any part of the window.
    ///
    /// [`invalidate`]: WindowHandle::invalidate
    /// [`invalidate_rect`]: WindowHandle::invalidate_rect
    /// [`paint`]: WinHandler::paint
    /// [`prepare_paint`]: WinHandler::prepare_paint
    pub fn request_anim_frame(&self) {
        self.0.request_redraw();
    }

    /// Request invalidation of the entire window contents.
    pub fn invalidate(&self) {
        self.0.request_redraw();
    }

    /// Request invalidation of a region of the window.
    pub fn invalidate_rect(&self, rect: Rect) {
        self.0.request_redraw();
    }

    /// Set the title for this menu.
    pub fn set_title(&self, title: &str) {
        self.0.set_title(title)
    }

    /// Set the top-level menu for this window.
    pub fn set_menu(&self, menu: Menu) {}

    /// Get access to a type that can perform text layout.
    // pub fn text(&self) -> PietText {
    //     PietText::new()
    // }

    /// Register a new text input receiver for this window.
    ///
    /// This method should be called any time a new editable text field is
    /// created inside a window.  Any text field with a `TextFieldToken` that
    /// has not yet been destroyed with `remove_text_field` *must* be ready to
    /// accept input from the platform via `WinHandler::text_input` at any time,
    /// even if it is not currently focused.
    ///
    /// Returns the `TextFieldToken` associated with this new text input.
    pub fn add_text_field(&self) -> TextFieldToken {
        TextFieldToken(0)
    }

    /// Unregister a previously registered text input receiver.
    ///
    /// If `token` is the text field currently focused, the platform automatically
    /// sets the focused field to `None`.
    pub fn remove_text_field(&self, token: TextFieldToken) {}

    /// Notify the platform that the focused text input receiver has changed.
    ///
    /// This must be called any time focus changes to a different text input, or
    /// when focus switches away from a text input.
    pub fn set_focused_text_field(&self, active_field: Option<TextFieldToken>) {}

    /// Notify the platform that some text input state has changed, such as the
    /// selection, contents, etc.
    ///
    /// This method should *never* be called in response to edits from a
    /// `InputHandler`; only in response to changes from the application:
    /// scrolling, remote edits, etc.
    pub fn update_text_field(&self, token: TextFieldToken, update: Event) {}

    /// Schedule a timer.
    ///
    /// This causes a [`WinHandler::timer`] call at the deadline. The
    /// return value is a token that can be used to associate the request
    /// with the handler call.
    ///
    /// Note that this is not a precise timer. On Windows, the typical
    /// resolution is around 10ms. Therefore, it's best used for things
    /// like blinking a cursor or triggering tooltips, not for anything
    /// requiring precision.
    pub fn request_timer(&self, deadline: Duration) -> TimerToken {
        let token = TimerToken::next();
        self.1
            .send_event(WinitEvent::Timer(self.id(), token, deadline));
        token
    }

    /// Set the cursor icon.
    pub fn set_cursor(&mut self, cursor: &Cursor) {
        let cursor = match cursor {
            Cursor::Arrow => CursorIcon::Arrow,
            Cursor::IBeam => CursorIcon::Text,
            Cursor::Pointer => CursorIcon::Hand,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            Cursor::ResizeLeftRight => CursorIcon::ColResize,
            Cursor::ResizeUpDown => CursorIcon::RowResize,
        };
        self.0.set_cursor_icon(cursor);
    }

    pub fn make_cursor(&self, desc: &CursorDesc) -> Option<Cursor> {
        None
    }

    /// Prompt the user to choose a file to open.
    ///
    /// This won't block immediately; the file dialog will be shown whenever control returns to
    /// `druid-shell`, and the [`WinHandler::open_file`] method will be called when the dialog is
    /// closed.
    pub fn open_file(&mut self, options: FileDialogOptions) -> Option<FileDialogToken> {
        None
    }

    /// Prompt the user to choose a path for saving.
    ///
    /// This won't block immediately; the file dialog will be shown whenever control returns to
    /// `druid-shell`, and the [`WinHandler::save_as`] method will be called when the dialog is
    /// closed.
    pub fn save_as(&mut self, options: FileDialogOptions) -> Option<FileDialogToken> {
        None
    }

    /// Display a pop-up menu at the given position.
    ///
    /// `pos` is in the coordinate space of the window.
    pub fn show_context_menu(&self, menu: Menu, pos: Point) {}

    /// Get a handle that can be used to schedule an idle task.
    pub fn get_idle_handle(&self) -> Option<IdleHandle> {
        Some(IdleHandle(self.1.clone()))
    }

    /// Get the DPI scale of the window.
    ///
    /// The returned [`Scale`](crate::Scale) is a copy and thus its information will be stale after
    /// the platform DPI changes. This means you should not stash it and rely on it later; it is
    /// only guaranteed to be valid for the current pass of the runloop.
    pub fn get_scale(&self) -> f64 {
        self.0.scale_factor()
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}

/// A builder type for creating new windows.
pub struct WindowBuilder(
    winit::window::WindowBuilder,
    Arc<EventLoopProxy<WinitEvent>>,
);

impl WindowBuilder {
    /// Create a new `WindowBuilder`.
    ///
    /// Takes the [`Application`](crate::Application) that this window is for.
    pub fn new(app: Application) -> WindowBuilder {
        let event_proxy = app.state.borrow().event_proxy.clone();
        WindowBuilder(winit::window::WindowBuilder::new(), event_proxy)
    }

    /// Set the [`WinHandler`] for this window.
    ///
    /// This is the object that will receive callbacks from this window.
    pub fn set_handler(&mut self, handler: Box<dyn WinHandler>) {}

    /// Set the window's initial drawing area size in [display points].
    ///
    /// The actual window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the size of the window.  The
    /// platform might choose a different size depending on its DPI or other platform-dependent
    /// configuration.  To know the actual size of the window you should handle the
    /// [`WinHandler::size`] method.
    ///
    /// [display points]: crate::Scale
    pub fn set_size(mut self, size: Size) -> Self {
        self.0 = self
            .0
            .with_inner_size(LogicalSize::new(size.width, size.height));
        self
    }

    /// Set the window's minimum drawing area size in [display points].
    ///
    /// The actual minimum window size in pixels will depend on the platform DPI settings.
    ///
    /// This should be considered a request to the platform to set the minimum size of the window.
    /// The platform might increase the size a tiny bit due to DPI.
    ///
    /// [display points]: crate::Scale
    pub fn set_min_size(mut self, size: Size) -> Self {
        self.0 = self
            .0
            .with_min_inner_size(LogicalSize::new(size.width, size.height));
        self
    }

    /// Set whether the window should be resizable.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.0 = self.0.with_resizable(resizable);
        self
    }

    /// Set whether the window should have a titlebar and decorations.
    #[cfg(target_os = "macos")]
    pub fn show_titlebar(mut self, show_titlebar: bool) -> Self {
        if !show_titlebar {
            self.0 = self
                .0
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true);
        }
        self
    }

    #[cfg(not(target_os = "macos"))]
    pub fn show_titlebar(mut self, show_titlebar: bool) -> Self {
        self
    }

    /// Set whether the window background should be transparent
    pub fn set_transparent(&mut self, transparent: bool) {}

    /// Sets the initial window position in [display points], relative to the origin of the
    /// virtual screen.
    ///
    /// [display points]: crate::Scale
    pub fn set_position(mut self, position: Point) -> Self {
        self.0 = self
            .0
            .with_position(LogicalPosition::new(position.x, position.y));
        self
    }

    /// Sets the initial [`WindowLevel`].
    pub fn set_level(&mut self, level: WindowLevel) {}

    /// Set the window's initial title.
    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.0 = self.0.with_title(title);
        self
    }

    /// Set the window's menu.
    pub fn set_menu(&mut self, menu: Menu) {}

    /// Sets the initial state of the window.
    pub fn set_window_state(mut self, state: WindowState) -> Self {
        match state {
            WindowState::Maximized => self.0 = self.0.with_maximized(true),
            WindowState::Minimized => (),
            WindowState::Restored => (),
        }
        self
    }

    /// Attempt to construct the platform window.
    ///
    /// If this fails, your application should exit.
    pub fn build<T: 'static>(
        self,
        window_target: &EventLoopWindowTarget<T>,
    ) -> Result<WindowHandle, Error> {
        let event_proxy = self.1.clone();
        self.0
            .build(window_target)
            .map(|w| WindowHandle(Arc::new(w), event_proxy))
            .map_err(|e| Error::Other(std::sync::Arc::new(anyhow::anyhow!("{}", e))))
    }
}

/// App behavior, supplied by the app.
///
/// Many of the "window procedure" messages map to calls to this trait.
/// The methods are non-mut because the window procedure can be called
/// recursively; implementers are expected to use `RefCell` or the like,
/// but should be careful to keep the lifetime of the borrow short.
pub trait WinHandler {
    /// Provide the handler with a handle to the window so that it can
    /// invalidate or make other requests.
    ///
    /// This method passes the `WindowHandle` directly, because the handler may
    /// wish to stash it.
    fn connect(&mut self, handle: &WindowHandle);

    /// Called when the size of the window has changed.
    ///
    /// The `size` parameter is the new size in [display points](crate::Scale).
    #[allow(unused_variables)]
    fn size(&mut self, size: Size) {}

    /// Called when the [scale](crate::Scale) of the window has changed.
    ///
    /// This is always called before the accompanying [`size`](WinHandler::size).
    #[allow(unused_variables)]
    fn scale(&mut self, scale: Scale) {}

    /// Request the handler to prepare to paint the window contents.  In particular, if there are
    /// any regions that need to be repainted on the next call to `paint`, the handler should
    /// invalidate those regions by calling [`WindowHandle::invalidate_rect`] or
    /// [`WindowHandle::invalidate`].
    fn prepare_paint(&mut self);

    /// Request the handler to paint the window contents.  `invalid` is the region in [display
    /// points](crate::Scale) that needs to be repainted; painting outside the invalid region will
    /// have no effect.
    fn paint(&mut self);

    /// Called when the resources need to be rebuilt.
    ///
    /// Discussion: this function is mostly motivated by using
    /// `GenericRenderTarget` on Direct2D. If we move to `DeviceContext`
    /// instead, then it's possible we don't need this.
    #[allow(unused_variables)]
    fn rebuild_resources(&mut self) {}

    /// Called when a menu item is selected.
    #[allow(unused_variables)]
    fn command(&mut self, id: u32) {}

    /// Called when a "Save As" dialog is closed.
    ///
    /// `token` is the value returned by [`WindowHandle::save_as`]. `file` contains the information
    /// of the chosen path, or `None` if the save dialog was cancelled.
    #[allow(unused_variables)]
    fn save_as(&mut self, token: FileDialogToken, file: Option<FileInfo>) {}

    /// Called when an "Open" dialog is closed.
    ///
    /// `token` is the value returned by [`WindowHandle::open_file`]. `file` contains the information
    /// of the chosen path, or `None` if the save dialog was cancelled.
    #[allow(unused_variables)]
    fn open_file(&mut self, token: FileDialogToken, file: Option<FileInfo>) {}

    /// Called on a key down event.
    ///
    /// Return `true` if the event is handled.
    #[allow(unused_variables)]
    fn key_down(&mut self, event: KeyEvent) -> bool {
        false
    }

    /// Called when a key is released. This corresponds to the WM_KEYUP message
    /// on Windows, or keyUp(withEvent:) on macOS.
    #[allow(unused_variables)]
    fn key_up(&mut self, event: KeyEvent) {}

    /// Take a lock for the text document specified by `token`.
    ///
    /// All calls to this method must be balanced with a call to
    /// [`release_input_lock`].
    ///
    /// If `mutable` is true, the lock should be a write lock, and allow calling
    /// mutating methods on InputHandler.  This method is called from the top
    /// level of the event loop and expects to acquire a lock successfully.
    ///
    /// For more information, see [the text input documentation](crate::text).
    ///
    /// [`release_input_lock`]: WinHandler::release_input_lock
    #[allow(unused_variables)]
    fn acquire_input_lock(
        &mut self,
        token: TextFieldToken,
        mutable: bool,
    ) -> Box<dyn InputHandler> {
        panic!("acquire_input_lock was called on a WinHandler that did not expect text input.")
    }

    /// Release a lock previously acquired by [`acquire_input_lock`].
    ///
    /// [`acquire_input_lock`]: WinHandler::acquire_input_lock
    #[allow(unused_variables)]
    fn release_input_lock(&mut self, token: TextFieldToken) {
        panic!("release_input_lock was called on a WinHandler that did not expect text input.")
    }

    /// Called on a mouse wheel event.
    ///
    /// The polarity is the amount to be added to the scroll position,
    /// in other words the opposite of the direction the content should
    /// move on scrolling. This polarity is consistent with the
    /// deltaX and deltaY values in a web [WheelEvent].
    ///
    /// [WheelEvent]: https://w3c.github.io/uievents/#event-type-wheel
    #[allow(unused_variables)]
    fn wheel(&mut self, event: &MouseEvent) {}

    /// Called when a platform-defined zoom gesture occurs (such as pinching
    /// on the trackpad).
    #[allow(unused_variables)]
    fn zoom(&mut self, delta: f64) {}

    /// Called when the mouse moves.
    #[allow(unused_variables)]
    fn mouse_move(&mut self, event: &MouseEvent) {}

    /// Called on mouse button down.
    #[allow(unused_variables)]
    fn mouse_down(&mut self, event: &MouseEvent) {}

    /// Called on mouse button up.
    #[allow(unused_variables)]
    fn mouse_up(&mut self, event: &MouseEvent) {}

    /// Called when the mouse cursor has left the application window
    fn mouse_leave(&mut self) {}

    /// Called on timer event.
    ///
    /// This is called at (approximately) the requested deadline by a
    /// [`WindowHandle::request_timer()`] call. The token argument here is the same
    /// as the return value of that call.
    #[allow(unused_variables)]
    fn timer(&mut self, token: TimerToken) {}

    /// Called when this window becomes the focused window.
    #[allow(unused_variables)]
    fn got_focus(&mut self) {}

    /// Called when this window stops being the focused window.
    #[allow(unused_variables)]
    fn lost_focus(&mut self) {}

    /// Called when the shell requests to close the window, for example because the user clicked
    /// the little "X" in the titlebar.
    ///
    /// If you want to actually close the window in response to this request, call
    /// [`WindowHandle::close`]. If you don't implement this method, clicking the titlebar "X" will
    /// have no effect.
    fn request_close(&mut self) {}

    /// Called when the window is being destroyed. Note that this happens
    /// earlier in the sequence than drop (at WM_DESTROY, while the latter is
    /// WM_NCDESTROY).
    #[allow(unused_variables)]
    fn destroy(&mut self) {}

    /// Called when a idle token is requested by [`IdleHandle::schedule_idle()`] call.
    #[allow(unused_variables)]
    fn idle(&mut self, token: IdleToken) {}

    /// Get a reference to the handler state. Used mostly by idle handlers.
    fn as_any(&mut self) -> &mut dyn Any;
}
