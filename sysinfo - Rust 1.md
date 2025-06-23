---
title: "sysinfo - Rust"
source: "https://docs.rs/sysinfo/latest/sysinfo/index.html#usage"
author:
published:
created: 2025-06-23
description: "sysinfo"
tags:
  - "clippings"
---
## Crate sysinfo

[Source](https://docs.rs/sysinfo/latest/src/sysinfo/lib.rs.html#3-352)

Expand description

## sysinfo

`sysinfo` is a crate used to get a system’s information.

### Supported OSes

It currently supports the following OSes (alphabetically sorted):

- Android
- FreeBSD
- iOS
- Linux
- macOS
- Raspberry Pi
- Windows

You can still use `sysinfo` on non-supported OSes, it’ll simply do nothing and always return empty values. You can check in your program directly if an OS is supported by checking the [`IS_SUPPORTED_SYSTEM`](https://docs.rs/sysinfo/latest/sysinfo/constant.IS_SUPPORTED_SYSTEM.html "constant sysinfo::IS_SUPPORTED_SYSTEM") constant.

The minimum-supported version of `rustc` is **1.75**.

### Usage

If you want to migrate from an older version, don’t hesitate to take a look at the [CHANGELOG](https://github.com/GuillaumeGomez/sysinfo/blob/master/CHANGELOG.md) and at the [migration guide](https://github.com/GuillaumeGomez/sysinfo/blob/master/migration_guide.md).

⚠️ Before any attempt to read the different structs’ information, you need to update them to get up-to-date information because for most of them, it works on diff between the current value and the old one.

Which is why, it’s much better to keep the same instance of [`System`](https://docs.rs/sysinfo/latest/sysinfo/struct.System.html "struct sysinfo::System") around instead of recreating it multiple times.

You have an example into the `examples` folder. You can run it with `cargo run --example simple`.

Otherwise, here is a little code sample:

```rust
use sysinfo::{
    Components, Disks, Networks, System,
};

// Please note that we use "new_all" to ensure that all lists of
// CPUs and processes are filled!
let mut sys = System::new_all();

// First we update all information of our \`System\` struct.
sys.refresh_all();

println!("=> system:");
// RAM and swap information:
println!("total memory: {} bytes", sys.total_memory());
println!("used memory : {} bytes", sys.used_memory());
println!("total swap  : {} bytes", sys.total_swap());
println!("used swap   : {} bytes", sys.used_swap());

// Display system information:
println!("System name:             {:?}", System::name());
println!("System kernel version:   {:?}", System::kernel_version());
println!("System OS version:       {:?}", System::os_version());
println!("System host name:        {:?}", System::host_name());

// Number of CPUs:
println!("NB CPUs: {}", sys.cpus().len());

// Display processes ID, name and disk usage:
for (pid, process) in sys.processes() {
    println!("[{pid}] {:?} {:?}", process.name(), process.disk_usage());
}

// We display all disks' information:
println!("=> disks:");
let disks = Disks::new_with_refreshed_list();
for disk in &disks {
    println!("{disk:?}");
}

// Network interfaces name, total data received and total data transmitted:
let networks = Networks::new_with_refreshed_list();
println!("=> networks:");
for (interface_name, data) in &networks {
    println!(
        "{interface_name}: {} B (down) / {} B (up)",
        data.total_received(),
        data.total_transmitted(),
    );
    // If you want the amount of data received/transmitted since last call
    // to \`Networks::refresh\`, use \`received\`/\`transmitted\`.
}

// Components temperature:
let components = Components::new_with_refreshed_list();
println!("=> components:");
for component in &components {
    println!("{component:?}");
}
```

Please remember that to have some up-to-date information, you need to call the equivalent `refresh` method. For example, for the CPU usage:

```rust
use sysinfo::System;

let mut sys = System::new();

loop {
    sys.refresh_cpu_usage(); // Refreshing CPU usage.
    for cpu in sys.cpus() {
        print!("{}% ", cpu.cpu_usage());
    }
    // Sleeping to let time for the system to run for long
    // enough to have useful information.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
}
```

By default, `sysinfo` uses multiple threads. However, this can increase the memory usage on some platforms (macOS for example). The behavior can be disabled by setting `default-features = false` in `Cargo.toml` (which disables the `multithread` cargo feature).

#### Good practice / Performance tips

Most of the time, you don’t want all information provided by `sysinfo` but just a subset of it. In this case, it’s recommended to use `refresh_specifics(...)` methods with only what you need to have much better performance.

Another issue frequently encountered: unless you know what you’re doing, it’s almost all the time better to instantiate the `System` struct once and use this one instance through your program. The reason is because a lot of information needs a previous measure to be computed (the CPU usage for example). Another example why it’s much better: in case you want to list all running processes, `sysinfo` needs to allocate all memory for the `Process` struct list, which takes quite some time on the first run.

If your program needs to use a lot of file descriptors, you’d better use:

```rust
sysinfo::set_open_files_limit(0);
```

as `sysinfo` keeps a number of file descriptors open to have better performance on some targets when refreshing processes.

#### Running on Raspberry Pi

It’ll be difficult to build on Raspberry Pi. A good way-around is to cross-build, then send the executable to your Raspberry Pi.

First install the arm toolchain, for example on Ubuntu:

```bash
> sudo apt-get install gcc-multilib-arm-linux-gnueabihf
```

Then configure cargo to use the corresponding toolchain:

```bash
cat << EOF > ~/.cargo/config
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
EOF
```

Finally, cross compile:

```bash
rustup target add armv7-unknown-linux-gnueabihf
cargo build --target=armv7-unknown-linux-gnueabihf
```

#### Linux on Docker & Windows Subsystem for Linux (WSL)

Virtual Linux systems, such as those run through Docker and Windows Subsystem for Linux (WSL), do not receive host hardware information via `/sys/class/hwmon` or `/sys/class/thermal`. As such, querying for components may return no results (or unexpected results) when using this library on virtual systems.

#### Use in binaries running inside the macOS or iOS Sandbox/stores

Apple has restrictions as to which APIs can be linked into binaries that are distributed through the app store. By default, `sysinfo` is not compatible with these restrictions. You can use the `apple-app-store` feature flag to disable the Apple prohibited features. This also enables the `apple-sandbox` feature. In the case of applications using the sandbox outside of the app store, the `apple-sandbox` feature can be used alone to avoid causing policy violations at runtime.

#### How it works

I wrote a blog post you can find [here](https://blog.guillaume-gomez.fr/articles/2021-09-06+sysinfo%3A+how+to+extract+systems%27+information) which explains how `sysinfo` extracts information on the different systems.

#### Running tests

Because we’re looking at system information, some tests have a better chance to succeed when there is a limited number of parallel running tests. To ensure they all pass, use:

```bash
cargo test -- --test-threads=1
```

#### C interface

It’s possible to use this crate directly from C. Take a look at the `Makefile` and at the `examples/simple.c` file.

To build the C example, just run:

```bash
> make
> ./simple
# If needed:
> LD_LIBRARY_PATH=target/debug/ ./simple
```

#### Benchmarks

You can run the benchmarks locally with rust **nightly** by doing:

```bash
> cargo bench
```

If you appreciate my work and want to support me, you can do it with [github sponsors](https://github.com/sponsors/GuillaumeGomez) or with [patreon](https://www.patreon.com/GuillaumeGomez).

With the `serde` feature enabled, you can then serialize `sysinfo` types. Let’s see an example with `serde_json`:

```rust
use sysinfo::System;

let mut sys = System::new_all();
// First we update all information of our \`System\` struct.
sys.refresh_all();

println!("{}", serde_json::to_string(&sys).unwrap());
```

## Structs

[CGroup  Limits](https://docs.rs/sysinfo/latest/sysinfo/struct.CGroupLimits.html "struct sysinfo::CGroupLimits")

Contains memory limits for the current process.

[Component](https://docs.rs/sysinfo/latest/sysinfo/struct.Component.html "struct sysinfo::Component")

Getting a component temperature information.

[Components](https://docs.rs/sysinfo/latest/sysinfo/struct.Components.html "struct sysinfo::Components")

Interacting with components.

[Cpu](https://docs.rs/sysinfo/latest/sysinfo/struct.Cpu.html "struct sysinfo::Cpu")

Contains all the methods of the [`Cpu`](https://docs.rs/sysinfo/latest/sysinfo/struct.Cpu.html "struct sysinfo::Cpu") struct.

[CpuRefresh  Kind](https://docs.rs/sysinfo/latest/sysinfo/struct.CpuRefreshKind.html "struct sysinfo::CpuRefreshKind")

Used to determine what you want to refresh specifically on the [`Cpu`](https://docs.rs/sysinfo/latest/sysinfo/struct.Cpu.html "struct sysinfo::Cpu") type.

[Disk](https://docs.rs/sysinfo/latest/sysinfo/struct.Disk.html "struct sysinfo::Disk")

Struct containing a disk information.

[Disk  Refresh  Kind](https://docs.rs/sysinfo/latest/sysinfo/struct.DiskRefreshKind.html "struct sysinfo::DiskRefreshKind")

Used to determine what you want to refresh specifically on the [`Disk`](https://docs.rs/sysinfo/latest/sysinfo/struct.Disk.html "struct sysinfo::Disk") type.

[Disk  Usage](https://docs.rs/sysinfo/latest/sysinfo/struct.DiskUsage.html "struct sysinfo::DiskUsage")

Type containing read and written bytes.

[Disks](https://docs.rs/sysinfo/latest/sysinfo/struct.Disks.html "struct sysinfo::Disks")

Disks interface.

[Gid](https://docs.rs/sysinfo/latest/sysinfo/struct.Gid.html "struct sysinfo::Gid")

A group id wrapping a platform specific type.

[Group](https://docs.rs/sysinfo/latest/sysinfo/struct.Group.html "struct sysinfo::Group")

Type containing group information.

[Groups](https://docs.rs/sysinfo/latest/sysinfo/struct.Groups.html "struct sysinfo::Groups")

Interacting with groups.

[IpNetwork](https://docs.rs/sysinfo/latest/sysinfo/struct.IpNetwork.html "struct sysinfo::IpNetwork")

IP networks address for network interface.

[LoadAvg](https://docs.rs/sysinfo/latest/sysinfo/struct.LoadAvg.html "struct sysinfo::LoadAvg")

A struct representing system load average value.

[MacAddr](https://docs.rs/sysinfo/latest/sysinfo/struct.MacAddr.html "struct sysinfo::MacAddr")

MAC address for network interface.

[Memory  Refresh  Kind](https://docs.rs/sysinfo/latest/sysinfo/struct.MemoryRefreshKind.html "struct sysinfo::MemoryRefreshKind")

Used to determine which memory you want to refresh specifically.

[Network  Data](https://docs.rs/sysinfo/latest/sysinfo/struct.NetworkData.html "struct sysinfo::NetworkData")

Getting volume of received and transmitted data.

[Networks](https://docs.rs/sysinfo/latest/sysinfo/struct.Networks.html "struct sysinfo::Networks")

Interacting with network interfaces.

[Pid](https://docs.rs/sysinfo/latest/sysinfo/struct.Pid.html "struct sysinfo::Pid")

Process ID.

[Process](https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html "struct sysinfo::Process")

Struct containing information of a process.

[Process  Refresh  Kind](https://docs.rs/sysinfo/latest/sysinfo/struct.ProcessRefreshKind.html "struct sysinfo::ProcessRefreshKind")

Used to determine what you want to refresh specifically on the [`Process`](https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html "struct sysinfo::Process") type.

[Refresh  Kind](https://docs.rs/sysinfo/latest/sysinfo/struct.RefreshKind.html "struct sysinfo::RefreshKind")

Used to determine what you want to refresh specifically on the [`System`](https://docs.rs/sysinfo/latest/sysinfo/struct.System.html "struct sysinfo::System") type.

[System](https://docs.rs/sysinfo/latest/sysinfo/struct.System.html "struct sysinfo::System")

Structs containing system’s information such as processes, memory and CPU.

[Uid](https://docs.rs/sysinfo/latest/sysinfo/struct.Uid.html "struct sysinfo::Uid")

A user id wrapping a platform specific type.

[User](https://docs.rs/sysinfo/latest/sysinfo/struct.User.html "struct sysinfo::User")

Type containing user information.

[Users](https://docs.rs/sysinfo/latest/sysinfo/struct.Users.html "struct sysinfo::Users")

Interacting with users.

## Enums

[Disk  Kind](https://docs.rs/sysinfo/latest/sysinfo/enum.DiskKind.html "enum sysinfo::DiskKind")

Enum containing the different supported kinds of disks.

[IpNetwork  From  StrError](https://docs.rs/sysinfo/latest/sysinfo/enum.IpNetworkFromStrError.html "enum sysinfo::IpNetworkFromStrError")

Error type returned from `MacAddr::from_str` implementation.

[Kill  Error](https://docs.rs/sysinfo/latest/sysinfo/enum.KillError.html "enum sysinfo::KillError")

Enum describing possible [`Process::kill_and_wait`](https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html#method.kill_and_wait "method sysinfo::Process::kill_and_wait") errors.

[MacAddr  From  StrError](https://docs.rs/sysinfo/latest/sysinfo/enum.MacAddrFromStrError.html "enum sysinfo::MacAddrFromStrError")

Error type returned from `MacAddr::from_str` implementation.

[Process  Status](https://docs.rs/sysinfo/latest/sysinfo/enum.ProcessStatus.html "enum sysinfo::ProcessStatus")

Enum describing the different status of a process.

[Processes  ToUpdate](https://docs.rs/sysinfo/latest/sysinfo/enum.ProcessesToUpdate.html "enum sysinfo::ProcessesToUpdate")

This enum allows you to specify if you want all processes to be updated or just some of them.

[Signal](https://docs.rs/sysinfo/latest/sysinfo/enum.Signal.html "enum sysinfo::Signal")

An enum representing signals on UNIX-like systems.

[Thread  Kind](https://docs.rs/sysinfo/latest/sysinfo/enum.ThreadKind.html "enum sysinfo::ThreadKind")

Enum describing the different kind of threads.

[Update  Kind](https://docs.rs/sysinfo/latest/sysinfo/enum.UpdateKind.html "enum sysinfo::UpdateKind")

This enum allows you to specify when you want the related information to be updated.

## Constants

[IS\_  SUPPORTED\_  SYSTEM](https://docs.rs/sysinfo/latest/sysinfo/constant.IS_SUPPORTED_SYSTEM.html "constant sysinfo::IS_SUPPORTED_SYSTEM")

Returns `true` if this OS is supported. Please refer to the [crate-level documentation](https://docs.rs/sysinfo/latest/sysinfo/index.html) to get the list of supported OSes.

[MINIMUM\_  CPU\_  UPDATE\_  INTERVAL](https://docs.rs/sysinfo/latest/sysinfo/constant.MINIMUM_CPU_UPDATE_INTERVAL.html "constant sysinfo::MINIMUM_CPU_UPDATE_INTERVAL")

This is the minimum interval time used internally by `sysinfo` to refresh the CPU time.

[SUPPORTED\_  SIGNALS](https://docs.rs/sysinfo/latest/sysinfo/constant.SUPPORTED_SIGNALS.html "constant sysinfo::SUPPORTED_SIGNALS")

Returns the list of the supported signals on this system (used by [`Process::kill_with`](https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html#method.kill_with "method sysinfo::Process::kill_with")).

## Functions

[get\_  current\_  pid](https://docs.rs/sysinfo/latest/sysinfo/fn.get_current_pid.html "fn sysinfo::get_current_pid")

Returns the pid for the current process.

[set\_  open\_  files\_  limit](https://docs.rs/sysinfo/latest/sysinfo/fn.set_open_files_limit.html "fn sysinfo::set_open_files_limit")

This function is only used on Linux targets, when the `system` feature is enabled. In other cases, it does nothing and returns `false`.