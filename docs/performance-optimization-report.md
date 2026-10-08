# OpenChat performans optimizasyonu ve ölçüm raporu

Tarih: 2026-10-09  
Kapsam: Rust servis ve Flutter/Dart uygulamasının mevcut debug/test davranışı

## Sonuç

Ölçümle doğrulanan iki Rust hot path iyileştirildi: provider isteği için bağlam metni token tahmini ve OpenAI uyumlu provider HTTP client edinimi. Sabit bağlam iş yükünde token tahmini p50 süresi %68,28 düştü. Client edinimi aynı debug çalışmasında taze client oluşturma ile havuzlanmış client klonunu karşılaştırınca p50/p95/p99 yaklaşık %99,8 daha düşük çıktı. Keep-alive bağlantı yeniden kullanımı yerel TCP testiyle doğrulandı.

Flutter UI koduna, animasyonlara, transition sürelerine veya görünür davranışa performans amacıyla dokunulmadı. Debug profili için yalnızca açıkça verilen veri kökünü kabul eden ve release modunda devre dışı kalan bir servis başlatma seçeneği eklendi. Tam Flutter DevTools heap/GC/frame profili alınamadığından Flutter performans kazanımı iddiası yoktur.

## Mimari başlangıç noktası

- Uygulama `lib/main.dart` içinden `OpenChatApp`'i başlatıyor.
- Flutter ile Rust arasında FFI veya `flutter_rust_bridge` yok. Flutter, yanındaki `openchat_service.exe` sürecini başlatıyor; JSON-RPC isteklerini stdin'e satır olarak yazıyor ve stdout satırlarını yanıt/olay olarak ayrıştırıyor. Bu nedenle FFI overhead bu mimaride uygulanabilir bir ölçüm değildir. JSON marshalling ve süreç IPC maliyeti bu çalışmada bağımsız ölçülmedi.
- `native/openchat-service` ve `native/openchat-launcher` ayrı Cargo crate'leri; kök Cargo workspace yok. Servis Tokio çok iş parçacıklı runtime kullanıyor ve SQLite depolaması içeriyor.
- Dart tarafındaki açıkça görülen `Isolate.run`, mevcut SQLite dosyasını doğrulayan `OpenChatDatabase.verifyExistingFile` yolunda. Rust tarafında garbage collector yok; bellek yönetimi ownership ve allocator davranışına bağlı.
- Token veya API anahtarı göndermeyen benchmarklarda provider ağı ve model düşünme süresi kullanılmadı. Bu sonuçlar LLM latency veya time-to-first-token sonucu değildir.

## Değişiklikler

1. `native/openchat-service/src/context_compaction.rs`: `request_text_token_estimate` aynı karakter sınıflandırmalarını, ASCII/Unicode sayılarını ve token tahmin kurallarını tek karakter geçişinde topluyor. Mevcut tahmin sonuçlarını koruyan ASCII, Türkçe, boşluklu, yoğun noktalama ve Unicode sınır testleri eklendi.
2. `native/openchat-service/src/openai_compatible.rs`: provider isteklerinde her turda yeni `reqwest::Client` kurmak yerine başarılı client ilk kullanımda `OnceLock` içinde tutuluyor ve ucuz klonla alınıyor. Client kurulumu başarısız olursa hata saklanmıyor; sonraki çağrı yeniden deneyebilir. Kimlik doğrulama başlıkları istek başına oluşturulmaya devam ediyor. İki yerel HTTP isteğinin aynı keep-alive bağlantısını kullandığını doğrulayan test eklendi.
3. `lib/platform/windows/openchat_service_client.dart`: debug çalıştırmasında `OPENCHAT_DEBUG_DATA_ROOT` derleme tanımı verilirse yalnızca Rust servis sürecine `--data-root` aktarılıyor. Tanım verilmezse eski argümansız başlatma korunuyor; release modunda bu tanım kullanılmıyor.
4. `native/openchat-service/src/chatgpt_store/memory/tests.rs`: manuel arşiv benchmark fixture'ına eksik iki bellek ayar tablosu eklendi. Bu, yalnızca test fixture değişikliğidir; üretim şeması değişmedi.
5. `lib/features/chat/presentation/widgets/tool_terminal_view.dart`: mevcut kullanıcı değişikliğinde kullanılmayan bir `theme` yerel değişkeni kaldırıldı. Widget davranışı değişmedi.

Yeni ortak crate, reusable widget veya soyutlama eklenmedi; tekrar eden kod kaldırıldığına dair ölçülmüş bir bulgu yoktu. Büyük çaplı refactor, API sözleşmesi, tool schema, provider yönlendirmesi, streaming semantiği, oturum kalıcılığı veya kullanıcı özellikleri değiştirilmedi.

## Eş koşullu debug mikrobenchmarkları

Rust ölçümleri Windows 11, AMD Ryzen 7 5700X üzerinde Cargo test profiliyle (`unoptimized + debuginfo`) alındı. Release veya profile build alınmadı. p50/p95/p99 sütunları her biri beş tekrarın kendi quantile değerlerinin medyanıdır.

Ortam: Windows 11 Pro build 26200, Flutter 3.47.1, Dart 3.13.1, Rust/Cargo 1.95.0, 47,9 GiB sistem belleği. Bu bellek değeri uygulama RSS veya Dart heap ölçümü değildir.

### İstek bağlamı token tahmini

Sabit JSON istek gövdesindeki metin alanları toplam 364.032 UTF-8 byte İngilizce, Türkçe ve araç benzeri içerik kullandı. Her çalıştırmada 20 ısınma ölçümü ve 200 örnekten oluşan 5 tekrar alındı. Önce/sonra aynı test, veri ve debug modudur.

| Quantile | Önce | Sonra | Süre değişimi |
| --- | ---: | ---: | ---: |
| p50 | 15,285 ms | 4,848 ms | -%68,28 |
| p95 | 17,901 ms | 5,580 ms | -%68,83 |
| p99 | 19,246 ms | 6,076 ms | -%68,43 |

Ölçüm yalnızca yerel bağlam tahmin fonksiyonunu kapsıyor. JSON istek oluşturma, SQLite geçmiş okuma, provider gönderimi veya model yanıtı bu süreye dahil değil.

### HTTP client edinimi

Tek debug çalışmasında eski davranışla eşdeğer `build_client()` ve yeni `client()` yolları eşleştirilerek ölçüldü. Her tekrar 20 ısınma grubu ve 200 işlemli 200 ölçüm grubu içerdi. Aşağıdaki her değer beş tekrar quantile medyanıdır.

| Quantile | Her çağrıda yeni client | Havuzlanmış client klonu | Client edinimindeki değişim |
| --- | ---: | ---: | ---: |
| p50 | 11,578 µs | 19 ns | -%99,84 |
| p95 | 12,318 µs | 21 ns | -%99,83 |
| p99 | 13,698 µs | 26 ns | -%99,81 |

Bu, client kurma/edinme maliyetinin karşılaştırmasıdır; HTTP round trip, TLS handshake veya sağlayıcı latency ölçümü değildir. Keep-alive testi yerel sunucuda iki isteğin tek TCP bağlantısında işlendiğini doğruladı. Farklı provider uçlarının gerçek ağ kazancı bu testten nicel olarak çıkarılamaz.

### Dosya araçları ve arşiv belleği karakterizasyonu

Mevcut manuel araç benchmarkı 501 sentetik test dosyası ve 3.239.014 byte üzerinde 1 ısınma ve 25 örnek kullandı. Bu alanda üretim kodu değiştirilmedi; değerler yalnızca mevcut debug karakterizasyonudur. Önceki ve sonraki çalıştırmalar arasında dosya sistemi sıcak önbelleği farklı olduğundan bunlardan performans kazanımı çıkarılmamalıdır.

| Araç | p50 | p95 | Çıktı |
| --- | ---: | ---: | ---: |
| `list_files` | 19,030 ms | 20,759 ms | 4.248 byte |
| `search_files` | 7,688 ms | 8,549 ms | 5.046 byte |
| Eşleşmesiz `search_files` | 27,950 ms | 28,916 ms | 50 byte |
| `read_file` | 0,517 ms | 0,539 ms | 17.175 byte |
| Geç satırdan `read_file` | 0,363 ms | 0,395 ms | 8.669 byte |
| `get_file_info` | 0,112 ms | 0,125 ms | 54 byte |

Mevcut araç benchmarkı p99 hesaplamıyor; 25 örnekle p99 raporlamak güvenilir olmayacağından değer uydurulmadı.

Arşiv arama benchmarkı 50.000 mesaj ve 500 araç etkinliği üzerinde bir seferde backfill + arama için 468,1 ms, indeksli mesaj/araç aramaları için 24,8 ms ölçtü. İkinci ayrı çalıştırma sırasıyla 464,8 ms ve 27,0 ms verdi. Bu iki tekil gözlem p50/p95/p99 değildir ve bellek büyümesi ölçümü değildir.

## Doğrulama

- `cargo test --manifest-path native/openchat-service/Cargo.toml`: 422 geçti, 11 manuel/benchmark testi `ignored`; ayrıca `process_rpc` entegrasyon testi geçti.
- `cargo fmt --manifest-path native/openchat-service/Cargo.toml -- --check`: geçti.
- `cargo fmt --manifest-path native/openchat-launcher/Cargo.toml -- --check`: geçti.
- `cargo clippy --manifest-path native/openchat-service/Cargo.toml --all-targets --all-features`: exit code 0; mevcut kodda 53 kütüphane ve test hedefinde toplam 65 lint uyarısı var. `-D warnings` biçimi bu mevcut uyarılar yüzünden başarısızdır. Değiştirilen Rust hot path'lerinde yeni Clippy uyarısı görünmedi.
- `flutter analyze`: son çalıştırmada temiz.
- İlk Flutter baseline koşusu 273 testi geçti. Daha sonraki tam koşuda, değişmekte olan kullanıcı UI/test dosyalarında 229 test geçti ve 44 test başarısız oldu. Hata çıktılarında `RenderFlex` alt kenarda 7 px taşma raporlandı; örnekler `test/fidelity_screenshot_test.dart` ve `test/chat_composer_tool_capability_test.dart` içindeydi. Bu ekran/layout değişikliklerine müdahale edilmedi.
- `dart format --output=none --set-exit-if-changed lib test` mevcut çalışma ağacındaki sekiz kullanıcı değişikliği için değişiklik bildirdi: `openchat_page_header.dart`, `outputs_page.dart`, `chat_navigation_rail.dart`, `conversation_pane.dart`, `goal_status_bar.dart`, `tool_permission_card.dart`, `tool_web_search_view.dart` ve `openchat_theme_test.dart`. Bu dosyalara formatter uygulanmadı.
- `git diff --check`: geçti; yalnızca Windows CRLF/LF çalışma kopyası uyarıları görüldü.
- Windows uygulaması yalnızca `flutter run -d windows --debug` ile başlatıldı. Çıktı Debug Windows çalıştırılabilir dosyasını ve Rust `dev` profilini doğruladı. İzole veri kökünde servis SQLite dosyası oluşturdu. Uygulama daha sonra kapatıldı. Release veya profile build alınmadı.

## Ölçülemeyenler ve açık işler

Flutter DevTools ve Dart VM Service başlatıldı; ancak Codex'in erişilebilir in-app browser'ı localhost DevTools sayfasını açmayı reddetti. Bu nedenle gerçek uygulama oturumunda Dart heap ve retained object profili, allocation rate, GC sıklığı/pause süresi, frame build/raster süresi, dropped frame, UI/raster thread kullanımı, shader/image cache veya uzun oturum bellek büyümesi ölçülmedi. Flutter test sonucu bu runtime metriklerinin yerine geçmez.

Agent initialization, provider routing, gerçek stream time-to-first-token/inter-token latency, tool orchestration toplamı, session recovery, retry/backoff, concurrency scaling, process startup p50/p95/p99, CPU, peak RSS ve IPC JSON marshalling/flush overhead'i bu çalışmada ölçülmedi. LLM model/network latency'si yerel harness overhead'inden ayrı tutuldu; dış provider çağrısı yapılmadı.

Başka açık kaynak harness'lerle eşdeğer cihaz, sağlayıcı/model, bağlam boyutu, tool schema ve görev koşullarında çalıştırılmış karşılaştırma yoktur. Eş koşullar sağlanmadığı için harici yüzde veya latency sonucu raporlanmadı.

Statik incelemede mesaj akışı değiştikçe konuşma geçmişinin tekrar eşlenmesi/JSON çözülmesi ve streaming Markdown'ın tekrar işlenmesi takip profiling adayları olarak kaldı. Bunların darboğaz olduğu ölçümle doğrulanmadı; bu nedenle kodları değiştirilmedi. Per-event stdout flush ve diğer araç/streaming yolları da ayrı benchmark gerektiriyor.

Debug izole profil dizini test verisi içeriyor ve workspace dışında bırakıldı. Üretim profili kullanılmadı. Bu rapor debug/test sonuçlarını anlatır ve release performansı hakkında iddia içermez.
