# Barracuda Windows Olay Zaman Çizelgesi

[Windows EXE'yi indir](https://github.com/musaoguzdeniz-cc/barracuda-event-timeline/releases/latest/download/BarracudaEventTimeline.exe)

Windows 10/11 için yerel olay günlüğü okuyucusu. **Canlı günlükleri oku** son yedi günde System, Application ve Windows Update günlüklerindeki ilgili olayları getirir. **.evtx dosyalarını aç** ile başka bir bilgisayardan alınmış System ve Application günlükleri birlikte incelenebilir.

- **Açılış çevresi** seçili açılış işaretinden önceki ve sonraki 30 dakikayı aynı bilgisayar için gösterir.
- Tür filtreleri ve arama, uygulama çökmelerini, güncellemeleri, servis olaylarını ve yeniden başlatma kayıtlarını bulmayı kolaylaştırır.
- Bir satırın ayrıntısında olay kimliği, sağlayıcı, kaynak, açıklama ve özgün Windows XML’i bulunur. Sağlayıcı açıklaması yüklenemediğinde XML incelenebilir.
- **Görünüm → Tema** altında dört Barracuda teması bulunur. Seçim yerelde saklanır.

Canlı okuma ilgili hata, uyarı ve bilgi olaylarıyla sınırlıdır. Her kaynak için en yeni 850 eşleşme, tek seferde en fazla beş EVTX dosyası okunur. Uzun açıklama ve XML önizlemeleri kısaltılabilir. Okuma sorunları ve sınırlar ekranda gösterilir. Zamanlar görüntüleyen bilgisayarın yerel saatine çevrilir. Yakın zamanlı olaylar bir arada görünür ve bir olayın diğerine neden olduğunu göstermez.

Windows x64 ve Rust ile derleme:

```powershell
cargo test --locked
cargo build --release --locked
.\target\release\BarracudaEventTimeline.exe
```
