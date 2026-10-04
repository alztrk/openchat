use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header::LOCATION};
use tokio::{net::lookup_host, time::timeout};
use url::{Host, Url};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REDIRECTS: usize = 5;

pub(super) struct FetchedPage {
    pub(super) url: Url,
    pub(super) content_type: String,
    pub(super) bytes: Vec<u8>,
    pub(super) truncated: bool,
}

pub(super) async fn fetch_public_page(
    original_url: Url,
    maximum_bytes: usize,
) -> Result<FetchedPage, String> {
    let mut url = validate_http_url(original_url)?;

    for redirect_number in 0..=MAX_REDIRECTS {
        let client = pinned_client(&url).await?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| "The requested web page could not be reached.".to_owned())?;

        if is_redirect(response.status()) {
            if redirect_number == MAX_REDIRECTS {
                return Err("The requested web page redirected too many times.".to_owned());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "The web page returned an invalid redirect.".to_owned())?;
            url = redirect_url(&url, location)?;
            continue;
        }

        if !response.status().is_success() {
            return Err(format!("Web server returned HTTP {}", response.status()));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let body = read_bounded_body(response, maximum_bytes).await?;
        return Ok(FetchedPage {
            url,
            content_type,
            bytes: body.bytes,
            truncated: body.truncated,
        });
    }

    Err("The requested web page could not be reached.".to_owned())
}

pub(super) fn validate_http_url(url: Url) -> Result<Url, String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http and https protocols are supported.".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URLs containing login credentials are not supported.".to_owned());
    }
    let host = url
        .host()
        .ok_or_else(|| "The URL must include a host name.".to_owned())?;
    let normalized_domain = match host {
        Host::Ipv4(address) if !is_public_ipv4(address) => {
            return Err("The URL host resolves to a restricted network address.".to_owned());
        }
        Host::Ipv6(address) if !is_public_ipv6(address) => {
            return Err("The URL host resolves to a restricted network address.".to_owned());
        }
        Host::Domain(domain) => {
            let normalized = domain.trim_end_matches('.');
            let lower = normalized.to_ascii_lowercase();
            if normalized.is_empty()
                || lower == "localhost"
                || lower.ends_with(".localhost")
                || lower.ends_with(".local")
            {
                return Err("The URL resolves to a restricted network name.".to_owned());
            }
            (normalized != domain).then(|| normalized.to_owned())
        }
        _ => None,
    };
    if let Some(normalized) = normalized_domain {
        let mut normalized_url = url;
        normalized_url
            .set_host(Some(&normalized))
            .map_err(|_| "The URL contains an invalid host name.".to_owned())?;
        return Ok(normalized_url);
    }
    Ok(url)
}

fn redirect_url(current_url: &Url, location: &str) -> Result<Url, String> {
    let target = current_url
        .join(location)
        .map_err(|_| "The web page returned an invalid redirect.".to_owned())?;
    validate_http_url(target)
}

async fn pinned_client(url: &Url) -> Result<Client, String> {
    let host = url
        .host()
        .ok_or_else(|| "The URL must include a host name.".to_owned())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "The URL contains an unsupported port.".to_owned())?;
    let addresses = match host {
        Host::Ipv4(address) => vec![SocketAddr::new(IpAddr::V4(address), 0)],
        Host::Ipv6(address) => vec![SocketAddr::new(IpAddr::V6(address), 0)],
        Host::Domain(domain) => {
            let resolved = timeout(DNS_TIMEOUT, lookup_host((domain, port)))
                .await
                .map_err(|_| "The URL host name could not be resolved in time.".to_owned())?
                .map_err(|_| "The URL host name could not be resolved.".to_owned())?;
            let addresses = resolved
                .map(|address| SocketAddr::new(address.ip(), 0))
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return Err("The URL host name has no network addresses.".to_owned());
            }
            addresses
        }
    };

    if addresses
        .iter()
        .any(|address| !is_public_address(address.ip()))
    {
        return Err("The URL host resolves to a restricted network address.".to_owned());
    }

    let mut builder = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    if let Host::Domain(domain) = host {
        builder = builder.resolve_to_addrs(domain, &addresses);
    }
    builder
        .build()
        .map_err(|_| "The web request could not be prepared.".to_owned())
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

struct BoundedBody {
    bytes: Vec<u8>,
    truncated: bool,
}

async fn read_bounded_body(
    response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<BoundedBody, String> {
    let advertised_length_exceeds_limit = response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64);
    let mut bytes = Vec::with_capacity(maximum_bytes.min(64 * 1024));
    let mut body = response.bytes_stream();
    let mut truncated = advertised_length_exceeds_limit;

    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| "The web page response could not be read.".to_owned())?;
        let remaining = maximum_bytes.saturating_sub(bytes.len());
        let copied = chunk.len().min(remaining);
        bytes.extend_from_slice(&chunk[..copied]);
        if copied < chunk.len() || bytes.len() == maximum_bytes {
            truncated = true;
            break;
        }
    }

    Ok(BoundedBody { bytes, truncated })
}

fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    if address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
    {
        return false;
    }

    let value = u32::from(address);
    [
        (u32::from(Ipv4Addr::new(0, 0, 0, 0)), 8),
        (u32::from(Ipv4Addr::new(100, 64, 0, 0)), 10),
        (u32::from(Ipv4Addr::new(192, 0, 0, 0)), 24),
        (u32::from(Ipv4Addr::new(192, 0, 2, 0)), 24),
        (u32::from(Ipv4Addr::new(192, 88, 99, 0)), 24),
        (u32::from(Ipv4Addr::new(198, 18, 0, 0)), 15),
        (u32::from(Ipv4Addr::new(198, 51, 100, 0)), 24),
        (u32::from(Ipv4Addr::new(203, 0, 113, 0)), 24),
        (u32::from(Ipv4Addr::new(240, 0, 0, 0)), 4),
    ]
    .iter()
    .all(|(network, prefix)| !matches_ipv4_prefix(value, *network, *prefix))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || address.is_unique_local()
        || address.is_unicast_link_local()
        || !matches_ipv6_prefix(address, Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3)
    {
        return false;
    }

    ![
        (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
        (Ipv6Addr::new(0x2001, 2, 0, 0, 0, 0, 0, 0), 48),
        (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
        (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
        (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
    ]
    .iter()
    .any(|(network, prefix)| matches_ipv6_prefix(address, *network, *prefix))
}

fn matches_ipv4_prefix(address: u32, network: u32, prefix: u8) -> bool {
    address >> (32 - prefix) == network >> (32 - prefix)
}

fn matches_ipv6_prefix(address: Ipv6Addr, network: Ipv6Addr, prefix: u8) -> bool {
    let mask = u128::MAX << (128 - prefix);
    u128::from(address) & mask == u128::from(network) & mask
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{is_public_address, redirect_url, validate_http_url};
    use url::Url;

    #[test]
    fn accepts_global_addresses_and_rejects_reserved_ranges() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            IpAddr::V6("2606:4700:4700::1111".parse().expect("valid global IPv6")),
        ] {
            assert!(
                is_public_address(address),
                "expected global address {address}"
            );
        }

        for address in [
            IpAddr::V4(Ipv4Addr::new(0, 1, 2, 3)),
            IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
            IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(240, 0, 0, 1)),
            IpAddr::V6("::1".parse().expect("valid loopback IPv6")),
            IpAddr::V6("fc00::1".parse().expect("valid unique-local IPv6")),
            IpAddr::V6("fe80::1".parse().expect("valid link-local IPv6")),
            IpAddr::V6("2001:db8::1".parse().expect("valid documentation IPv6")),
            IpAddr::V6("2002:0808:0808::1".parse().expect("valid 6to4 IPv6")),
        ] {
            assert!(
                !is_public_address(address),
                "expected restricted address {address}"
            );
        }
    }

    #[test]
    fn rejects_credentials_and_non_http_schemes() {
        for value in [
            "file:///etc/passwd",
            "http://user:secret@example.com/",
            "http://127.0.0.1/",
            "http://localhost/",
        ] {
            let url = Url::parse(value).expect("test URL should parse");
            assert!(
                validate_http_url(url).is_err(),
                "URL should be rejected: {value}"
            );
        }
    }

    #[test]
    fn redirects_are_resolved_and_restricted_targets_are_rejected() {
        let current = Url::parse("https://example.com/articles/page").expect("valid base URL");
        let relative = redirect_url(&current, "../next").expect("valid relative redirect");
        assert_eq!(relative.as_str(), "https://example.com/next");

        assert!(redirect_url(&current, "http://127.0.0.1/admin").is_err());
        assert!(redirect_url(&current, "file:///etc/passwd").is_err());
    }
}
