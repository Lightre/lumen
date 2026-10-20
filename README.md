# Lumen

Windows için Rust ile geliştirilen sade, modern bir fotoğraf görüntüleyicisi.

Proje hazırlık aşamasında; henüz grafik arayüz veya fotoğraf açma özelliği yoktur.

## İlk sürüm kapsamı

- Windows masaüstü uygulaması.
- JPEG, PNG ve WebP açma.
- Aynı klasörde önceki ve sonraki fotoğrafa geçiş.
- Yakınlaştırma, görüntüyü sürükleyerek gezinme ve tam ekran.
- Siyaha yakın koyu gri, sade bir arayüz.

Düzenleme ve çizim kapsam dışıdır. Döndürme, tema ayarları, ek biçimler ve diğer
işletim sistemleri sonraki sürümlerde değerlendirilecektir.

## Geliştirme

Rust stable, MSVC araç zinciri ve Windows C++ derleme araçları gerekir.

```powershell
cargo run
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`Cargo.toml` proje bilgilerini ve bağımlılıklarını, `Cargo.lock` bağımlılıkların
kesin sürümlerini tutar. Uygulamanın giriş noktası `src/main.rs` dosyasıdır.

## Sıradaki adımlar

1. Koyu temalı ilk pencere ve boş ekran.
2. Dosya açma ve tek fotoğraf görüntüleme.
3. Klasörde gezinme, yakınlaştırma, sürükleme ve tam ekran.
4. Hata durumları, büyük fotoğraflar ve performans ölçümleri.

Arayüz için ilk aday `egui/eframe`, görüntü çözümleme için `image` kütüphanesidir.
Bağımlılıklar ilk pencere aşamasında eklenecektir. Görüntü çözümleme arayüzü
kilitlemeyecek şekilde tasarlanacak; bellek kullanımı ve ön yükleme ölçümlerle
sınırlandırılacaktır.

Çalışma düzeni için [CONTRIBUTING.md](CONTRIBUTING.md) dosyasına bakın.
