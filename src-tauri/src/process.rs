use std::ffi::OsStr;
use std::process::Command;

pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    sanitize_env(&mut command);
    command
}

fn sanitize_env(command: &mut Command) {
    for key in ["LD_LIBRARY_PATH", "LD_PRELOAD"] {
        command.env_remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_command_for_the_program() {
        let command = command("echo");
        assert_eq!(command.get_program(), OsStr::new("echo"));
    }

    #[test]
    fn strips_appimage_library_path() {
        let command = command("echo");
        let envs: Vec<_> = command.get_envs().collect();
        for key in ["LD_LIBRARY_PATH", "LD_PRELOAD"] {
            assert!(
                envs.iter().any(|(name, value)| *name == OsStr::new(key) && value.is_none()),
                "{key} should be removed for host subprocesses"
            );
        }
    }
}
