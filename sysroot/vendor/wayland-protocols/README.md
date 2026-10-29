# Wayland protocol inputs

Selected XML files from **wayland-protocols 1.47**, maintained by the Wayland
project: <https://gitlab.freedesktop.org/wayland/wayland-protocols/-/tree/1.47>.
Each XML retains its upstream copyright and MIT-style license notice.

`scripts/generate_window_protocols.py` uses **wayland-scanner 1.24.0** from
<https://gitlab.freedesktop.org/wayland/wayland/-/tree/1.24.0> to generate the
committed client headers and private protocol C data in the `scoop.window` Cone.
Run it explicitly after changing the maintained inputs; normal builds do not
run a scanner or fetch protocol definitions.

Only protocols consumed by the Linux window backend are included. Core Wayland
interfaces come from the matching system libwayland development headers.
