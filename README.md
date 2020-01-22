# rust-cmon-agent

This is an experimental project to port [cmon-agent](https://github.com/joyent/triton-cmon-agent) to rust. Basically it serves as a PoC for now.


### Prerequisites

This code requires the latest stable version of rust (1.40.0). The plan is to make use of the new async/await syntax to simplify development.

```
$ uname -a
SunOS cfa81f16-ec3d-cdef-897b-f483b066ad85 5.11 joyent_20191029T142723Z i86pc i386 i86pc illumos
$ rustc --version
rustc 1.40.0
$
```

### Installing

```
$ git clone https://github.com/khalfella/rust-cmon-agent
$ cd rust-cmon-agent
$ cargo run
```
Give the example

Currently the agent is hardcoded to export cpu utilization metrics in global zone.

```
$ curl http://127.0.0.1:3000
# HELP cpu_idle_seconds_total CPU idle time in seconds
# TYPE cpu_idle_seconds_total counter
cpu_idle_seconds_total{cpu_id="0"} 77340.5814115281
cpu_idle_seconds_total{cpu_id="1"} 81793.9255571045
cpu_idle_seconds_total{cpu_id="2"} 91785.9375454156
cpu_idle_seconds_total{cpu_id="3"} 77691.4233084146
# HELP cpu_kernel_seconds_total CPU kernel time in seconds
# TYPE cpu_kernel_seconds_total counter
cpu_kernel_seconds_total{cpu_id="0"} 31436.6625996249
cpu_kernel_seconds_total{cpu_id="1"} 23648.168109081
cpu_kernel_seconds_total{cpu_id="2"} 26298.0675954131
cpu_kernel_seconds_total{cpu_id="3"} 24153.2615456163
# HELP cpu_user_seconds_total CPU user time in seconds
# TYPE cpu_user_seconds_total counter
cpu_user_seconds_total{cpu_id="0"} 56788.1967863367
cpu_user_seconds_total{cpu_id="1"} 60123.3238271402
cpu_user_seconds_total{cpu_id="2"} 47481.4096889726
cpu_user_seconds_total{cpu_id="3"} 63720.7276556447
# HELP cpu_dtrace_seconds_total CPU dtrace time in seconds
# TYPE cpu_dtrace_seconds_total counter
cpu_dtrace_seconds_total{cpu_id="0"} 0
cpu_dtrace_seconds_total{cpu_id="1"} 0
cpu_dtrace_seconds_total{cpu_id="2"} 0
cpu_dtrace_seconds_total{cpu_id="3"} 0
$
```
