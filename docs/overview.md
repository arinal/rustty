# Rustty Documentation

This directory contains detailed documentation for the Rustty terminal emulator project.

## Documentation Files

### [terminal-fundamentals.md](terminal-fundamentals.md)
Comprehensive guide to understanding how terminal emulators work.

**Contents:**
- What is a terminal emulator and why it exists
- PTY (Pseudo-Terminal) architecture explained
- Shell processes and process hierarchy
- File descriptors in Linux/Unix
- Rustty's two-thread architecture
- Complete data flow from keyboard to screen
- Visual diagrams and code references
- Glossary of terminal concepts

**Use this when:**
- You're new to terminal emulator development
- You want to understand PTY, shells, and file descriptors
- You need to learn how Rustty's architecture works
- You're curious about "what is a FILE in Linux"
- You want visual diagrams of data flow and threading

---

### [ansi-implementation.md](ansi-implementation.md)
Complete reference guide for ANSI escape sequence support in Rustty.

**Contents:**
- Enum types for ANSI sequences (`CsiCommand`, `DecPrivateMode`, `SgrParameter`, etc.)
- Implementation status (implemented, partially implemented, not implemented)
- Usage examples and code snippets
- Future work and roadmap for ANSI support

**Use this when:**
- You want to know what ANSI sequences are supported
- You're implementing new ANSI features
- You need to understand the typed enum system

---

### [debugging.md](debugging.md)
Guide for debugging and tracing ANSI escape sequences in Rustty.

**Contents:**
- ANSI sequence logging features
- Log message format and categories
- How to use logs during development
- Examples of analyzing missing features
- Tips for testing specific applications

**Use this when:**
- You want to see what ANSI sequences are being sent
- You're debugging why an application doesn't display correctly
- You need to prioritize which features to implement next
- You want to understand what sequences vim/tmux/etc use

---

## Quick Start

For general usage and getting started, see the main [README.md](../README.md) in the project root.

## Contributing to Documentation

When adding new features to Rustty:

1. Update `ansi-implementation.md` if you add ANSI sequence support
2. Update `debugging.md` if you add debugging features
3. Update `terminal-fundamentals.md` if you change core architecture concepts
4. Update `../CLAUDE.md` (root) if you change the implementation or architecture
5. Keep examples and code snippets up to date

## Additional Resources

- [ANSI Escape Sequences (Wikipedia)](https://en.wikipedia.org/wiki/ANSI_escape_code)
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [VT100 User Guide](https://vt100.net/docs/vt100-ug/)
- [Alacritty VTE Parser](https://github.com/alacritty/vte)
