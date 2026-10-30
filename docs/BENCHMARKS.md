# Benchmarks

Measured on 2026-10-30 with a release build on Ubuntu 26.04 (GNOME 50.1, Wayland), on a machine
with 12 logical CPUs and 7 GB RAM. Other machines will differ. Reproduce with the commands below.

## Resource use

Measured across the whole process tree, as proportional memory including swapped pages.

| State | Memory | CPU |
| --- | --- | --- |
| Window open | about 220 MB | Not measured |
| Window closed, after one minute | about 96 MB | 0.07% of one core |

The background figure breaks down as about 77 MB for the DayTrace process and about 20 MB for
`WebKitNetworkProcess`. The page process exits after the window closes. Most of the remainder is
the GTK and WebKit libraries, which stay mapped because they are linked into the application.
Calling `malloc_trim` after the window closes was tested and saved only about 3 MB, so it was not
kept. A much smaller background footprint would need tracking in a separate process.

```bash
tests/gnome/session.sh python3 -I tests/gnome/measure.py target/release/daytrace
```

## Binary and package sizes

| Artifact | Size |
| --- | --- |
| `daytrace` release binary | 9.4 MB |
| `.deb` | 4.5 MB |
| `.rpm` | 4.5 MB |
| AppImage, which bundles GTK and WebKit | 90 MB |

## Query speed

A synthetic year of 219,000 sessions, far more than typical use, in a 30 MB database. Times are
the best of several runs.

| Operation | Time |
| --- | --- |
| Insert all sessions | 2.1 s |
| Report for today | 12 ms |
| Report for 7 days | 15 ms |
| Report for 30 days | 24 ms |
| Report for 365 days | 172 ms |
| Timeline for one day | 12 ms |
| Application usage for 30 days | 104 ms |

These results are why there is no `daily_summaries` table. Reports are fast from the sessions
themselves, and a cache would need invalidating on every edit, rule change and timezone change.

```bash
cargo run --release -p daytrace-core --example bench
```
