use std::ffi::CString;

use nix::{sched::CloneFlags, unistd::ForkResult};
use crate::ActionResult;

#[derive(Debug)]
pub struct ContainerConfig {
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub rootfs: String,
}

pub struct Container {
    pub config: ContainerConfig,
}

impl Container {
    pub fn new(config: ContainerConfig) -> Self {
        Container { config }
    }

    /// Create child container proccess
    ///
    pub fn run(&self) {
        match unsafe { nix::unistd::fork().expect("Error forking new child process") } {
            ForkResult::Parent { child } => {
                nix::sys::wait::waitpid(child, None).expect("Error waiting for child");
            }
            ForkResult::Child => {
                self.setup_container().expect("Could not setup container");
                self.exec_command().expect("Could not exec command");
            }
        }
    }
}

impl Container {
    /// Unshare, setup fs and hostname for newly decoupled process
    ///
    fn setup_container(&self) -> ActionResult {
        /* ensure new process is completely isolated */
        let flags = CloneFlags::CLONE_NEWPID
                                | CloneFlags::CLONE_NEWNS
                                | CloneFlags::CLONE_NEWUTS
                                | CloneFlags::CLONE_NEWIPC
                                | CloneFlags::CLONE_NEWNET;

        /* apply process isolation */
        nix::sched::unshare(flags)?;

        /* mount fs */
        self.setup_filesystem()?;

        /* define hostname */
        self.setup_hostname()?;

        Ok(())
    }


    fn setup_filesystem(&self) -> ActionResult {
        use nix::mount::{mount, MsFlags};
        use nix::unistd::chroot;
        use std::path::Path;

        /* Mount base current root fs and container fs to process filesys */
        mount(
            Some("none"),
            "/",
            Some(""),
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            Some("")
        )?;

        /* mount new fs */
        let rootfs = Path::new(&self.config.rootfs);

        mount(
            Some(rootfs),
            rootfs,
            Some(""),
            MsFlags::MS_BIND | MsFlags::MS_REC,
            Some("")
        )?;

        /* chroot to new created fs */
        std::env::set_current_dir(rootfs)?;
        chroot(".")?;
        std::env::set_current_dir("/")?;

        /* mount essential fs */
        self.mount_essential_fs()?;

        Ok(())
    }

    fn setup_hostname(&self) -> ActionResult {
        nix::unistd::sethostname("woody_container")?;

        Ok(())
    }

    fn exec_command(&self) -> ActionResult {
        let program = CString::new(self.config.command[0].clone())?;
        let args: Result<Vec<CString>, _> = self.config.args
            .iter()
            .map(|arg| CString::new(arg.clone()))
            .collect();

        nix::unistd::execv(&program, &args?)?;
        Ok(())
    }


    /// Works with / rootfs in order to create essential folders n mounts
    ///
    fn mount_essential_fs(&self) -> ActionResult {
        use nix::mount::{mount, MsFlags};
        use std::fs::{create_dir_all as cd};

        /* create dir if they do not exist */
        cd("/proc")?;
        cd("/sys")?;
        cd("/dev")?;
        cd("/tmp")?;

        /* mount sysfs */
        mount(
            Some("sysfs"),
            "/sys",
            Some("sysfs"),
            MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
            Some("")
        )?;

        /* mount working fs */
        mount(
            Some("tmpfs"),
            "/tmp",
            Some("tmpfs"),
            MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("size=64m")
        )?;

        Ok(())
    }
}
