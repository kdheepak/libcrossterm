use std::io::Write;

use bitflags::bitflags;
use crossterm::{self, queue};
use log::trace;

#[cfg(not(feature = "docsrs"))]
include!(concat!(env!("OUT_DIR"), "/colors.rs"));

#[cfg(feature = "docsrs")]
pub static COLORS: &str = r##"[{"error": "Feature `docsrs` is enabled. COLORS json not available."}]"##;

fn convert_string_to_c_char(string: String) -> *mut libc::c_char {
  // Convert the String to a CString
  let c_string = match std::ffi::CString::new(string.clone()) {
    Ok(c_string) => c_string,
    Err(_) => {
      set_last_error(anyhow::anyhow!("Unable to convert {} to CString", &string));
      return std::ptr::null_mut();
    },
  };

  // Allocate space for the string
  let string_len = c_string.as_bytes_with_nul().len();
  let addr = unsafe {
    let addr = libc::malloc(string_len) as *mut libc::c_char;
    if addr.is_null() {
      set_last_error(anyhow::anyhow!("Unable to malloc for {}", &string));
      return std::ptr::null_mut();
    }
    addr
  };

  // Copy the string into the allocated space
  unsafe {
    std::ptr::copy_nonoverlapping(c_string.as_ptr(), addr, string_len);
  }
  addr
}

// ensure that we always set a C exception instead of `panic`ing
pub trait CUnwrapper<T> {
  fn c_unwrap(self) -> T;
}

impl<T> CUnwrapper<T> for anyhow::Result<T>
where
  T: Default,
{
  fn c_unwrap(self) -> T {
    match self {
      Ok(t) => {
        RESULT.with(|r| {
          *r.borrow_mut() = 0;
        });
        take_last_error();
        t
      },
      Err(err) => {
        RESULT.with(|r| {
          *r.borrow_mut() = -1;
        });
        set_last_error(err);
        T::default()
      },
    }
  }
}

impl<T> CUnwrapper<T> for Result<T, std::io::Error>
where
  T: Default,
{
  fn c_unwrap(self) -> T {
    match self {
      Ok(t) => {
        RESULT.with(|r| {
          *r.borrow_mut() = 0;
        });
        t
      },
      Err(err) => {
        RESULT.with(|r| {
          *r.borrow_mut() = -1;
        });
        set_last_error(err.into());
        T::default()
      },
    }
  }
}

thread_local! {
  static LAST_ERROR: std::cell::RefCell<Option<anyhow::Error>> = std::cell::RefCell::new(None);
  static RESULT: std::cell::RefCell<libc::c_int> = std::cell::RefCell::new(0);
}

macro_rules! r {
  () => {
    RESULT.with(|r| r.borrow().clone())
  };
}

fn set_last_error(err: anyhow::Error) {
  trace!("Set last error");
  LAST_ERROR.with(|e| {
    *e.borrow_mut() = Some(err);
  });
}

/// Take the most recent error, clearing `LAST_ERROR` in the process.
pub fn take_last_error() -> Option<anyhow::Error> {
  LAST_ERROR.with(|prev| prev.borrow_mut().take())
}

/// Check whether error has been set.
#[no_mangle]
pub extern "C" fn crossterm_has_error() -> bool {
  LAST_ERROR.with(|prev| prev.borrow().is_some())
}

#[no_mangle]
pub extern "C" fn crossterm_clear_last_error() {
  let _ = take_last_error();
}

/// Peek at the most recent error and get its error message as a Rust `String`.
pub fn error_message() -> Option<String> {
  LAST_ERROR.with(|prev| prev.borrow().as_ref().map(|e| format!("{:#}", e)))
}

/// Calculate the number of bytes in the last error's error message including a
/// trailing `null` character. If there are no recent error, then this returns
/// `0`.
#[no_mangle]
pub extern "C" fn crossterm_last_error_length() -> libc::c_int {
  LAST_ERROR.with(|prev| {
    match *prev.borrow() {
      Some(ref err) => format!("{:#}", err).len() as libc::c_int + 1,
      None => 0,
    }
  })
}

/// Return most recent error message into a UTF-8 string buffer.
///
/// Null character is stored in the last location of buffer.
/// Caller is responsible for memory associated with string buffer.
/// Use [`crossterm_free_c_char`] to free data.
#[no_mangle]
pub extern "C" fn crossterm_last_error_message() -> *const libc::c_char {
  let last_error = take_last_error()
    .unwrap_or(anyhow::anyhow!("No error message found. Check library documentation for more information."));
  let string = format!("{:#}", last_error);
  convert_string_to_c_char(string)
}

/// Frees data behind pointer to UTF-8 string allocated by this crate
///
/// Null character is stored in the last location of buffer.
#[no_mangle]
pub extern "C" fn crossterm_free_c_char(s: *mut libc::c_char) -> libc::c_int {
  if !s.is_null() {
    unsafe {
      libc::free(s as *mut libc::c_void);
    }
    0
  } else {
    set_last_error(anyhow::anyhow!("Received null pointer to free"));
    -1
  }
}

/// Represents a media key (as part of [`KeyCode::Media`]).
#[repr(C)]
pub enum MediaKeyCode {
  /// Play media key.
  Play,
  /// Pause media key.
  Pause,
  /// Play/Pause media key.
  PlayPause,
  /// Reverse media key.
  Reverse,
  /// Stop media key.
  Stop,
  /// Fast-forward media key.
  FastForward,
  /// Rewind media key.
  Rewind,
  /// Next-track media key.
  TrackNext,
  /// Previous-track media key.
  TrackPrevious,
  /// Record media key.
  Record,
  /// Lower-volume media key.
  LowerVolume,
  /// Raise-volume media key.
  RaiseVolume,
  /// Mute media key.
  MuteVolume,
}

/// Represents a modifier key (as part of [`KeyCode::Modifier`]).
#[repr(C)]
pub enum ModifierKeyCode {
  /// Left Shift key.
  LeftShift,
  /// Left Control key.
  LeftControl,
  /// Left Alt key.
  LeftAlt,
  /// Left Super key.
  LeftSuper,
  /// Left Hyper key.
  LeftHyper,
  /// Left Meta key.
  LeftMeta,
  /// Right Shift key.
  RightShift,
  /// Right Control key.
  RightControl,
  /// Right Alt key.
  RightAlt,
  /// Right Super key.
  RightSuper,
  /// Right Hyper key.
  RightHyper,
  /// Right Meta key.
  RightMeta,
  /// Iso Level3 Shift key.
  IsoLevel3Shift,
  /// Iso Level5 Shift key.
  IsoLevel5Shift,
}

/// Represents a key.
#[repr(C)]
pub enum KeyCode {
  /// Backspace key.
  Backspace,
  /// Enter key.
  Enter,
  /// Left arrow key.
  Left,
  /// Right arrow key.
  Right,
  /// Up arrow key.
  Up,
  /// Down arrow key.
  Down,
  /// Home key.
  Home,
  /// End key.
  End,
  /// Page up key.
  PageUp,
  /// Page down key.
  PageDown,
  /// Tab key.
  Tab,
  /// Shift + Tab key.
  BackTab,
  /// Delete key.
  Delete,
  /// Insert key.
  Insert,
  /// F key.
  ///
  /// `KeyCode::F(1)` represents F1 key, etc.
  F(u8),
  /// A character.
  ///
  /// `KeyCode::Char('c')` represents `c` character, etc.
  Char(char),
  /// Null.
  Null,
  /// Escape key.
  Esc,
  /// Caps Lock key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  CapsLock,
  /// Scroll Lock key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  ScrollLock,
  /// Num Lock key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  NumLock,
  /// Print Screen key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  PrintScreen,
  /// Pause key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  Pause,
  /// Menu key.
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  Menu,
  /// The "Begin" key (often mapped to the 5 key when Num Lock is turned on).
  ///
  /// **Note:** this key can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  KeypadBegin,
  /// A media key.
  ///
  /// **Note:** these keys can only be read if
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  Media(MediaKeyCode),
  /// A modifier key.
  ///
  /// **Note:** these keys can only be read if **both**
  /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] and
  /// [`KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES`] have been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  Modifier(ModifierKeyCode),
}

bitflags! {
    /// Represents key modifiers (shift, control, alt, etc.).
    ///
    /// **Note:** `SUPER`, `HYPER`, and `META` can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`PushKeyboardEnhancementFlags`].
    #[repr(C)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
        const SUPER = 0b0000_1000;
        const HYPER = 0b0001_0000;
        const META = 0b0010_0000;
        const NONE = 0b0000_0000;
    }
}

/// Represents a keyboard event kind.
#[repr(C)]
pub enum KeyEventKind {
  Press,
  Repeat,
  Release,
}

bitflags! {
    /// Represents extra state about the key event.
    ///
    /// **Note:** This state can only be read if
    /// [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
    /// [`crossterm_event_push_keyboard_enhancement_flags`].
    #[repr(C)]
    pub struct KeyEventState: u8 {
        /// The key event origins from the keypad.
        const KEYPAD = 0b0000_0001;
        /// Caps Lock was enabled for this key event.
        ///
        /// **Note:** this is set for the initial press of Caps Lock itself.
        const CAPS_LOCK = 0b0000_1000;
        /// Num Lock was enabled for this key event.
        ///
        /// **Note:** this is set for the initial press of Num Lock itself.
        const NUM_LOCK = 0b0000_1000;
        const NONE = 0b0000_0000;
    }
}

/// Represents a key event.
#[repr(C)]
pub struct KeyEvent {
  /// The key itself.
  pub code: KeyCode,
  /// Additional key modifiers.
  pub modifiers: KeyModifiers,
  /// Kind of event.
  ///
  /// Only set if:
  /// - Unix: [`KeyboardEnhancementFlags::REPORT_EVENT_TYPES`] has been enabled with [`crossterm_event_push_keyboard_enhancement_flags`].
  /// - Windows: always
  pub kind: KeyEventKind,
  /// Keyboard state.
  ///
  /// Only set if [`KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`] has been enabled with
  /// [`PushKeyboardEnhancementFlags`].
  pub state: KeyEventState,
}

/// A mouse event kind.
///
/// # Platform-specific Notes
///
/// ## Mouse Buttons
///
/// Some platforms/terminals do not report mouse button for the
/// `MouseEventKind::Up` and `MouseEventKind::Drag` events. `MouseButton::Left`
/// is returned if we don't know which button was used.
#[repr(C)]
pub enum MouseEventKind {
  /// Pressed mouse button. Contains the button that was pressed.
  Down(MouseButton),
  /// Released mouse button. Contains the button that was released.
  Up(MouseButton),
  /// Moved the mouse cursor while pressing the contained mouse button.
  Drag(MouseButton),
  /// Moved the mouse cursor while not pressing a mouse button.
  Moved,
  /// Scrolled mouse wheel downwards (towards the user).
  ScrollDown,
  /// Scrolled mouse wheel upwards (away from the user).
  ScrollUp,
}

/// Represents a mouse button.
#[repr(C)]
pub enum MouseButton {
  /// Left mouse button.
  Left,
  /// Right mouse button.
  Right,
  /// Middle mouse button.
  Middle,
}

/// Represents a mouse event.
///
/// # Platform-specific Notes
///
/// ## Mouse Buttons
///
/// Some platforms/terminals do not report mouse button for the
/// `MouseEventKind::Up` and `MouseEventKind::Drag` events. `MouseButton::Left`
/// is returned if we don't know which button was used.
///
/// ## Key Modifiers
///
/// Some platforms/terminals does not report all key modifiers
/// combinations for all mouse event types. For example - macOS reports
/// `Ctrl` + left mouse button click as a right mouse button click.
#[repr(C)]
pub struct MouseEvent {
  /// The kind of mouse event that was caused.
  pub kind: MouseEventKind,
  /// The col that the event occurred on.
  pub col: u16,
  /// The row that the event occurred on.
  pub row: u16,
  /// The key modifiers active when the event occurred.
  pub modifiers: KeyModifiers,
}

/// Represents an event.
#[repr(C)]
pub enum Event {
  /// The terminal gained focus
  FocusGained,
  /// The terminal lost focus
  FocusLost,
  /// A single key event with additional pressed modifiers.
  Key(KeyEvent),
  /// A single mouse event with additional pressed modifiers.
  Mouse(MouseEvent),
  /// A string that was pasted into the terminal. Only emitted if bracketed paste has been
  /// enabled.
  Paste(*const libc::c_char),
  /// An resize event with new dimensions after resize (cols, rows).
  /// **Note** that resize events can occur in batches.
  Resize(u16, u16),
}

/// Checks if there is an [`Event`] available.
///
/// Returns `1` if an [`Event`] is available, it returns `0` if no [`Event`] is available, returns -1 if error has occurred.
///
/// When it returns `1`, that guarantees that subsequent call to the [`crossterm_event_read`] function
/// won't block.
///
/// # Arguments
///
/// * `timeout_secs` - maximum waiting time for event availability
/// * `timeout_nanos` - maximum waiting time for event availability
#[no_mangle]
pub extern "C" fn crossterm_event_poll(secs: u64, nanos: u32) -> libc::c_int {
  let r = crossterm::event::poll(std::time::Duration::new(secs, nanos)).c_unwrap();
  if crossterm_has_error() {
    r!()
  } else {
    r.into()
  }
}

/// Reads a single [`Event`] as a UTF-8 JSON string.
/// If error occurs during read, error will be returned as a UTF-8 JSON string.
///
/// This function blocks until an [`Event`] is available.
/// Combine it with the [`crossterm_event_poll`] function to get non-blocking reads.
///
/// Caller is responsible for memory associated with string buffer.
/// Use [`crossterm_free_c_char`] to free data.
#[no_mangle]
pub extern "C" fn crossterm_event_read() -> *const libc::c_char {
  let string = match crossterm::event::read() {
    Ok(evt) => {
      serde_json::to_string(&evt).unwrap_or(
        serde_json::json!({
          "error": format!("Unable to convert event {:?} to JSON", evt),
        })
        .to_string(),
      )
    },
    Err(e) => {
      serde_json::json!({
        "error": format!("Something went wrong with crossterm_event_read(): {:?}", anyhow::anyhow!(e)),
      })
      .to_string()
    },
  };
  convert_string_to_c_char(string)
}

/// Sleeps for n seconds where n is the argument to this function
#[no_mangle]
pub extern "C" fn crossterm_sleep(seconds: f64) {
  let duration = std::time::Duration::from_nanos((seconds * 1e9).round() as u64);
  std::thread::sleep(duration);
}

/// Set cursor position (col, row)
///
/// # Notes
/// * Top left cell is represented as `0,0`.
#[no_mangle]
pub extern "C" fn crossterm_cursor_position_set(col: u16, row: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row)).c_unwrap();
  r!()
}

/// Get cursor position (col, row)
///
/// # Notes
/// * Top left cell is represented as `0,0`.
#[no_mangle]
pub extern "C" fn crossterm_cursor_position(col: &mut u16, row: &mut u16) -> libc::c_int {
  let (c, r) = crossterm::cursor::position().c_unwrap();
  *col = c;
  *row = r;
  r!()
}

/// Moves the terminal cursor to the given position (col, row).
///
/// # Notes
/// * Top left cell is represented as `0,0`.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to(col: u16, row: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row)).c_unwrap();
  r!()
}

/// Moves the terminal cursor down the given number of lines and moves it to the first col.
///
/// # Notes
/// * This command is 1 based, meaning `crossterm_cursor_move_to_next_line(1)` moves to the next line.
/// * Most terminals default 0 argument to 1.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_next_line(n: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveToNextLine(n)).c_unwrap();
  r!()
}

/// Moves the terminal cursor up the given number of lines and moves it to the first col.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_previous_line(n: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveToPreviousLine(n)).c_unwrap();
  r!()
}

/// Moves the terminal cursor to the given col on the current row.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_column(col: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveToColumn(col)).c_unwrap();
  r!()
}

/// Moves the terminal cursor to the given row on the current col.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_row(row: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveToRow(row)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of rows up.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_up(rows: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveUp(rows)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of cols to the right.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_right(cols: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveRight(cols)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of rows down.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_down(rows: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveDown(rows)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of cols to the left.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_left(cols: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::MoveLeft(cols)).c_unwrap();
  r!()
}

/// Saves the current terminal cursor position.
#[no_mangle]
pub extern "C" fn crossterm_cursor_save_position() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SavePosition).c_unwrap();
  r!()
}

/// Restores the saved terminal cursor position.
#[no_mangle]
pub extern "C" fn crossterm_cursor_restore_position() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::RestorePosition).c_unwrap();
  r!()
}

/// Hides the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_hide() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::Hide).c_unwrap();
  r!()
}

/// Shows the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_show() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::Show).c_unwrap();
  r!()
}

/// Enables blinking of the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_enable_blinking() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::EnableBlinking).c_unwrap();
  r!()
}

/// Disables blinking of the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_disable_blinking() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::DisableBlinking).c_unwrap();
  r!()
}

/// Style of the cursor.
/// It uses two types of escape codes, one to control blinking, and the other the shape.
#[repr(C)]
#[derive(Clone, Copy)]
pub enum CursorStyle {
  /// Default cursor shape configured by the user.
  DefaultUserShape,
  /// A blinking block cursor shape (■).
  BlinkingBlock,
  /// A non blinking block cursor shape (inverse of `BlinkingBlock`).
  SteadyBlock,
  /// A blinking underscore cursor shape(_).
  BlinkingUnderScore,
  /// A non blinking underscore cursor shape (inverse of `BlinkingUnderScore`).
  SteadyUnderScore,
  /// A blinking cursor bar shape (|)
  BlinkingBar,
  /// A steady cursor bar shape (inverse of `BlinkingBar`).
  SteadyBar,
}

/// Sets the style of the cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style(cursor_style: CursorStyle) -> libc::c_int {
  let cs = match cursor_style {
    CursorStyle::DefaultUserShape => crossterm::cursor::SetCursorStyle::DefaultUserShape,
    CursorStyle::BlinkingBlock => crossterm::cursor::SetCursorStyle::BlinkingBlock,
    CursorStyle::SteadyBlock => crossterm::cursor::SetCursorStyle::SteadyBlock,
    CursorStyle::BlinkingUnderScore => crossterm::cursor::SetCursorStyle::BlinkingUnderScore,
    CursorStyle::SteadyUnderScore => crossterm::cursor::SetCursorStyle::SteadyUnderScore,
    CursorStyle::BlinkingBar => crossterm::cursor::SetCursorStyle::BlinkingBar,
    CursorStyle::SteadyBar => crossterm::cursor::SetCursorStyle::SteadyBar,
  };
  queue!(std::io::stdout(), cs).c_unwrap();
  r!()
}

/// Sets the style of the cursor to default user shape.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_default_user_shape() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::DefaultUserShape).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking block.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_blinking_block() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingBlock).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady block.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_steady_block() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyBlock).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking underscore.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_blinking_underscore() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingUnderScore).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady underscore.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_steady_underscore() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyUnderScore).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking bar.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_blinking_bar() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingBar).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady bar.
#[no_mangle]
pub extern "C" fn crossterm_cursor_style_steady_bar() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyBar).c_unwrap();
  r!()
}

/// Enable mouse event capturing.
#[no_mangle]
pub extern "C" fn crossterm_event_enable_mouse_capture() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::EnableMouseCapture).c_unwrap();
  r!()
}

/// Disable mouse event capturing.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_mouse_capture() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::DisableMouseCapture).c_unwrap();
  r!()
}

/// Represents special flags that tell compatible terminals to add extra information to keyboard events.
///
/// See <https://sw.kovidgoyal.net/kitty/keyboard-protocol/#progressive-enhancement> for more information.
///
/// Alternate keys and Unicode codepoints are not yet supported by crossterm.
#[repr(u8)]
pub enum KeyboardEnhancementFlags {
  /// Represent Escape and modified keys using CSI-u sequences, so they can be unambiguously
  /// read.
  DisambiguateEscapeCodes = 0b0000_0001,
  /// Add extra events with [`KeyEvent.kind`] set to [`KeyEventKind::Repeat`] or
  /// [`KeyEventKind::Release`] when keys are autorepeated or released.
  ReportEventTypes = 0b0000_0010,
  // Send [alternate keycodes](https://sw.kovidgoyal.net/kitty/keyboard-protocol/#key-codes)
  // in addition to the base keycode. The alternate keycode overrides the base keycode in
  // resulting `KeyEvent`s.
  ReportAlternateKeys = 0b0000_0100,
  /// Represent all keyboard events as CSI-u sequences. This is required to get repeat/release
  /// events for plain-text keys.
  ReportAllKeysAsEscapeCodes = 0b0000_1000,
}

/// Enables the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/),
/// which adds extra information to keyboard events and removes ambiguity for modifier keys.
/// It should be paired with [`crossterm_pop_keyboard_enhancement_flags`] at the end of execution.
#[no_mangle]
pub extern "C" fn crossterm_event_push_keyboard_enhancement_flags(flags: u8) -> libc::c_int {
  let flags = crossterm::event::KeyboardEnhancementFlags::from_bits(flags).unwrap();
  queue!(std::io::stdout(), crossterm::event::PushKeyboardEnhancementFlags(flags)).c_unwrap();
  r!()
}

/// Disables extra kinds of keyboard events.
#[no_mangle]
pub extern "C" fn crossterm_event_pop_keyboard_enhancement_flags() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::PopKeyboardEnhancementFlags).c_unwrap();
  r!()
}

/// Enable focus event emission.
///
/// It should be paired with [`crossterm_event_disable_focus_change`] at the end of execution.
///
/// Focus events can be captured with [`crossterm_event_read`].
#[no_mangle]
pub extern "C" fn crossterm_event_enable_focus_change() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::EnableFocusChange).c_unwrap();
  r!()
}

/// Disable focus event emission.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_focus_change() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::DisableFocusChange).c_unwrap();
  r!()
}

/// Enables [bracketed paste mode](https://en.wikipedia.org/wiki/Bracketed-paste).
///
/// It should be paired with [`crossterm_event_disable_bracketed_paste`] at the end of execution.
///
/// This is not supported in older Windows terminals without
/// [virtual terminal sequences](https://docs.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences).
#[no_mangle]
pub extern "C" fn crossterm_event_enable_bracketed_paste() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::EnableBracketedPaste).c_unwrap();
  r!()
}

/// Disables bracketed paste mode.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_bracketed_paste() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::event::DisableBracketedPaste).c_unwrap();
  r!()
}

#[repr(C)]
pub enum Attribute {
  /// Resets all the attributes.
  Reset,
  /// Increases the text intensity.
  Bold,
  /// Decreases the text intensity.
  Dim,
  /// Emphasises the text.
  Italic,
  /// Underlines the text.
  Underlined,
  /// Double underlines the text.
  DoubleUnderlined,
  /// Undercurls the text.
  Undercurled,
  /// Underdots the text.
  Underdotted,
  /// Underdashes the text.
  Underdashed,
  /// Makes the text blinking (< 150 per minute).
  SlowBlink,
  /// Makes the text blinking (>= 150 per minute).
  RapidBlink,
  /// Swaps foreground and background colors.
  Reverse,
  /// Hides the text (also known as Conceal).
  Hidden,
  /// Crosses the text.
  CrossedOut,
  /// Sets the [Fraktur](https://en.wikipedia.org/wiki/Fraktur) typeface.
  ///
  /// Mostly used for [mathematical alphanumeric symbols](https://en.wikipedia.org/wiki/Mathematical_Alphanumeric_Symbols).
  Fraktur,
  /// Turns off the `Bold` attribute. - Inconsistent - Prefer to use NormalIntensity
  NoBold,
  /// Switches the text back to normal intensity (no bold, italic).
  NormalIntensity,
  /// Turns off the `Italic` attribute.
  NoItalic,
  /// Turns off the `Underlined` attribute.
  NoUnderline,
  /// Turns off the text blinking (`SlowBlink` or `RapidBlink`).
  NoBlink,
  /// Turns off the `Reverse` attribute.
  NoReverse,
  /// Turns off the `Hidden` attribute.
  NoHidden,
  /// Turns off the `CrossedOut` attribute.
  NotCrossedOut,
  /// Makes the text framed.
  Framed,
  /// Makes the text encircled.
  Encircled,
  /// Draws a line at the top of the text.
  OverLined,
  /// Turns off the `Frame` and `Encircled` attributes.
  NotFramedOrEncircled,
  /// Turns off the `OverLined` attribute.
  NotOverLined,
}

impl From<Attribute> for crossterm::style::Attribute {
  fn from(value: Attribute) -> Self {
    match value {
      Attribute::Reset => crossterm::style::Attribute::Reset,
      Attribute::Bold => crossterm::style::Attribute::Bold,
      Attribute::Dim => crossterm::style::Attribute::Dim,
      Attribute::Italic => crossterm::style::Attribute::Italic,
      Attribute::Underlined => crossterm::style::Attribute::Underlined,
      Attribute::DoubleUnderlined => crossterm::style::Attribute::DoubleUnderlined,
      Attribute::Undercurled => crossterm::style::Attribute::Undercurled,
      Attribute::Underdotted => crossterm::style::Attribute::Underdotted,
      Attribute::Underdashed => crossterm::style::Attribute::Underdashed,
      Attribute::SlowBlink => crossterm::style::Attribute::SlowBlink,
      Attribute::RapidBlink => crossterm::style::Attribute::RapidBlink,
      Attribute::Reverse => crossterm::style::Attribute::Reverse,
      Attribute::Hidden => crossterm::style::Attribute::Hidden,
      Attribute::CrossedOut => crossterm::style::Attribute::CrossedOut,
      Attribute::Fraktur => crossterm::style::Attribute::Fraktur,
      Attribute::NoBold => crossterm::style::Attribute::NoBold,
      Attribute::NormalIntensity => crossterm::style::Attribute::NormalIntensity,
      Attribute::NoItalic => crossterm::style::Attribute::NoItalic,
      Attribute::NoUnderline => crossterm::style::Attribute::NoUnderline,
      Attribute::NoBlink => crossterm::style::Attribute::NoBlink,
      Attribute::NoReverse => crossterm::style::Attribute::NoReverse,
      Attribute::NoHidden => crossterm::style::Attribute::NoHidden,
      Attribute::NotCrossedOut => crossterm::style::Attribute::NotCrossedOut,
      Attribute::Framed => crossterm::style::Attribute::Framed,
      Attribute::Encircled => crossterm::style::Attribute::Encircled,
      Attribute::OverLined => crossterm::style::Attribute::OverLined,
      Attribute::NotFramedOrEncircled => crossterm::style::Attribute::NotFramedOrEncircled,
      Attribute::NotOverLined => crossterm::style::Attribute::NotOverLined,
    }
  }
}

/// a bitset for all possible attributes
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Attributes(u32);

/// Sets an attribute.
///
/// See [`Attribute`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute(attr: Attribute) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(attr.into())).c_unwrap();
  r!()
}

/// Sets the `Reset` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_reset() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Reset.into())).c_unwrap();
  r!()
}

/// Sets the `Bold` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_bold() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Bold.into())).c_unwrap();
  r!()
}

/// Sets the `Dim` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_dim() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Dim.into())).c_unwrap();
  r!()
}

/// Sets the `Italic` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_italic() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Italic.into())).c_unwrap();
  r!()
}

/// Sets the `Underlined` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_underlined() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Underlined.into())).c_unwrap();
  r!()
}

/// Sets the `DoubleUnderlined` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_double_underlined() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::DoubleUnderlined.into())).c_unwrap();
  r!()
}

/// Sets the `Undercurled` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_undercurled() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Undercurled.into())).c_unwrap();
  r!()
}

/// Sets the `Underdotted` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_underdotted() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Underdotted.into())).c_unwrap();
  r!()
}

/// Sets the `Underdashed` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_underdashed() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Underdashed.into())).c_unwrap();
  r!()
}

/// Sets the `SlowBlink` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_slow_blink() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::SlowBlink.into())).c_unwrap();
  r!()
}

/// Sets the `RapidBlink` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_rapid_blink() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::RapidBlink.into())).c_unwrap();
  r!()
}

/// Sets the `Reverse` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_reverse() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Reverse.into())).c_unwrap();
  r!()
}

/// Sets the `Hidden` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_hidden() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Hidden.into())).c_unwrap();
  r!()
}

/// Sets the `CrossedOut` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_crossed_out() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::CrossedOut.into())).c_unwrap();
  r!()
}

/// Sets the `Fraktur` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_fraktur() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::Fraktur.into())).c_unwrap();
  r!()
}

/// Sets the `NoBold` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_bold() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoBold.into())).c_unwrap();
  r!()
}

/// Sets the `NormalIntensity` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_normal_intensity() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NormalIntensity.into())).c_unwrap();
  r!()
}

/// Sets the `NoItalic` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_italic() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoItalic.into())).c_unwrap();
  r!()
}

/// Sets the `NoUnderline` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_underline() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoUnderline.into())).c_unwrap();
  r!()
}

/// Sets the `NoBlink` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_blink() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoBlink.into())).c_unwrap();
  r!()
}

/// Sets the `NoReverse` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_reverse() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoReverse.into())).c_unwrap();
  r!()
}

/// Sets the `NoHidden` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_no_hidden() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NoHidden.into())).c_unwrap();
  r!()
}

/// Sets the `NotCrossedOut` attribute.
#[no_mangle]
pub extern "C" fn crossterm_style_attribute_not_crossed_out() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetAttribute(Attribute::NotCrossedOut.into())).c_unwrap();
  r!()
}

/// Print char to stdout
#[no_mangle]
pub extern "C" fn crossterm_style_print_char(c: u32) -> libc::c_int {
  if let Some(ch) = std::char::from_u32(c) {
    queue!(std::io::stdout(), crossterm::style::Print(ch)).c_unwrap();
    r!()
  } else {
    set_last_error(anyhow::anyhow!("Unable to convert {} to valid char", c));
    -1
  }
}

/// Print string to stdout
///
/// # Safety
///
/// This function takes a raw pointer as argument. As such, the caller must ensure that:
/// - The `title` pointer points to a valid null-terminated string.
/// - This function borrows a slice to a valid null-terminated string and the memory referenced by `title` won't be deallocated or modified for the duration of the function call..
/// - The `title` pointer is correctly aligned and `title` points to an initialized memory.
///
/// If these conditions are not met, the behavior is undefined.
#[no_mangle]
pub unsafe extern "C" fn crossterm_style_print_string(s: *const libc::c_char) -> libc::c_int {
  if s.is_null() {
    RESULT.with(|r| {
      *r.borrow_mut() = -1;
    });
    set_last_error(anyhow::anyhow!("Received null pointer for print string"));
    return r!();
  };
  let c_str: &std::ffi::CStr = unsafe { std::ffi::CStr::from_ptr(s) };
  if let Ok(string) = c_str.to_str() {
    queue!(std::io::stdout(), crossterm::style::Print(string)).c_unwrap();
    r!()
  } else {
    RESULT.with(|r| {
      *r.borrow_mut() = -1;
    });
    set_last_error(anyhow::anyhow!("Received invalid UTF-8 string for print string"));
    r!()
  }
}

/// Print string to stdout
///
/// # Safety
///
/// This function takes a raw pointer as argument. As such, the caller must ensure that:
/// - The `title` pointer points to a valid null-terminated string.
/// - This function borrows a slice to a valid null-terminated string and the memory referenced by `title` won't be deallocated or modified for the duration of the function call..
/// - The `title` pointer is correctly aligned and `title` points to an initialized memory.
///
/// If these conditions are not met, the behavior is undefined.
#[no_mangle]
pub unsafe extern "C" fn crossterm_style_print(s: *const libc::c_char) -> libc::c_int {
  unsafe { crossterm_style_print_string(s) }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Color {
  /// Resets the terminal color.
  Reset,
  /// Black color.
  Black,
  /// Dark grey color.
  DarkGrey,
  /// Light red color.
  Red,
  /// Dark red color.
  DarkRed,
  /// Light green color.
  Green,
  /// Dark green color.
  DarkGreen,
  /// Light yellow color.
  Yellow,
  /// Dark yellow color.
  DarkYellow,
  /// Light blue color.
  Blue,
  /// Dark blue color.
  DarkBlue,
  /// Light magenta color.
  Magenta,
  /// Dark magenta color.
  DarkMagenta,
  /// Light cyan color.
  Cyan,
  /// Dark cyan color.
  DarkCyan,
  /// White color.
  White,
  /// Grey color.
  Grey,
  /// An RGB color. See [RGB color model](https://en.wikipedia.org/wiki/RGB_color_model) for more info.
  ///
  /// Most UNIX terminals and Windows 10 supported only.
  Rgb { r: u8, g: u8, b: u8 },
  /// An ANSI color. See [256 colors - cheat sheet](https://jonasjacek.github.io/colors/) for more info.
  ///
  /// Most UNIX terminals and Windows 10 supported only.
  AnsiValue(u8),
}

impl From<Color> for crossterm::style::Color {
  fn from(color: Color) -> Self {
    match color {
      Color::Reset => crossterm::style::Color::Reset,
      Color::Black => crossterm::style::Color::Black,
      Color::DarkGrey => crossterm::style::Color::DarkGrey,
      Color::Red => crossterm::style::Color::Red,
      Color::DarkRed => crossterm::style::Color::DarkRed,
      Color::Green => crossterm::style::Color::Green,
      Color::DarkGreen => crossterm::style::Color::DarkGreen,
      Color::Yellow => crossterm::style::Color::Yellow,
      Color::DarkYellow => crossterm::style::Color::DarkYellow,
      Color::Blue => crossterm::style::Color::Blue,
      Color::DarkBlue => crossterm::style::Color::DarkBlue,
      Color::Magenta => crossterm::style::Color::Magenta,
      Color::DarkMagenta => crossterm::style::Color::DarkMagenta,
      Color::Cyan => crossterm::style::Color::Cyan,
      Color::DarkCyan => crossterm::style::Color::DarkCyan,
      Color::White => crossterm::style::Color::White,
      Color::Grey => crossterm::style::Color::Grey,
      Color::Rgb { r, g, b } => crossterm::style::Color::Rgb { r, g, b },
      Color::AnsiValue(v) => crossterm::style::Color::AnsiValue(v),
    }
  }
}

/// Sets the the background color.
///
/// See [`Color`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color(color: Color) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetBackgroundColor(color.into())).c_unwrap();
  r!()
}

/// Sets the the background color in RGB.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_rgb(r: u8, g: u8, b: u8) -> libc::c_int {
  crossterm_style_background_color(Color::Rgb { r, g, b })
}

/// Sets the the background color to an ANSI value.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_ansi(value: u8) -> libc::c_int {
  crossterm_style_background_color(Color::AnsiValue(value))
}

/// Sets the the background color to Reset.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_reset() -> libc::c_int {
  crossterm_style_background_color(Color::Reset)
}

/// Sets the the background color to Black.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_black() -> libc::c_int {
  crossterm_style_background_color(Color::Black)
}

/// Sets the the background color to DarkGrey.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_grey() -> libc::c_int {
  crossterm_style_background_color(Color::DarkGrey)
}

/// Sets the the background color to Red.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_red() -> libc::c_int {
  crossterm_style_background_color(Color::Red)
}

/// Sets the the background color to DarkRed.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_red() -> libc::c_int {
  crossterm_style_background_color(Color::DarkRed)
}

/// Sets the the background color to Green.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_green() -> libc::c_int {
  crossterm_style_background_color(Color::Green)
}

/// Sets the the background color to DarkGreen.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_green() -> libc::c_int {
  crossterm_style_background_color(Color::DarkGreen)
}

/// Sets the the background color to Yellow.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_yellow() -> libc::c_int {
  crossterm_style_background_color(Color::Yellow)
}

/// Sets the the background color to DarkYellow.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_yellow() -> libc::c_int {
  crossterm_style_background_color(Color::DarkYellow)
}

/// Sets the the background color to Blue.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_blue() -> libc::c_int {
  crossterm_style_background_color(Color::Blue)
}

/// Sets the the background color to DarkBlue.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_blue() -> libc::c_int {
  crossterm_style_background_color(Color::DarkBlue)
}

/// Sets the the background color to Magenta.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_magenta() -> libc::c_int {
  crossterm_style_background_color(Color::Magenta)
}

/// Sets the the background color to DarkMagenta.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_magenta() -> libc::c_int {
  crossterm_style_background_color(Color::DarkMagenta)
}

/// Sets the the background color to Cyan.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_cyan() -> libc::c_int {
  crossterm_style_background_color(Color::Cyan)
}

/// Sets the the background color to DarkCyan.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_dark_cyan() -> libc::c_int {
  crossterm_style_background_color(Color::DarkCyan)
}

/// Sets the the background color to White.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_white() -> libc::c_int {
  crossterm_style_background_color(Color::White)
}

/// Sets the the background color to Grey.
#[no_mangle]
pub extern "C" fn crossterm_style_background_color_grey() -> libc::c_int {
  crossterm_style_background_color(Color::Grey)
}

/// Sets the the foreground color.
///
/// See [`Color`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color(color: Color) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetForegroundColor(color.into())).c_unwrap();
  r!()
}

/// Sets the the foreground color in RGB.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_rgb(r: u8, g: u8, b: u8) -> libc::c_int {
  crossterm_style_foreground_color(Color::Rgb { r, g, b })
}

/// Sets the the foreground color to an ANSI value.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_ansi(value: u8) -> libc::c_int {
  crossterm_style_foreground_color(Color::AnsiValue(value))
}

/// Sets the the foreground color to Reset.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_reset() -> libc::c_int {
  crossterm_style_foreground_color(Color::Reset)
}

/// Sets the the foreground color to Black.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_black() -> libc::c_int {
  crossterm_style_foreground_color(Color::Black)
}

/// Sets the the foreground color to DarkGrey.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_grey() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkGrey)
}

/// Sets the the foreground color to Red.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_red() -> libc::c_int {
  crossterm_style_foreground_color(Color::Red)
}

/// Sets the the foreground color to DarkRed.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_red() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkRed)
}

/// Sets the the foreground color to Green.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_green() -> libc::c_int {
  crossterm_style_foreground_color(Color::Green)
}

/// Sets the the foreground color to DarkGreen.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_green() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkGreen)
}

/// Sets the the foreground color to Yellow.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_yellow() -> libc::c_int {
  crossterm_style_foreground_color(Color::Yellow)
}

/// Sets the the foreground color to DarkYellow.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_yellow() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkYellow)
}

/// Sets the the foreground color to Blue.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_blue() -> libc::c_int {
  crossterm_style_foreground_color(Color::Blue)
}

/// Sets the the foreground color to DarkBlue.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_blue() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkBlue)
}

/// Sets the the foreground color to Magenta.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_magenta() -> libc::c_int {
  crossterm_style_foreground_color(Color::Magenta)
}

/// Sets the the foreground color to DarkMagenta.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_magenta() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkMagenta)
}

/// Sets the the foreground color to Cyan.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_cyan() -> libc::c_int {
  crossterm_style_foreground_color(Color::Cyan)
}

/// Sets the the foreground color to DarkCyan.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_dark_cyan() -> libc::c_int {
  crossterm_style_foreground_color(Color::DarkCyan)
}

/// Sets the the foreground color to White.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_white() -> libc::c_int {
  crossterm_style_foreground_color(Color::White)
}

/// Sets the the foreground color to Grey.
#[no_mangle]
pub extern "C" fn crossterm_style_foreground_color_grey() -> libc::c_int {
  crossterm_style_foreground_color(Color::Grey)
}

/// Sets the the underline color.
///
/// See [`Color`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color(color: Color) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::SetUnderlineColor(color.into())).c_unwrap();
  r!()
}

/// Sets the the underline color in RGB.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_rgb(r: u8, g: u8, b: u8) -> libc::c_int {
  crossterm_style_underline_color(Color::Rgb { r, g, b })
}

/// Sets the the underline color to an ANSI value.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_ansi(value: u8) -> libc::c_int {
  crossterm_style_underline_color(Color::AnsiValue(value))
}

/// Sets the the underline color to Reset.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_reset() -> libc::c_int {
  crossterm_style_underline_color(Color::Reset)
}

/// Sets the the underline color to Black.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_black() -> libc::c_int {
  crossterm_style_underline_color(Color::Black)
}

/// Sets the the underline color to DarkGrey.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_grey() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkGrey)
}

/// Sets the the underline color to Red.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_red() -> libc::c_int {
  crossterm_style_underline_color(Color::Red)
}

/// Sets the the underline color to DarkRed.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_red() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkRed)
}

/// Sets the the underline color to Green.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_green() -> libc::c_int {
  crossterm_style_underline_color(Color::Green)
}

/// Sets the the underline color to DarkGreen.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_green() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkGreen)
}

/// Sets the the underline color to Yellow.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_yellow() -> libc::c_int {
  crossterm_style_underline_color(Color::Yellow)
}

/// Sets the the underline color to DarkYellow.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_yellow() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkYellow)
}

/// Sets the the underline color to Blue.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_blue() -> libc::c_int {
  crossterm_style_underline_color(Color::Blue)
}

/// Sets the the underline color to DarkBlue.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_blue() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkBlue)
}

/// Sets the the underline color to Magenta.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_magenta() -> libc::c_int {
  crossterm_style_underline_color(Color::Magenta)
}

/// Sets the the underline color to DarkMagenta.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_magenta() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkMagenta)
}

/// Sets the the underline color to Cyan.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_cyan() -> libc::c_int {
  crossterm_style_underline_color(Color::Cyan)
}

/// Sets the the underline color to DarkCyan.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_dark_cyan() -> libc::c_int {
  crossterm_style_underline_color(Color::DarkCyan)
}

/// Sets the the underline color to White.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_white() -> libc::c_int {
  crossterm_style_underline_color(Color::White)
}

/// Sets the the underline color to Grey.
#[no_mangle]
pub extern "C" fn crossterm_style_underline_color_grey() -> libc::c_int {
  crossterm_style_underline_color(Color::Grey)
}

/// Resets the colors back to default.
#[no_mangle]
pub extern "C" fn crossterm_style_reset_color() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::ResetColor).c_unwrap();
  r!()
}

/// Returns colors as a UTF-8 JSON string.
///
/// Null character is stored in the last location of buffer.
/// Caller is responsible for memory associated with string buffer.
/// Use [`crossterm_free_c_char`] to free data.
#[no_mangle]
pub extern "C" fn crossterm_colors() -> *const libc::c_char {
  convert_string_to_c_char(COLORS.to_string())
}

/// Tells whether the raw mode is enabled.
///
/// Check error message to see if this function failed
#[no_mangle]
pub fn crossterm_terminal_is_raw_mode_enabled() -> bool {
  crossterm::terminal::is_raw_mode_enabled().c_unwrap()
}

/// Disables raw mode.
#[no_mangle]
pub extern "C" fn crossterm_terminal_disable_raw_mode() -> libc::c_int {
  crossterm::terminal::disable_raw_mode().c_unwrap();
  r!()
}

/// Enables raw mode.
#[no_mangle]
pub extern "C" fn crossterm_terminal_enable_raw_mode() -> libc::c_int {
  crossterm::terminal::enable_raw_mode().c_unwrap();
  r!()
}

/// Get terminal size
#[no_mangle]
pub extern "C" fn crossterm_terminal_size(width: &mut u16, height: &mut u16) -> libc::c_int {
  let (w, h) = crossterm::terminal::size().c_unwrap();
  *width = w;
  *height = h;
  r!()
}

/// Sets the terminal buffer size `(cols, rows)`.
#[no_mangle]
pub extern "C" fn crossterm_terminal_size_set(cols: u16, rows: u16) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::SetSize(cols, rows)).c_unwrap();
  r!()
}

/// Disables line wrapping.
#[no_mangle]
pub extern "C" fn crossterm_terminal_disable_line_wrap() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::DisableLineWrap).c_unwrap();
  r!()
}

/// Enables line wrapping.
#[no_mangle]
pub extern "C" fn crossterm_terminal_enable_line_wrap() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::EnableLineWrap).c_unwrap();
  r!()
}

/// Enters alternate screen.
#[no_mangle]
pub extern "C" fn crossterm_terminal_enter_alternate_screen() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen).c_unwrap();
  r!()
}

/// Leaves alternate screen.
#[no_mangle]
pub extern "C" fn crossterm_terminal_leave_alternate_screen() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).c_unwrap();
  r!()
}

/// Different ways to clear the terminal buffer.
#[repr(C)]
pub enum ClearType {
  /// All cells.
  All,
  /// All plus history
  Purge,
  /// All cells from the cursor position downwards.
  FromCursorDown,
  /// All cells from the cursor position upwards.
  FromCursorUp,
  /// All cells at the cursor row.
  CurrentLine,
  /// All cells from the cursor position until the new line.
  UntilNewLine,
}

impl From<ClearType> for crossterm::terminal::ClearType {
  fn from(value: ClearType) -> Self {
    match value {
      ClearType::All => crossterm::terminal::ClearType::All,
      ClearType::Purge => crossterm::terminal::ClearType::Purge,
      ClearType::FromCursorDown => crossterm::terminal::ClearType::FromCursorDown,
      ClearType::FromCursorUp => crossterm::terminal::ClearType::FromCursorUp,
      ClearType::CurrentLine => crossterm::terminal::ClearType::CurrentLine,
      ClearType::UntilNewLine => crossterm::terminal::ClearType::UntilNewLine,
    }
  }
}

/// Scroll up command.
#[no_mangle]
pub extern "C" fn crossterm_terminal_scroll_up(n: libc::c_ushort) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::ScrollUp(n)).c_unwrap();
  r!()
}

/// Scroll down command.
#[no_mangle]
pub extern "C" fn crossterm_terminal_scroll_down(n: libc::c_ushort) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::ScrollDown(n)).c_unwrap();
  r!()
}

/// Clear screen command.
#[no_mangle]
pub extern "C" fn crossterm_terminal_clear(ct: ClearType) -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::Clear(ct.into())).c_unwrap();
  r!()
}

/// Sets terminal title.
///
/// # Safety
///
/// This function takes a raw pointer as argument. As such, the caller must ensure that:
/// - The `title` pointer points to a valid null-terminated string.
/// - This function borrows a slice to a valid null-terminated string and the memory referenced by `title` won't be deallocated or modified for the duration of the function call..
/// - The `title` pointer is correctly aligned and `title` points to an initialized memory.
///
/// If these conditions are not met, the behavior is undefined.
#[no_mangle]
pub unsafe extern "C" fn crossterm_terminal_title(title: *const libc::c_char) -> libc::c_int {
  if title.is_null() {
    RESULT.with(|r| {
      *r.borrow_mut() = -1;
    });
    set_last_error(anyhow::anyhow!("Received null pointer for title string"));
    return r!();
  };
  let c_str: &std::ffi::CStr = unsafe { std::ffi::CStr::from_ptr(title) };
  if let Ok(string) = c_str.to_str() {
    queue!(std::io::stdout(), crossterm::terminal::SetTitle(string)).c_unwrap();
    r!()
  } else {
    RESULT.with(|r| {
      *r.borrow_mut() = -1;
    });
    set_last_error(anyhow::anyhow!("Received invalid UTF-8 string for title"));
    r!()
  }
}

/// Instructs the terminal emulator to begin a synchronized frame.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [`crossterm_terminal_end_synchronized_update`] command to leave the entered alternate screen.
///
/// When rendering the screen of the terminal, the Emulator usually iterates through each visible grid cell and
/// renders its current state. With applications updating the screen a at higher frequency this can cause tearing.
///
/// This mode attempts to mitigate that.
///
/// When the synchronization mode is enabled following render calls will keep rendering the last rendered state.
/// The terminal Emulator keeps processing incoming text and sequences. When the synchronized update mode is disabled
/// again the renderer may fetch the latest screen buffer state again, effectively avoiding the tearing effect
/// by unintentionally rendering in the middle a of an application screen update.
#[no_mangle]
pub extern "C" fn crossterm_terminal_begin_synchronized_update() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::BeginSynchronizedUpdate).c_unwrap();
  r!()
}

/// Instructs the terminal to end a synchronized frame.
///
/// # Notes
///
/// * Commands must be executed/queued for execution otherwise they do nothing.
/// * Use [`crossterm_terminal_begin_synchronized_update`] to enter the alternate screen.
///
/// When rendering the screen of the terminal, the Emulator usually iterates through each visible grid cell and
/// renders its current state. With applications updating the screen a at higher frequency this can cause tearing.
///
/// This mode attempts to mitigate that.
///
/// When the synchronization mode is enabled following render calls will keep rendering the last rendered state.
/// The terminal Emulator keeps processing incoming text and sequences. When the synchronized update mode is disabled
/// again the renderer may fetch the latest screen buffer state again, effectively avoiding the tearing effect
/// by unintentionally rendering in the middle a of an application screen update.
#[no_mangle]
pub extern "C" fn crossterm_terminal_end_synchronized_update() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::terminal::EndSynchronizedUpdate).c_unwrap();
  r!()
}

/// Instructs the terminal to send a bell.
#[no_mangle]
pub extern "C" fn crossterm_terminal_ring_bell() -> libc::c_int {
  queue!(std::io::stdout(), crossterm::style::Print("\x07")).c_unwrap();
  r!()
}

/// Flush the stdout stream, ensuring that all intermediately buffered contents reach their destination.
///
/// It is considered an error if not all bytes could be written due to I/O errors or EOF being reached.
#[no_mangle]
pub extern "C" fn crossterm_flush() -> libc::c_int {
  if let Err(err) = std::io::stdout().flush() {
    set_last_error(anyhow::anyhow!(err))
  }
  r!()
}
