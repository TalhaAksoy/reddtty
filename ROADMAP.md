# ReddTTY — Ürün/Özellik Yol Haritası

Bu dosya, ReddTTY için yapılan ürün/özellik araştırmasının tam kaydıdır.
Tamamlanan maddeler `[x]`, henüz yapılmayanlar `[ ]` olarak işaretlenir.

Son güncelleme: 2026-11-24

---

## Kısa değerlendirme

ReddTTY'nin okuyucu tarafı artık olgun:

- Feed, post detayı, recursive yorumlar, galeri, görsel renderer
- Pagination, loading feedback, responsive layout
- Favorites, recents, config preset'leri, first-run setup, Settings
- Dark/light + 4 Catppuccin tema
- Demo, Reddit OAuth, Old Reddit HTML, Redlib HTML
- 229 test, PTY doğrulaması, stress harness

**Asıl darboğaz artık özellik eksikliği değil, canlı kaynak güvenilirliği.** Reddit'in
public API'si kapanıyor; Redlib instance'ları sık sık 403/429/CAPTCHA/502 veriyor.
Bu yüzden yeni özelliklerde öncelik **yerel, kaynak bağımsız ve offline çalışan**
şeyler olmalı.

---

## P0 — En mantıklı sonraki adımlar

### 1. Config/provider değişimini restart gerektirmeden uygulama

Şu an Settings'ten config değişince dosya doğru yazılıyor, ama provider restart'ta
yeniden kuruluyor. Bu, en görünür UX boşluklarından biri.

**Yapılacak:** Yeni provider'ı çalışırken kur, mevcut nav stack'ini güvenli şekilde
sıfırla, eski in-flight istekleri geçersiz kıl, favorites/recents/theme'i koru.
Hata olursa eski provider'a dönme değil, açık hata ekranı.

- Etki: **Yüksek**
- Efor: **Orta**
- Risk: Orta; request/view id ve image manager durumu dikkat ister.

- [x] Restart gerektirmeyen provider/config değişimi — tamamlandı (2026-11-24): config seçimi provider'ı çalışırken yeniden kuruyor, navigasyonu sıfırlıyor ve eski istekleri yok sayıyor.

### 2. Yerel bookmark / kaydedilenler ve okundu durumu

Reddit'e "save" yazmak mevcut erişim koşullarında gerçekçi değil. Ama **uygulama
içi yerel kaydetme** tamamen mümkün ve çok değerli.

- Postu/comment'i yerel bookmark'a ekle
- "Read later" listesi
- Okundu/okunmadı işareti
- Okunmuşları gizle
- Kaydedilen postun snapshot'ını offline sakla
- Yerel etiket/klasör ve arama
- `saved` olarak provider'a değil, yerel kütüphaneye bağla

- Etki: **Çok yüksek**
- Efor: **Orta**
- Avantaj: Demo, Redlib ve ileride eklenecek local provider'larla aynı çalışır.

- [x] Yerel bookmark / kaydedilenler ve okundu durumu — tamamlandı (2026-11-24): `library.toml`, offline post snapshot'ı, bookmark listesi, okundu/okunmadı durumu.

### 3. Yorum okuma navigasyonunu güçlendir

Mevcut yorum ağacı iyi; ama uzun thread'lerde gezinme hâlâ sınırlı.

- Sonraki/önceki kök yoruma atla
- Parent yoruma çık
- Tüm yorumları collapse/expand et
- Sadece OP yorumlarını göster
- "Sonraki okunmamış yorum"
- Yorum permalink'ini kopyala / tarayıcıda aç
- Seçili yorumun linkini panoya kopyala

- Etki: **Yüksek**
- Efor: **Düşük–Orta**
- Risk: Düşük.

- [x] Yorum navigasyonu (root/parent/collapse all/OP-only/next unread/permalink) — tamamlandı (2026-11-24): `[ ]`, `p`, `z`, `O`, `n`, `y`/`Y` ve root/parent atlamaları.

### 4. Kaynak sağlığı / bağlantı durumu ekranı

Redlib instance'ları değişken. Kullanıcı "uygulama mı bozuk, kaynak mı bozuk?"
ayrımını net görmeli.

Settings veya ayrı bir "Source" ekranında:

- Aktif kaynak ve host
- Son HTTP durumu
- Son başarılı istek zamanı
- Ortalama gecikme
- Son hata kategorisi (`Access denied`, `Rate limited`, `HTML changed`, `Timeout`)
- Cache durumu
- "Şimdi dene" butonu
- "Kaynağı değiştir" (otomatik rotasyon değil, kullanıcı seçimi)

- Etki: **Yüksek**
- Efor: **Düşük–Orta**
- Risk: Düşük.

- [x] Kaynak sağlığı / bağlantı durumu ekranı — tamamlandı (2026-11-24): host, durum, son başarı, gecikme, hata, retry ve kaynak değiştirme.

---

## P1 — Orta vadede güçlü katkı

### 5. Kalıcı yerel cache

Şu an cache memory-only. Redlib yavaş/instabil olduğu için SQLite veya benzeri
kalıcı cache büyük fark yaratır.

- Feed sayfaları TTL'li cache
- Post/comment snapshot
- Görsel metadata cache
- Offline açılışta son feed
- "Cache'ten göster" göstergesi
- Manuel cache temizleme

- Etki: **Yüksek**
- Efor: **Orta–Yüksek**
- Risk: Veri silinmesi/removed post politikası; TTL ve açık "cached" etiketi gerekir.

- [ ] Kalıcı yerel cache

### 6. Yerel filtreler

Reddit hesabına bağlı olmayan, tamamen yerel filtreler:

- Kelime filtresi
- Subreddit mute
- Yazar mute
- Domain filtresi
- Okunmuşları gizle
- NSFW/spoiler varsayılanları
- "Bu başlığı bir daha gösterme"

- Etki: **Orta**
- Efor: **Düşük**
- Risk: Düşük.

- [x] Yerel filtreler — tamamlandı (2026-11-24): hide-read, keyword, subreddit, author ve domain filtreleri; kalıcı ve yerel.

### 7. Çoklu feed / multireddit benzeri akışlar

Örneğin:

```text
r/rust+linux+programming
```

- Birden fazla subreddit'i tek feed'de göster
- Kaydedilmiş aramalar
- Favori feed'leri sabitle
- Farklı feed profilleri

- Etki: **Orta–Yüksek**
- Efor: **Orta**
- Not: Redlib/HTML tarafta bu URL'lerin gerçekten çalıştığı fixture + canlı test ile
  doğrulanmalı.

- [ ] Çoklu feed / multireddit benzeri akışlar

### 8. Komut paleti

Klavye kullanıcıları için keşfedilebilirliği artırır:

```text
:sort new
:theme mocha
:source
:bookmarks
:filter add ...
```

veya `Ctrl+P` ile aranabilir aksiyon listesi.

- Etki: **Orta**
- Efor: **Orta**
- Risk: Düşük.

- [x] Komut paleti — tamamlandı (2026-11-24): `:` veya `Ctrl+P`, yazarken filtreleme ve aksiyon çalıştırma.

### 9. Clipboard ve link aksiyonları

- Post linkini kopyala
- Comment permalink'ini kopyala
- Görsel URL'sini kopyala
- "Yorumları tarayıcıda aç"
- "Subreddit'i tarayıcıda aç"

- Etki: **Orta**
- Efor: **Düşük**
- Platform bağımlılığı: pbcopy / wl-copy / Windows clipboard.

- [ ] Clipboard ve link aksiyonları

### 10. Görsel görüntüleyiciyi geliştir

- Zoom/pan
- Galeri thumbnail şeridi
- Sonraki görseli önceden yükle
- Görseli harici aç / URL kopyala
- Kitty/iTerm2/Sixel için gerçek GUI doğrulaması
- Sixel residue testleri

- Etki: **Orta**
- Efor: **Orta**
- Not: Protokol desteği iddiası ancak gerçek terminal ekran görüntüsüyle doğrulanmalı.

- [ ] Görsel görüntüleyici iyileştirmeleri

---

## P2 — Stratejik / daha büyük işler

### 11. Reddit resmî veri export'unu içe aktarma

Reddit'in kullanıcıya verdiği GDPR/data export dosyalarını yerel provider olarak
okumak.

Bu, 2027 sonrasında da çalışabilecek en güçlü "offline Reddit arşivi" yönü. Yeni
API erişimi gerektirmez; kullanıcının kendi verisidir.

- Etki: **Çok yüksek (stratejik)**
- Efor: **Yüksek**
- Risk: Export formatı değişken; post/comment/media eşleme gerekir.

- [ ] Reddit resmî veri export'unu içe aktarma

### 12. Yerel arşiv + full-text arama

- Kaydedilen post/comment'leri SQLite FTS ile ara
- Markdown/JSON export
- Tarih/subreddit/yazar filtreleri
- Arşivden offline okuma

- Etki: **Yüksek**
- Efor: **Yüksek**

- [ ] Yerel arşiv + full-text arama

### 13. Dağıtım ve paketleme

- GitHub Releases ile macOS/Linux binary
- Homebrew formülü
- AUR PKGBUILD
- Shell completion
- man page
- `cargo install` dışında kolay kurulum

- Etki: **Orta**
- Efor: **Orta**
- Risk: Düşük; kullanıcı tabanını artırır.

- [ ] Dağıtım ve paketleme

### 14. HTML parser dayanıklılığı

Redlib HTML'i sık değişiyor. Şunlar uzun vadede değerli:

- Gerçek HTML snapshot'larından fixture güncelleme
- Content hash / "HTML changed" erken uyarı
- Parser fuzz testleri
- Instance bazlı compatibility matrisi

- Etki: **Orta–Yüksek**
- Efor: **Orta**
- Kullanıcıya görünmez ama bakım maliyetini düşürür.

- [ ] HTML parser dayanıklılığı

### 15. Erişilebilirlik

- `--no-color` / monochrome mod
- Yüksek kontrast teması
- Reduced motion
- Daha büyük hedef alanları / dar terminalde daha iyi yardım

- Etki: **Orta**
- Efor: **Düşük–Orta**

- [ ] Erişilebilirlik

---

## Yapılmaması mantıklı olanlar

Bunlar teknik olarak cazip görünse de mevcut koşullarda doğru yatırım değil:

- **HTML üzerinden vote/save/comment/post yazma** — login ve CSRF gerektirir; erişim
  koşullarına aykırı.
- **Otomatik Redlib instance rotasyonu** — kullanıcıyı habersiz başka sunucuya
  göndermek privacy ve güven açısından kötü.
- **Scraping/proxy/cookie ile engel aşma** — kesinlikle hayır.
- **Devvit'i standalone client sanmak** — Devvit Reddit içinde çalışır; terminal
  uygulamasının yerini almaz.
- **Her tuşta canlı subreddit autocomplete** — gereksiz ağ yükü ve rate-limit riski.
  Ancak opt-in + debounce + cache ile düşünülebilir.
- **Tam video oynatma / web browser** — TUI kapsamı dışında.
- **AI özetleri** — privacy, maliyet ve gerçek veri erişimi sorunları; çekirdek ürün
  değil.

---

## Önerilen sıralama

En mantıklı 5'li:

1. [x] Restart gerektirmeyen provider/config değişimi — tamamlandı.
2. [x] Yerel bookmark + okundu durumu — tamamlandı.
3. [x] Yorum navigasyonu (parent/root/OP/collapse all) — tamamlandı.
4. [x] Kaynak sağlığı ve manuel retry ekranı — tamamlandı.
5. [x] Yerel filtreler + clipboard — tamamlandı (filtreler + permalink kopyalama).

Bundan sonra:

6. [ ] Kalıcı cache / offline mod
7. [ ] Multifeed ve kaydedilmiş aramalar
8. [x] Komut paleti — tamamlandı.
9. [ ] Görsel viewer iyileştirmeleri
10. [ ] Reddit export import / yerel arşiv

## Sonuç

ReddTTY'yi "Reddit'e yazma client'ı" yapmaya çalışmak mevcut erişim koşullarında
mantıklı değil. En güçlü yön **hızlı, klavye-first, yerel-first bir Reddit okuyucu
ve arşivleyici** olmak.

Bu yüzden özellik kararlarında ölçüt şu olmalı:

> "Bu özellik Reddit API'si veya tek bir Redlib instance'ı olmadan da çalışır mı?"

Cevap "evet" ise önceliklendirilmeli.
