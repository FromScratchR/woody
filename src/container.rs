use std::ffi::CString;

use nix::{sched::CloneFlags, unistd::ForkResult};
use crate::{cgroups::{CgroupManager}, ActionResult};

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
        // 1. SETUP CGROUP (IN PARENT, BEFORE FORK)
        // Use a unique ID for the container, e.g., a UUID or a name.
        let container_id = "container-123";
        let cgroups = CgroupManager::new(container_id);

        // Create the parent cgroup directory and enable controllers first.
        // These might fail if they already exist, so you might want to handle that.
        std::fs::create_dir_all("/sys/fs/cgroup/woody").ok();
        cgroups.enable_controllers().expect("Failed to enable cgroup controllers");

        // Now create the container-specific cgroup and set its limits.
        cgroups.create().expect("Could not create Cgroup folder");
        println!("[Parent] Cgroup created at: {}", cgroups.cgroup_path);

        // Set a 50MB memory limit and 100 process limit
        cgroups.set_memory_limit(50 * 1024 * 1024).expect("Could not set memory_limit");
        cgroups.set_pid_limit(100).expect("Could not set pid_limit");
        println!("[Parent] Cgroup limits set.");

        match unsafe { nix::unistd::fork().expect("Error forking new child process") } {
            ForkResult::Parent { child } => {
                let cgroups = CgroupManager::new(&std::process::id().to_string());
                cgroups.create().expect("Could not create Cgroup folder");
                cgroups.set_memory_limit(200).expect("Could not set mem_limit");

                nix::sys::wait::waitpid(child, None).expect("Error waiting for child");
            }
            ForkResult::Child => {
                // let cgroups = CgroupManager::new(&std::process::id().to_string());
                // cgroups.create().expect("Could not create Cgroup folder");
                // cgroups.set_memory_limit(50000).expect("Could not set mem_limit");
                // std::fs::write("/sys/fs/cgroup/cgroup.subtree_control", "+pids +memory").expect("Failed to create subtree_control");
                 let pid = nix::unistd::getpid();
                println!("[Child] My PID is {}. Adding myself to the cgroup.", pid);
                cgroups.add_process(pid).expect("Child failed to join cgroup");

                // 4. CHILD CONTINUES SETUP AND EXECUTES COMMAND
                println!("[Child] Cgroup joined. Setting up container environment...");

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
    fn setup_container(&self) {
        /* ensure new process is completely isolated */
        let flags = CloneFlags::CLONE_NEWPID
                                | CloneFlags::CLONE_NEWNS
                                | CloneFlags::CLONE_NEWUTS
                                | CloneFlags::CLONE_NEWIPC
                                | CloneFlags::CLONE_NEWNET;

        /* apply parent process unbound */
        nix::sched::unshare(flags).expect("Could not unshare container process");

        /* mount fs */
        self.setup_filesystem().expect("Could not setup fs");

        /* define hostname */
        self.setup_hostname().expect("Could not set hostname");
    }


    fn setup_filesystem(&self) -> ActionResult {
        use std::path::Path;

        /* mount new fs */
        let rootfs = Path::new(&self.config.rootfs);

        std::fs::create_dir_all(rootfs)?;
        std::env::set_current_dir(rootfs)?;

        println!("Initializing container on: {:?}", std::env::current_dir().unwrap());

        /* mount essential fs */
        self.mount_essential_fs();
        println!("[Container]: Success on fs mount");

        /* bind process' vision of OS */
        nix::unistd::chroot(".")?;
        println!("[Container]: Changed root");

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

        
        /* create dir if they do not exist */
        let dirs: Vec<&str> = vec![
            "./proc",
            "./sys",
            "./dev",
            "./tmp",
            "./bin",
            "./usr/bin",
            "./usr/lib",
            "./usr/lib64",
            "./lib",
            "./lib64",
        ];

        for dir in dirs {
            cd(dir).expect("Could not create essential dir [{dir}]");
        };

        let mount_flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV;

        // proc
        mount(
            None::<&str>,
            "./proc",
            Some("proc"),
            MsFlags::empty(),
            None::<&str>
        ).expect("Could not mount proc");

        // sys
        mount(
            None::<&str>,
            "./sys",
            Some("sysfs"),
            MsFlags::empty(),
            None::<&str>
        ).expect("Could not mount sys");

        // dev
        mount(
            None::<&str>,
            "./dev",
            Some("tmpfs"),
            MsFlags::empty(),
            Some("mode=0755,size=65536k")
        ).expect("Could not mount dev");

        // tmp
        mount(
            None::<&str>,
            "./tmp",
            Some("tmpfs"),
            MsFlags::empty(),
            None::<&str> // Some("size=64m")
        ).expect("Could not mount tmp");

        // bin
        mount(
            Some("/bin"),
            "./bin",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount bin");

        // Bind mount /usr/bin
        mount(
            Some("/usr/bin"),
            "./usr/bin",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount usr/bin");

        // Bind mount /lib and /lib64 for shared libraries
        mount(
            Some("/lib"),
            "./lib",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount lib");

        mount(
            Some("/lib64"),
            "./lib64",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount lib64");

        mount(
            Some("/usr/lib"),
            "./usr/lib",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount usr/lib");

        mount(
            Some("/usr/lib64"),
            "./usr/lib64",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>
        ).expect("Could not mount usr/lib64");

        let dev_internals = vec!["./dev/pts", "./dev/shm"];
        dev_internals
            .iter()
            .for_each(|dir|
                cd(dir).expect("Could not create dev internals")
            );

        // dev/pts - For pseudo-terminals (like the shell)
        mount(
            None::<&str>,
            "./dev/pts",
            Some("devpts"),
            MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("newinstance,ptmxmode=0666,gid=5")
        ).expect("Could not mount dev/pts");
        
        // dev/shm - Crucial for POSIX shared memory
        mount(
            None::<&str>,
            "./dev/shm",
            Some("tmpfs"),
            MsFlags::empty(), // Add NOEXEC for security
            Some("mode=1777") // 64M size limit
        ).expect("Could not mount /dev/shm tmpfs");
    }
}
