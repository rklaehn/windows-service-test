#!/bin/sh

# MSVC
# export CC_x86_64_pc_windows_msvc="/opt/homebrew/opt/llvm/bin/clang"
# export AR_x86_64_pc_windows_msvc="/opt/homebrew/opt/llvm/bin/llvm-ar"
# export CXX_x86_64_pc_windows_msvc="/opt/homebrew/opt/llvm/bin/clang++"
# export CFLAGS_x86_64_pc_windows_msvc="--target=x86_64-pc-windows-msvc"
# export CXXFLAGS_x86_64_pc_windows_msvc="--target=x86_64-pc-windows-msvc"

# GNU
export CC_x86_64_pc_windows_gnu="x86_64-w64-mingw32-gcc"
export CXX_x86_64_pc_windows_gnu="x86_64-w64-mingw32-g++"
export AR_x86_64_pc_windows_gnu="x86_64-w64-mingw32-ar"
export RANLIB_x86_64_pc_windows_gnu="x86_64-w64-mingw32-ranlib"