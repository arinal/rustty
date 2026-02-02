# ANSI Escape Sequences in Rustty

**A friendly guide to understanding those weird `\x1b[31m` codes**

## Table of Contents

1. [What Are ANSI Escape Sequences?](#what-are-ansi-escape-sequences)
2. [Why Do They Exist?](#why-do-they-exist)
3. [How Do They Work?](#how-do-they-work)
4. [The Different Types](#the-different-types)
5. [How Rustty Handles Them](#how-rustty-handles-them)
6. [Practical Examples](#practical-examples)
7. [Debugging ANSI Sequences](#debugging-ansi-sequences)
8. [Code References](#code-references)

---

## What Are ANSI Escape Sequences?

Have you ever seen output like this in logs or terminal debugging?

```
\x1b[31mRed Text\x1b[0m
\x1b[1;32mBold Green\x1b[0m
```

Those `\x1b[...m` things are **ANSI escape sequences**.

### What Are the Actual Bytes?

Let's look at what's really being sent. Here's `\x1b[31mRed Text\x1b[0m` as actual bytes:

```
Offset  Hex                                          ASCII
------  -------------------------------------------  ------------------
0000    1b 5b 33 31 6d 52 65 64 20 54 65 78 74       .[31mRed Text
000d    1b 5b 30 6d                                  .[0m
```

**Breaking it down:**
- `1b` = ESC character (byte 27, `\x1b`)
- `5b` = `[` character
- `33 31` = ASCII "31" (two characters: '3' and '1')
- `6d` = `m` character
- `52 65 64 20 54 65 78 74` = "Red Text" (ASCII)
- `1b 5b 30 6d` = `ESC[0m` (reset sequence)

So when you see `\x1b[31m`:
- It's really **5 bytes**: `1b 5b 33 31 6d`
- Not a single "weird character" but a sequence of normal bytes

Another example - `\x1b[1;32mBold Green\x1b[0m`:

```
Offset  Hex                                          ASCII
------  -------------------------------------------  ------------------
0000    1b 5b 31 3b 33 32 6d 42 6f 6c 64 20 47     .[1;32mBold G
000d    72 65 65 6e 1b 5b 30 6d                     reen.[0m
```

**Breaking it down:**
- `1b 5b` = ESC[
- `31 3b 33 32` = "1;32" (four characters: '1', ';', '3', '2')
- `6d` = `m`
- `42 6f 6c 64 20 47 72 65 65 6e` = "Bold Green"
- `1b 5b 30 6d` = ESC[0m (reset)

### What Do They Do?

These special byte sequences tell the terminal to do something other than just "print this character":
- Change text color
- Move the cursor around
- Clear the screen
- Switch to alternate screen (for full-screen apps like vim)

Think of them as **commands embedded in the text stream**.

When bash outputs `\x1b[31mHello`, it's saying:
- `\x1b[31m` = "set color to red" (5 bytes: `1b 5b 33 31 6d`)
- `Hello` = "print these letters" (5 bytes: `48 65 6c 6c 6f`)

The terminal reads these bytes one by one, recognizes the escape sequence, changes the color to red, then prints "Hello" in red.

---

## Why Do They Exist?

Back in the 1970s and 1980s, terminals were **physical hardware devices** (like the VT100).

These terminals had:
- A screen (CRT monitor)
- A keyboard
- A serial connection to a mainframe computer

But the mainframe couldn't directly control the terminal's screen. It could only send **bytes** through the serial connection.

So they invented a protocol: special byte sequences that the terminal would interpret as commands.

For example:
- Send `\x1b[2J` → Terminal clears the screen
- Send `\x1b[5;10H` → Terminal moves cursor to row 5, column 10
- Send `\x1b[31m` → Terminal changes text color to red

This became the **ANSI standard** (from the American National Standards Institute).

**Today**, we don't use hardware terminals anymore. But terminal emulators like Rustty **pretend to be those old hardware terminals**, so all the same escape sequences still work!

That's why your bash prompt has colors, why vim can draw its UI, and why `htop` can update in place.

---

## How Do They Work?

An ANSI escape sequence is just a **sequence of bytes** that starts with `ESC` (the escape character, byte `0x1B` or `\x1b`).

The general format:

```
ESC [ parameters final_byte
```

Where:
- **ESC** = `\x1b` (byte 27)
- **[** = introduces a CSI (Control Sequence Introducer) sequence
- **parameters** = numbers separated by semicolons
- **final_byte** = a letter that says what command this is

### Examples

**Move cursor to row 5, column 10:**
```
ESC [ 5 ; 10 H
\x1b[5;10H
```
- `ESC[` = start CSI sequence
- `5;10` = parameters (row 5, column 10)
- `H` = final byte meaning "move cursor to position"

**Set text color to red:**
```
ESC [ 31 m
\x1b[31m
```
- `ESC[` = start CSI sequence
- `31` = parameter (foreground color red)
- `m` = final byte meaning "SGR" (Select Graphic Rendition)

**Clear entire screen:**
```
ESC [ 2 J
\x1b[2J
```
- `ESC[` = start CSI sequence
- `2` = parameter (clear entire screen)
- `J` = final byte meaning "erase in display"

The terminal reads these byte-by-byte, recognizes the escape sequence, executes the command, then continues printing normal text.

---

## The Different Types

There are several types of escape sequences. Here are the main ones:

### CSI Sequences (Control Sequence Introducer)

**Format:** `ESC[{parameters}{final_byte}`

These are the most common. They control:
- **Cursor movement** - move cursor up/down/left/right, or to specific position
- **Screen clearing** - clear screen or clear line
- **Text styling** - colors, bold, underline (via SGR)

**Examples:**
- `ESC[A` - Move cursor up
- `ESC[5;10H` - Move cursor to row 5, column 10
- `ESC[2J` - Clear entire screen
- `ESC[31m` - Set foreground color to red

### DEC Private Mode Sequences

**Format:** `ESC[?{mode}h` (set) or `ESC[?{mode}l` (reset)

These turn terminal **features** on or off. The `?` indicates it's a "private" (DEC-specific) mode.

**Examples:**
- `ESC[?1049h` - Switch to alternate screen buffer (vim uses this)
- `ESC[?1049l` - Switch back to main screen buffer
- `ESC[?25h` - Show cursor
- `ESC[?25l` - Hide cursor

### SGR Sequences (Select Graphic Rendition)

**Format:** `ESC[{codes}m`

These are a special type of CSI sequence (final byte is `m`) that control **text appearance**:
- Colors (foreground and background)
- Bold, italic, underline
- Reverse video

**Examples:**
- `ESC[0m` - Reset all attributes
- `ESC[1m` - Bold text
- `ESC[31m` - Red foreground
- `ESC[42m` - Green background
- `ESC[38;5;196m` - 256-color red foreground
- `ESC[48;2;255;0;0m` - RGB red background

### OSC Sequences (Operating System Command)

**Format:** `ESC]{command};{data}BEL` or `ESC]{command};{data}ESC\`

These send commands to the terminal emulator itself (not the screen).

**Examples:**
- `ESC]0;Window TitleBEL` - Set window title
- `ESC]2;Window TitleBEL` - Set window title (alternate)

*(Rustty doesn't implement OSC sequences yet)*

---

## How Rustty Handles Them

### The Problem with Raw Bytes

When the PTY sends bytes like `\x1b[31mHello`, how does Rustty know:
- This is an escape sequence?
- It's a CSI sequence?
- Parameter is 31?
- Final byte is `m`?
- This means "set foreground color to red"?

You could parse it manually with a bunch of if-statements checking bytes. But that's error-prone and hard to maintain.

### Rustty's Solution: Typed Enums

Rustty uses **typed enums** to represent ANSI sequences. Instead of working with raw bytes, we work with Rust types.

All the enum definitions are in `src/ansi.rs`.

**The enums:**
- `CsiCommand` - CSI sequences (cursor movement, clearing, SGR)
- `DecPrivateMode` - DEC private modes (alternate screen, cursor visibility, etc.)
- `SgrParameter` - SGR parameters (colors, bold, italic, etc.)
- `EraseMode` - Erase directions (to end, to beginning, all)

### Why Enums?

**Benefits:**
1. **Type safety** - Can't mix up cursor movement with colors
2. **Compile-time checks** - Typos caught at compile time
3. **Pattern matching** - Rust's `match` ensures we handle all cases
4. **Documentation** - Enum variants are self-documenting
5. **Maintainability** - Easy to add new sequences

**Example in `src/ansi.rs`:**

```rust
pub enum CsiCommand {
    CursorPosition,       // H or f
    CursorUp,             // A
    CursorDown,           // B
    CursorForward,        // C
    CursorBack,           // D
    EraseInDisplay,       // J
    EraseInLine,          // K
    SelectGraphicRendition, // m
    // ... more
}

pub enum SgrParameter {
    Reset,                         // 0
    Bold,                          // 1
    Italic,                        // 3
    Underline,                     // 4
    ForegroundColor(u8),           // 30-37
    BackgroundColor(u8),           // 40-47
    ExtendedForeground(ExtendedColor), // 38;5;{index} or 38;2;{r};{g};{b}
    // ... more
}
```

### The Flow

Here's how ANSI sequences flow through Rustty:

```
1. PTY master has bytes: b"\x1b[31mHello"
   ↓
2. PTY reader thread reads bytes
   ↓
3. Sends to main thread via channel
   ↓
4. Main thread receives: Vec<u8>
   ↓
5. Feeds bytes to VTE Parser (vte crate)
   ↓
6. VTE Parser recognizes: ESC[31m
   ↓
7. VTE Parser calls: csi_dispatch(params=[31], final_byte='m')
   ↓
8. TerminalParser converts to: SgrParameter::ForegroundColor(1)
   ↓
9. TerminalParser updates grid: current_fg = RED
   ↓
10. Parser sees "Hello" → prints with red color
   ↓
11. Render draws red text
```

**Key files:**
- `src/terminal/command.rs` - ANSI command enum definitions (CsiCommand, SgrParameter, etc.)
- `src/terminal/mod.rs` - Terminal implements VTE Perform trait, handles sequences
- `src/terminal/grid.rs` - Stores cell colors and attributes

### The VTE Parser

Rustty uses the [vte crate](https://github.com/alacritty/vte) (from the Alacritty project) for parsing.

VTE is a **state machine** that processes bytes one at a time:
- When it sees `\x1b`, it enters "escape" state
- When it sees `[`, it enters "CSI" state
- It accumulates parameters (like `31`)
- When it sees final byte `m`, it calls our callback

This handles all the edge cases:
- Escape sequences split across read boundaries
- Invalid sequences
- Partial sequences

The `Terminal` struct implements the `vte::Perform` trait, which has methods like:
- `print(char)` - regular character to display
- `execute(byte)` - control character like `\n`, `\r`, `\t`
- `csi_dispatch(params, final_byte)` - CSI sequence
- `esc_dispatch(intermediates, final_byte)` - Escape sequence

See `src/terminal/mod.rs` for the VTE Perform implementation.

---

## Practical Examples

### Example 1: Red Text

**Sequence:** `\x1b[31mHello\x1b[0m`

**Byte-by-byte:**
```
\x1b  → VTE: Enter escape state
[     → VTE: Enter CSI state
31    → VTE: Accumulate parameter: 31
m     → VTE: Final byte! Call csi_dispatch(params=[31], final='m')
      → TerminalParser: Parse SGR parameter 31
      → SgrParameter::ForegroundColor(1) // 1 = red (30-37 → 0-7)
      → Set current_fg_color = RED
H     → VTE: Call print('H')
      → TerminalParser: Put 'H' in grid with RED foreground
e     → Print 'e' in RED
l     → Print 'l' in RED
l     → Print 'l' in RED
o     → Print 'o' in RED
\x1b  → VTE: Enter escape state
[     → VTE: Enter CSI state
0     → VTE: Accumulate parameter: 0
m     → VTE: Call csi_dispatch(params=[0], final='m')
      → TerminalParser: Parse SGR parameter 0
      → SgrParameter::Reset
      → Reset all colors to defaults
```

Result: "Hello" appears in red, then colors reset to normal.

### Example 2: Cursor Movement

**Sequence:** `\x1b[5;10H`

**Flow:**
```
\x1b[5;10H
    ↓
VTE parser recognizes: CSI 5;10 H
    ↓
Calls: csi_dispatch(params=[5, 10], final='H')
    ↓
TerminalParser: Match on 'H' → CsiCommand::CursorPosition
    ↓
Set cursor.row = 5, cursor.col = 10
    ↓
Next characters print at position (5, 10)
```

### Example 3: Alternate Screen

**Vim starts up:**
```
\x1b[?1049h
    ↓
VTE recognizes: DEC Private Mode 1049, set (h)
    ↓
Calls: csi_dispatch with private flag
    ↓
TerminalParser: DecPrivateMode::AlternateScreenBuffer
    ↓
Grid: Switch to alternate buffer
    ↓
Vim draws its UI on alternate screen
```

**Vim exits:**
```
\x1b[?1049l
    ↓
DEC Private Mode 1049, reset (l)
    ↓
Grid: Switch back to main buffer
    ↓
Your original terminal content is back!
```

### Example 4: 256-Color

**Sequence:** `\x1b[38;5;196mBright Red`

**Flow:**
```
\x1b[38;5;196m
    ↓
VTE: params = [38, 5, 196], final = 'm'
    ↓
TerminalParser: See params[0] = 38 → ExtendedForeground
    ↓
Parser: See params[1] = 5 → 256-color mode
    ↓
Parser: See params[2] = 196 → color index 196
    ↓
SgrParameter::ExtendedForeground(ExtendedColor::Indexed(196))
    ↓
Color::from_ansi_index(196) → RGB(255, 0, 0)
    ↓
Set foreground to RGB(255, 0, 0)
```

### Example 5: RGB True Color

**Sequence:** `\x1b[48;2;100;150;200mBlue Background`

**Flow:**
```
\x1b[48;2;100;150;200m
    ↓
VTE: params = [48, 2, 100, 150, 200], final = 'm'
    ↓
TerminalParser: params[0] = 48 → ExtendedBackground
    ↓
Parser: params[1] = 2 → RGB mode
    ↓
Parser: r=100, g=150, b=200
    ↓
SgrParameter::ExtendedBackground(ExtendedColor::Rgb{r: 100, g: 150, b: 200})
    ↓
Set background to RGB(100, 150, 200)
```

---

## Debugging ANSI Sequences

Want to see what escape sequences are actually being sent?

### Method 1: Use Rustty's Logging

*(If logging is enabled in Rustty's parser)*

Run Rustty with stderr redirected:
```bash
cargo run 2> debug.log
```

Then check `debug.log` for parsed sequences.

### Method 2: Use cat -v

Show escape sequences visually:
```bash
echo -e "\x1b[31mRed\x1b[0m" | cat -v
```

Output: `^[[31mRed^[[0m`

Where `^[` represents the ESC character.

### Method 3: Use hexdump

See actual bytes:
```bash
echo -e "\x1b[31mRed" | hexdump -C
```

Output:
```
00000000  1b 5b 33 31 6d 52 65 64  0a                       |.[31mRed.|
```

- `1b` = ESC
- `5b` = `[`
- `33 31` = `31` (ASCII)
- `6d` = `m`
- `52 65 64` = `Red`

### Method 4: Read the debugging.md

See `docs/debugging.md` for comprehensive ANSI debugging strategies specific to Rustty.

### Common Sequences You'll See

**bash prompt:**
- `\x1b[01;32m` - Bold green (username)
- `\x1b[01;34m` - Bold blue (directory)
- `\x1b[0m` - Reset

**vim:**
- `\x1b[?1049h` - Enter alternate screen
- `\x1b[H` - Move cursor to home
- `\x1b[2J` - Clear screen
- Lots of cursor movement (`\x1b[{row};{col}H`)
- Lots of colors (`\x1b[{code}m`)
- `\x1b[?1049l` - Exit alternate screen

**ls --color:**
- `\x1b[01;34m` - Directories (bold blue)
- `\x1b[01;32m` - Executables (bold green)
- `\x1b[0m` - Reset between entries

---

## Code References

Want to see the actual implementation?

### Enum Definitions

**File:** `src/terminal/command.rs`

All ANSI command enums are defined here with structured variants carrying their parameters:
- `CsiCommand` enum - CSI sequences (cursor movement, clearing, scrolling, etc.)
- `DecPrivateMode` enum - DEC private modes (alternate screen, cursor keys, mouse modes, etc.)
- `SgrParameter` enum - Text styling parameters (colors, bold, italic, underline, etc.)
- `EraseMode` enum - Screen/line clearing modes (ToEnd, ToBeginning, All, Scrollback)

### Parsing Logic

**File:** `src/terminal/mod.rs`

The `Terminal` struct implements `vte::Perform` trait with these key methods:
- `print()` - Printing regular characters
- `execute()` - Handling control characters (`\n`, `\r`, `\t`)
- `csi_dispatch()` - Handling CSI sequences (parses using `CsiCommand::parse()`)
- `esc_dispatch()` - Handling escape sequences
- `handle_sgr()` - Parses SGR parameters for text styling
- `handle_extended_color()` - Parses 256-color and RGB sequences

**ANSI Command Definitions:**

**File:** `src/terminal/command.rs`

- `CsiCommand` enum - CSI sequences with parameters (cursor movement, clearing, etc.)
- `SgrParameter` enum - Text styling parameters (colors, bold, italic, underline)
- `DecPrivateMode` enum - DEC private modes (alternate screen, cursor visibility, etc.)
- `EraseMode` enum - Screen/line clearing modes

### Grid Updates

**File:** `src/terminal/grid.rs`

- `Cell` struct - Stores character + RGB colors + text attributes (bold, italic, underline)
- `put_cell()` - Puts character at cursor position, auto-grows grid
- `use_alternate_screen()` - Swaps main and alternate screen buffers
- `use_main_screen()` - Swaps back to main buffer

### Color Conversion

**File:** `src/terminal/color.rs`

- `Color` struct - RGB color representation
- `from_ansi_index()` - Converts 256-color index to RGB
- ANSI 16 colors palette (standard + bright)
- 216-color cube calculation (6×6×6)
- 24 grayscale ramp (232-255)

---

## What's Implemented in Rustty?

For a complete list of implemented ANSI features, see the OpenSpec specification:

```bash
openspec show ansi-sequences
```

This tracks:
- ✅ What's fully implemented
- ⏳ What's partially implemented
- ❌ What's planned but not done

**Quick summary:**

**Implemented:**
- Basic cursor movement (up, down, left, right, position)
- Screen clearing (full screen, line)
- All 256 colors + RGB true color
- Alternate screen buffer
- SGR reset

**Not yet implemented:**
- Text attributes (bold, italic, underline) - *stored but not rendered*
- Cursor visibility (show/hide)
- Scrolling regions
- Insert/delete lines
- Mouse support
- Bracketed paste mode

---

## Want to Learn More?

### Related Documentation

- **[Terminal Fundamentals](terminal-fundamentals.md)** - How terminals work (PTY, shells, etc.)
- **[Debugging Guide](debugging.md)** - How to debug ANSI sequences in action

### External Resources

- **[ANSI Escape Code (Wikipedia)](https://en.wikipedia.org/wiki/ANSI_escape_code)** - Comprehensive reference
- **[XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)** - The definitive guide
- **[VT100 User Guide](https://vt100.net/docs/vt100-ug/)** - Original DEC VT100 terminal manual
- **[VTE Parser (Alacritty)](https://github.com/alacritty/vte)** - The parser library Rustty uses

---

**Remember:** ANSI escape sequences are just **commands embedded in the text stream**. When you see `\x1b[31m`, it's not "weird garbage"—it's the program telling the terminal "hey, make the text red!"
