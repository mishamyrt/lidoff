use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NEW_BINARY: &str = r#"#!/bin/bash
case "$1" in
    --version) echo 'lidoff 0.5.0' ;;
    install) echo 'new install' >> "$LIDOFF_TEST_EVENTS" ;;
    *) exit 1 ;;
esac
"#;

const OLD_BINARY: &str = r#"#!/bin/bash
[ "$1" = uninstall ] || exit 1
echo 'old uninstall' >> "$LIDOFF_TEST_EVENTS"
"#;

struct InstallerTest {
    root: PathBuf,
}

impl InstallerTest {
    fn new(arch: &str, binary: &str) -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir()
            .join(format!("lidoff installer test-{}-{timestamp}", std::process::id()));
        let test = Self { root };
        fs::create_dir_all(test.root.join("tools")).unwrap();
        let installer = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"),
        )
        .unwrap();
        // Redirect test paths without changing the process home directory.
        fs::write(
            test.root.join("install.sh"),
            installer.replace("$HOME", "$LIDOFF_TEST_ROOT"),
        )
        .unwrap();
        test.write_executable(
            "tools/uname",
            "#!/bin/bash\ncase \"$1\" in -s) echo Darwin ;; -m) echo \"$LIDOFF_TEST_ARCH\" ;; esac\n",
        );
        test.write_executable(
            "tools/curl",
            r#"#!/bin/bash
set -eu
output=''
url=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) shift; output="$1" ;;
        https:*) url="$1" ;;
    esac
    shift
done
[ "$url" = "https://github.com/mishamyrt/lidoff/releases/latest/download/lidoff-$LIDOFF_TEST_TARGET.tar.xz" ] || exit 1
if [ "$LIDOFF_TEST_FAIL_DOWNLOAD" = 1 ]; then
    echo partial > "$output"
    exit 22
fi
cp "$LIDOFF_TEST_ARCHIVE" "$output"
"#,
        );
        let directory = format!("lidoff-{}-apple-darwin", Self::target_arch(arch));
        test.write_executable(&format!("release/{directory}/lidoff"), binary);
        let archived = Command::new("tar")
            .arg("-cJf")
            .arg(test.root.join("release.tar.xz"))
            .arg("-C")
            .arg(test.root.join("release"))
            .arg(directory)
            .status()
            .unwrap();
        assert!(archived.success());
        test
    }

    fn target_arch(arch: &str) -> &str {
        if arch == "arm64" { "aarch64" } else { arch }
    }

    fn write_executable(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn install_old_service(&self) {
        self.write_executable(".local/bin/lidoff", OLD_BINARY);
        let agents = self.root.join("Library/LaunchAgents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(agents.join("co.myrt.lidoff.plist"), b"existing agent").unwrap();
    }

    fn run(&self, arch: &str, fail_download: bool) -> Output {
        Command::new("/bin/bash")
            .arg(self.root.join("install.sh"))
            .env("LIDOFF_TEST_ROOT", &self.root)
            .env("LIDOFF_TEST_ARCH", arch)
            .env("LIDOFF_TEST_TARGET", format!("{}-apple-darwin", Self::target_arch(arch)))
            .env("LIDOFF_TEST_ARCHIVE", self.root.join("release.tar.xz"))
            .env("LIDOFF_TEST_EVENTS", self.root.join("events"))
            .env("LIDOFF_TEST_FAIL_DOWNLOAD", if fail_download { "1" } else { "0" })
            .env(
                "PATH",
                format!(
                    "{}:{}:/usr/bin:/bin:/usr/sbin:/sbin",
                    self.root.join("tools").display(),
                    self.root.join(".local/bin").display()
                ),
            )
            .output()
            .unwrap()
    }
}

impl Drop for InstallerTest {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn installer_extracts_the_matching_architecture() {
    for arch in ["arm64", "x86_64"] {
        let test = InstallerTest::new(arch, NEW_BINARY);
        let output = test.run(arch, false);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(
            fs::read_to_string(test.root.join(".local/bin/lidoff")).unwrap(),
            NEW_BINARY
        );
        assert!(!test.root.join("events").exists());
        assert_eq!(fs::read_dir(test.root.join(".local/bin")).unwrap().count(), 1);
    }
}

#[test]
fn installer_updates_an_existing_service_with_subcommands() {
    let test = InstallerTest::new("arm64", NEW_BINARY);
    test.install_old_service();
    let output = test.run("arm64", false);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        fs::read_to_string(test.root.join("events")).unwrap(),
        "old uninstall\nnew install\n"
    );
    assert_eq!(fs::read_to_string(test.root.join(".local/bin/lidoff")).unwrap(), NEW_BINARY);
}

#[test]
fn installer_keeps_the_old_service_when_release_validation_fails() {
    for failure in ["download", "archive", "binary", "architecture"] {
        let binary = if failure == "binary" { "#!/bin/bash\nexit 1\n" } else { NEW_BINARY };
        let test = InstallerTest::new("arm64", binary);
        test.install_old_service();
        if failure == "archive" {
            fs::write(test.root.join("release.tar.xz"), b"invalid archive").unwrap();
        }
        let arch = if failure == "architecture" { "unsupported" } else { "arm64" };
        assert!(!test.run(arch, failure == "download").status.success(), "{failure}");
        assert_eq!(
            fs::read_to_string(test.root.join(".local/bin/lidoff")).unwrap(),
            OLD_BINARY
        );
        assert!(!test.root.join("events").exists(), "{failure}");
        assert_eq!(fs::read_dir(test.root.join(".local/bin")).unwrap().count(), 1);
    }
}
