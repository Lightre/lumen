# Çalışma düzeni

## Git ve GitHub

- `main` çalışır durumda tutulur.
- Her anlamlı iş için kısa ömürlü bir dal açılır: `feat/open-image`,
  `fix/zoom-anchor`, `chore/ci` gibi.
- Commit'ler tek bir mantıksal değişikliği içerir. Başlıklar kısa ve İngilizcedir:
  `feat: open local images`, `fix: preserve zoom position` gibi.
- GitHub bağlantısı kurulduğunda değişiklikler PR ile birleştirilir.
- PR açıklaması sonucu, doğrulama adımlarını ve varsa bilinen eksikleri belirtir.
- PR kontrolleri geçtikten sonra squash merge tercih edilir; tamamlanan dal silinir.
- Sürümler `v0.1.0` gibi etiketlerle işaretlenir. Yayın paketleme daha sonra eklenir.

İlk hazırlık commit'i doğrudan `main` üzerinde oluşturulur. Henüz GitHub uzak
deposu, dal koruması veya otomatik birleştirme yapılandırılmamıştır.

## Doğrulama

PR öncesinde `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets -- -D warnings` ve `cargo test --locked`
çalıştırılır. GitHub Actions aynı kontrolleri Windows üzerinde çalıştırır.
Davranış testleri özellikler eklendikçe yazılır; grafik arayüz ayrıca elle kontrol edilir.

## Birlikte geliştirme

Başlangıçta tek ajanla ilerlenir. Alt ajan kullanımı kullanıcı talebiyle yapılır.
İşler küçük, gözden geçirilebilir parçalara ayrılır. Teknik kararların nedeni ve
ilgili Rust kavramları geliştirme sırasında kısa açıklamalarla paylaşılır.
