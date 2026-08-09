# Maintainer: Seo Gihyeon <from104@gmail.com>
pkgname=unim
pkgver=0.4.0
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
# DOCS-V4: source=() 이던 채로 build()/package() 가 "$srcdir/$pkgname-$pkgver"
# 로 cd 하면 makepkg 가 그 디렉터리를 채운 적이 없어 즉시 실패한다. rpm/unim.spec
# Source0 과 동일하게 GitHub 릴리스 태그 tarball 을 받는다(태그 v$pkgver → 압축
# 해제 디렉터리는 $pkgname-$pkgver 로 rpm 쪽에서 이미 확인된 규칙과 동일).
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
# 태깅 전에는 실제 릴리스 tarball 이 없어 해시를 미리 계산할 수 없다 —
# 태깅 직후 `updpkgsums` 로 실값을 채울 것.
sha256sums=('SKIP')

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
