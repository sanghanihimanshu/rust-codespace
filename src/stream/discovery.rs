use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use tracing::{info, warn};

/// Announces the SRT stream as an mDNS service so OBS (or other tools)
/// can discover it on the LAN without needing to type an IP address.
///
/// Service type: `_srt._tcp.local.`
/// OBS-NDI-like naming: clients search for `_srt._tcp.local.` and find us.
pub struct MdnsAnnouncer {
    daemon: ServiceDaemon,
    service_name: String,
}

impl MdnsAnnouncer {
    pub fn start(service_name: &str, port: u16, passphrase_hint: &str) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| anyhow::anyhow!("mDNS daemon: {e}"))?;

        let instance = format!("{service_name}._srt._tcp.local.");

        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("proto".into(), "srt".into());
        properties.insert("enc".into(), "AES-256".into());
        if !passphrase_hint.is_empty() {
            // Advertise that a passphrase is required, but NOT the passphrase itself.
            properties.insert("auth".into(), "required".into());
        }

        let info = ServiceInfo::new(
            "_srt._tcp.local.",
            service_name,
            &instance,
            "",
            port,
            Some(properties),
        )
        .map_err(|e| anyhow::anyhow!("mDNS ServiceInfo: {e}"))?;

        daemon
            .register(info)
            .map_err(|e| anyhow::anyhow!("mDNS register: {e}"))?;

        info!(
            "mDNS: announced '{}' on port {} via _srt._tcp.local.",
            service_name, port
        );

        Ok(Self {
            daemon,
            service_name: service_name.to_owned(),
        })
    }

    pub fn stop(self) {
        let full_name = format!("{}._srt._tcp.local.", self.service_name);
        if let Err(e) = self.daemon.unregister(&full_name) {
            warn!("mDNS unregister: {e}");
        }
        if let Err(e) = self.daemon.shutdown() {
            warn!("mDNS shutdown: {e}");
        }
        info!("mDNS: service unregistered");
    }
}
