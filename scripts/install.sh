#!/bin/bash
set -e

# UNIM Installation Script
# This script builds and installs all UNIM components.

PROJECT_ROOT=$(pwd)
INSTALL_PREFIX="/usr/local"

echo "--- Building UNIM Core and C-API (Release) ---"
cargo build --release
cargo build --release -p unim-capi

echo "--- Installing C-API ---"
sudo cp target/release/libunim_capi.so /usr/local/lib/
sudo cp unim-capi/include/unim.h /usr/local/include/
sudo ldconfig

echo "--- Installing Binaries ---"
sudo cp target/release/unim-daemon /usr/local/bin/
sudo cp target/release/unim-xim /usr/local/bin/
sudo cp target/release/unim-wayland /usr/local/bin/
sudo cp target/release/unim-tauri /usr/local/bin/

echo "--- Building and Installing GTK3 Module ---"
cd unim-frontends/gtk3
rm -rf build && mkdir build && cd build
cmake .. && make
sudo make install
sudo /usr/lib/x86_64-linux-gnu/libgtk-3-0t64/gtk-query-immodules-3.0 --update-cache

echo "--- Building and Installing Qt5 Module ---"
cd $PROJECT_ROOT/unim-frontends/qt5
rm -rf build && mkdir build && cd build
CMAKE_PREFIX_PATH=/usr/lib/x86_64-linux-gnu/cmake/Qt5 cmake .. && make
sudo make install

echo "--- Building and Installing Qt6 Module ---"
cd $PROJECT_ROOT/unim-frontends/qt6
rm -rf build && mkdir build && cd build
cmake .. && make
sudo make install

echo "--- Installing im-config Scripts ---"
sudo cp $PROJECT_ROOT/im-config/25_unim.conf /usr/share/im-config/data/
sudo cp $PROJECT_ROOT/im-config/25_unim.rc /usr/share/im-config/data/

echo "--- Installation Complete! ---"
echo "You can now use 'im-config -n unim' to set UNIM as your primary input method."
echo "Log out and log back in to apply changes."
