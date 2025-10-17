use std::io::IsTerminal;

/// Taken from grep-cli::is_readable_stdin by BurntSushi
///
/// https://docs.rs/grep-cli/latest/grep_cli/fn.is_readable_stdin.html
pub fn is_stdin_readable() -> bool {
    #[cfg(unix)]
    fn imp() -> bool {
        use std::{
            fs::File,
            os::{fd::AsFd, unix::fs::FileTypeExt},
        };

        let stdin = std::io::stdin();
        let fd = match stdin.as_fd().try_clone_to_owned() {
            Ok(fd) => fd,
            Err(_) => {
                return false;
            }
        };
        let file = File::from(fd);
        let md = match file.metadata() {
            Ok(md) => md,
            Err(_) => {
                return false;
            }
        };
        let ft = md.file_type();
        let is_file = ft.is_file();
        let is_fifo = ft.is_fifo();
        let is_socket = ft.is_socket();

        is_file || is_fifo || is_socket
    }

    // TODO: windows
    #[cfg(not(any(unix, windows)))]
    fn imp() -> bool {
        false
    }

    !std::io::stdin().is_terminal() && imp()
}
