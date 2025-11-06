use std::{
    env,
    io::{self, IsTerminal},
    sync::LazyLock,
};

static SHOULD_USE_COLORS: LazyLock<bool> = LazyLock::new(|| {
    io::stdout().is_terminal() && io::stderr().is_terminal() && env::var_os("NO_COLOR").is_none()
});

pub fn should_use_colors() -> bool {
    *SHOULD_USE_COLORS
}

/// Taken from grep_cli::is_readable_stdin by BurntSushi (MIT)
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
            Err(_err) => {
                return false;
            }
        };
        let file = File::from(fd);
        let md = match file.metadata() {
            Ok(md) => md,
            Err(_err) => {
                return false;
            }
        };
        let ft = md.file_type();
        let is_file = ft.is_file();
        let is_fifo = ft.is_fifo();
        let is_socket = ft.is_socket();

        is_file || is_fifo || is_socket
    }

    #[cfg(windows)]
    fn imp() -> bool {
        let stdin = winapi_util::HandleRef::stdin();
        let typ = match winapi_util::file::typ(stdin) {
            Ok(typ) => typ,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Windows, \
                     could not get file type of stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let is_disk = typ.is_disk();
        let is_pipe = typ.is_pipe();
        let is_readable = is_disk || is_pipe;

        is_readable
    }

    #[cfg(not(any(unix, windows)))]
    fn imp() -> bool {
        false
    }

    !std::io::stdin().is_terminal() && imp()
}
