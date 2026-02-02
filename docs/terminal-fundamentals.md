# Terminal Emulator Fundamentals

**A simple guide to understanding how terminals really work**

## Table of Contents

1. [Let's Start with What You Know](#lets-start-with-what-you-know)
2. [Everything is Just a File](#everything-is-just-a-file)
3. [So How Does Your Hello World Get Displayed?](#so-how-does-your-hello-world-get-displayed)
4. [But Wait... Is Your Program Really Connected to /dev/tty1?](#but-wait-is-your-program-really-connected-to-devtty1)
5. [What About Terminal Emulators?](#what-about-terminal-emulators)
6. [The PTY Trick](#the-pty-trick)
7. [So How Does printf("Hello") Get Displayed?](#so-how-does-printf-hello-get-displayed)
8. [How Rustty Starts the Shell](#how-rustty-starts-the-shell)
9. [How Rustty Actually Works](#how-rustty-actually-works)
10. [Following the Data](#following-the-data)
11. [Quick Reference](#quick-reference)
12. [Want to Learn More?](#want-to-learn-more)

---

## Let's Start with What You Know

Console application is the first experience we have in programming.

When you call `println!()` in Rust, or `print()` in Python, or `printf()` in C, do you know what's actually happening?

**The common misconception:** Your program is somehow directly accessing the display device and putting text on the screen.

**The reality:** It's much simpler! Your program is just writing to a FILE called **stdout**.

What about reading input? Like `std::io::stdin().read_line()` in Rust?

You guessed it—nothing in that function is actually accessing your keyboard device. It's just reading from a FILE called **stdin**.

---

## Everything is Just a File

In Linux (and Unix), FILE is an abstraction. It's something that can be read from or written to.

What happens when you write to a particular file? Well, it depends on which file:

- If it's a **real file** in the filesystem → the data gets saved to disk
- If it's a **TTY file** (like `/dev/pts/0`) → the data gets displayed on screen
- If it's a **network socket** → the data gets sent over the network
- If it's `/dev/null` → the data disappears into the void

Same concept for reading:

- Read from a **real file** → you get data from disk
- Read from a **TTY file** → you get what the user types
- Read from a **network socket** → you get data from the network

Every process gets three files automatically:

```
File Descriptor 0 = stdin   (standard input)
File Descriptor 1 = stdout  (standard output)
File Descriptor 2 = stderr  (standard error output)
```

Your `print()` statements? They just write to file descriptor 1.

Your `read()` calls? They just read from file descriptor 0.

That's it!

---

## So How Does Your Hello World Get Displayed?

Okay, so your program writes to stdout (file descriptor 1). But how does that text actually appear on your screen?

Simple answer: **If stdout is connected to a TTY device file, the kernel displays it.**

For example, if your program's stdout is connected to `/dev/tty1`, then when you write to stdout, the kernel takes that text and displays it on the physical console (or virtual console).

```
Your Program
    ↓
    write("Hello World")
    ↓
stdout → /dev/tty1
    ↓
Kernel displays on screen
```

That's how your first "Hello World" program worked!

---

## But Wait... Is Your Program Really Connected to /dev/tty1?

Here's the thing: **your console application's stdout is NOT directly connected to `/dev/tty1`**.

When you run a program, you don't actually run it directly. You run it **from a shell**.

What's a **shell**? It's just another program! Programs like:
- `bash` (Bourne Again Shell)
- `zsh` (Z Shell)
- `fish` (Friendly Interactive Shell)

The shell is a program that:
1. Displays a prompt (like `$ `)
2. Reads commands you type
3. Runs those commands for you
4. Displays the output

So the actual flow is:

```
You type: ./hello
    ↓
Shell (bash) reads your command
    ↓
Shell runs your program (fork + exec)
    ↓
Your program's stdout is connected to... what?
```

Here's what the **shell does** when it runs your program:

1. **fork()** - Creates a copy of the shell process
2. The copied process gets a **copy of all file descriptors**
   - If bash's stdin/stdout/stderr are connected to `/dev/tty1`
   - The forked child also has stdin/stdout/stderr connected to `/dev/tty1`
3. **exec()** - Replaces the child process with your program
   - But the file descriptors stay the same!

So the shell **orchestrates the connection**. It sets up the file descriptors.

```
/dev/tty1
    ↓
Connected to bash's stdin/stdout/stderr (fd 0, 1, 2)
    ↓
bash forks → child process gets copies of fd 0, 1, 2
    ↓
child execs ./hello → process becomes hello program
    ↓
hello still has fd 0, 1, 2 connected to /dev/tty1
    ↓
hello writes "Hello World" to stdout (fd 1)
    ↓
Appears on /dev/tty1
```

So you're not directly connected to `/dev/tty1`. The **shell orchestrates it** by forking (which copies the file descriptors) and then exec-ing your program (which keeps those file descriptors).

---

## What About Terminal Emulators?

Now we can finally talk about terminal emulators like Rustty!

In the old days, shells were connected to physical terminal devices (like `/dev/tty1` on the console, or `/dev/ttyS0` on a serial port).

But today, we mostly use **terminal emulator programs**:
- Rustty
- GNOME Terminal
- Alacritty
- iTerm2

These are graphical applications that **pretend to be a terminal device**.

Instead of the shell being connected to `/dev/tty1`, it's connected to a **pseudo-terminal** (PTY), which the terminal emulator controls.

```
You press a key in Rustty window
    ↓
Rustty writes the character to PTY
    ↓
Shell reads from stdin (connected to PTY)
    ↓
Shell processes command
    ↓
Shell writes output to stdout (connected to PTY)
    ↓
Rustty reads from PTY
    ↓
Rustty draws text as pixels in window
```

But how does this work? Can we just use a pipe to connect them?

```
Rustty → Pipe → Shell (❌ Doesn't work well)
```

This doesn't work because:
- The shell has no way to know the terminal size (how many columns/rows)
- Programs can't control the cursor or colors
- Programs can't detect "Am I running in a terminal?"

We need something smarter...

---

## The PTY Trick

This is where **PTY (Pseudo-Terminal)** comes in.

A PTY is a pair of connected files:

```
┌─────────────────────────┐       ┌─────────────────────────┐
│  PTY Master             │       │  PTY Slave              │
│  /dev/ptmx              │ ←───→ │  /dev/pts/0             │
│  (Terminal side)        │       │  (Program side)         │
└─────────────────────────┘       └─────────────────────────┘
```

### What Happens When You Write to PTY Master?

When the terminal emulator (like Rustty) writes data to the **PTY master**:

```
Rustty writes "l" to PTY master
    ↓
Kernel's PTY driver receives it
    ↓
Data becomes available to read from PTY slave
    ↓
Shell reads from PTY slave (its stdin)
    ↓
Shell sees: "l"
```

**In other words:** Write to master → data appears on slave's read side.

### What Happens When You Write to PTY Slave?

When a program (like bash) writes data to the **PTY slave**:

```
Bash writes "Hello" to PTY slave (its stdout)
    ↓
Kernel's PTY driver receives it
    ↓
Data becomes available to read from PTY master
    ↓
Rustty reads from PTY master
    ↓
Rustty sees: "Hello"
```

**In other words:** Write to slave → data appears on master's read side.

### What Happens When You Read from PTY Master?

When Rustty reads from the **PTY master**:

```
Rustty calls: read(pty_master_fd, buffer)
    ↓
If data is available (shell wrote something):
    → read() returns the data
    ↓
If no data is available:
    → read() blocks (waits) until data arrives
    → OR returns error if non-blocking mode
```

### What Happens When You Read from PTY Slave?

When bash reads from the **PTY slave** (stdin):

```
Bash calls: read(stdin, buffer)  // stdin is PTY slave
    ↓
If data is available (terminal wrote something):
    → read() returns the data
    ↓
If no data is available:
    → read() blocks (waits) until data arrives
```

### The Key Insight

The PTY is like a **bidirectional pipe**, but with special terminal features:

- Write to master → read from slave
- Write to slave → read from master
- Plus: the slave pretends to be a real terminal device!

This means:
- The program can ask "how big is the terminal?" → PTY knows (via ioctl)
- The program can send color codes → PTY passes them through
- The program can detect "Am I running in a terminal?" → PTY says yes!

Everything just works.

---

## So How Does printf("Hello") Get Displayed?

Let's answer the main question: When you run a program that calls `printf("Hello")`, how does that text appear in your terminal emulator window?

Here's the complete flow:

```
1. You run: ./hello
   ↓
2. Shell forks and execs your program
   ↓
3. Your program's stdout is connected to PTY slave
   ↓
4. printf("Hello") writes to stdout
   ↓
5. Data goes to PTY slave
   ↓
6. Kernel's PTY driver passes it through
   ↓
7. Data becomes available on PTY master
   ↓
8. Terminal emulator reads from PTY master
   ↓
9. Terminal emulator sees: "Hello"
   ↓
10. Terminal emulator renders "Hello" as pixels in window
   ↓
11. You see: Hello
```

**The key insight:** A terminal emulator is just a program with a window that can render text!

When the terminal emulator reads "Hello" from the PTY master, it:
1. Knows which font to use (monospace, usually)
2. Knows where the cursor is on the screen (row 5, column 10, for example)
3. Draws each character as pixels at that position
4. Updates the window

That's it! It's just:
- Read bytes from PTY master
- Interpret those bytes (plain text or ANSI escape codes)
- Draw pixels in the window

Terminal emulators like Rustty, GNOME Terminal, or Alacritty are essentially:
- A window (using libraries like winit, GTK, or SDL)
- A text renderer (using libraries like Raqote, fontconfig, or FreeType)
- A PTY manager (creating and reading from PTY master)
- An ANSI parser (interpreting escape sequences for colors, cursor movement, etc.)

Now let's see how Rustty specifically implements this...

---

## How Rustty Starts the Shell

When you open Rustty, the first thing it does is start a **shell** (like bash, zsh, or fish).

But how does it connect the shell to the PTY?

### Step 1: Create the PTY

```rust
// Create a PTY pair
let pty = openpty()?;
// Now we have:
//   pty.master → terminal emulator will use this
//   pty.slave  → shell will use this
```

### Step 2: Fork the Process

```rust
match fork()? {
    Parent { child_pid } => {
        // This is the terminal emulator
        // We keep the PTY master
        // We close the PTY slave (child uses it)
    }
    Child => {
        // This is a copy of the process
        // We'll turn this into the shell
    }
}
```

After `fork()`, you have two identical processes running. But now they can do different things!

### Step 3: Child Becomes the Shell

Now we need to connect the child process's stdin/stdout/stderr to the PTY slave.

Remember, the child process currently has:
- stdin (fd 0) → probably connected to where the terminal was
- stdout (fd 1) → probably connected to where the terminal was
- stderr (fd 2) → probably connected to where the terminal was
- Plus: PTY slave (some other fd number, like fd 3)

We want to **redirect** stdin/stdout/stderr to point to the PTY slave instead.

**What is `dup2()`?**

`dup2(oldfd, newfd)` is a system call that means: "Make `newfd` point to the same file as `oldfd`"

So `dup2(slave, 0)` means: "Make fd 0 (stdin) point to the same file as `slave` (PTY slave)"

After `dup2(slave, 0)`:
- stdin (fd 0) now points to PTY slave
- The old connection is closed

Here's how we redirect all three:

```rust
// In the child process:

// 1. Make stdin/stdout/stderr point to PTY slave
dup2(slave, 0);  // Make fd 0 (stdin)  point to PTY slave
dup2(slave, 1);  // Make fd 1 (stdout) point to PTY slave
dup2(slave, 2);  // Make fd 2 (stderr) point to PTY slave

// Now stdin/stdout/stderr all point to PTY slave!

// 2. Replace this process with the shell program
exec("/bin/bash");  // This process is now bash!
```

After these `dup2()` calls, any time bash reads from stdin or writes to stdout/stderr, it's actually reading/writing the PTY slave!

Now you have:

```
Rustty Process
  ├─ Has PTY master open
  └─ fork() created →

Bash Process
  ├─ stdin/stdout/stderr all connected to PTY slave
  └─ Running the bash program
```

When bash writes to stdout, Rustty reads it from PTY master!
When Rustty writes to PTY master, bash reads it from stdin!

Perfect!

---

## How Rustty Actually Works

Now let's see how Rustty is structured.

### The Problem

Rustty needs to do two things at the same time:
1. Handle keyboard input and draw the window (needs to be fast and responsive)
2. Wait for output from the shell (might take a while)

If we do both in one thread, the UI will freeze while waiting for shell output.

### The Solution: Two Threads

```
┌──────────────────────────────────────┐
│         Main Thread                  │
│  - Handle window events (keyboard)   │
│  - Draw the screen                   │
│  - Check: any new data from shell?   │
└──────────────────────────────────────┘
              ↑
              │ Channel (sends Vec<u8>)
              │
┌──────────────────────────────────────┐
│      PTY Reader Thread               │
│  - read() from PTY master            │
│  - (sleeps here, uses 0% CPU)        │
│  - Got data? Send to main thread!    │
└──────────────────────────────────────┘
```

The PTY reader thread does a **blocking read**:

```rust
loop {
    let mut buf = vec![0u8; 4096];

    // This line blocks (waits) until data arrives
    // While waiting, the thread sleeps and uses 0% CPU!
    let n = read(pty_master_fd, &mut buf)?;

    // Got data! Send to main thread via channel
    buf.truncate(n);
    tx.send(buf)?;
}
```

The main thread checks the channel without blocking:

```rust
// In the event loop
loop {
    // try_recv() returns immediately (doesn't wait)
    match channel.try_recv() {
        Ok(data) => {
            // Got data from shell! Process it
            parse_and_update_screen(data);
        }
        Err(_) => {
            // No data yet, that's fine, keep going
        }
    }
}
```

This way:
- PTY reader thread sleeps when shell is quiet (0% CPU)
- Main thread stays responsive (handles keyboard/drawing)
- When shell outputs something, reader wakes up instantly

Efficient!

---

## Following the Data

### When You Type a Character

Let's say you type the letter "l":

```
1. Your keyboard generates a key event
2. Window manager (X11/Wayland) sends it to Rustty window
3. Rustty receives: KeyboardInput { text: "l" }
4. Rustty writes b"l" to PTY master
5. Kernel's PTY driver passes it through
6. Bash reads from stdin (PTY slave)
7. Bash sees: "l"
```

But wait, you see "l" appear on screen. How?

```
8. Bash echoes it back by writing "l" to stdout
9. PTY slave → PTY master
10. PTY reader thread: read() returns b"l"
11. Sends b"l" through channel
12. Main thread: try_recv() gets b"l"
13. Parser: "print character 'l' at cursor position"
14. Grid: put 'l' in current cell
15. Render: draw the 'l' on screen
16. You see: l█
```

That's why there's a tiny delay between pressing a key and seeing it (called "latency"). The character has to make a round trip!

### When Shell Outputs Something

Let's say bash runs `echo "Hello"`:

```
1. Bash writes "Hello\n" to stdout (PTY slave)
2. Kernel's PTY driver has data available
3. PTY reader thread wakes up from read()
4. read() returns b"Hello\n"
5. Sends through channel to main thread
6. Main thread: try_recv() gets b"Hello\n"
7. Parser processes each byte:
   - 'H' → put in grid at cursor position
   - 'e' → put in grid, cursor moves right
   - 'l' → put in grid, cursor moves right
   - 'l' → put in grid, cursor moves right
   - 'o' → put in grid, cursor moves right
   - '\n' → cursor moves to next line
8. Render: draw all the new characters
9. You see: Hello
```

### When Shell Uses Colors

Let's say bash outputs `\x1b[31mRed Text\x1b[0m`:

```
1. PTY reader thread reads: b"\x1b[31mRed Text\x1b[0m"
2. Sends to main thread
3. Parser sees \x1b (ESC) → start escape sequence
4. Parser sees [31m → "set foreground color to red"
5. Parser updates: current_color = RED
6. Parser sees "Red Text" → put in grid with RED color
7. Parser sees \x1b[0m → "reset colors to default"
8. Parser updates: current_color = DEFAULT
9. Render: draw "Red Text" in red color
10. You see: Red Text (in red!)
```

That's how ANSI escape sequences work. They're just special byte sequences that the parser interprets as commands instead of text.

---

## Quick Reference

### Key Concepts

**File Descriptor (FD):**
Just a number that represents an open file. 0 = stdin, 1 = stdout, 2 = stderr.

**PTY (Pseudo-Terminal):**
A pair of connected files (master and slave) that makes programs think they're connected to a real terminal.

**Shell:**
A program (bash, zsh, fish) that reads commands and runs other programs. Just a normal program that happens to read from stdin and write to stdout.

**Terminal Emulator:**
A program (Rustty!) that creates a window, handles keyboard input, displays text, and manages a PTY to connect with a shell.

**stdin/stdout/stderr:**
Three files every process gets automatically. Programs read from stdin (0), write normal output to stdout (1), and write errors to stderr (2).

**Blocking vs Non-Blocking:**
- Blocking: `read()` waits until data arrives (thread sleeps, 0% CPU)
- Non-blocking: `read()` returns immediately with error if no data

**Channel:**
A way for threads to send data to each other. Rustty uses `std::sync::mpsc` to send PTY output from reader thread to main thread.

### The Mental Model

Think of it like this:

```
YOU
 ↓ (press keys)
RUSTTY
 ↓ (write bytes to file)
PTY MASTER FILE
 ↓ (kernel passes through)
PTY SLAVE FILE
 ↓ (read bytes from file)
BASH
 ↓ (write bytes to file)
PTY SLAVE FILE
 ↓ (kernel passes through)
PTY MASTER FILE
 ↓ (read bytes from file)
RUSTTY
 ↓ (draw pixels)
YOU
 ↓ (see text on screen)
```

Everything is just reading and writing files!

### Important Files in Rustty

Want to see the code?

- `src/terminal/shell.rs` - Creating PTY, forking, starting shell, reader thread setup
- `src/app.rs` - Main event loop: checking channel for data, handling keyboard events, rendering
- `src/terminal/mod.rs` - Terminal struct with VTE Perform implementation (converting bytes to screen actions)
- `src/terminal/grid.rs` - Storing the text and colors
- `src/terminal/command.rs` - ANSI command enums (CsiCommand, SgrParameter, etc.)

---

## Want to Learn More?

### Read These Next

If you found this guide helpful, check out these other docs:

- **[ANSI Implementation](ansi-implementation.md)** - What are those `\x1b[31m` sequences?
- **[Debugging Guide](debugging.md)** - How to see what sequences are being sent
- **[CLAUDE.md](../CLAUDE.md)** - Detailed implementation notes

### Explore the Code

The best way to learn is to read the code:

1. Start with `src/main.rs` - only ~20 lines!
2. Look at `src/terminal/shell.rs` - see PTY creation, forking, and I/O setup
3. Look at `src/terminal/mod.rs` - see how VTE sequences are handled
4. Look at `src/app.rs` - see the event loop and rendering
5. Run Rustty with debug logs: `cargo run 2> debug.log`

### External Resources

Want to go deeper?

- **[The TTY demystified](http://www.linusakesson.net/programming/tty/)** - The best deep dive into terminals and PTY
- **[Linux Programming Interface](https://man7.org/tlpi/)** - The bible of Linux system programming
- **[VTE Parser](https://github.com/alacritty/vte)** - The ANSI parser library Rustty uses

### Questions?

If something is confusing or you found a mistake, please open an issue on GitHub!

---

**Remember:** It's all just files. Programs read from files and write to files. The PTY is just a special file that connects the terminal to the shell. That's the whole secret!
