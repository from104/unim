#!/bin/bash
# UNIM Debian Package Builder (Split Packages)
# debian/rules와 debian/control의 구조를 반영하여 여러 개의 .deb 패키지를 생성합니다.

set -e

VERSION="0.1.0"
ARCH="amd64"
BASE_BUILD_DIR="pkg-build"

echo "=== UNIM Debian Package Builder (Split Mode) ==="
echo "Version: ${VERSION}"
echo "Architecture: ${ARCH}"

# 1. 필수 바이너리 확인
echo ""
echo "[1/3] 빌드 결과물 확인..."
REQUIRED_FILES=(
    "target/release/libunim_capi.so"
    "target/release/unim-cli"
    "target/release/unim-daemon"
    "target/release/unim-xim"
    "target/release/unim-wayland"
    "unim-frontends/gtk3/build/libim-unim.so"
    "unim-gnome-extension/schemas/gschemas.compiled"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        echo "ERROR: 필수 파일 누락: $file"
        echo "먼저 'cargo build --release', 'make build' 및 frontend 빌드를 실행하세요."
        exit 1
    fi
done
echo "  ✓ 모든 필수 바이너리 확인됨"

# 초기화
rm -rf "${BASE_BUILD_DIR}"
mkdir -p "${BASE_BUILD_DIR}"

echo ""
echo "[2/3] 패키지 별 생성 및 빌드..."

# -----------------------------------------------------------------------------
# 1. unim-core
# -----------------------------------------------------------------------------
PKG="unim-core"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/lib/x86_64-linux-gnu"
mkdir -p "${DIR}/usr/libexec"
mkdir -p "${DIR}/usr/include"
mkdir -p "${DIR}/usr/share/im-config/data"
mkdir -p "${DIR}/usr/share/doc/${PKG}"
mkdir -p "${DIR}/DEBIAN"

cp target/release/libunim_capi.so "${DIR}/usr/lib/x86_64-linux-gnu/"
cp target/release/unim-daemon "${DIR}/usr/libexec/"
cp unim-capi/include/unim.h "${DIR}/usr/include/"
cp im-config/25_unim.conf "${DIR}/usr/share/im-config/data/"
cp im-config/25_unim.rc "${DIR}/usr/share/im-config/data/"
cp README.md "${DIR}/usr/share/doc/${PKG}/"

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: libc6 (>= 2.17), libglib2.0-0
Recommends: im-config
Description: Korean Input Method Engine (Core)
 UNIM is a modular Korean Input Method Engine.
 This package contains the core daemon, C-API library, and configuration integration.
EOF

# postinst/postrm for ldconfig
cat > "${DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e
ldconfig || true
exit 0
EOF
chmod 755 "${DIR}/DEBIAN/postinst"

cat > "${DIR}/DEBIAN/postrm" << 'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ]; then
    ldconfig || true
fi
exit 0
EOF
chmod 755 "${DIR}/DEBIAN/postrm"

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"


# -----------------------------------------------------------------------------
# 2. unim-cli
# -----------------------------------------------------------------------------
PKG="unim-cli"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/bin"
mkdir -p "${DIR}/DEBIAN"

cp target/release/unim-cli "${DIR}/usr/bin/"

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-core (= ${VERSION}), libc6 (>= 2.17)
Description: Korean Input Method Engine (CLI Tool)
 UNIM command-line interface for text conversion and testing.
EOF

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"


# -----------------------------------------------------------------------------
# 3. unim-xim
# -----------------------------------------------------------------------------
PKG="unim-xim"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/libexec"
mkdir -p "${DIR}/DEBIAN"

cp target/release/unim-xim "${DIR}/usr/libexec/"

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-core (= ${VERSION}), libc6 (>= 2.17)
Description: Korean Input Method Engine (XIM Server)
 XIM server for UNIM to support X11 legacy applications.
EOF

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"


# -----------------------------------------------------------------------------
# 4. unim-wayland
# -----------------------------------------------------------------------------
PKG="unim-wayland"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/libexec"
mkdir -p "${DIR}/DEBIAN"

cp target/release/unim-wayland "${DIR}/usr/libexec/"

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-core (= ${VERSION}), libc6 (>= 2.17)
Description: Korean Input Method Engine (Wayland support)
 Wayland text-input protocol support for UNIM.
EOF

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"


# -----------------------------------------------------------------------------
# 5. unim-gtk
# -----------------------------------------------------------------------------
PKG="unim-gtk"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules"
mkdir -p "${DIR}/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules"
mkdir -p "${DIR}/DEBIAN"

cp unim-frontends/gtk3/build/libim-unim.so "${DIR}/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/"
if [[ -f "unim-frontends/gtk4/build/libim-unim-gtk4.so" ]]; then
    cp unim-frontends/gtk4/build/libim-unim-gtk4.so "${DIR}/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/"
fi

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-core (= ${VERSION}), libc6 (>= 2.17), libglib2.0-0
Description: Korean Input Method Engine (GTK modules)
 GTK3 and GTK4 input method modules for UNIM.
EOF

# postinst/postrm for gtk-query-immodules
cat > "${DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e
if command -v gtk-query-immodules-3.0 >/dev/null 2>&1; then
    gtk-query-immodules-3.0 --update-cache || true
fi
exit 0
EOF
chmod 755 "${DIR}/DEBIAN/postinst"

cat > "${DIR}/DEBIAN/postrm" << 'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ]; then
    if command -v gtk-query-immodules-3.0 >/dev/null 2>&1; then
        gtk-query-immodules-3.0 --update-cache || true
    fi
fi
exit 0
EOF
chmod 755 "${DIR}/DEBIAN/postrm"

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"


# -----------------------------------------------------------------------------
# 6. unim-qt
# -----------------------------------------------------------------------------
PKG="unim-qt"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts"
mkdir -p "${DIR}/usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts"
mkdir -p "${DIR}/DEBIAN"

FOUND_QT=false
if [[ -f "unim-frontends/qt5/build/libunimplatforminputcontextplugin.so" ]]; then
    cp unim-frontends/qt5/build/libunimplatforminputcontextplugin.so "${DIR}/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/"
    FOUND_QT=true
fi
if [[ -f "unim-frontends/qt6/build/libunimplatforminputcontextplugin.so" ]]; then
    cp unim-frontends/qt6/build/libunimplatforminputcontextplugin.so "${DIR}/usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/"
    FOUND_QT=true
fi

if [ "$FOUND_QT" = true ]; then
    INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
    cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-core (= ${VERSION}), libc6 (>= 2.17)
Description: Korean Input Method Engine (Qt plugins)
 Qt5 and Qt6 platform input context plugins for UNIM.
EOF
    dpkg-deb --build --root-owner-group "${DIR}"
    echo "    ✓ ${PKG} done"
else
    echo "    - ${PKG} skipped (no binaries found)"
fi


# -----------------------------------------------------------------------------
# 7. gnome-shell-extension-unim
# -----------------------------------------------------------------------------
PKG="gnome-shell-extension-unim"
ARCH_ALL="all"
DIR="${BASE_BUILD_DIR}/${PKG}_${VERSION}_${ARCH_ALL}"
echo "  Building ${PKG}..."

mkdir -p "${DIR}/usr/share/gnome-shell/extensions/unim-typefix@from104.github.io"
mkdir -p "${DIR}/DEBIAN"

cp -r unim-gnome-extension/* "${DIR}/usr/share/gnome-shell/extensions/unim-typefix@from104.github.io/"

INSTALLED_SIZE=$(du -sk "${DIR}" | cut -f1)
cat > "${DIR}/DEBIAN/control" << EOF
Package: ${PKG}
Version: ${VERSION}
Section: gnome
Priority: optional
Architecture: ${ARCH_ALL}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Depends: unim-cli (>= ${VERSION}), gnome-shell
Description: UNIM GNOME Shell Extension
 GNOME Shell extension for UNIM, providing native integration and hotkeys.
EOF

dpkg-deb --build --root-owner-group "${DIR}"
echo "    ✓ ${PKG} done"

# -----------------------------------------------------------------------------
# 정리
# -----------------------------------------------------------------------------
echo ""
echo "[3/3] 결과 이동..."
mv ${BASE_BUILD_DIR}/*.deb .

echo ""
echo "=== 빌드 완료 ==="
ls -1 *.deb | grep "${VERSION}"

echo ""
echo "설치 예시: sudo dpkg -i unim-core_*.deb unim-cli_*.deb unim-gtk_*.deb ..."