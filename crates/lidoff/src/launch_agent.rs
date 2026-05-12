use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

const LAUNCH_AGENT_LABEL: &str = "co.myrt.lidoff";
const LAUNCH_AGENT_RELATIVE_PATH: &str = "Library/LaunchAgents/co.myrt.lidoff.plist";

pub(crate) fn install(threshold: u32) -> bool {
    match install_inner(threshold) {
        Ok(()) => true,
        Err(err) => {
            error(err);
            false
        }
    }
}

pub(crate) fn uninstall() -> bool {
    match uninstall_inner() {
        Ok(()) => true,
        Err(err) => {
            error(err);
            false
        }
    }
}

fn install_inner(threshold: u32) -> Result<(), String> {
    let plist_path = launch_agent_plist_path().map_err(io_error)?;
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }

    let content = generate_plist_content(threshold).map_err(io_error)?;
    fs::write(&plist_path, content).map_err(io_error)?;

    let output =
        run_launchctl(["load", plist_path.to_string_lossy().as_ref()]).map_err(io_error)?;
    if !output.status.success() {
        return Err(format_launchctl_error("load", &output.stderr));
    }

    info(format!("installed ({})", plist_path.display()));
    info(format!("threshold: {threshold}°"));
    Ok(())
}

fn uninstall_inner() -> Result<(), String> {
    let plist_path = launch_agent_plist_path().map_err(io_error)?;
    if !plist_path.exists() {
        info("not installed");
        return Ok(());
    }

    let output =
        run_launchctl(["unload", plist_path.to_string_lossy().as_ref()]).map_err(io_error)?;
    if !output.status.success() {
        return Err(format_launchctl_error("unload", &output.stderr));
    }

    fs::remove_file(&plist_path).map_err(io_error)?;
    info("uninstalled");
    Ok(())
}

fn launch_agent_plist_path() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(home.join(LAUNCH_AGENT_RELATIVE_PATH))
}

fn resolve_executable_path() -> io::Result<PathBuf> {
    let exec_path = std::env::current_exe()?;
    let real_path = match fs::read_link(&exec_path) {
        Ok(path) => path,
        Err(_) => exec_path,
    };

    if real_path.is_absolute() {
        Ok(real_path)
    } else {
        Ok(std::env::current_dir()?.join(real_path))
    }
}

fn generate_plist_content(threshold: u32) -> io::Result<String> {
    let real_path = resolve_executable_path()?;
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>Label</key>\n\
    <string>{LAUNCH_AGENT_LABEL}</string>\n\
    <key>ProgramArguments</key>\n\
    <array>\n\
        <string>{}</string>\n\
        <string>-t</string>\n\
        <string>{threshold}</string>\n\
    </array>\n\
    <key>RunAtLoad</key>\n\
    <true/>\n\
    <key>KeepAlive</key>\n\
    <true/>\n\
    <key>StandardOutPath</key>\n\
    <string>/tmp/lidoff.log</string>\n\
    <key>StandardErrorPath</key>\n\
    <string>/tmp/lidoff.err</string>\n\
</dict>\n\
</plist>\n",
        real_path.display()
    ))
}

fn run_launchctl<'a>(
    arguments: impl IntoIterator<Item = &'a str>,
) -> io::Result<std::process::Output> {
    Command::new("/bin/launchctl").args(arguments).output()
}

fn format_launchctl_error(action: &str, stderr: &[u8]) -> String {
    let output = String::from_utf8_lossy(stderr);
    if output.trim().is_empty() {
        format!("launchctl {action} failed")
    } else {
        format!("launchctl {action} failed: {}", output.trim())
    }
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[allow(clippy::print_stdout)]
fn info(message: impl AsRef<str>) {
    println!("lidoff[info]: {}", message.as_ref());
}

#[allow(clippy::print_stderr)]
fn error(message: impl AsRef<str>) {
    eprintln!("lidoff[error]: {}", message.as_ref());
}
