#!/bin/sh
# Source this file from the repository root when using project-local SDKs.
if [ -d "$PWD/.tools/cargo" ]; then
  export CARGO_HOME="$PWD/.tools/cargo"
  export RUSTUP_HOME="$PWD/.tools/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
if [ -d "$PWD/.tools/flutter" ]; then
  export PATH="$PWD/.tools/flutter/bin:$PATH"
fi
export PUB_CACHE="${PUB_CACHE:-$PWD/.tools/pub-cache}"
export CLANG_MODULE_CACHE_PATH="$PWD/.tools/clang-cache"

