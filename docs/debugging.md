# Debugging ANSI Sequences in Rustty

This document describes the debugging features available in Rustty for tracking ANSI escape sequence handling.

## ANSI Sequence Logging

Rustty now logs all unknown and not-yet-implemented ANSI escape sequences to stderr using `eprintln!`. This helps developers understand what sequences are being sent by applications but not yet supported.

### Log Format

All ANSI debug messages are prefixed with `[ANSI]` for easy filtering:

```
[ANSI] Unknown CSI command: 'X'
[ANSI] Not yet implemented CSI command: InsertLines
[ANSI] Unknown DEC private mode (set): 9999
[ANSI] Not yet implemented DEC private mode (set): ShowCursor
[ANSI] Unknown SGR parameter: 99
[ANSI] Not yet implemented SGR attribute: Bold
```

### Categories of Logged Sequences

#### 1. CSI Commands (Control Sequence Introducer)

**Unknown Commands:**
- Format: `ESC[...{unknown_char}`
- Example: `[ANSI] Unknown CSI command: 'Z'`
- These are completely unrecognized CSI final bytes

**Not Yet Implemented:**
- Format: Recognized but not implemented
- Example: `[ANSI] Not yet implemented CSI command: InsertLines`
- These are defined in the enum but not handled in the parser

Currently implemented CSI commands:
- ✅ `CursorPosition` (H, f)
- ✅ `CursorUp` (A)
- ✅ `CursorDown` (B)
- ✅ `CursorForward` (C)
- ✅ `CursorBack` (D)
- ✅ `EraseInDisplay` (J)
- ✅ `EraseInLine` (K)
- ✅ `SelectGraphicRendition` (m)

Not yet implemented:
- ❌ `InsertLines` (L)
- ❌ `DeleteLines` (M)
- ❌ `SetScrollingRegion` (r)
- ❌ `DeviceStatusReport` (n)

#### 2. DEC Private Mode Sequences

**Unknown Modes:**
- Format: `ESC[?{unknown_number}h` or `ESC[?{unknown_number}l`
- Example: `[ANSI] Unknown DEC private mode (set): 9999`
- These are mode numbers not defined in the enum

**Not Yet Implemented:**
- Example: `[ANSI] Not yet implemented DEC private mode (set): ShowCursor`
- These are defined in the enum but not handled

Currently implemented:
- ✅ `AlternateScreenBuffer` (mode 1049, 47)

Not yet implemented:
- ❌ `ApplicationCursorKeys` (mode 1)
- ❌ `ShowCursor` (mode 25)
- ❌ `MouseTracking` (modes 1000-1006)
- ❌ `BracketedPaste` (mode 2004)
- And many more...

#### 3. SGR (Select Graphic Rendition) Parameters

**Unknown Parameters:**
- Format: `ESC[{unknown_number}m`
- Example: `[ANSI] Unknown SGR parameter: 99`
- These are SGR codes not defined in the enum

**Not Yet Implemented Attributes:**
- Example: `[ANSI] Not yet implemented SGR attribute: Bold`
- These text attributes are defined but not yet rendered

Currently implemented:
- ✅ Reset (0)
- ✅ Foreground colors (30-37)
- ✅ Background colors (40-47)
- ✅ Bright colors (90-97, 100-107)
- ✅ Extended colors (38, 48) - 256-color and RGB

Not yet implemented:
- ❌ Bold (1)
- ❌ Italic (3)
- ❌ Underline (4)
- ❌ Reverse video (7)
- And more...

## Using the Logs

### During Development

Run Rustty and redirect stderr to see what sequences are being used:

```bash
# Run and see all ANSI debug messages
cargo run 2>&1 | grep "\[ANSI\]"

# Run and save to a file
cargo run 2> ansi_debug.log

# Filter specific types
cargo run 2>&1 | grep "\[ANSI\].*DEC private mode"
```

### Testing Specific Applications

To see what ANSI sequences an application uses:

```bash
# Run vim and see what sequences it sends
cargo run 2>&1 | grep "\[ANSI\]" &
# Then run vim inside the terminal
```

### Analyzing Missing Features

The logs help prioritize what to implement next:

```bash
# Count how many times each unimplemented feature is requested
cargo run 2>&1 | grep "\[ANSI\]" | sort | uniq -c | sort -rn
```

## Example Output

When running vim inside Rustty, you might see:

```
[ANSI] Not yet implemented DEC private mode (set): ShowCursor
[ANSI] Not yet implemented DEC private mode (reset): ShowCursor
[ANSI] Not yet implemented SGR attribute: Bold
[ANSI] Not yet implemented CSI command: SetScrollingRegion
[ANSI] Not yet implemented DEC private mode (set): MouseTracking
```

This tells us that vim uses:
1. Cursor visibility control (ShowCursor mode 25)
2. Bold text attributes
3. Scrolling regions for efficient scrolling
4. Mouse tracking for interactive features

## Disabling Logs

If the logs become too verbose during normal use, you can filter them out:

```bash
# Run without ANSI debug messages
cargo run 2>&1 | grep -v "\[ANSI\]"

# Or redirect stderr to /dev/null
cargo run 2>/dev/null
```

## Contributing

When implementing a new ANSI sequence:

1. Check the logs to see if it's frequently requested
2. Implement the sequence in the appropriate handler
3. Update the match statement to handle the enum variant
4. The logging will automatically stop once the sequence is implemented

## References

- See `ANSI_IMPLEMENTATION.md` for the complete list of supported sequences
- See `src/terminal/command.rs` for the ANSI command enum definitions
- See `src/terminal/mod.rs` for the VTE Perform implementation (sequence handlers)