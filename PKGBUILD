# Maintainer: Seo Gihyeon <from104@gmail.com>
pkgname=unim
pkgver=0.3.0
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

    # Man pages — docs/man/ 에 실제로 존재하는 것만. 목록이 어긋나면 install 이
    # 실패해 package() 가 통째로 죽으므로, man page 추가·제거 시 함께 갱신할 것.
    for _page in docs/man/*.1; do
        install -Dm644 "$_page" "$pkgdir/usr/share/man/man1/$(basename "$_page")"
    done

    # License
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
