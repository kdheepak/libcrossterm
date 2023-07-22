use crossterm::{self, execute};
use log::trace;

fn convert_string_to_c_char(string: String) -> *mut libc::c_char {
  // Convert the String to a CString
  let c_string = match std::ffi::CString::new(string) {
    Ok(c_string) => c_string,
    Err(_) => return std::ptr::null_mut(),
  };

  // Allocate space for the string
  let string_len = c_string.as_bytes_with_nul().len();
  let addr = unsafe {
    let addr = libc::malloc(string_len) as *mut libc::c_char;
    if addr.is_null() {
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
/// Caller is responsible to memory associated with string buffer.
/// Use [`crossterm_free_c_char`] to free data.
#[no_mangle]
pub extern "C" fn crossterm_last_error_message() -> *const libc::c_char {
  let last_error = take_last_error()
    .unwrap_or(anyhow::anyhow!("No error message found. Check library documentation for more information."));
  let string = format!("{:#}", last_error);
  convert_string_to_c_char(string)
}

#[no_mangle]
pub extern "C" fn crossterm_free_c_char(s: *mut libc::c_char) -> libc::c_int {
  if !s.is_null() {
    unsafe {
      libc::free(s as *mut libc::c_void);
    }
  }
  0
}

/// Checks if there is an `Event` available.
///
/// Returns `true` if an `Event` is available otherwise it returns `false`.
///
/// `true` guarantees that subsequent call to the [`crossterm_event_read`] function
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

/// Reads a single Event as a UTF-8 json string.
///
/// This function blocks until an Event is available.
/// Combine it with the [`crossterm_event_poll`] function to get non-blocking reads.
/// User is responsible to free string. Use [`crossterm_free_c_char`] to free data
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

/// Sleeps for `seconds` seconds
#[no_mangle]
pub extern "C" fn crossterm_sleep(seconds: f64) {
  let duration = std::time::Duration::from_nanos((seconds * 1e9).round() as u64);
  std::thread::sleep(duration);
}

/// CursorPosition struct
#[repr(C)]
pub struct CursorPosition {
  pub column: u16,
  pub row: u16,
}

/// Get cursor position (column, row)
#[no_mangle]
pub extern "C" fn crossterm_cursor_position_get(pos: &mut CursorPosition) -> libc::c_int {
  let (column, row) = crossterm::cursor::position().c_unwrap();
  pos.column = column;
  pos.row = row;
  r!()
}

/// Set cursor position (column, row)
#[no_mangle]
pub extern "C" fn crossterm_cursor_position_set(pos: CursorPosition) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveTo(pos.column, pos.row)).c_unwrap();
  r!()
}

/// Moves the terminal cursor to the given position (column, row).
///
/// # Notes
/// * Top left cell is represented as `0,0`.
#[no_mangle]
pub extern "C" fn crossterm_cursor_moveto(x: u16, y: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveTo(x, y)).c_unwrap();
  r!()
}

/// Moves the terminal cursor down the given number of lines and moves it to the first column.
///
/// # Notes
/// * This command is 1 based, meaning `crossterm_cursor_move_to_next_line(1)` moves to the next line.
/// * Most terminals default 0 argument to 1.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_next_line(n: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveToNextLine(n)).c_unwrap();
  r!()
}

/// Moves the terminal cursor up the given number of lines and moves it to the first column.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_previous_line(n: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveToPreviousLine(n)).c_unwrap();
  r!()
}

/// Moves the terminal cursor to the given column on the current row.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_column(column: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveToColumn(column)).c_unwrap();
  r!()
}

/// Moves the terminal cursor to the given row on the current column.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_to_row(row: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveToRow(row)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of rows up.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_up(rows: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveUp(rows)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of columns to the right.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_right(columns: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveRight(columns)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of rows down.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_down(rows: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveDown(rows)).c_unwrap();
  r!()
}

/// Moves the terminal cursor a given number of columns to the left.
#[no_mangle]
pub extern "C" fn crossterm_cursor_move_left(columns: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::MoveLeft(columns)).c_unwrap();
  r!()
}

/// Saves the current terminal cursor position.
#[no_mangle]
pub extern "C" fn crossterm_cursor_save_position() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SavePosition).c_unwrap();
  r!()
}

/// Restores the saved terminal cursor position.
#[no_mangle]
pub extern "C" fn crossterm_cursor_restore_position() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::RestorePosition).c_unwrap();
  r!()
}

/// Hides the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_hide() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::Hide).c_unwrap();
  r!()
}

/// Shows the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_show() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::Show).c_unwrap();
  r!()
}

/// Enables blinking of the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_enable_blinking() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::EnableBlinking).c_unwrap();
  r!()
}

/// Disables blinking of the terminal cursor.
#[no_mangle]
pub extern "C" fn crossterm_cursor_disable_blinking() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::DisableBlinking).c_unwrap();
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
pub extern "C" fn crossterm_cursor_set_style(cursor_style: CursorStyle) -> libc::c_int {
  let cs = match cursor_style {
    CursorStyle::DefaultUserShape => crossterm::cursor::SetCursorStyle::DefaultUserShape,
    CursorStyle::BlinkingBlock => crossterm::cursor::SetCursorStyle::BlinkingBlock,
    CursorStyle::SteadyBlock => crossterm::cursor::SetCursorStyle::SteadyBlock,
    CursorStyle::BlinkingUnderScore => crossterm::cursor::SetCursorStyle::BlinkingUnderScore,
    CursorStyle::SteadyUnderScore => crossterm::cursor::SetCursorStyle::SteadyUnderScore,
    CursorStyle::BlinkingBar => crossterm::cursor::SetCursorStyle::BlinkingBar,
    CursorStyle::SteadyBar => crossterm::cursor::SetCursorStyle::SteadyBar,
  };
  execute!(std::io::stdout(), cs).c_unwrap();
  r!()
}

/// Sets the style of the cursor to default user shape.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_default_user_shape() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::DefaultUserShape).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking block.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_blinking_block() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingBlock).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady block.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_steady_block() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyBlock).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking underscore.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_blinking_underscore() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingUnderScore).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady underscore.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_steady_underscore() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyUnderScore).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a blinking bar.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_blinking_bar() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::BlinkingBar).c_unwrap();
  r!()
}

/// Sets the style of the cursor to a steady bar.
#[no_mangle]
pub extern "C" fn crossterm_cursor_set_style_steady_bar() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::cursor::SetCursorStyle::SteadyBar).c_unwrap();
  r!()
}

/// Enable mouse event capturing.
#[no_mangle]
pub extern "C" fn crossterm_event_enable_mouse_capture() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::EnableMouseCapture).c_unwrap();
  r!()
}

/// Disable mouse event capturing.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_mouse_capture() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::DisableMouseCapture).c_unwrap();
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
  execute!(std::io::stdout(), crossterm::event::PushKeyboardEnhancementFlags(flags)).c_unwrap();
  r!()
}

/// Disables extra kinds of keyboard events.
#[no_mangle]
pub extern "C" fn crossterm_event_pop_keyboard_enhancement_flags() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::PopKeyboardEnhancementFlags).c_unwrap();
  r!()
}

/// Enable focus event emission.
///
/// It should be paired with [`crossterm_event_disable_focus_change`] at the end of execution.
///
/// Focus events can be captured with [`crossterm_event_read`].
#[no_mangle]
pub extern "C" fn crossterm_event_enable_focus_change() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::EnableFocusChange).c_unwrap();
  r!()
}

/// Disable focus event emission.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_focus_change() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::DisableFocusChange).c_unwrap();
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
  execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste).c_unwrap();
  r!()
}

/// Disables bracketed paste mode.
#[no_mangle]
pub extern "C" fn crossterm_event_disable_bracketed_paste() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste).c_unwrap();
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
pub extern "C" fn crossterm_style_set_attribute(attr: Attribute) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::style::SetAttribute(attr.into())).c_unwrap();
  r!()
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
pub extern "C" fn crossterm_style_set_background_color(color: Color) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::style::SetBackgroundColor(color.into())).c_unwrap();
  r!()
}

/// Sets the the foreground color.
///
/// See [`Color`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_set_foreground_color(color: Color) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::style::SetForegroundColor(color.into())).c_unwrap();
  r!()
}

/// Sets the the underline color.
///
/// See [`Color`] for more info.
#[no_mangle]
pub extern "C" fn crossterm_style_set_underline_color(color: Color) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::style::SetUnderlineColor(color.into())).c_unwrap();
  r!()
}

/// Resets the colors back to default.
#[no_mangle]
pub extern "C" fn crossterm_style_reset_color() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::style::ResetColor).c_unwrap();
  r!()
}

/// Tells whether the raw mode is enabled.
///
/// Check error message to see if this function failed
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

/// TerminalSize
#[repr(C)]
pub struct TerminalSize {
  pub width: u16,
  pub height: u16,
}

/// Get terminal size
#[no_mangle]
pub extern "C" fn crossterm_terminal_size(size: &mut TerminalSize) -> libc::c_int {
  let (width, height) = crossterm::terminal::size().c_unwrap();
  size.width = width;
  size.height = height;
  r!()
}

/// Disables line wrapping.
#[no_mangle]
pub extern "C" fn crossterm_disable_line_wrap() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::DisableLineWrap).c_unwrap();
  r!()
}

/// Enables line wrapping.
#[no_mangle]
pub extern "C" fn crossterm_enable_line_wrap() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::EnableLineWrap).c_unwrap();
  r!()
}

/// Enters alternate screen.
#[no_mangle]
pub extern "C" fn crossterm_enter_alternate_screen() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen).c_unwrap();
  r!()
}

/// Leaves alternate screen.
#[no_mangle]
pub extern "C" fn crossterm_leave_alternate_screen() -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).c_unwrap();
  r!()
}

/// Different ways to clear the terminal buffer.
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
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
  execute!(std::io::stdout(), crossterm::terminal::ScrollUp(n)).c_unwrap();
  r!()
}

/// Scroll down command.
#[no_mangle]
pub extern "C" fn crossterm_terminal_scroll_down(n: libc::c_ushort) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::ScrollDown(n)).c_unwrap();
  r!()
}

/// Clear screen command.
#[no_mangle]
pub extern "C" fn crossterm_terminal_clear(ct: ClearType) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::Clear(ct.into())).c_unwrap();
  r!()
}

/// Sets the terminal buffer size `(columns, rows)`.
#[no_mangle]
pub extern "C" fn crossterm_terminal_set_size(columns: u16, rows: u16) -> libc::c_int {
  execute!(std::io::stdout(), crossterm::terminal::SetSize(columns, rows)).c_unwrap();
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
pub unsafe extern "C" fn crossterm_terminal_set_title(title: *const libc::c_char) -> libc::c_int {
  let c_str: &std::ffi::CStr = unsafe { std::ffi::CStr::from_ptr(title) };
  let string = c_str.to_str().unwrap();
  execute!(std::io::stdout(), crossterm::terminal::SetTitle(string)).c_unwrap();
  r!()
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
  execute!(std::io::stdout(), crossterm::terminal::BeginSynchronizedUpdate).c_unwrap();
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
  execute!(std::io::stdout(), crossterm::terminal::EndSynchronizedUpdate).c_unwrap();
  r!()
}
