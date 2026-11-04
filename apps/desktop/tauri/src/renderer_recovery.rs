use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex,
};
use url::{Host, Url};

const MAX_AUTOMATIC_RELOADS: u8 = 2;
const MAX_RECOVERY_URL_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RecoveryLocale {
    #[default]
    English,
    German,
    Spanish,
    French,
    Turkish,
}

impl RecoveryLocale {
    fn from_language(language: &str) -> Self {
        let language = language
            .split('-')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match language.as_str() {
            "de" => Self::German,
            "es" => Self::Spanish,
            "fr" => Self::French,
            "tr" => Self::Turkish,
            _ => Self::English,
        }
    }

    fn copy(self) -> RecoveryCopy {
        match self {
            Self::English => RecoveryCopy {
                language: "en",
                title: "OpenChat could not start",
                description: "The application view stopped unexpectedly. Try loading OpenChat again. If the problem continues, close OpenChat and start it again later.",
                retry: "Reload application",
                missing_url: "The application address could not be verified. Close OpenChat and start it again.",
            },
            Self::German => RecoveryCopy {
                language: "de",
                title: "OpenChat konnte nicht gestartet werden",
                description: "Die Anwendungsansicht wurde unerwartet beendet. Lade OpenChat erneut. Falls das Problem bleibt, schließe OpenChat und starte es später noch einmal.",
                retry: "Anwendung neu laden",
                missing_url: "Die Anwendungsadresse konnte nicht überprüft werden. Schließe OpenChat und starte es erneut.",
            },
            Self::Spanish => RecoveryCopy {
                language: "es",
                title: "No se pudo iniciar OpenChat",
                description: "La vista de la aplicación se detuvo inesperadamente. Vuelve a cargar OpenChat. Si el problema continúa, cierra OpenChat y vuelve a abrirlo más tarde.",
                retry: "Recargar la aplicación",
                missing_url: "No se pudo verificar la dirección de la aplicación. Cierra OpenChat y vuelve a abrirlo.",
            },
            Self::French => RecoveryCopy {
                language: "fr",
                title: "OpenChat n’a pas pu démarrer",
                description: "La vue de l’application s’est arrêtée de façon inattendue. Rechargez OpenChat. Si le problème persiste, fermez OpenChat et relancez-le plus tard.",
                retry: "Recharger l’application",
                missing_url: "L’adresse de l’application n’a pas pu être vérifiée. Fermez OpenChat et relancez-le.",
            },
            Self::Turkish => RecoveryCopy {
                language: "tr",
                title: "OpenChat başlatılamadı",
                description: "Uygulama görünümü beklenmedik şekilde durdu. OpenChat’i yeniden yükleyin. Sorun sürerse OpenChat’i kapatıp daha sonra tekrar açın.",
                retry: "Uygulamayı yeniden yükle",
                missing_url: "Uygulama adresi doğrulanamadı. OpenChat’i kapatıp yeniden açın.",
            },
        }
    }
}

struct RecoveryCopy {
    language: &'static str,
    title: &'static str,
    description: &'static str,
    retry: &'static str,
    missing_url: &'static str,
}

#[derive(Clone, Default)]
struct RecoveryContext {
    app_url: Option<Url>,
    locale: RecoveryLocale,
}

#[derive(Clone, Default)]
pub(super) struct RendererRecoveryTracker {
    failures: Arc<AtomicU8>,
    context: Arc<Mutex<RecoveryContext>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RendererRecoveryAction {
    Reload { attempt: u8 },
    ShowRecovery,
}

impl RendererRecoveryTracker {
    pub(super) fn record_failure(&self) -> RendererRecoveryAction {
        let previous_failure_count = self
            .failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                Some(count.saturating_add(1))
            })
            .unwrap_or_else(|count| count);

        if previous_failure_count < MAX_AUTOMATIC_RELOADS {
            RendererRecoveryAction::Reload {
                attempt: previous_failure_count + 1,
            }
        } else {
            RendererRecoveryAction::ShowRecovery
        }
    }

    pub(super) fn renderer_ready(&self, app_url: Option<Url>, language: &str) {
        let mut context = match self.context.lock() {
            Ok(context) => context,
            Err(error) => {
                eprintln!("OpenChat renderer recovery context was poisoned: {error}");
                error.into_inner()
            }
        };
        if let Some(app_url) = app_url {
            if is_trusted_app_url(&app_url) {
                context.app_url = Some(app_url);
            } else {
                eprintln!("OpenChat ignored an untrusted renderer recovery URL.");
            }
        }
        context.locale = RecoveryLocale::from_language(language);
        self.failures.store(0, Ordering::SeqCst);
    }

    pub(super) fn has_app_url(&self) -> bool {
        match self.context.lock() {
            Ok(context) => context.app_url.is_some(),
            Err(error) => {
                eprintln!("OpenChat renderer recovery context was poisoned: {error}");
                error.into_inner().app_url.is_some()
            }
        }
    }

    pub(super) fn recovery_page(&self, current_source: Option<&str>) -> String {
        let context = match self.context.lock() {
            Ok(context) => context.clone(),
            Err(error) => {
                eprintln!("OpenChat renderer recovery context was poisoned: {error}");
                error.into_inner().clone()
            }
        };
        let target_url = context
            .app_url
            .or_else(|| current_source.and_then(parse_trusted_app_url));
        build_recovery_page(target_url.as_ref(), context.locale)
    }
}

fn parse_trusted_app_url(value: &str) -> Option<Url> {
    if value.len() > MAX_RECOVERY_URL_BYTES {
        return None;
    }
    let url = Url::parse(value).ok()?;
    is_trusted_app_url(&url).then_some(url)
}

fn is_trusted_app_url(url: &Url) -> bool {
    if url.as_str().len() > MAX_RECOVERY_URL_BYTES
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }

    let local_host = match url.host() {
        Some(Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("tauri.localhost")
        }
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };

    match url.scheme() {
        "tauri" => url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("localhost")),
        "http" | "https" => local_host,
        _ => false,
    }
}

fn build_recovery_page(app_url: Option<&Url>, locale: RecoveryLocale) -> String {
    let copy = locale.copy();
    let action = if let Some(app_url) = app_url {
        format!(
            "<a href=\"{}\">{}</a>",
            escape_html(app_url.as_str()),
            escape_html(copy.retry)
        )
    } else {
        format!("<p>{}</p>", escape_html(copy.missing_url))
    };

    format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>{}</title></head><body><main><h1>{}</h1><p>{}</p>{}</main></body></html>",
        copy.language,
        escape_html(copy.title),
        escape_html(copy.title),
        escape_html(copy.description),
        action
    )
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        build_recovery_page, escape_html, parse_trusted_app_url, RecoveryLocale,
        RendererRecoveryAction, RendererRecoveryTracker,
    };
    use url::Url;

    #[test]
    fn allows_two_automatic_reloads_before_recovery() {
        let tracker = RendererRecoveryTracker::default();

        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 1 }
        );
        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 2 }
        );
        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::ShowRecovery
        );
    }

    #[test]
    fn a_ready_renderer_starts_a_fresh_failure_sequence() {
        let tracker = RendererRecoveryTracker::default();
        tracker.record_failure();
        tracker.record_failure();
        tracker.renderer_ready(None, "tr");

        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 1 }
        );
    }

    #[test]
    fn failure_count_saturates_instead_of_wrapping() {
        let tracker = RendererRecoveryTracker::default();
        tracker.record_failure();
        tracker.record_failure();

        for _ in 0..=u8::MAX {
            assert_eq!(
                tracker.record_failure(),
                RendererRecoveryAction::ShowRecovery
            );
        }
    }

    #[test]
    fn recovery_uses_the_last_trusted_app_url_before_the_current_source() {
        let tracker = RendererRecoveryTracker::default();
        tracker.renderer_ready(
            Some(Url::parse("http://127.0.0.1:5173/").expect("valid dev URL")),
            "en",
        );

        let page = tracker.recovery_page(Some("https://example.com/"));

        assert!(page.contains("href=\"http://127.0.0.1:5173/\""));
        assert!(!page.contains("example.com"));
    }

    #[test]
    fn reports_when_a_trusted_app_url_is_already_stored() {
        let tracker = RendererRecoveryTracker::default();
        assert!(!tracker.has_app_url());

        tracker.renderer_ready(
            Some(Url::parse("http://127.0.0.1:5173/").expect("valid dev URL")),
            "en",
        );

        assert!(tracker.has_app_url());
    }

    #[test]
    fn recovery_uses_a_trusted_current_source_when_no_ready_url_was_recorded() {
        let tracker = RendererRecoveryTracker::default();

        let page = tracker.recovery_page(Some("tauri://localhost/index.html"));

        assert!(page.contains("href=\"tauri://localhost/index.html\""));
    }

    #[test]
    fn recovery_rejects_external_urls_and_gives_restart_instructions() {
        let tracker = RendererRecoveryTracker::default();

        let page = tracker.recovery_page(Some("https://example.com/"));

        assert!(!page.contains("href="));
        assert!(page.contains("The application address could not be verified."));
    }

    #[test]
    fn recovery_url_allows_only_app_schemes_and_loopback_hosts() {
        assert!(parse_trusted_app_url("tauri://localhost/index.html").is_some());
        assert!(parse_trusted_app_url("http://127.0.0.1:5173/").is_some());
        assert!(parse_trusted_app_url("https://[::1]:5173/").is_some());
        assert!(parse_trusted_app_url("http://tauri.localhost/").is_some());
        assert!(parse_trusted_app_url("https://example.com/").is_none());
        assert!(parse_trusted_app_url("http://localhost.example.com/").is_none());
        assert!(parse_trusted_app_url("http://openchat.localhost/").is_none());
        assert!(parse_trusted_app_url("http://user:pass@localhost/").is_none());
        assert!(parse_trusted_app_url("file:///C:/app/index.html").is_none());
        assert!(parse_trusted_app_url(&format!("http://localhost/{}", "a".repeat(4096))).is_none());
    }

    #[test]
    fn recovery_page_escapes_url_markup_and_has_no_inline_script() {
        let url = Url::parse("http://localhost:5173/?first=1&second=2").expect("valid URL");
        let page = build_recovery_page(Some(&url), RecoveryLocale::English);

        assert!(page.contains("?first=1&amp;second=2"));
        assert!(!page.contains("<script"));
        assert!(!page.contains("onclick="));
        assert_eq!(escape_html("&<>\"'"), "&amp;&lt;&gt;&quot;&#39;");
    }

    #[test]
    fn recovery_copy_uses_each_supported_locale() {
        for (language, title) in [
            ("en", "OpenChat could not start"),
            ("de", "OpenChat konnte nicht gestartet werden"),
            ("es", "No se pudo iniciar OpenChat"),
            ("fr", "OpenChat n’a pas pu démarrer"),
            ("tr", "OpenChat başlatılamadı"),
        ] {
            let page = build_recovery_page(None, RecoveryLocale::from_language(language));

            assert!(page.contains(&format!("<html lang=\"{language}\">")));
            assert!(page.contains(&format!("<h1>{title}</h1>")));
        }
    }
}
