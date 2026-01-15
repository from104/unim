#!/bin/bash
# UNIM Debian Package Builder
# 사전 빌드된 바이너리를 사용하여 .deb 패키지 생성

set -e

VERSION="0.1.0"
ARCH="amd64"
PKG_NAME="unim"
PKG_DIR="pkg-build/${PKG_NAME}_${VERSION}_${ARCH}"
DEB_FILE="${PKG_NAME}_${VERSION}_${ARCH}.deb"

echo "=== UNIM Debian Package Builder ==="
echo "Version: ${VERSION}"
echo "Architecture: ${ARCH}"

# 필수 바이너리 확인
echo ""
echo "[1/5] 빌드 결과물 확인..."
REQUIRED_FILES=(
    "target/release/libunim_capi.so"
    "target/release/unim-cli"
    "target/release/unim-daemon"
    "target/release/unim-xim"
    "target/release/unim-wayland"
    "unim-frontends/gtk3/build/libim-unim.so"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        echo "ERROR: 필수 파일 누락: $file"
        echo "먼저 'cargo build --release' 및 frontend 빌드를 실행하세요."
        exit 1
    fi
done
echo "  ✓ 모든 필수 바이너리 확인됨"

# 패키지 디렉터리 구조 생성
echo ""
echo "[2/5] 패키지 디렉터리 구조 생성..."
rm -rf pkg-build
mkdir -p "${PKG_DIR}/DEBIAN"
mkdir -p "${PKG_DIR}/usr/bin"
mkdir -p "${PKG_DIR}/usr/lib/x86_64-linux-gnu"
mkdir -p "${PKG_DIR}/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules"
mkdir -p "${PKG_DIR}/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules"
mkdir -p "${PKG_DIR}/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts"
mkdir -p "${PKG_DIR}/usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts"
mkdir -p "${PKG_DIR}/usr/libexec"
mkdir -p "${PKG_DIR}/usr/include"
mkdir -p "${PKG_DIR}/usr/share/im-config/data"
mkdir -p "${PKG_DIR}/usr/share/doc/${PKG_NAME}"

# 파일 복사
echo ""
echo "[3/5] 파일 복사..."

# Core
cp target/release/libunim_capi.so "${PKG_DIR}/usr/lib/x86_64-linux-gnu/"
cp target/release/unim-cli "${PKG_DIR}/usr/bin/"
cp unim-capi/include/unim.h "${PKG_DIR}/usr/include/"

# Daemons
cp target/release/unim-daemon "${PKG_DIR}/usr/libexec/"
cp target/release/unim-xim "${PKG_DIR}/usr/libexec/"
cp target/release/unim-wayland "${PKG_DIR}/usr/libexec/"

# GTK3 IM Module
cp unim-frontends/gtk3/build/libim-unim.so "${PKG_DIR}/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/"

# GTK4 IM Module (optional)
if [[ -f "unim-frontends/gtk4/build/libim-unim-gtk4.so" ]]; then
    cp unim-frontends/gtk4/build/libim-unim-gtk4.so "${PKG_DIR}/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/"
fi

# Qt5 Plugin (optional)
if [[ -f "unim-frontends/qt5/build/libunimplatforminputcontextplugin.so" ]]; then
    cp unim-frontends/qt5/build/libunimplatforminputcontextplugin.so "${PKG_DIR}/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/"
fi

# Qt6 Plugin (optional)
if [[ -f "unim-frontends/qt6/build/libunimplatforminputcontextplugin.so" ]]; then
    cp unim-frontends/qt6/build/libunimplatforminputcontextplugin.so "${PKG_DIR}/usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/"
fi

# im-config integration
cp im-config/25_unim.conf "${PKG_DIR}/usr/share/im-config/data/"
cp im-config/25_unim.rc "${PKG_DIR}/usr/share/im-config/data/"

# Documentation
cp README.md "${PKG_DIR}/usr/share/doc/${PKG_NAME}/" 2>/dev/null || echo "README.md skipped"

echo "  ✓ 파일 복사 완료"

# DEBIAN/control 생성
echo ""
echo "[4/5] DEBIAN 제어 파일 생성..."
INSTALLED_SIZE=$(du -sk "${PKG_DIR}" | cut -f1)

cat > "${PKG_DIR}/DEBIAN/control" << EOF
Package: ${PKG_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: from104 <from104@github.com>
Homepage: https://github.com/from104/unim
Depends: libc6 (>= 2.17), libglib2.0-0
Recommends: im-config
Description: Korean Input Method Engine
 UNIM is a modular Korean Input Method Engine supporting 2-bul (두벌식)
 and 3-bul (세벌식) keyboard layouts.
 .
 Features:
  * GTK3/GTK4 IM modules for native integration
  * Qt5/Qt6 platform input context plugins
  * XIM server for X11 legacy applications
  * Wayland text-input protocol support
  * Command-line tool for text conversion
EOF

# postinst 스크립트
cat > "${PKG_DIR}/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e

# Update GTK3 IM modules cache
if command -v gtk-query-immodules-3.0 >/dev/null 2>&1; then
    gtk-query-immodules-3.0 --update-cache || true
fi

# Update library cache
ldconfig || true

exit 0
EOF
chmod 755 "${PKG_DIR}/DEBIAN/postinst"

# postrm 스크립트
cat > "${PKG_DIR}/DEBIAN/postrm" << 'EOF'
#!/bin/sh
set -e

if [ "$1" = "remove" ]; then
    # Update GTK3 IM modules cache
    if command -v gtk-query-immodules-3.0 >/dev/null 2>&1; then
        gtk-query-immodules-3.0 --update-cache || true
    fi
    ldconfig || true
fi

exit 0
EOF
chmod 755 "${PKG_DIR}/DEBIAN/postrm"

echo "  ✓ 제어 파일 생성 완료"

# 패키지 빌드
echo ""
echo "[5/5] .deb 패키지 생성..."
dpkg-deb --build --root-owner-group "${PKG_DIR}"
mv "pkg-build/${DEB_FILE}" .

echo ""
echo "=== 빌드 완료 ==="
echo "생성된 패키지: ${DEB_FILE}"
echo ""
echo "설치: sudo dpkg -i ${DEB_FILE}"
echo "제거: sudo dpkg -r ${PKG_NAME}"
