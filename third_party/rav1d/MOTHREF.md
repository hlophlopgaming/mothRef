# Local rav1d patch

The code is from the `rav1d` 1.1.0 crate published on crates.io, upstream
revision [`782dab2135ea64a057c097088a13eb8ed3cc3320`](https://github.com/memorysafety/rav1d/tree/782dab2135ea64a057c097088a13eb8ed3cc3320).
The upstream copyright and BSD 2-Clause terms are in `COPYING`.

The manifest aliases `pastey` as `paste` to address RustSec advisory
RUSTSEC-2024-0436. The decoder and build script are copied unchanged. Cargo
profiles and workspace tools belong to upstream and are omitted here.
Remove the two Cargo patches when a compatible published decoder no longer
depends on `paste`.
