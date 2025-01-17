// Native re-exports
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{build_swarm, init_logger, sleep, Instant, RedisClient};

// Wasm re-exports
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{build_swarm, init_logger, sleep, Instant, RedisClient};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod native {
    use std::time::Duration;

    use anyhow::{bail, Context, Result};
    use env_logger::{Env, Target};
    use futures::future::BoxFuture;
    use futures::FutureExt;
    use libp2p::identity::Keypair;
    use libp2p::swarm::{NetworkBehaviour, Swarm};
    use libp2p::{noise, tcp, tls, yamux};
    use libp2p_mplex as mplex;
    use libp2p_webrtc as webrtc;
    use redis::AsyncCommands;

    use crate::{Muxer, SecProtocol, Transport};

    pub(crate) type Instant = std::time::Instant;

    pub(crate) fn init_logger() {
        env_logger::Builder::from_env(Env::default().default_filter_or("info"))
            .target(Target::Stdout)
            .init();
    }

    pub(crate) fn sleep(duration: Duration) -> BoxFuture<'static, ()> {
        tokio::time::sleep(duration).boxed()
    }

    pub(crate) async fn build_swarm<B: NetworkBehaviour>(
        ip: &str,
        transport: Transport,
        sec_protocol: Option<SecProtocol>,
        muxer: Option<Muxer>,
        behaviour_constructor: impl FnOnce(&Keypair) -> B,
    ) -> Result<(Swarm<B>, String)> {
        let (swarm, addr) = match (transport, sec_protocol, muxer) {
            (Transport::QuicV1, None, None) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_quic()
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/udp/0/quic-v1"),
            ),
            (Transport::Tcp, Some(SecProtocol::Tls), Some(Muxer::Mplex)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_tcp(
                        tcp::Config::default(),
                        tls::Config::new,
                        mplex::MplexConfig::default,
                    )?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0"),
            ),
            (Transport::Tcp, Some(SecProtocol::Tls), Some(Muxer::Yamux)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_tcp(
                        tcp::Config::default(),
                        tls::Config::new,
                        yamux::Config::default,
                    )?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0"),
            ),
            (Transport::Tcp, Some(SecProtocol::Noise), Some(Muxer::Mplex)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_tcp(
                        tcp::Config::default(),
                        noise::Config::new,
                        mplex::MplexConfig::default,
                    )?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0"),
            ),
            (Transport::Tcp, Some(SecProtocol::Noise), Some(Muxer::Yamux)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_tcp(
                        tcp::Config::default(),
                        noise::Config::new,
                        yamux::Config::default,
                    )?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0"),
            ),
            (Transport::Ws, Some(SecProtocol::Tls), Some(Muxer::Mplex)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_websocket(tls::Config::new, mplex::MplexConfig::default)
                    .await?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/ws"),
            ),
            (Transport::Ws, Some(SecProtocol::Tls), Some(Muxer::Yamux)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_websocket(tls::Config::new, yamux::Config::default)
                    .await?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/ws"),
            ),
            (Transport::Ws, Some(SecProtocol::Noise), Some(Muxer::Mplex)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_websocket(noise::Config::new, mplex::MplexConfig::default)
                    .await?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/ws"),
            ),
            (Transport::Ws, Some(SecProtocol::Noise), Some(Muxer::Yamux)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_websocket(noise::Config::new, yamux::Config::default)
                    .await?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/ws"),
            ),
            (Transport::WebRtcDirect, None, None) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_tokio()
                    .with_other_transport(|key| {
                        Ok(webrtc::tokio::Transport::new(
                            key.clone(),
                            webrtc::tokio::Certificate::generate(&mut rand::thread_rng())?,
                        ))
                    })?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/udp/0/webrtc-direct"),
            ),
            (t, s, m) => bail!("Unsupported combination: {t:?} {s:?} {m:?}"),
        };
        Ok((swarm, addr))
    }

    pub(crate) struct RedisClient(redis::Client);

    impl RedisClient {
        pub(crate) fn new(redis_addr: &str) -> Result<Self> {
            Ok(Self(
                redis::Client::open(redis_addr).context("Could not connect to redis")?,
            ))
        }

        pub(crate) async fn blpop(&self, key: &str, timeout: u64) -> Result<Vec<String>> {
            let mut conn = self.0.get_async_connection().await?;
            Ok(conn.blpop(key, timeout as usize).await?)
        }

        pub(crate) async fn rpush(&self, key: &str, value: String) -> Result<()> {
            let mut conn = self.0.get_async_connection().await?;
            conn.rpush(key, value).await?;
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use anyhow::{bail, Context, Result};
    use futures::future::{BoxFuture, FutureExt};
    use libp2p::core::upgrade::Version;
    use libp2p::identity::Keypair;
    use libp2p::swarm::{NetworkBehaviour, Swarm};
    use libp2p::{noise, websocket_websys, webtransport_websys, yamux, Transport as _};
    use libp2p_mplex as mplex;
    use libp2p_webrtc_websys as webrtc_websys;
    use std::time::Duration;

    use crate::{BlpopRequest, Muxer, SecProtocol, Transport};

    pub(crate) type Instant = instant::Instant;

    pub(crate) fn init_logger() {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::default());
    }

    pub(crate) fn sleep(duration: Duration) -> BoxFuture<'static, ()> {
        futures_timer::Delay::new(duration).boxed()
    }

    pub(crate) async fn build_swarm<B: NetworkBehaviour>(
        ip: &str,
        transport: Transport,
        sec_protocol: Option<SecProtocol>,
        muxer: Option<Muxer>,
        behaviour_constructor: impl FnOnce(&Keypair) -> B,
    ) -> Result<(Swarm<B>, String)> {
        Ok(match (transport, sec_protocol, muxer) {
            (Transport::Webtransport, None, None) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_wasm_bindgen()
                    .with_other_transport(|local_key| {
                        webtransport_websys::Transport::new(webtransport_websys::Config::new(
                            &local_key,
                        ))
                    })?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/udp/0/quic/webtransport"),
            ),
            (Transport::Ws, Some(SecProtocol::Noise), Some(Muxer::Mplex)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_wasm_bindgen()
                    .with_other_transport(|local_key| {
                        Ok(websocket_websys::Transport::default()
                            .upgrade(Version::V1Lazy)
                            .authenticate(
                                noise::Config::new(&local_key)
                                    .context("failed to initialise noise")?,
                            )
                            .multiplex(mplex::MplexConfig::new()))
                    })?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/wss"),
            ),
            (Transport::Ws, Some(SecProtocol::Noise), Some(Muxer::Yamux)) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_wasm_bindgen()
                    .with_other_transport(|local_key| {
                        Ok(websocket_websys::Transport::default()
                            .upgrade(Version::V1Lazy)
                            .authenticate(
                                noise::Config::new(&local_key)
                                    .context("failed to initialise noise")?,
                            )
                            .multiplex(yamux::Config::default()))
                    })?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/tcp/0/wss"),
            ),
            (Transport::WebRtcDirect, None, None) => (
                libp2p::SwarmBuilder::with_new_identity()
                    .with_wasm_bindgen()
                    .with_other_transport(|local_key| {
                        webrtc_websys::Transport::new(webrtc_websys::Config::new(&local_key))
                    })?
                    .with_behaviour(behaviour_constructor)?
                    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
                    .build(),
                format!("/ip4/{ip}/udp/0/webrtc-direct"),
            ),
            (t, s, m) => bail!("Unsupported combination: {t:?} {s:?} {m:?}"),
        })
    }

    pub(crate) struct RedisClient(String);

    impl RedisClient {
        pub(crate) fn new(base_url: &str) -> Result<Self> {
            Ok(Self(base_url.to_owned()))
        }

        pub(crate) async fn blpop(&self, key: &str, timeout: u64) -> Result<Vec<String>> {
            let res = reqwest::Client::new()
                .post(&format!("http://{}/blpop", self.0))
                .json(&BlpopRequest {
                    key: key.to_owned(),
                    timeout,
                })
                .send()
                .await?
                .json()
                .await?;
            Ok(res)
        }

        pub(crate) async fn rpush(&self, _: &str, _: String) -> Result<()> {
            bail!("unimplemented")
        }
    }
}
