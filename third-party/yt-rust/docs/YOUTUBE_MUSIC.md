# YouTube Music provider guide

## Permission, stability, and intended use

The `youtube-music` feature targets the YouTube Music web interface, not a documented public API. Endpoint names, request contexts, response layouts, and availability can change without notice. Use it only when you have express authorization for your application and comply with the relevant terms and laws. The crate does not attempt to evade access restrictions, anti-abuse controls, authentication, subscriptions, or player protections.

This provider is a best-effort catalog metadata client, not a supported YouTube SDK. Do not rely on it for production guarantees or security-critical decisions. The default is public/unauthenticated catalog requests; account-specific features require an explicitly supplied caller session. The library never extracts session cookies from a browser, stores them, refreshes them, or writes them to logs. Keep API keys and session credentials in a secret manager or environment, never commit them, and rotate/revoke credentials if exposed.

Requests are directed to the fixed `music.youtube.com` origin. Redirects are disabled by the default HTTP client. An injected custom client is caller-controlled; configure it carefully and do not install redirect/proxy behavior that forwards credentials to other hosts.

## Feature flags

- `youtube-data`: documented YouTube Data API provider; enabled by default.
- `youtube-music`: undocumented Music catalog adapter; opt in explicitly.
- `full`: both adapters.

The Music web API key is a distinct configuration value from the YouTube Data API key. Exact request context fields can become stale; update only against authorized, current documentation or permission-approved references.

## Capabilities

| Capability | Music adapter state | Notes |
| --- | --- | --- |
| Search | Best effort; renderer-mapped | Sends filter-specific params and parses recognized `musicShelfRenderer` / `musicResponsiveListItemRenderer` results. Unknown shelves/renderers are skipped. Search continuation tokens are forwarded opaquely; layout changes may produce a decode/provider error. |
| Suggestions | Best effort; renderer-mapped | Reads the search-suggestions section, preserves service order, and removes exact duplicates. No history-deletion/mutation API. |
| Hot/chart results | Best effort; partial | Requests the Music charts browse surface with requested region in client context and reads recognized music shelves. Exact chart contents vary by locale/account. Chart continuation is unsupported. |
| Track metadata | Unsupported | Search results may contain normalized track metadata. Standalone metadata lookup is not implemented because a verified metadata-only request mapping is not available here. |
| Album metadata | Unsupported | No standalone browse mapping is implemented. |
| Artist metadata | Unsupported | No standalone browse mapping is implemented. |
| Related tracks | Unsupported | No verified related-track response mapping is implemented; local ranking is available separately. |
| Artwork | Search-result thumbnails | URLs and sizes are returned where present; no independent artwork search or image download. |
| Player handoff | Available for track video IDs | Use YouTube watch/embed URLs; this library does not provide media URLs. |
| Audio streaming | Not supported | No direct stream resolution, downloading, deciphering, or DRM bypass. |

## Locale, IDs, and paging

Use `YouTubeMusicConfig::with_locale(api_key, country, language)` for a public configuration; defaults are US/en. Country must be a 2-letter code, and language is a BCP-47-like tag. `MusicId` is an opaque Music catalog ID and is not interchangeable with the official Data API `video_id`. A `MusicTrack.video_id`, when available, is for YouTube player handoff.

Search responses may contain an opaque continuation token, which the adapter forwards in a continuation request. This request mapping is best effort and may change with the web interface. Chart continuation is currently `Unsupported`. Region is sent as the chart request's client country context; region-specific content may be unavailable, unranked, or account dependent.

## Session-authenticated mode

Only pass credentials deliberately obtained and supplied by the application owner. Do not request browser profiles or use a third-party user's account without authorization. `MusicSession` redacts its value from `Debug`; `YouTubeMusicConfig` also redacts API key output. Avoid logging HTTP request URLs (the API key is currently a query parameter), headers, serialized config, or raw error payloads. Never expose session cookies through application errors or diagnostics.

Authenticated data is account-sensitive. Keep caller-provided history and results private; this library does not persist history, but the calling application may.

## Playback and rights

`PlaybackHandoff` provides identifiers and player URLs only. The YouTube player remains responsible for playback eligibility, ads, regional restrictions, and user entitlement. This crate does not return direct audio URLs and must not be extended to extract, decrypt, or download protected streams.

## Testing

Unit tests use representative captured/sanitized response shapes and do not need credentials. Mock the HTTP boundary for request-shape and credential-redaction tests. Do not run live calls in CI by default; live tests can consume quota or access an account and should require an explicit opt-in.
