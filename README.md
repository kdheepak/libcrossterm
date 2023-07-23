# libcrossterm

`libcrossterm` is a Rust `cdylib` library providing a C-API to access the rich functionality to control and manipulate terminal interfaces provided the excellent [`crossterm`](https://docs.rs/crossterm/latest) crate.
This includes operations like cursor movement, color and style management, and much more.

This library is built on the `crossterm` crate and exposes the functionality in a C ABI friendly way so that it is callable from C and other languages.
See the [documentation](https://docs.rs/crate/libcrossterm/) for more information.

## Features

- Cursor movement (up, down, left, right, etc.)
- Color manipulation (foreground, background)
- Style manipulation (bold, underline, etc.)
- Terminal manipulation (clearing, resizing, etc.)
- Scroll operations
- Screen buffering
