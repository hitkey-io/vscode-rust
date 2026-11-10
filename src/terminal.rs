//! The integrated terminal's process side (terminalProcess.ts /
//! terminalEnvironment.ts): the user's shell on a pseudo-terminal, as a
//! login shell on macOS, with `TERM=xterm-256color`, `COLORTERM=truecolor`
//! and `TERM_PROGRAM=vscode`, started in the open folder. Output goes
//! through a VT parser (`vt100`) that keeps the screen and scrollback; the
//! panel paints that screen (`workbench/panel.rs`).

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

/// Lines kept above the screen (`terminal.integrated.scrollback`).
pub const SCROLLBACK: usize = 1000;

pub struct Terminal {
    /// The shell's name, as the terminal tab shows it ("zsh").
    pub title: String,
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    exited: Arc<AtomicBool>,
    pub rows: u16,
    pub cols: u16,
    /// Lines scrolled back from the bottom (0 = following the output).
    pub scroll_offset: usize,
}

/// The shell VS Code starts by default (`terminal.integrated.defaultProfile`
/// unset): `$SHELL`, else zsh.
pub fn default_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into())
}

impl Terminal {
    /// Starts the shell in `cwd` with a `rows` × `cols` screen. `wake` is
    /// called whenever output arrives (to repaint).
    pub fn spawn(cwd: Option<&Path>, rows: u16, cols: u16, wake: impl Fn() + Send + 'static) -> std::io::Result<Terminal> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let shell = default_shell();
        let mut cmd = CommandBuilder::new(&shell);
        if cfg!(target_os = "macos") {
            cmd.arg("-l");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "vscode");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        if let Some(dir) = cwd.map(Path::to_path_buf).or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from)) {
            cmd.cwd(dir);
        }
        let child = pair.slave.spawn_command(cmd).map_err(|e| std::io::Error::other(e.to_string()))?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| std::io::Error::other(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| std::io::Error::other(e.to_string()))?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let exited = Arc::new(AtomicBool::new(false));
        {
            let parser = parser.clone();
            let exited = exited.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut p) = parser.lock() {
                                p.process(&buf[..n]);
                            }
                            wake();
                        }
                    }
                }
                exited.store(true, Ordering::SeqCst);
                wake();
            });
        }
        let title = Path::new(&shell).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(shell);
        Ok(Terminal { title, parser, writer, master: pair.master, child, exited, rows, cols, scroll_offset: 0 })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.scroll_offset = 0;
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 || (rows == self.rows && cols == self.cols) {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
    }

    /// Runs `f` on the screen as it would be shown (scrolled back by
    /// `scroll_offset`).
    pub fn with_screen<R>(&mut self, f: impl FnOnce(&vt100::Screen) -> R) -> R {
        let mut p = self.parser.lock().unwrap_or_else(|e| e.into_inner());
        p.set_scrollback(self.scroll_offset);
        // The parser clamps the offset to what it has.
        self.scroll_offset = p.screen().scrollback();
        let r = f(p.screen());
        p.set_scrollback(0);
        r
    }

    /// Bracketed paste when the shell asked for it.
    pub fn paste(&mut self, text: &str) {
        let bracketed = self.parser.lock().map(|p| p.screen().bracketed_paste()).unwrap_or(false);
        if bracketed {
            self.write(b"\x1b[200~");
            self.write(text.as_bytes());
            self.write(b"\x1b[201~");
        } else {
            self.write(text.replace("\r\n", "\r").replace('\n', "\r").as_bytes());
        }
    }

    pub fn application_cursor(&self) -> bool {
        self.parser.lock().map(|p| p.screen().application_cursor()).unwrap_or(false)
    }

    pub fn has_exited(&mut self) -> bool {
        self.exited.load(Ordering::SeqCst) || matches!(self.child.try_wait(), Ok(Some(_)))
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.kill();
    }
}

/// The bytes a key sends (xterm's keyboard protocol, as xterm.js).
pub fn key_bytes(key: egui::Key, mods: egui::Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    use egui::Key;
    let arrow = |c: char| {
        let m = 1 + mods.shift as u8 + 2 * mods.alt as u8 + 4 * mods.ctrl as u8;
        if m > 1 {
            format!("\x1b[1;{m}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    Some(match key {
        Key::Enter => b"\r".to_vec(),
        Key::Backspace if mods.alt => b"\x1b\x7f".to_vec(),
        Key::Backspace => b"\x7f".to_vec(),
        Key::Tab if mods.shift => b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Escape => b"\x1b".to_vec(),
        Key::ArrowUp => arrow('A'),
        Key::ArrowDown => arrow('B'),
        Key::ArrowRight if mods.alt && !mods.ctrl => b"\x1bf".to_vec(),
        Key::ArrowLeft if mods.alt && !mods.ctrl => b"\x1bb".to_vec(),
        Key::ArrowRight => arrow('C'),
        Key::ArrowLeft => arrow('D'),
        Key::Home => arrow('H'),
        Key::End => arrow('F'),
        Key::Delete => b"\x1b[3~".to_vec(),
        Key::PageUp => b"\x1b[5~".to_vec(),
        Key::PageDown => b"\x1b[6~".to_vec(),
        k if mods.ctrl && !mods.alt => {
            let name = k.name();
            let c = name.chars().next()?;
            if name.len() == 1 && c.is_ascii_alphabetic() {
                vec![(c.to_ascii_uppercase() as u8) & 0x1f]
            } else {
                match k {
                    Key::OpenBracket => vec![0x1b],
                    Key::Backslash => vec![0x1c],
                    Key::CloseBracket => vec![0x1d],
                    Key::Space => vec![0],
                    _ => return None,
                }
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_xterm_sequences() {
        let none = egui::Modifiers::NONE;
        assert_eq!(key_bytes(egui::Key::Enter, none, false), Some(b"\r".to_vec()));
        assert_eq!(key_bytes(egui::Key::ArrowUp, none, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes(egui::Key::ArrowUp, none, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key_bytes(egui::Key::C, egui::Modifiers::CTRL, false), Some(vec![3]));
        assert_eq!(key_bytes(egui::Key::A, none, false), None);
    }

    #[test]
    fn shell_runs_and_echoes() {
        let mut t = Terminal::spawn(None, 24, 80, || {}).expect("pty");
        t.write(b"echo vscode-rust-$((40+2))\r");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let text = t.with_screen(|s| s.contents());
            if text.contains("vscode-rust-42") {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "no echo: {text}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        t.write(b"exit\r");
    }
}
