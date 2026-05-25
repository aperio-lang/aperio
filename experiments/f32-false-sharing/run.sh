#!/usr/bin/env bash
# F.32-1 (2026-05-24) — false-sharing ceiling microbench runner.
#
# Builds the bench and runs it. See bench.c for the three
# configurations measured (packed / padded_64 / padded_128) and
# the methodology.

set -euo pipefail

cd "$(dirname "$0")"

gcc -O2 -Wall -Wextra -pthread -o bench bench.c
./bench
