# Maintainer: Seo Gihyeon <from104@gmail.com>
pkgname=unim
pkgver=0.1.0
pkgrel=1
pkgdesc="Universal Next-generation Input Method for Korean"
arch=('x86_64')
url="https://github.com/from104/unim"
license=('MIT')
depends=(
    'gtk3'
    'gtk4'
    'qt5-base'
    'qt6-base'
    'dbus'
    'libx11'
    'libxkbcommon'
    'pango'
)
makedepends=(
    'rust'
    'cargo'
    'cmake'
    'pkg-config'
    'gtk3'
    'gtk4'
    'qt5-base'
    'qt6-base'
)
optdepends=(
    'gnome-shell: GNOME Shell extension support'
)
source=()
sha256sums=()

build() {
    cd "$srcdir/$pkgname-$pkgver"
    make build
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    make install DESTDIR="$pkgdir" PREFIX=/usr

    # Man page
    install -Dm644 docs/unim.1 "$pkgdir/usr/share/man/man1/unim.1"

    # License
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
