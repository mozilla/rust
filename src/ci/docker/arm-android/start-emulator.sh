#!/bin/sh
# Copyright 2016 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

set -ex

# Setting SHELL to a file instead on a symlink helps android
# emulator identify the system
export SHELL=/bin/bash
stat $LD_PRELOAD
stat /usr/lib/i386-linux-gnu/libeatmydata.so
LD_PRELOAD="/usr/lib/i386-linux-gnu/libeatmydata.so $LD_PRELOAD" nohup nohup emulator @arm-18 -no-boot-anim -no-window -partition-size 2047 0<&- &>/dev/null &
exec "$@"
