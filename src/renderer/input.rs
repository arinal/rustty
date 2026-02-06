//! Input handling for terminal UI
//!
//! This module handles keyboard input, mouse events, clipboard operations,
//! and focus events for the terminal emulator.

use crate::TerminalSession;
use arboard::Clipboard;
use std::time::Instant;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Generate mouse event escape sequence
///
/// Converts mouse events to appropriate ANSI escape sequences based on the
/// active mouse tracking mode.
///
/// # Arguments
///
/// * `state` - Terminal state to check which mouse mode is active
/// * `button` - Mouse button (0=left, 1=middle, 2=right)
/// * `col` - Grid column (0-indexed)
/// * `row` - Grid row (0-indexed)
/// * `pressed` - true for press, false for release
///
/// # Returns
///
/// ANSI escape sequence bytes, or empty vec if no mouse mode is active
pub fn generate_mouse_sequence(
    state: &crate::TerminalState,
    button: u8,
    col: usize,
    row: usize,
    pressed: bool,
) -> Vec<u8> {
    // Convert button to protocol value (0=left, 1=middle, 2=right, 3=release)
    let cb = if !pressed {
        3 // Release
    } else {
        button
    };

    if state.mouse_sgr {
        // SGR mouse protocol: ESC[<Cb;Cx;CyM/m
        // M for press, m for release
        let suffix = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", cb, col + 1, row + 1, suffix).into_bytes()
    } else if state.mouse_tracking || state.mouse_cell_motion {
        // X10/X11 mouse protocol: ESC[MCbCxCy
        // Coordinates are encoded as value + 32 + 1 (1-indexed)
        let encoded_button = (cb + 32) as u8;
        let encoded_col = (col + 1 + 32).min(255) as u8;
        let encoded_row = (row + 1 + 32).min(255) as u8;
        vec![0x1b, b'[', b'M', encoded_button, encoded_col, encoded_row]
    } else {
        vec![]
    }
}

/// Handle keyboard input and generate appropriate sequences
///
/// This function processes keyboard events and sends the appropriate
/// escape sequences or characters to the terminal session.
///
/// Returns None if paste was triggered (Ctrl+V), otherwise returns the bytes to send.
pub fn handle_keyboard_input(
    session: &mut TerminalSession,
    key: &Key,
    text: Option<&str>,
    modifiers: &ModifiersState,
) -> Option<Vec<u8>> {
    match key {
        Key::Named(named) => match named {
            NamedKey::Enter => Some(b"\r".to_vec()),
            NamedKey::Backspace => Some(b"\x7f".to_vec()),
            NamedKey::Tab => Some(b"\t".to_vec()),
            NamedKey::Space => Some(b" ".to_vec()),
            NamedKey::Escape => Some(b"\x1b".to_vec()),
            NamedKey::ArrowUp => {
                if session.state().application_cursor_keys {
                    Some(b"\x1bOA".to_vec())
                } else {
                    Some(b"\x1b[A".to_vec())
                }
            }
            NamedKey::ArrowDown => {
                if session.state().application_cursor_keys {
                    Some(b"\x1bOB".to_vec())
                } else {
                    Some(b"\x1b[B".to_vec())
                }
            }
            NamedKey::ArrowRight => {
                if session.state().application_cursor_keys {
                    Some(b"\x1bOC".to_vec())
                } else {
                    Some(b"\x1b[C".to_vec())
                }
            }
            NamedKey::ArrowLeft => {
                if session.state().application_cursor_keys {
                    Some(b"\x1bOD".to_vec())
                } else {
                    Some(b"\x1b[D".to_vec())
                }
            }
            NamedKey::Home => Some(b"\x1b[H".to_vec()),
            NamedKey::End => Some(b"\x1b[F".to_vec()),
            NamedKey::PageUp => Some(b"\x1b[5~".to_vec()),
            NamedKey::PageDown => Some(b"\x1b[6~".to_vec()),
            NamedKey::Delete => Some(b"\x1b[3~".to_vec()),
            NamedKey::Insert => Some(b"\x1b[2~".to_vec()),
            _ => None,
        },
        Key::Character(s) => {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() == 1 {
                let ch = chars[0];

                // Check if Ctrl modifier is pressed
                if modifiers.control_key() && ch.is_ascii_alphabetic() {
                    let lower = ch.to_ascii_lowercase();

                    // Intercept Ctrl+V for paste - return None as signal
                    if lower == 'v' {
                        return None;
                    }

                    // Ctrl+letter produces control codes 1-26
                    let ctrl_code = (lower as u8) - b'a' + 1;
                    Some(vec![ctrl_code])
                } else if let Some(text_str) = text {
                    Some(text_str.as_bytes().to_vec())
                } else {
                    Some(s.as_bytes().to_vec())
                }
            } else if let Some(text_str) = text {
                Some(text_str.as_bytes().to_vec())
            } else {
                Some(s.as_bytes().to_vec())
            }
        }
        _ => None,
    }
}

/// Handle clipboard paste operation
///
/// Reads text from the clipboard and sends it to the terminal,
/// optionally wrapping it with bracketed paste sequences.
pub fn handle_paste(session: &mut TerminalSession, clipboard: &mut Option<Clipboard>) {
    if let Some(clipboard) = clipboard {
        match clipboard.get_text() {
            Ok(text) => {
                let data = if session.state().bracketed_paste {
                    // Wrap pasted text with bracketed paste sequences
                    let mut result = Vec::new();
                    result.extend_from_slice(b"\x1b[200~");
                    result.extend_from_slice(text.as_bytes());
                    result.extend_from_slice(b"\x1b[201~");
                    result
                } else {
                    text.as_bytes().to_vec()
                };

                if let Err(e) = session.write_input(&data) {
                    eprintln!("Failed to write paste: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to read clipboard: {}", e);
            }
        }
    }
}

/// Reset cursor blink state to visible
pub fn reset_cursor_blink(cursor_visible_phase: &mut bool, last_blink_toggle: &mut Instant) {
    *cursor_visible_phase = true;
    *last_blink_toggle = Instant::now();
}

/// Handle focus events (focus in/out)
pub fn handle_focus_event(session: &mut TerminalSession, focused: bool) {
    if session.state().focus_events {
        let sequence = if focused { b"\x1b[I" } else { b"\x1b[O" };
        if let Err(e) = session.write_input(sequence) {
            eprintln!("Failed to write focus event: {}", e);
        }
    }
}

/// Handle mouse button press/release events
pub fn handle_mouse_button(
    session: &mut TerminalSession,
    button_code: u8,
    pressed: bool,
    mouse_buttons_pressed: &mut u8,
    last_mouse_position: Option<(usize, usize)>,
) -> bool {
    if pressed {
        *mouse_buttons_pressed |= 1 << button_code;
    } else {
        *mouse_buttons_pressed &= !(1 << button_code);
    }

    if let Some((col, row)) = last_mouse_position {
        let term_state = session.state();
        if term_state.mouse_tracking || term_state.mouse_cell_motion || term_state.mouse_sgr {
            let sequence = generate_mouse_sequence(term_state, button_code, col, row, pressed);
            if !sequence.is_empty() {
                if let Err(e) = session.write_input(&sequence) {
                    eprintln!("Failed to write mouse event: {}", e);
                    return false;
                }
                return true;
            }
        }
    }
    false
}

/// Handle cursor moved events for mouse tracking
pub fn handle_cursor_moved(
    session: &mut TerminalSession,
    col: usize,
    row: usize,
    prev_position: Option<(usize, usize)>,
    mouse_buttons_pressed: u8,
) -> bool {
    let term_state = session.state();
    if term_state.mouse_cell_motion || term_state.mouse_sgr {
        if mouse_buttons_pressed != 0 && prev_position != Some((col, row)) {
            let button_code = mouse_buttons_pressed.trailing_zeros() as u8;
            let sequence = generate_mouse_sequence(term_state, button_code, col, row, true);
            if !sequence.is_empty() {
                if let Err(e) = session.write_input(&sequence) {
                    eprintln!("Failed to write mouse motion: {}", e);
                    return false;
                }
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mouse_sequence_sgr_press() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_sgr = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, b"\x1b[<0;6;11M");
    }

    #[test]
    fn test_generate_mouse_sequence_sgr_release() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_sgr = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, false);
        assert_eq!(seq, b"\x1b[<3;6;11m");
    }

    #[test]
    fn test_generate_mouse_sequence_x11() {
        let mut state = crate::TerminalState::new(80, 24);
        state.mouse_tracking = true;

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, vec![0x1b, b'[', b'M', 32, 38, 43]);
    }

    #[test]
    fn test_generate_mouse_sequence_no_mode() {
        let state = crate::TerminalState::new(80, 24);

        let seq = generate_mouse_sequence(&state, 0, 5, 10, true);
        assert_eq!(seq, Vec::<u8>::new());
    }
}
