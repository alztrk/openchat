# OpenChat performans optimizasyonu ve ölçüm raporu

Tarih: 2026-10-09  
Kapsam: Rust servis ve Flutter/Dart uygulamasının mevcut debug/test davranışı

## Sonuç

Ölçümle doğrulanan iki Rust hot path iyileştirildi: provider isteği için bağlam metni token tahmini ve OpenAI uyumlu provider HTTP client edinimi. Sabit bağlam iş yükünde token tahmini p50 süresi %68,28 düştü. Client edinimi aynı debug çalışmasında taze client oluşturma ile havuzlanmış client klonunu karşılaştırınca p50/p95/p99 yaklaşık %99,8 daha düşük çıktı. Keep-alive bağlantı yeniden kullanımı yerel TCP testiyle doğrulandı. Flutter tarafında 256 mesajlık geçmiş eşlemesinde p50 `getMessages` süresi %64,61 ve stream'in ilk snapshot süresi %84,81 düştü.

Flutter optimizasyonu mesaj geçmişi eşlemesindeki ölçülmüş CPU/allocation yüküyle sınırlı kaldı; animasyonlara, transition sürelerine, easing değerlerine ve görsel tasarıma dokunulmadı. Ayrı bir mevcut Windows debug oturumunda Dart VM Service üzerinden heap ve tarihsel timeline verisi de toplandı. Bu runtime snapshot'ı kontrollü bir kullanıcı iş akışı olmadığından değişiklik öncesi/sonrası render veya GC kazanımı sayılmaz.

## Mimari başlangıç noktası

- Uygulama `lib/main.dart` içinden `OpenChatApp`'i başlatıyor.
- Flutter ile Rust arasında FFI veya `flutter_rust_bridge` yok. Flutter, yanındaki `openchat_service.exe` sürecini başlatıyor; JSON-RPC isteklerini stdin'e satır olarak yazıyor ve stdout satırlarını yanıt/olay olarak ayrıştırıyor. Bu nedenle FFI overhead bu mimaride uygulanabilir bir ölçüm değildir. `system.health` için uçtan uca servis IPC round-trip'i ölçüldü; JSON encode/decode, pipe, parse, dispatch ve flush maliyetleri kendi aralarında ayrıştırılmadı.
- `native/openchat-service` ve `native/openchat-launcher` ayrı Cargo crate'leri; kök Cargo workspace yok. Servis Tokio çok iş parçacıklı runtime kullanıyor ve SQLite depolaması içeriyor.
- Dart tarafındaki açıkça görülen `Isolate.run`, mevcut SQLite dosyasını doğrulayan `OpenChatDatabase.verifyExistingFile` yolunda. Rust tarafında garbage collector yok; bellek yönetimi ownership ve allocator davranışına bağlı.
- Token veya API anahtarı göndermeyen benchmarklarda provider ağı ve model düşünme süresi kullanılmadı. Bu sonuçlar LLM latency veya time-to-first-token sonucu değildir.

## Değişiklikler

1. `native/openchat-service/src/context_compaction.rs`: `request_text_token_estimate` aynı karakter sınıflandırmalarını, ASCII/Unicode sayılarını ve token tahmin kurallarını tek karakter geçişinde topluyor. Mevcut tahmin sonuçlarını koruyan ASCII, Türkçe, boşluklu, yoğun noktalama ve Unicode sınır testleri eklendi.
2. `native/openchat-service/src/openai_compatible.rs`: provider isteklerinde her turda yeni `reqwest::Client` kurmak yerine başarılı client ilk kullanımda `OnceLock` içinde tutuluyor ve ucuz klonla alınıyor. Client kurulumu başarısız olursa hata saklanmıyor; sonraki çağrı yeniden deneyebilir. Kimlik doğrulama başlıkları istek başına oluşturulmaya devam ediyor. İki yerel HTTP isteğinin aynı keep-alive bağlantısını kullandığını doğrulayan test eklendi.
3. `lib/platform/windows/openchat_service_client.dart`: debug çalıştırmasında `OPENCHAT_DEBUG_DATA_ROOT` derleme tanımı verilirse yalnızca Rust servis sürecine `--data-root` aktarılıyor. Tanım verilmezse eski argümansız başlatma korunuyor; release modunda bu tanım kullanılmıyor.
4. `native/openchat-service/src/chatgpt_store/memory/tests.rs`: manuel arşiv benchmark fixture'ına eksik iki bellek ayar tablosu eklendi. Bu, yalnızca test fixture değişikliğidir; üretim şeması değişmedi.
5. `lib/features/chat/presentation/widgets/tool_terminal_view.dart`: mevcut kullanıcı değişikliğinde kullanılmayan bir `theme` yerel değişkeni kaldırıldı. Widget davranışı değişmedi.
6. `lib/features/chat/data/chat_repository.dart`: mesaj satırları tek geçişte Dart modellerine eşleniyor. Ek içermeyen konuşmalarda her mesaj için async wrapper/Future üretmeden sabit uzunluklu sonuç dönülüyor. Ek içeren konuşmalarda dosya okumaları paralel kalıyor; eksik dosyaların unavailable işaretlenmesi ve okuma hatalarının iletilmesi korunuyor.
7. `native/openchat-service/src/tools/tests.rs`: manuel araç benchmarkı p99 ve JSON serialization quantile'larını ayrı ölçüyor; üretim araç davranışı değiştirilmedi.

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

### Mevcut Windows debug oturumu: Dart heap, GC timeline, frame ve süreç belleği

Flutter 3.47.1 / Dart 3.13.1 Windows debug-JIT oturumunun Dart VM Service API'si doğrudan kullanıldı. Bu, Codex UI ile erişilemeyen DevTools arayüzünün yerine VM Service'in desteklenen heap ve timeline API'lerini kullanır. Profil sırasında zaten çalışmakta olan kullanıcı uygulaması durdurulmadı; uygulama `build/openchat-dev-data` izole veri kökünü kullanıyordu. Oturum öncesi kullanıcı etkileşimleri, açılış zamanı ve iş yükü denetlenmedi. Aşağıdaki değerler tanımlı benchmark iş yüküne ait değildir.

VM iki isolate bildirdi: `main` ve Drift worker. `getMemoryUsage` iki isolate için de aynı heap sayısını verdi; değer isolate grubunun ortak heap'idir ve iki kere toplanmamalıdır. `getAllocationProfile(gc: true)` çağrısı öncesi heap 179.100.176 byte (179,1 MB / 170,8 MiB), sonrası 176.312.544 byte (176,3 MB / 168,1 MiB) ölçüldü; tek istekte 2.787.632 byte (yaklaşık 2,7 MiB, %1,56) daha az heap raporlandı. Bu tek GC talebi gözlemidir, optimizasyon kazanımı veya leak doğrulaması değildir. Son profilde 795.397 canlı instance sayıldı. En büyük sınıflar `_TwoByteString` 50,9 MB, `_OneByteString` 36,8 MB, `_List` 22,9 MB, `Instructions` 11,4 MB, `Function` 10,0 MB, `_Uint32List` 8,9 MB, `ICData` 5,0 MB ve `Code` 3,6 MB idi. İlk sekiz sınıf raporlanan canlı byte'ların %84,8'ini oluşturdu; debug JIT kodu ve runtime sınıfları da dahil olduğundan bu dağılım tek başına uygulama veri sızıntısı göstermez.

Allocation profile'ın birikim başlangıcı kesin olarak bilinmiyor; bu snapshot'tan allocation throughput, nesne yaşam süresi veya fragmentasyon çıkarılmadı. Kısa VM Service aboneliğinde dört `MarkSweep` olayı görüldü; profil isteğinin GC etkisini ayırmadığı ve gözlem süresi/iş yükü kontrollü olmadığı için doğal GC sıklığı olarak yorumlanamaz. Dart VM'in bildirdiği heap kapasitesi yaklaşık 188 MB idi. Ayrı young/old heap boyutları bu kayıtta alınmadı.

VM timeline yaklaşık 452,4 saniyeye ve 9.583 tarihsel olaya yayıldı. GC süreleri `B`/`E` span çiftlerinden eşleştirildi:

| Timeline span | Örnek sayısı | p50 | p95 | Aralık |
| --- | ---: | ---: | ---: | ---: |
| `CollectNewGeneration` | 18 | 0,377 ms | 4,201 ms | 0,106–4,410 ms |
| `CollectOldGeneration` | 19 | 1,796 ms | 38,460 ms | 0,195–40,815 ms |
| Dart `Frame` | 17 | 32,313 ms | 119,913 ms | 1,499–675,066 ms |
| `GPURasterizer::Draw` | 16 | 2,653 ms | 15,904 ms | 1,781–40,764 ms |

Örnek sayıları 100'ün altında olduğundan p99 verilmedi. GC değerleri collection span süresidir; timeline'daki toplam koleksiyon aralığı stop-the-world pause süresini tek başına ayırmaz. Frame/raster örnekleri tarihsel debug timeline'ından, bilinmeyen açılış ve kullanıcı etkileşimlerini de içerebilir. Kontrollü render workload'u, gerçek dropped-frame sayısı, widget rebuild sayısı veya UI/raster thread utilization ölçülmedi. Bu süreler gerçek etkileşim performansı ya da değişiklik öncesi/sonrası kıyası değildir.

Mevcut UI ve Rust servis süreçleri iki ayrı 10 saniyelik gözlemde örneklendi. UI CPU kullanımı bir çekirdeğe göre gözlemler arasında %0,16 ve %0,94; son anlık görüntüde UI working set 442.839.040 byte (422,3 MiB), private bytes 548.675.584 byte (523,3 MiB), süreç ömrü working-set tepesi 467.566.592 byte (445,9 MiB) idi. Rust sidecar son görüntüde %0 CPU, 33.804.288 byte (32,2 MiB) working set ve 13.123.584 byte private bytes gösterdi; süreç tepe working set'i 94.121.984 byte (89,8 MiB) idi. CPU örnekleri boşta/denetlenmemiş kullanıcı workload'unda alındı; uygulama kapasitesini veya peak task yükünü temsil etmez.

Flutter debug araç zinciri dahil önceki süreç ağacı working set toplamı yaklaşık 1.949,4 MiB (1,90 GiB) idi. Bunun içinde `dartaotruntime` frontend server tek başına yaklaşık 1,12 GiB working set, Dart Development Service yaklaşık 183,8 MiB, debug VM yaklaşık 151,7 MiB tutuyordu. Bu araç süreçleri UI uygulaması ya da release uygulaması RSS'si değildir. Süreçlerin peak working set değerleri farklı zamanlarda oluştuğundan toplanmadı. Windows raporundaki working set/private bytes değerleri RSS/heap profilleriyle aynı ölçüm değildir.

#### Debug bellek tüketiminin güncel süreçlere göre ayrımı

2026-10-09 tarihli yeni bir süreç örneklemesinde Flutter debug araçları ve uygulama süreçlerinin working set toplamı 1.956 MiB (1,91 GiB) oldu:

| Süreç | Working set | Private bytes | Yorum |
| --- | ---: | ---: | --- |
| Dart `frontend_server` (`dartaotruntime`) | 1.149,9 MiB | 1.273,7 MiB | Incremental frontend compiler; süreç argümanlarında `--incremental`, `--track-widget-creation` ve `--enable-asserts` doğrulandı |
| Dart DevTools | 183,9 MiB | 199,7 MiB | Geliştirme/profil aracı |
| Flutter tool'un Dart VM ana süreci | 151,9 MiB | 143,5 MiB | Debug çalıştırma ve hot reload desteği |
| Flutter tool süreci | 9,3 MiB | 1,7 MiB | Derleme/çalıştırma kontrolü |
| OpenChat UI süreci | 428,8 MiB | 519,0 MiB | Flutter engine, Dart debug VM ve uygulama heap'i dahil |
| Rust sidecar | 32,2 MiB | 12,5 MiB | Servis süreci |

Toplamın yaklaşık %76'sı (1.495 MiB) uygulama dışındaki Flutter derleme/debug araçlarıdır. En büyük tekil kaynak incremental `frontend_server` sürecidir. Flutter debug modu hızlı geliştirme, JIT ve hot reload için yapılandırılır; uygulama çalıştırma ve bellek verimliliği için optimize edilmez ([Flutter build modes](https://docs.flutter.dev/testing/build-modes), [Flutter UI performance](https://docs.flutter.dev/perf/ui-performance)). Bu profil otonom bir memory leak kanıtlamaz; yalnızca açık kalan debug oturumunda compiler state'in yüksek bellekte tutulduğunu gösterir.

`frontend_server`, OpenChat UI süreci ve Rust sidecar için yapılan ikinci 10 saniyelik boşta örneklemesinde working set ile private bytes değerleri ölçüm çözünürlüğünde değişmedi. Bu kısa süreli kararlılık uzun oturum büyümesini/leak'i dışlamaz.

Aynı gün yapılan ek süreç örneğinde UI 428,9 MiB working set / 520,0 MiB private bytes, Rust sidecar 32,2 / 12,5 MiB, `frontend_server` 1.149,9 / 1.273,7 MiB ölçüldü. Bu tekrar, uygulamanın yaklaşık 429 MiB boşta kullanımının tek bir kısa örneğe özgü olmadığını gösteriyor; tek başına artış/leak kanıtı değildir.

Bu bellek düzeltmesi sırasında, 2026-10-09 12:04:12–12:04:22 arasında açık debug uygulaması kapatılmadan üç ek süreç örneği alındı. UI working set değerleri 464,9 / 464,9 / 463,9 MiB, private bytes 577,5 / 577,5 / 576,5 MiB idi; Rust sidecar 21,0 / 6,7 MiB civarında sabit kaldı. Bu örneklerde kullanıcı iş yükü denetlenmedi ve kod değişikliğinin hot reload edilip edilmediği bellek kıyası için doğrulanmadı. Önceki 428,9 MiB örnekle doğrudan karşılaştırılamaz; artış nedeni belirlenmiş değildir ve bu kod değişikliğine atfedilemez.

Aktif sidecar'ın gösterdiği izole debug veri kökü içerik dosyaları okunmadan yalnızca dosya boyutlarıyla incelendi: SQLite, WAL/SHM ve log dahil dört dosyanın toplamı 1,06 MiB. Veritabanı read-only açılarak yapılan metadata sorgusunda `messages` ve `conversations` tablolarının ikisi de sıfır satır gösterdi; `messages.content` toplam karakter uzunluğu da sıfırdı. Büyük bir saklanmış sohbet veritabanı veya bu oturumun konuşma geçmişi heap büyüklüğünü açıklamıyor. Kodda `watchMessages` seçili konuşmanın bütün satırlarını `LIMIT` olmadan yükleyip model listesinde tutuyor; `ConversationPane` görünür widget'ları `SliverChildBuilderDelegate` ile tembel oluşturuyor ([`chat_repository.dart`](../lib/features/chat/data/chat_repository.dart), [`conversation_pane.dart`](../lib/features/chat/presentation/widgets/conversation_pane.dart)). Bu düzen çok büyük tek bir konuşmada model belleğini artırabilir, ancak mevcut boş debug veritabanı bunu bu ölçümün nedeni yapmıyor.

GC sonrası VM sınıf profilinde `_TwoByteString` ve `_OneByteString` toplamı yaklaşık 87,7 MB, `_List` 22,9 MB; `Instructions`, `Function`, `ICData` ve `Code` debug/JIT runtime sınıfları toplamı yaklaşık 30 MB idi. String sınıflarının heap'te büyük pay tuttuğu doğrulandı, ancak retained-path snapshot alınmadığı için bunların ne kadarının sohbet metni, dönüştürülmüş Markdown, framework/debug metadata veya başka uygulama durumundan geldiği ayrıştırılamıyor. Doğrudan retained-path VM Service sorgusu yerel politika tarafından çalıştırılmadan engellendi; URI/token çıktılanmadı ve bu sınırı aşmaya çalışılmadı. Mevcut ölçümden belirli bir widget/cache'i suçlamak veya leak ilan etmek doğru olmaz.

Kod denetiminde ek iş yükünde büyüyebilecek iki bellek yolu bulundu. Görsel önizleme yolu için ölçülebilir bir decode sınırı eklendi; Markdown tutulumunun bu boş oturumdaki payı hâlâ ölçülemedi:

- Sohbet eki küçük önizlemesi artık `ResizeImagePolicy.fit` ile widget boyutunu cihaz piksel oranı ve 3× decode ayrıntı payıyla sınırlandırıyor; tam boyutlu kaynak, kullanıcı önizlemeyi açana kadar yüklenmiyor ve dialog kapandıktan sonra kaynak cache girdisi evict ediliyor ([`chat_attachment_gallery.dart`](../lib/features/chat/presentation/widgets/chat_attachment_gallery.dart)). 220×120 mantıksal piksel kutu ve 2× DPR için decode sınırı 1320×720 pikseldir. 3840×2160 kaynak bu sınıra 16:9 oranını koruyarak 1280×720 piksel olarak sığar: RGBA 4 byte/piksel hesabıyla önizleme yaklaşık 3.686.400 byte, doğal boyutlu bitmap ise 33.177.600 byte olur; tek thumbnail cache girdisi için hesaplanan fark yaklaşık %88,89'dur. Bu boyut hesabıdır, uygulama RSS veya canlı cache üzerinde ölçülmüş kazanç değildir; kodlanmış attachment byte'ları modelde tutulmaya devam eder. Ürün sınırları ek başına en fazla 10 MiB kodlanmış görsel, en fazla üç görsel ve tüm ekler için 14 MiB belirliyor ([`chat_attachment.dart`](../lib/features/chat/domain/chat_attachment.dart)). Kodlanmış boyut çözülmüş piksel belleğini sınırlamaz; Flutter belgeleri 4K görselin 30 MB'tan fazla kullanabileceğini ve decode boyutunun düşürülebileceğini doğruluyor ([Image memory usage](https://api.flutter.dev/flutter/widgets/Image-class.html)). Bu değişiklik attachment galerisi görüntülenirken bellek tepesini azaltmayı hedefler; boş oturumdaki yaklaşık 429 MiB idle kullanımı açıklamaz.
- Markdown'lı assistant mesajı görünürken `_AssistantResponseContentState`, gelen içerik dizgesine ek olarak sanitize edilmiş HTML dizgesini de tutuyor ve `HtmlWidget`'ı cache etkin olarak kuruyor ([`assistant_message.dart`](../lib/features/chat/presentation/widgets/assistant_message.dart)). Uzun/çok sayıda Markdown mesajında bu dönüşüm ek string ve widget parse state'i tutabilir; ancak mevcut veritabanında mesaj yok, görünür widget retained path'i alınamadı ve bu yolun bu heap'teki payı ölçülmedi.
- Sohbet ekranı, kaydedilmiş yanıtların yalnızca konuşma/mesaj kimliklerini kullanmasına rağmen başlangıçta `watchSavedOutputs()` üzerinden başlık ve yanıt gövdelerinin tamamını okuyordu. Bu sorgu bütün kaydedilmiş yanıt metinlerini gereksiz yere decode edip kısa ömürlü Dart nesneleri oluşturabiliyordu; veri miktarı büyüdükçe açılış ve sonraki tablo güncellemelerindeki tahsis artıyordu. `ChatRepository.watchSavedOutputMessageKeys()` artık yalnızca iki kimlik sütununu seçiyor ve sohbet ekranı kaydedilme işaretleri için bu akışı kullanıyor; tam `watchSavedOutputs()` akışı, içerik gerektiren Kaydedilen Çıktılar sayfasında değişmeden kaldı ([`chat_repository.dart`](../lib/features/chat/data/chat_repository.dart), [`chat_screen.dart`](../lib/features/chat/presentation/chat_screen.dart)). Bu kaynak düzeltmesi sorgudan okunan gövde/başlık byte'larını ortadan kaldırır; mevcut kullanıcı verisi okunmadı ve uygulama RSS/Dart heap önce-sonra ölçümü alınmadı, dolayısıyla yüzde kazanım iddia edilmiyor.

Kodda uygulamaya özel `ImageCache` sınırı ayarı bulunmadı. Flutter'ın varsayılan LRU image cache'i en çok 1.000 giriş ve 100 MB için ayarlanır; bu kapasite üst sınırıdır, mevcut kullanım ölçümü değildir ve cache'teki canlı referanslar ayrıca izlenir ([`ImageCache`](https://api.flutter.dev/flutter/painting/ImageCache-class.html)). O anki byte/adet değeri DevTools Memory veya `ImageCache.currentSizeBytes` ile alınamadı. Windows çalışma kümesi 428,9 MiB ile GC sonrası Dart heap 176,3 MB aynı kapsamı ölçmez; aradaki farkı bu heap histogramından native engine, decoded image, raster cache, VM ve eşlenen sayfalar arasında paylaştıramıyoruz. DevTools Memory görünümü Dart heap, native bellek, raster cache ve RSS'yi ayrı izleyebiliyor ([DevTools Memory view](https://docs.flutter.dev/tools/devtools/memory)); eldeki retained-path ve image-cache erişim engeli nedeniyle bu ayrıştırma tamamlanamadı.

### Flutter konuşma geçmişi eşleme

`test/chat_repository_performance_test.dart` yalnızca `OPENCHAT_RUN_PERF_BENCHMARK=true` ile çalışan tekrarlanabilir bir debug mikrobenchmark içeriyor. `NativeDatabase.memory()` üzerinde 256 sabit mesaj, 12 warm-up çifti ve 100 ölçüm çifti kullanıldı. Her mesajın gövdesi sabit metin, ek/citation/reasoning/tool metadata alanları boş. Her metrik için üç bağımsız test çalıştırması yapıldı; tabloda bu üç çalıştırmadaki p50/p95/p99 değerlerinin medyanı var. DB kurulumu ve warm-up ölçüm dışında; `getMessages` ile stream'in ilk snapshot'ı aynı mesaj sırasını ve sayısını doğruluyor.

| Yol | Quantile | Önce | Sonra | Süre değişimi |
| --- | --- | ---: | ---: | ---: |
| `getMessages` | p50 | 3,6045 ms | 1,2755 ms | -%64,61 |
| `getMessages` | p95 | 4,2411 ms | 1,8396 ms | -%56,62 |
| `getMessages` | p99 | 4,4658 ms | 1,9531 ms | -%56,27 |
| `watchMessages(...).first` | p50 | 3,6710 ms | 0,5575 ms | -%84,81 |
| `watchMessages(...).first` | p95 | 4,1953 ms | 0,9721 ms | -%76,83 |
| `watchMessages(...).first` | p99 | 4,5292 ms | 1,0231 ms | -%77,41 |

Benchmark ayrıca sıralı ham Drift `select.get()` ölçtü: p50 üç çalıştırma medyanı 1,1905 ms'den 1,0920 ms'ye değişti. SQL sorgusu değiştirilmedi; p99 ham sorgu değeri çalıştırmalar arasında yüksek oynaklık gösterdi. Bu nedenle ana kazanım, bu fixture'da mesaj eşleme yolundaki per-message async/Future yüküyle uyumludur; gerçek uygulama UI frame süresi veya model/provider süresi değildir. Benchmark test runner'ın debug/JIT koşulunu ölçer; gerçek Windows uygulamasının RSS, Dart heap, GC veya render-thread profilini vermez. Release davranışı hakkında sonuç çıkarılmadı.

### Rust servis başlangıcı ve Flutter→Rust IPC sınırı

`native/openchat-service/tests/process_rpc.rs` içindeki manuel benchmark, yalnızca Cargo test profiliyle derlenmiş debug servis sürecini kullanıyor. Her tam çalıştırmada 100 yeni veri kökünde process spawn'dan ilk `system.health` yanıtına kadar soğuk başlangıç, aynı backend SQLite kökü tekrar kullanılarak 100 sıcak servis başlangıcı ve sıcak süreçte 20 warm-up sonrası 200 sıralı `system.health` round-trip'i ölçüldü. Her tam benchmark üç kez çalıştırıldı; tabloda bağımsız çalıştırmaların p50/p95/p99 medyanı yer alıyor.

| Ölçüm | Quantile | Süre |
| --- | --- | ---: |
| Soğuk servis başlangıcı, process spawn → ilk `system.health` | p50 | 28,662 ms |
| Soğuk servis başlangıcı, process spawn → ilk `system.health` | p95 | 32,112 ms |
| Soğuk servis başlangıcı, process spawn → ilk `system.health` | p99 | 34,659 ms |
| Sıcak servis başlangıcı, process spawn → ilk `system.health` | p50 | 18,124 ms |
| Sıcak servis başlangıcı, process spawn → ilk `system.health` | p95 | 19,985 ms |
| Sıcak servis başlangıcı, process spawn → ilk `system.health` | p99 | 22,836 ms |
| Sıcak `system.health` NDJSON round-trip | p50 | 216,4 µs |
| Sıcak `system.health` NDJSON round-trip | p95 | 254,8 µs |
| Sıcak `system.health` NDJSON round-trip | p99 | 317,2 µs |

Round-trip ölçümü test istemcisindeki `serde_json` encode/decode, OS pipe, servis satır okuma/JSON parse, Tokio request task'i, dispatch ve `EventSink` JSON yazma/flush işlemlerini birlikte kapsar. Provider, model, tool executor, Flutter/Dart serializer'ı veya uygulamanın açılış ekranı bu ölçümde yoktur; FFI kullanılmadığı için FFI overhead yine uygulanabilir değildir. Soğuk/sıcak başlangıç kıyası aynı debug koşulundaki farklı storage durumlarını gösterir, önce/sonra optimizasyon kıyası değildir.

Rust-only fixture Flutter/Drift'in ortak SQLite dosyasındaki `conversations` tablosunu oluşturmuyor. Bu yüzden benchmark `system.initialize`/Drift migration adımına değil, servisin `system.health` ile hazır yanıtına kadar olan sınıra dayanıyor. `system.initialize` süresini uygulama başlangıcı olarak raporlamak mevcut fixture ile doğrulanamaz.

### Dosya araçları ve araç çıktısı serileştirme karakterizasyonu

Manuel debug test benchmarkı 501 sabit fixture dosyası (3.239.014 byte), araç başına 10 warm-up ve 500 ölçüm kullanıyor. Üç bağımsız çalıştırmanın her quantile'ı ayrı hesaplandı; aşağıdaki değerler çalıştırma quantile'larının medyanı. Operasyon süresi ile `serde_json::to_vec` sonucu üretme/ayırma süresi ayrı kronometrelerle ölçüldü; JSON serialization operasyon süresine dahil değil. p99 her koşuda beş üst uç örneğine dayanıyor ve üç koşu arasında bazı tarama yollarında geniş oynadı. Bu nedenle tablo mevcut debug karakterizasyonudur, garanti edilen üst sınır değildir.

| Araç yolu | Operasyon p50 (ms) | Operasyon p95 (ms) | Operasyon p99 (ms) | JSON p50 (µs) | JSON p95 (µs) | JSON p99 (µs) | Çıktı (byte) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `list_files` | 19,499 | 25,483 | 31,361 | 174,4 | 280,2 | 298,8 | 4.248 |
| `list_files_direct_no_app_cache` | 23,998 | 30,058 | 33,535 | 217,5 | 313,1 | 373,9 | 4.248 |
| `search_files` | 11,789 | 15,127 | 16,822 | 278,0 | 338,0 | 475,0 | 5.046 |
| `search_files_direct_no_app_cache` | 10,997 | 14,367 | 17,403 | 272,4 | 331,9 | 410,0 | 5.046 |
| Eşleşmesiz `search_files` | 38,733 | 44,614 | 47,693 | 13,3 | 18,1 | 21,5 | 50 |
| `read_file` | 0,782 | 0,900 | 0,957 | 904,1 | 966,6 | 986,6 | 17.175 |
| Geç satırdan `read_file` | 0,550 | 0,643 | 0,736 | 453,4 | 488,3 | 565,7 | 8.669 |
| `get_file_info` | 0,110 | 0,201 | 0,221 | 4,0 | 6,6 | 7,5 | 54 |

Üç çalıştırmada operasyon p50 aralıkları `list_files` için 18,823–24,530 ms, `search_files` için 10,506–12,800 ms, eşleşmesiz arama için 30,649–38,957 ms oldu. p99 aralıkları aynı sırayla 22,525–36,245 ms, 15,026–43,554 ms ve 43,940–96,343 ms idi; yüksek uçlardaki bu saçılım dış sistem yükü ve yerel dosya sistemi etkilerinin ayrıştırılmadığını gösteriyor. Üretim araç kodu değiştirilmedi; bu ölçümlerden optimizasyon kazanımı çıkarılmadı.

Arşiv arama benchmarkı 50.000 mesaj ve 500 araç etkinliği üzerinde bir seferde backfill + arama için 468,1 ms, indeksli mesaj/araç aramaları için 24,8 ms ölçtü. İkinci ayrı çalıştırma sırasıyla 464,8 ms ve 27,0 ms verdi. Bu iki tekil gözlem p50/p95/p99 değildir ve bellek büyümesi ölçümü değildir.

## Doğrulama

- Güncel tam `cargo test --manifest-path native/openchat-service/Cargo.toml`: 426 unit test geçti, 11 `ignored`; `process_rpc` entegrasyon testi geçti, IPC benchmark varsayılan suite'de `ignored`.
- `cargo test --manifest-path native/openchat-service/Cargo.toml --lib tools::tests::benchmark_tool_latency_and_output_size -- --ignored --nocapture`: güncellenmiş 500 örnekli araç/JSON benchmarkı üç bağımsız çalıştırmanın tümünde geçti.
- `cargo test --manifest-path native/openchat-service/Cargo.toml --test process_rpc benchmarks_debug_service_startup_and_health_rpc -- --ignored --nocapture`: üç bağımsız ölçüm çalıştırmasının tümü geçti.
- `cargo fmt --manifest-path native/openchat-service/Cargo.toml -- --check`: geçti.
- `cargo fmt --manifest-path native/openchat-launcher/Cargo.toml -- --check`: geçti.
- `cargo clippy --manifest-path native/openchat-service/Cargo.toml --all-targets --all-features`: exit code 0; mevcut kodda 53 kütüphane ve test hedefinde toplam 65 lint uyarısı var. `-D warnings` biçimi bu mevcut uyarılar yüzünden başarısızdır. Değiştirilen Rust hot path'lerinde yeni Clippy uyarısı görünmedi.
- `flutter analyze --no-pub`: temiz.
- `flutter analyze --no-pub lib/features/chat/presentation/widgets/chat_attachment_gallery.dart test/chat_attachment_gallery_memory_test.dart`: değiştirilen dosyalar temiz.
- `flutter test --no-pub test/chat_attachment_gallery_memory_test.dart`: geçti; thumbnail decode sınırını, aspect-preserving policy'yi, tam çözünürlüklü preview kaynağını doğruladı. Test gerçek image-cache byte sayısını veya süreç RSS'sini ölçmez.
- `flutter test --no-pub test/chat_composer_tool_capability_test.dart`: 10 test geçti.
- `flutter test --no-pub test/chat_repository_test.dart`: 16 test geçti; attachment kaydetme/okuma/kopyalama ve geçmiş davranışları, ayrıca kaydedilmiş yanıt kimlik akışı ekleme/silme ile tam içerik snapshot akışı doğrulandı.
- `flutter test --no-pub test/workspaces_outputs_page_test.dart`: 7 test geçti; kaydedilmiş yanıtları gösterme, açma ve kaldırma akışı ile ilgili golden testler dahil.
- `flutter analyze --no-pub lib/features/chat/data/chat_repository.dart lib/features/chat/presentation/chat_screen.dart test/chat_repository_test.dart`: temiz.
- `dart format --output=none --set-exit-if-changed lib/features/chat/data/chat_repository.dart lib/features/chat/presentation/chat_screen.dart test/chat_repository_test.dart`: biçim farkı yok.
- `flutter test --no-pub --dart-define=OPENCHAT_RUN_PERF_BENCHMARK=true test/chat_repository_performance_test.dart`: önce ve sonra üçer çalıştırmada geçti; her çalıştırma 100 örnek/işlem ve 12 warm-up çifti kullandı.
- Güncel tam `flutter test --no-pub --reporter=failures-only`: 282 geçti, 1 atlandı, başarısız yok. `test/fidelity_screenshot_test.dart` golden testleri de geçti. Bu optimizasyon çalışmasında golden dosyaları değiştirilmedi.
- Önceki tam Flutter koşularında kullanıcı çalışma ağacındaki UI değişiklikleri nedeniyle farklı sonuçlar alınmıştı; son analyzer ve tam test çalıştırması o sıradaki güncel ağaç içindir.
- `dart format --output=none --set-exit-if-changed lib test` altı kullanıcı çalışma ağacı dosyası için biçim farkı bildirdi: `openchat_page_header.dart`, `outputs_page.dart`, `goal_status_bar.dart`, `tool_permission_card.dart`, `tool_web_search_view.dart` ve `openchat_theme_test.dart`. Bu dosyalara formatter uygulanmadı; bu görevde eklediğim Flutter benchmark testi format kontrolünden geçti.
- `git diff --check`: geçti; yalnızca Windows CRLF/LF çalışma kopyası uyarıları görüldü.
- Windows bundle yalnızca `flutter build windows --debug --no-pub` ile derlendi; Debug executable ve `kernel_blob.bin` üretimi doğrulandı. Profil sırasında önceden çalışan mevcut debug uygulaması korundu; ona attach etmek için açtığımız ayrı geçici `flutter attach` süreci bağlantı kuramadı ve yalnızca bu geçici süreç durduruldu. Rust testleri Cargo test/debug profili kullandı. Release veya Flutter profile build alınmadı.

## Ölçülemeyenler ve açık işler

Mevcut debug uygulamasından heap snapshot'ı ve tarihsel GC/frame/raster span'ları alındı; yukarıdaki veri sınırlı profil gözlemidir. Kontrollü workload altında allocation rate, retained object büyümesi/leak, GC'nin gerçek pause süresi, generation bazlı heap davranışı, fragmentation, dropped frame, rebuild sayısı, UI/raster thread kullanımı, shader/image cache veya uzun oturum bellek eğrisi hâlâ ölçülmedi. Flutter DevTools localhost UI'sı Codex içi browser'dan açılamadı; profiling için doğrudan Dart VM Service API kullanıldı. Flutter test sonucu bu runtime metriklerinin yerine geçmez.

Agent initialization, provider routing, gerçek stream time-to-first-token/inter-token latency, tool orchestration toplamı, session recovery, retry/backoff, concurrency scaling ve tam Flutter uygulama startup'ı bu çalışmada ölçülmedi. CPU ve süreç belleği yalnızca yukarıda belirtilen mevcut boşta/denetlenmemiş debug oturumunda örneklendi; kontrollü peak RSS veya uygulama startup kıyası değildir. Rust servisi için `system.health` round-trip p50/p95/p99 ölçümü vardır; IPC içindeki JSON marshalling, parse, dispatch ve flush payları ayrı ölçülmedi. LLM model/network latency'si yerel harness overhead'inden ayrı tutuldu; dış provider çağrısı yapılmadı.

Başka açık kaynak harness'lerle eşdeğer cihaz, sağlayıcı/model, bağlam boyutu, tool schema ve görev koşullarında çalıştırılmış karşılaştırma yoktur. Eş koşullar sağlanmadığı için harici yüzde veya latency sonucu raporlanmadı.

Statik incelemede mesaj akışı değiştikçe konuşma geçmişinin tekrar eşlenmesi/JSON çözülmesi ve streaming Markdown'ın tekrar işlenmesi takip profiling adayları olarak kaldı. Bunların darboğaz olduğu ölçümle doğrulanmadı; bu nedenle kodları değiştirilmedi. Per-event stdout flush ve diğer araç/streaming yolları da ayrı benchmark gerektiriyor.

Debug izole profil dizini test verisi içeriyor ve workspace dışında bırakıldı. Üretim profili kullanılmadı. Bu rapor debug/test sonuçlarını anlatır ve release performansı hakkında iddia içermez.
