//! Braccato provider-chain semantics: deterministic quality order, validation,
//! cancellation, and a bounded cache keyed by song as well as provider.
use super::types::Lyric;
use crate::lyrics::{LyricsAlternative, LyricsSyncType};
use std::collections::HashMap;
pub fn rank(quality: LyricsSyncType) -> u8 {
    match quality {
        LyricsSyncType::RichSync => 0,
        LyricsSyncType::Word => 1,
        LyricsSyncType::Synced => 2,
        LyricsSyncType::Plain => 3,
    }
}
pub fn similarity(a: &str, b: &str) -> f64 {
    // JS's comparison is over UTF-16 code units, including repeated bigrams.
    let a = a.to_lowercase().encode_utf16().collect::<Vec<_>>();
    let b = b.to_lowercase().encode_utf16().collect::<Vec<_>>();
    if a.len() < 2 || b.len() < 2 {
        return 0.;
    }
    let mut counts = HashMap::new();
    for w in a.windows(2) {
        *counts.entry((w[0], w[1])).or_insert(0usize) += 1;
    }
    let mut matched = 0;
    for w in b.windows(2) {
        if let Some(n) = counts.get_mut(&(w[0], w[1])).filter(|n| **n > 0) {
            *n -= 1;
            matched += 1;
        }
    }
    2. * matched as f64 / (a.len() + b.len() - 2) as f64
}
pub fn validate(reference: &str, lyrics: &[Lyric], threshold: f64) -> bool {
    !lyrics.is_empty()
        && similarity(
            reference,
            &lyrics
                .iter()
                .map(|l| l.words.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ) >= threshold
}
pub fn normalize_alternatives(alternatives: &mut Vec<LyricsAlternative>) {
    alternatives.retain(|a| !a.lyrics.lines.is_empty());
    // The rendering mode "richsync" covers both words and syllables. Prefer
    // actual syllable cues within that mode, without trusting provider labels.
    alternatives.sort_by_key(|a| {
        (
            !crate::lyrics::has_syllable_level_timing(&a.lyrics),
            rank(a.sync_type),
        )
    });
    let mut seen = std::collections::HashSet::new();
    alternatives.retain(|a| seen.insert((a.provider.clone(), a.format.clone(), a.sync_type as u8)));
}

#[cfg(not(target_arch = "wasm32"))]
pub mod chain {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    #[derive(Clone, Default)]
    pub struct Context {
        pub song: String,
        pub artist: String,
        pub album: String,
        pub duration_ms: i64,
        pub video_id: Option<String>,
        pub cancelled: Arc<AtomicBool>,
    }
    pub type Fetch = Arc<
        dyn Fn(
                Context,
            )
                -> Pin<Box<dyn Future<Output = Result<Option<LyricsAlternative>, String>> + Send>>
            + Send
            + Sync,
    >;
    pub struct Provider {
        pub key: String,
        pub quality: LyricsSyncType,
        pub fetch: Fetch,
    }
    #[derive(Default)]
    pub struct Options {
        pub priority: Vec<String>,
        pub disabled: Vec<String>,
        pub reference: Option<String>,
        pub threshold: Option<f64>,
    }
    #[derive(Default)]
    pub struct Report {
        pub result: Option<LyricsAlternative>,
        pub errors: Vec<(String, String)>,
        pub attempted: Vec<String>,
        pub cache_hits: usize,
    }
    #[derive(Default)]
    pub struct ProviderChain {
        providers: Vec<Provider>,
        cache: HashMap<String, (Instant, Option<LyricsAlternative>)>,
    }
    impl ProviderChain {
        pub fn register(&mut self, p: Provider) {
            self.unregister(&p.key);
            self.providers.push(p);
        }
        pub fn unregister(&mut self, key: &str) {
            self.providers.retain(|p| p.key != key);
            self.cache
                .retain(|k, _| !k.starts_with(&format!("{key}\0")));
        }
        pub fn clear_cache(&mut self) {
            self.cache.clear();
        }
        pub async fn fetch(&mut self, ctx: Context, opts: Options) -> Report {
            let mut order = (0..self.providers.len()).collect::<Vec<_>>();
            if opts.priority.is_empty() {
                order.sort_by_key(|i| rank(self.providers[*i].quality));
            } else {
                order = opts
                    .priority
                    .iter()
                    .filter_map(|k| self.providers.iter().position(|p| &p.key == k))
                    .collect();
            }
            let mut report = Report::default();
            for i in order {
                if ctx.cancelled.load(Ordering::Relaxed) {
                    break;
                }
                let p = &self.providers[i];
                if opts.disabled.contains(&p.key) {
                    continue;
                }
                let key = format!(
                    "{}\0{}\0{}\0{}\0{}\0{}",
                    p.key,
                    ctx.song,
                    ctx.artist,
                    ctx.album,
                    ctx.duration_ms,
                    ctx.video_id.as_deref().unwrap_or("")
                );
                report.attempted.push(p.key.clone());
                let result = if let Some((_, hit)) = self
                    .cache
                    .get(&key)
                    .filter(|(time, _)| time.elapsed() < Duration::from_secs(3600))
                {
                    report.cache_hits += 1;
                    Ok(hit.clone())
                } else {
                    (p.fetch)(ctx.clone()).await
                };
                if ctx.cancelled.load(Ordering::Relaxed) {
                    break;
                }
                match result {
                    Ok(result) => {
                        if self.cache.len() >= 256 {
                            if let Some(oldest) = self
                                .cache
                                .iter()
                                .min_by_key(|(_, v)| v.0)
                                .map(|(k, _)| k.clone())
                            {
                                self.cache.remove(&oldest);
                            }
                        }
                        self.cache.insert(key, (Instant::now(), result.clone()));
                        if let Some(result) = result.filter(|a| !a.lyrics.lines.is_empty()) {
                            let acceptable = opts.reference.as_ref().is_none_or(|reference| {
                                similarity(
                                    reference,
                                    &result
                                        .lyrics
                                        .lines
                                        .iter()
                                        .map(|l| l.text.as_str())
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                ) >= opts.threshold.unwrap_or(0.5)
                            });
                            if acceptable {
                                report.result = Some(result);
                                break;
                            }
                        }
                    }
                    Err(error) => report.errors.push((p.key.clone(), error)),
                }
            }
            report
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn cache_is_track_scoped_and_cancellation_stops_work() {
            let mut chain = ProviderChain::default();
            chain.register(Provider {
                key: "test".into(),
                quality: LyricsSyncType::Plain,
                fetch: Arc::new(|ctx| {
                    Box::pin(async move {
                        Ok(Some(LyricsAlternative {
                            provider: "test".into(),
                            sync_type: LyricsSyncType::Plain,
                            format: "txt".into(),
                            lyrics: crate::lyrics::parse_plain(&ctx.song),
                        }))
                    })
                }),
            });
            let a = Context {
                song: "First".into(),
                ..Default::default()
            };
            assert_eq!(
                chain
                    .fetch(a.clone(), Options::default())
                    .await
                    .result
                    .unwrap()
                    .lyrics
                    .lines[0]
                    .text,
                "First"
            );
            assert_eq!(chain.fetch(a, Options::default()).await.cache_hits, 1);
            let b = Context {
                song: "Second".into(),
                ..Default::default()
            };
            assert_eq!(
                chain
                    .fetch(b.clone(), Options::default())
                    .await
                    .result
                    .unwrap()
                    .lyrics
                    .lines[0]
                    .text,
                "Second"
            );
            b.cancelled.store(true, Ordering::Relaxed);
            assert!(chain
                .fetch(b, Options::default())
                .await
                .attempted
                .is_empty());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn similarity_counts_repeated_pairs() {
        assert_eq!(similarity("hello", "hello"), 1.);
        assert_eq!(similarity("a", "a"), 0.);
        assert!(similarity("aaaa", "aabb") < 1.);
    }
}
