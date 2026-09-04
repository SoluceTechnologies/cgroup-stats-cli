# cgroup-stats-cli

Live cgroup v2 resource usage: CPU as cores, memory, PIDs and block IO.

Replaces the `watch` + `awk` idiom for reading `cpu.stat` deltas by hand.

## Install

- **Debian/Ubuntu (apt):** see [DISTRIBUTION.md](DISTRIBUTION.md)
- **cargo:** `cargo install cgroup-stats-cli`

The installed binary is `cgroup-stats`.

## Usage

```
cgroup-stats [OPTIONS] --path <PATH>

  -p, --path <PATH>       cgroup path, absolute or relative to the hierarchy root
  -c, --cpu               show CPU statistics
  -m, --mem               show memory statistics
  -P, --pids              show PID statistics
  -b, --io                show block IO statistics
  -i, --interval <SECS>   sampling window for CPU/IO deltas [default: 1.0]
  -f, --format <FMT>      human | json | table [default: human]
```

With no metric flag, all four are shown.

```console
$ cgroup-stats --path /sys/fs/cgroup/system.slice/nginx.service
RAM:  1.2G / 4.0G
CPU:  0.53 / 2.00 cores
PIDs: 42 / 512
IO:   sda  r 1.2M/s  w 340.0K/s
```

Refreshing display:

```bash
watch -n 2 cgroup-stats --path /sys/fs/cgroup/system.slice/nginx.service
```

## Notes

CPU usage is a rate, so `--cpu` and `--io` sample twice around `--interval`.
`--mem` and `--pids` alone read once and return immediately.

Human and table output list only devices that moved data during the sampling window, and print `no activity` when none
did. The root cgroup's `io.stat`
enumerates every block device on the host, so an unfiltered view there is dozens of idle rows with any real traffic
buried among them. JSON is the fidelity layer and always reports every device, so a consumer can filter for itself.

`memory.high` is reported beside `memory.max` whenever it is set. The two are different limits: the kernel reclaims
aggressively and stalls a cgroup above
`high`, but only OOM-kills above `max`. A systemd unit with `MemoryHigh=` and no
`MemoryMax=` is capped in practice, so reporting it as unlimited would mislead.

Requires cgroup v2 (unified hierarchy). Metrics whose files are absent (the root cgroup has no `memory.current`, for
instance) report `n/a` rather than a misleading zero.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the required checks, the project's
conventions, and how releases are cut.

## Licence

Apache-2.0
