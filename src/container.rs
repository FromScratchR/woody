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
                self.setup_container();
                self.exec_command();
                std::process::exit(0);
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

        /* apply parent process unbound */
        nix::sched::unshare(flags)?;

        /* mount fs */
        self.setup_filesystem().expect("Could not setup fs");

        /* define hostname */
        self.setup_hostname().expect("Could not set hostname");

        Ok(())
    }


    fn setup_filesystem(&self) -> ActionResult {
        use nix::mount::{mount, MsFlags};
        use std::path::Path;

        /* mount new fs */
        let rootfs = Path::new(&self.config.rootfs);
        std::fs::create_dir_all(rootfs)?;
        std::env::set_current_dir(rootfs)?;
        dbg!(std::env::current_dir()?);

        /* mount essential fs */
        self.mount_essential_fs();

        /* Mount base current root fs and container fs to process filesys */
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>
        )?;

        /* bind process' vision of OS */
        nix::unistd::chroot(".")?;

        Ok(())
    }

    fn setup_hostname(&self) -> ActionResult {
        nix::unistd::sethostname("woody_container")?;

        Ok(())
    }

    fn exec_command(&self) {
        let program = CString::new(self.config.command[0].clone()).unwrap();
        let mut args: Vec<CString> = vec![program.clone()]; 

        let additional_args: Vec<CString> = self.config.args
            .iter()
            .map(|arg| CString::new(arg.clone()).unwrap())
            .collect();

        args.extend(additional_args);

        println!("[Container] Executing internal command...");
        nix::unistd::execv(&program, &args).expect("Could not execve");
    }

    fn mount_essential_fs(&self) {
        use nix::mount::{mount, MsFlags};
        use std::fs::{create_dir_all as cd};

        let rootfs = std::path::Path::new(&self.config.rootfs);
        let dirs: Vec<&str> = vec!["/proc", "/sys", "/dev", "/tmp"];

        /* create dir if they do not exist */
        dirs.iter().for_each(|dir| cd(rootfs.join(dir)).expect("Could not create essential dir [{dir}]"));

        dbg!(rootfs.join(dirs[0]));
        dbg!(std::path::Path::exists(std::path::Path::new("./proc")));

        // proc
        mount(
            Some("procs"),
            "/proc",
            Some("proc"),
            MsFlags::empty(),
            None::<&str>
        ).expect("Could not mount proc");

        // sys
        mount(
            Some("sysfs"),
            "/sys",
            Some("sysfs"),
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount sys");

        // dev
        mount(
            None::<&str>,
            "/dev",
            Some("devpts"),
            MsFlags::empty(),
            None::<&str>
        ).expect("Could not mount dev");

        // tmp
        mount(
            Some("tmpfs"),
            "/tmp",
            Some("tmpfs"),
            MsFlags::empty(),
            Some("size=64m")
        ).expect("Could not mount tmp");
    }
}
